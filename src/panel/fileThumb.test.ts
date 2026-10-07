import { test } from "node:test";
import assert from "node:assert/strict";
import { dragFileName, imageTag } from "./fileThumb.ts";

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
