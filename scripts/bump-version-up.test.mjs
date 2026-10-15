// Runs bump-version-up.sh against fixture copies of the four version files.
import assert from "node:assert/strict";
import { spawnSync } from "node:child_process";
import { copyFileSync, mkdirSync, mkdtempSync, readFileSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { test } from "node:test";
import { fileURLToPath } from "node:url";

const SCRIPT = fileURLToPath(new URL("../bump-version-up.sh", import.meta.url));
// The script is bash; Windows runners have no guaranteed bash on PATH.
const opts = { skip: process.platform === "win32" && "bump-version-up.sh needs bash" };

function fixture(version, crate = "vee-app") {
  const dir = mkdtempSync(join(tmpdir(), "bump-"));
  mkdirSync(join(dir, "src-tauri"));
  copyFileSync(SCRIPT, join(dir, "bump-version-up.sh"));
  writeFileSync(join(dir, "src-tauri/tauri.conf.json"), `{\n  "productName": "App",\n  "version": "${version}",\n  "identifier": "com.bobpark.app"\n}\n`);
  writeFileSync(join(dir, "package.json"), `{\n  "name": "vee-app",\n  "version": "${version}",\n  "dependencies": {\n    "react": "^19.3.0"\n  }\n}\n`);
  writeFileSync(join(dir, "src-tauri/Cargo.toml"), `[package]\nname = "vee-app"\nversion = "${version}"\nedition = "2024"\n\n[dependencies]\nserde = { version = "1", features = ["derive"] }\n`);
  writeFileSync(join(dir, "src-tauri/Cargo.lock"), `[[package]]\nname = "serde"\nversion = "1.0.0"\n\n[[package]]\nname = "${crate}"\nversion = "${version}"\ndependencies = [\n "serde",\n]\n`);
  return dir;
}

const FILES = ["src-tauri/tauri.conf.json", "package.json", "src-tauri/Cargo.toml", "src-tauri/Cargo.lock"];
const snapshot = (dir) => FILES.map((f) => readFileSync(join(dir, f), "utf8"));
const run = (dir, ...args) => spawnSync("bash", [join(dir, "bump-version-up.sh"), ...args], { encoding: "utf8" });

function assertVersion(dir, version) {
  const [conf, pkg, toml, lock] = snapshot(dir);
  assert.equal(JSON.parse(conf).version, version);
  assert.equal(JSON.parse(pkg).version, version);
  assert.match(toml, new RegExp(`\\[package\\]\\nname = "vee-app"\\nversion = "${version}"`));
  assert.match(lock, new RegExp(`name = "vee-app"\\nversion = "${version}"`));
}

test("patch by default", opts, () => {
  const dir = fixture("0.0.1");
  assert.equal(run(dir).status, 0);
  assertVersion(dir, "0.0.2");
  const [, pkg, toml, lock] = snapshot(dir);
  assert.equal(JSON.parse(pkg).dependencies.react, "^19.3.0");
  assert.match(toml, /serde = \{ version = "1"/);
  assert.match(lock, /name = "serde"\nversion = "1.0.0"/);
});

test("minor resets patch", opts, () => {
  const dir = fixture("1.2.3");
  assert.equal(run(dir, "minor").status, 0);
  assertVersion(dir, "1.3.0");
});

test("major resets minor and patch", opts, () => {
  const dir = fixture("1.2.3");
  assert.equal(run(dir, "major").status, 0);
  assertVersion(dir, "2.0.0");
});

test("rejects unknown bump type", opts, () => {
  const dir = fixture("0.0.1");
  const before = snapshot(dir);
  assert.notEqual(run(dir, "date").status, 0);
  assert.deepEqual(snapshot(dir), before);
});

test("rejects malformed version", opts, () => {
  const dir = fixture("0.1.0-rc1-20260101");
  const before = snapshot(dir);
  assert.notEqual(run(dir).status, 0);
  assert.deepEqual(snapshot(dir), before);
});

test("rejects a Cargo.lock without the crate (missed rename) and changes nothing", opts, () => {
  const dir = fixture("0.0.1", "my-renamed-app");
  const before = snapshot(dir);
  assert.notEqual(run(dir).status, 0);
  assert.deepEqual(snapshot(dir), before);
});
