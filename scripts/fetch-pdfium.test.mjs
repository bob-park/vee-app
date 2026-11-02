import { test } from "node:test";
import assert from "node:assert/strict";
import { LIBS, pickEntry } from "./fetch-pdfium.mjs";

test("pins chromium/8125 for macOS and Windows only", () => {
  assert.deepEqual(Object.keys(LIBS).sort(), ["darwin", "win32"]);
  for (const l of Object.values(LIBS)) {
    assert.match(l.url, /^https:\/\/github\.com\/bblanchon\/pdfium-binaries\/releases\/download\/chromium\/8125\//);
    assert.match(l.sha256, /^[0-9a-f]{64}$/);
    assert.match(l.libSha256, /^[0-9a-f]{64}$/);
  }
  assert.equal(LIBS.darwin.lib, "libpdfium.dylib");
  assert.equal(LIBS.win32.lib, "pdfium.dll");
});

test("picks the library entry, with or without a ./ prefix", () => {
  assert.equal(pickEntry(["include/fpdfview.h", "lib/libpdfium.dylib", "LICENSE"], "lib/libpdfium.dylib"), "lib/libpdfium.dylib");
  assert.equal(pickEntry(["./include/fpdfview.h", "./bin/pdfium.dll"], "bin/pdfium.dll"), "./bin/pdfium.dll");
  assert.equal(pickEntry(["lib/pdfium.dll.lib"], "bin/pdfium.dll"), null);
});
