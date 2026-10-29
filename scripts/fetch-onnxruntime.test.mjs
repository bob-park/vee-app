import { test } from "node:test";
import assert from "node:assert/strict";
import { RUNTIMES, pickEntry } from "./fetch-onnxruntime.mjs";

test("picks the library under lib/, not the dSYM copy", () => {
  const entries = [
    "./onnxruntime-osx-arm64-1.31.0/lib/libonnxruntime.1.31.0.dylib",
    "./onnxruntime-osx-arm64-1.31.0/lib/libonnxruntime.1.31.0.dylib.dSYM/Contents/Resources/DWARF/libonnxruntime.1.31.0.dylib",
    "./onnxruntime-osx-arm64-1.31.0/lib/libonnxruntime.dylib",
    "./onnxruntime-osx-arm64-1.31.0/testdata/libcustom_op_library.dylib",
  ];
  assert.equal(pickEntry(entries, "libonnxruntime.dylib"), "./onnxruntime-osx-arm64-1.31.0/lib/libonnxruntime.dylib");
});

test("picks the Windows dll and reports a missing one", () => {
  const entries = ["onnxruntime-win-x64-1.31.0/lib/onnxruntime_providers_shared.dll", "onnxruntime-win-x64-1.31.0/lib/onnxruntime.dll"];
  assert.equal(pickEntry(entries, "onnxruntime.dll"), "onnxruntime-win-x64-1.31.0/lib/onnxruntime.dll");
  assert.equal(pickEntry(["a/include/onnxruntime.dll"], "onnxruntime.dll"), null);
});

test("pins ORT 1.31.0 for macOS and Windows only", () => {
  assert.deepEqual(Object.keys(RUNTIMES).sort(), ["darwin", "win32"]);
  for (const r of Object.values(RUNTIMES)) {
    assert.match(r.url, /^https:\/\/github\.com\/microsoft\/onnxruntime\/releases\/download\/v1\.31\.0\//);
    assert.match(r.sha256, /^[0-9a-f]{64}$/);
    assert.match(r.libSha256, /^[0-9a-f]{64}$/);
  }
  assert.equal(RUNTIMES.darwin.lib, "libonnxruntime.dylib");
  assert.equal(RUNTIMES.win32.lib, "onnxruntime.dll");
});
