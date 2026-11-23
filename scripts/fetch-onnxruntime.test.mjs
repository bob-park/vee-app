import { test } from "node:test";
import assert from "node:assert/strict";
import { RUNTIMES, pickNotices } from "./fetch-onnxruntime.mjs";

test("pins ORT 1.31.0 from the NuGet package for macOS and Windows only", () => {
  assert.deepEqual(Object.keys(RUNTIMES).sort(), ["darwin", "win32"]);
  for (const r of Object.values(RUNTIMES)) {
    assert.equal(r.url, "https://api.nuget.org/v3-flatcontainer/microsoft.ml.onnxruntime/1.31.0/microsoft.ml.onnxruntime.1.31.0.nupkg");
    assert.match(r.sha256, /^[0-9a-f]{64}$/);
    assert.match(r.libSha256, /^[0-9a-f]{64}$/);
  }
  assert.equal(RUNTIMES.darwin.entry, "runtimes/osx-arm64/native/libonnxruntime.dylib");
  assert.equal(RUNTIMES.win32.entry, "runtimes/win-x64/native/onnxruntime.dll");
  assert.equal(RUNTIMES.darwin.lib, "libonnxruntime.dylib");
  assert.equal(RUNTIMES.win32.lib, "onnxruntime.dll");
});

test("keeps the library checksums pinned when the source was the GitHub release", () => {
  // The NuGet package carries the same binaries, so a change here means a different library.
  assert.equal(RUNTIMES.darwin.libSha256, "83973572676032b4ba9418afed0b51f45599d35bb4e4628ca8ae1954c8362e31");
  assert.equal(RUNTIMES.win32.libSha256, "0c42dc4e278f0968fdf9ebf1b1e54a74c657a9942e70803e453c91993451cf4c");
});

test("picks the package's top-level LICENSE and ThirdPartyNotices.txt as notices", () => {
  const entries = [
    "_rels/.rels",
    "Microsoft.ML.OnnxRuntime.nuspec",
    "ThirdPartyNotices.txt",
    "LICENSE",
    "README.md",
    "build/native/include/LICENSE",
    "runtimes/osx-arm64/native/libonnxruntime.dylib",
  ];
  assert.deepEqual(pickNotices(entries), [
    { entry: "ThirdPartyNotices.txt", name: "ThirdPartyNotices.txt" },
    { entry: "LICENSE", name: "LICENSE" },
  ]);
});
