import { test } from "node:test";
import assert from "node:assert/strict";
import { buildManifest } from "./latest-json.mjs";

const now = new Date("2026-10-07T00:00:00Z");
const base = "https://github.com/bob-park/vee-app/releases/download/v0.2.0";

test("builds both platform entries from their .sig assets", () => {
  const manifest = buildManifest(
    "v0.2.0",
    [
      { name: "Vee_0.2.0_aarch64.app.tar.gz.sig", body: "mac-sig\n" },
      { name: "Vee_0.2.0_x64-setup.exe.sig", body: "win-sig\n" },
    ],
    now,
  );
  assert.deepEqual(manifest, {
    version: "0.2.0",
    pub_date: "2026-10-07T00:00:00.000Z",
    platforms: {
      "darwin-aarch64": { signature: "mac-sig", url: `${base}/Vee_0.2.0_aarch64.app.tar.gz` },
      "windows-x86_64": { signature: "win-sig", url: `${base}/Vee_0.2.0_x64-setup.exe` },
    },
  });
});

test("includes only the platforms that were uploaded", () => {
  const manifest = buildManifest("v0.2.0", [{ name: "Vee_0.2.0_x64-setup.exe.sig", body: "win-sig" }], now);
  assert.deepEqual(Object.keys(manifest.platforms), ["windows-x86_64"]);
});

test("ignores assets that are not updater signatures", () => {
  const manifest = buildManifest("v0.2.0", [{ name: "Vee_0.2.0_aarch64.dmg.sig", body: "x" }], now);
  assert.deepEqual(manifest.platforms, {});
});
