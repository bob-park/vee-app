#!/usr/bin/env node
// Fetches the pinned ONNX Runtime library into src-tauri/vendor/onnxruntime/ so the app bundle can
// carry it (macOS: Contents/Frameworks, signed with Vee's identity; Windows: next to the exe).
// Its license notices go to src-tauri/vendor/onnxruntime/notices/ (bundled as `licenses/onnxruntime/`).
// Runs before `tauri dev` / `tauri build` (tauri.conf.json); does nothing when the file is already there.
// Usage: node scripts/fetch-onnxruntime.mjs
import { execFileSync } from "node:child_process";
import { createHash } from "node:crypto";
import { copyFileSync, existsSync, mkdirSync, mkdtempSync, readFileSync, rmSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { join, resolve } from "node:path";
import { fileURLToPath } from "node:url";

const BASE = "https://github.com/microsoft/onnxruntime/releases/download/v1.31.0";

/** ONNX Runtime 1.31.0 (the version `ort` 2.0.0-rc.15 targets) per `process.platform`. */
export const RUNTIMES = {
  darwin: {
    url: `${BASE}/onnxruntime-osx-arm64-1.31.0.tgz`,
    sha256: "1031b36dd3bdaa8d976a6130d9cf2a05e9be320b93ae804f8427b91dc3432afc",
    lib: "libonnxruntime.dylib",
    libSha256: "83973572676032b4ba9418afed0b51f45599d35bb4e4628ca8ae1954c8362e31",
  },
  win32: {
    url: `${BASE}/onnxruntime-win-x64-1.31.0.zip`,
    sha256: "3958d8a44984160692c1b4ffd0de3cb48f8c39f9d34bb31e433db06e30b82195",
    lib: "onnxruntime.dll",
    libSha256: "0c42dc4e278f0968fdf9ebf1b1e54a74c657a9942e70803e453c91993451cf4c",
  },
};

/**
 * The archive entry of the library: `<root>/lib/<lib>`. Skips the `.dSYM` copy, which sits deeper.
 * @param {string[]} entries  `tar -tf` lines
 * @param {string} lib
 */
export function pickEntry(entries, lib) {
  return entries.find((e) => e.split("/").slice(-2).join("/") === `lib/${lib}`) ?? null;
}

/**
 * The license notices to ship with the runtime: the top-level `LICENSE` and `ThirdPartyNotices.txt`
 * (one directory deep in both the .tgz and the .zip).
 * @param {string[]} entries  `tar -tf` lines
 * @returns {{ entry: string, name: string }[]}
 */
export function pickNotices(entries) {
  return entries
    .map((entry) => ({ entry, parts: entry.replace(/^\.\//, "").split("/") }))
    .filter(({ parts }) => parts.length === 2 && ["LICENSE", "ThirdPartyNotices.txt"].includes(parts[1]))
    .map(({ entry, parts }) => ({ entry, name: parts[1] }));
}

/** @param {Uint8Array} bytes */
const sha256 = (bytes) => createHash("sha256").update(bytes).digest("hex");

/** @param {string} path */
const fileSha256 = (path) => (existsSync(path) ? sha256(readFileSync(path)) : null);

const isMain = process.argv[1] && fileURLToPath(import.meta.url) === resolve(process.argv[1]);
if (isMain) {
  const runtime = RUNTIMES[process.platform];
  if (!runtime) {
    console.log(`onnxruntime: no pinned build for ${process.platform}, skipped`);
    process.exit(0);
  }
  const outDir = fileURLToPath(new URL("../src-tauri/vendor/onnxruntime/", import.meta.url));
  const out = join(outDir, runtime.lib);
  const noticeDir = join(outDir, "notices");
  if (fileSha256(out) === runtime.libSha256 && existsSync(join(noticeDir, "LICENSE"))) process.exit(0);

  console.log(`onnxruntime: downloading ${runtime.url}`);
  const res = await fetch(runtime.url);
  if (!res.ok) throw new Error(`onnxruntime: download failed with HTTP ${res.status}`);
  const archive = new Uint8Array(await res.arrayBuffer());
  if (sha256(archive) !== runtime.sha256) throw new Error("onnxruntime: archive checksum mismatch");

  const work = mkdtempSync(join(tmpdir(), "vee-onnxruntime-"));
  try {
    const file = join(work, runtime.url.split("/").pop());
    writeFileSync(file, archive);
    // The OS tar (bsdtar on macOS and Windows) reads both .tgz and .zip.
    const tar = process.platform === "win32" ? "tar.exe" : "tar";
    const entries = execFileSync(tar, ["-tf", file], { encoding: "utf8", maxBuffer: 1 << 26 }).split(/\r?\n/);
    const entry = pickEntry(entries, runtime.lib);
    if (!entry) throw new Error(`onnxruntime: ${runtime.lib} not found in the archive`);
    const notices = pickNotices(entries);
    if (!notices.some((n) => n.name === "LICENSE")) throw new Error("onnxruntime: LICENSE not found in the archive");
    execFileSync(tar, ["-xf", file, "-C", work, entry, ...notices.map((n) => n.entry)]);
    const extracted = join(work, entry);
    if (fileSha256(extracted) !== runtime.libSha256) throw new Error(`onnxruntime: ${runtime.lib} checksum mismatch`);
    mkdirSync(outDir, { recursive: true });
    copyFileSync(extracted, out);
    rmSync(noticeDir, { recursive: true, force: true });
    mkdirSync(noticeDir, { recursive: true });
    for (const n of notices) copyFileSync(join(work, n.entry), join(noticeDir, n.name));
  } finally {
    rmSync(work, { recursive: true, force: true });
  }
  console.log(`onnxruntime: ${out}`);
}
