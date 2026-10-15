import { test } from "node:test";
import assert from "node:assert/strict";
import { dragFileName, fileColor, fileExt, imageTag } from "./fileThumb.ts";

test("tags image files with their upper-cased extension", () => {
  assert.equal(imageTag("/Users/me/Desktop/Shot.png"), "PNG");
  assert.equal(imageTag("C:\\pics\\a.JPEG"), "JPEG");
});

test("non-image or extension-less paths get no tag", () => {
  assert.equal(imageTag("/a/report.pdf"), null);
  assert.equal(imageTag("/a/README"), null);
  assert.equal(imageTag("/a.b/file"), null);
});

test("dragged images are named after their copy time in local time", () => {
  assert.equal(dragFileName(new Date(2026, 9, 7, 14, 32, 5).getTime()), "Vee 2026-10-07 14.32.05");
  assert.equal(dragFileName(new Date(2026, 0, 2, 3, 4, 5).getTime()), "Vee 2026-01-02 03.04.05");
});

test("file extensions are upper-cased labels of at most four characters", () => {
  assert.equal(fileExt("/a/report.pdf"), "PDF");
  assert.equal(fileExt("C:\\x\\Budget.XLSX"), "XLSX");
  assert.equal(fileExt("/a/model.safetensors"), "SAFE");
  assert.equal(fileExt("/a/README"), "");
  assert.equal(fileExt("/a/.gitignore"), "");
});

test("file colours group files by kind", () => {
  assert.equal(fileColor("/a/x.pdf"), fileColor("/b/Y.PDF"));
  assert.equal(fileColor("/a/x.zip"), fileColor("/a/x.dmg"));
  assert.equal(fileColor("/a/x.pages"), fileColor("/a/x.hwp"));
  assert.equal(fileColor("/a/x.numbers"), fileColor("/a/x.csv"));
  assert.equal(fileColor("/a/x.key"), fileColor("/a/x.pptx"));
  assert.equal(fileColor("/a/x.rs"), fileColor("/a/x.md"));
  assert.equal(fileColor("/a/x.heic"), fileColor("/a/x.png"));
  assert.notEqual(fileColor("/a/x.pdf"), fileColor("/a/x.zip"));
  assert.equal(fileColor("/a/README"), fileColor("/a/x.whatever"));
});
