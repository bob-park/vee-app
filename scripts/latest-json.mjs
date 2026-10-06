#!/usr/bin/env node
// 릴리스에 올라온 .sig 자산을 모아 latest.json 을 만들고 같은 릴리스에 올린다.
// mac 과 Windows 를 다른 기계에서 빌드하므로, 둘 다 올린 뒤 아무 기계에서나 한 번 돌린다.
// 사용: node scripts/latest-json.mjs v0.1.0
import { execFileSync } from "node:child_process";
import { mkdtempSync, readdirSync, readFileSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { join, resolve } from "node:path";
import { fileURLToPath } from "node:url";

const REPO = "bob-park/vee-app";
// 업데이터는 나열된 플랫폼 블록이 모두 완전해야 파일 전체를 받아들인다 — 있는 것만 넣는다.
const PLATFORMS = [
  { suffix: ".app.tar.gz.sig", key: "darwin-aarch64" },
  { suffix: "-setup.exe.sig", key: "windows-x86_64" },
];

/**
 * @param {string} tag  예: "v0.1.0"
 * @param {{name: string, body: string}[]} sigs  .sig 파일 이름과 내용
 * @param {Date} [now]
 */
export function buildManifest(tag, sigs, now = new Date()) {
  /** @type {Record<string, {signature: string, url: string}>} */
  const platforms = {};
  for (const { suffix, key } of PLATFORMS) {
    const sig = sigs.find((s) => s.name.endsWith(suffix));
    if (!sig) continue;
    const asset = sig.name.slice(0, -".sig".length);
    platforms[key] = {
      signature: sig.body.trim(),
      url: `https://github.com/${REPO}/releases/download/${tag}/${asset}`,
    };
  }
  return { version: tag.replace(/^v/, ""), pub_date: now.toISOString(), platforms };
}

const isMain = process.argv[1] && fileURLToPath(import.meta.url) === resolve(process.argv[1]);
if (isMain) {
  const tag = process.argv[2];
  if (!tag) {
    console.error("usage: node scripts/latest-json.mjs <tag>");
    process.exit(1);
  }
  // 공개키가 비어 있으면 그 빌드는 업데이트를 검증하지 못한다 — 받은 뒤에야 실패하므로 여기서 막는다.
  const conf = JSON.parse(readFileSync(new URL("../src-tauri/tauri.conf.json", import.meta.url), "utf8"));
  if (!conf.plugins?.updater?.pubkey) {
    console.error("src-tauri/tauri.conf.json: plugins.updater.pubkey is empty — fill it before releasing (see docs/release.md)");
    process.exit(1);
  }
  const dir = mkdtempSync(join(tmpdir(), "vee-sig-"));
  execFileSync("gh", ["release", "download", tag, "-R", REPO, "-p", "*.sig", "-D", dir], { stdio: "inherit" });
  const sigs = readdirSync(dir).map((name) => ({ name, body: readFileSync(join(dir, name), "utf8") }));
  const manifest = buildManifest(tag, sigs);
  const found = Object.keys(manifest.platforms);
  if (found.length === 0) {
    console.error(`no .sig assets on release ${tag}`);
    process.exit(1);
  }
  const out = join(dir, "latest.json");
  writeFileSync(out, JSON.stringify(manifest, null, 2) + "\n");
  execFileSync("gh", ["release", "upload", tag, "-R", REPO, out, "--clobber"], { stdio: "inherit" });
  console.log(`latest.json uploaded to ${tag}: ${found.join(", ")}`);
}
