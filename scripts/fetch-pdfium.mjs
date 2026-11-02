#!/usr/bin/env node
// Fetches the pinned PDFium library into src-tauri/vendor/pdfium/ so the app bundle can carry it
// (macOS: Contents/Frameworks, signed with Vee's identity; Windows: next to the exe).
// Its license notices go to src-tauri/vendor/pdfium/notices/ (bundled as `licenses/pdfium/`).
// Only the `--pdf-worker` child loads it. Runs before `tauri dev` / `tauri build` (tauri.conf.json);
// does nothing when the file is already there.
// Usage: node scripts/fetch-pdfium.mjs
import { execFileSync } from "node:child_process";
import { createHash } from "node:crypto";
import { copyFileSync, existsSync, mkdirSync, mkdtempSync, readFileSync, rmSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { dirname, join, resolve } from "node:path";
import { fileURLToPath } from "node:url";

// The PDFium API version `pdfium-render`'s `pdfium_8125` feature binds to.
const BASE = "https://github.com/bblanchon/pdfium-binaries/releases/download/chromium/8125";

/** PDFium chromium/8125 per `process.platform`. Upstream publishes no checksums; measured when pinning. */
export const LIBS = {
  darwin: {
    url: `${BASE}/pdfium-mac-arm64.tgz`,
    sha256: "4939bd935ac83ddc09a3a4f038bc43a19cea1660316a3667f26cb07a070e139d",
    entry: "lib/libpdfium.dylib",
    lib: "libpdfium.dylib",
    libSha256: "a45c44645523968525c710462691dd974f4efb1013ce1c249919a0939cfb373a",
  },
  win32: {
    url: `${BASE}/pdfium-win-x64.tgz`,
    sha256: "9a964261760cff52570aabf038ab346d3a5034dd8b30d0d5fe8861235d356b8b",
    entry: "bin/pdfium.dll",
    lib: "pdfium.dll",
    libSha256: "4b0dec92555d69437bef0df37be630181ab3e83a6823ca264ede355a4f2ef994",
  },
};

/**
 * The archive entry that is `entry`, with or without a leading `./`.
 * @param {string[]} entries  `tar -tf` lines
 * @param {string} entry
 */
export function pickEntry(entries, entry) {
  return entries.find((e) => e.replace(/^\.\//, "") === entry) ?? null;
}

/**
 * The license notices to ship with the library: `LICENSE` and every file under `licenses/` (PDFium's
 * own and its third-party code's), each with its path relative to the archive root.
 * @param {string[]} entries  `tar -tf` lines
 * @returns {{ entry: string, name: string }[]}
 */
export function pickNotices(entries) {
  return entries
    .map((entry) => ({ entry, name: entry.replace(/^\.\//, "") }))
    .filter(({ name }) => name === "LICENSE" || (name.startsWith("licenses/") && !name.endsWith("/")));
}

/** @param {Uint8Array} bytes */
const sha256 = (bytes) => createHash("sha256").update(bytes).digest("hex");

/** @param {string} path */
const fileSha256 = (path) => (existsSync(path) ? sha256(readFileSync(path)) : null);

const isMain = process.argv[1] && fileURLToPath(import.meta.url) === resolve(process.argv[1]);
if (isMain) {
  const pin = LIBS[process.platform];
  if (!pin) {
    console.log(`pdfium: no pinned build for ${process.platform}, skipped`);
    process.exit(0);
  }
  const outDir = fileURLToPath(new URL("../src-tauri/vendor/pdfium/", import.meta.url));
  const out = join(outDir, pin.lib);
  const noticeDir = join(outDir, "notices");
  if (fileSha256(out) === pin.libSha256 && existsSync(join(noticeDir, "LICENSE"))) process.exit(0);

  console.log(`pdfium: downloading ${pin.url}`);
  const res = await fetch(pin.url);
  if (!res.ok) throw new Error(`pdfium: download failed with HTTP ${res.status}`);
  const archive = new Uint8Array(await res.arrayBuffer());
  if (sha256(archive) !== pin.sha256) throw new Error("pdfium: archive checksum mismatch");

  const work = mkdtempSync(join(tmpdir(), "vee-pdfium-"));
  try {
    const file = join(work, pin.url.split("/").pop());
    writeFileSync(file, archive);
    const tar = process.platform === "win32" ? "tar.exe" : "tar";
    const entries = execFileSync(tar, ["-tf", file], { encoding: "utf8", maxBuffer: 1 << 26 }).split(/\r?\n/);
    const entry = pickEntry(entries, pin.entry);
    if (!entry) throw new Error(`pdfium: ${pin.entry} not found in the archive`);
    const notices = pickNotices(entries);
    if (!notices.some((n) => n.name === "LICENSE")) throw new Error("pdfium: LICENSE not found in the archive");
    execFileSync(tar, ["-xf", file, "-C", work, entry, ...notices.map((n) => n.entry)]);
    const extracted = join(work, entry);
    if (fileSha256(extracted) !== pin.libSha256) throw new Error(`pdfium: ${pin.lib} checksum mismatch`);
    mkdirSync(outDir, { recursive: true });
    copyFileSync(extracted, out);
    rmSync(noticeDir, { recursive: true, force: true });
    for (const n of notices) {
      mkdirSync(dirname(join(noticeDir, n.name)), { recursive: true });
      copyFileSync(join(work, n.entry), join(noticeDir, n.name));
    }
  } finally {
    rmSync(work, { recursive: true, force: true });
  }
  console.log(`pdfium: ${out}`);
}
