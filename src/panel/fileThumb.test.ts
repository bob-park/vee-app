import { test } from "node:test";
import assert from "node:assert/strict";
import { imageTag } from "./fileThumb.ts";

test("tags image files with their upper-cased extension", () => {
  assert.equal(imageTag("/Users/me/Desktop/Shot.png"), "PNG");
  assert.equal(imageTag("C:\\pics\\a.JPEG"), "JPEG");
});

test("non-image or extension-less paths get no tag", () => {
  assert.equal(imageTag("/a/report.pdf"), null);
  assert.equal(imageTag("/a/README"), null);
  assert.equal(imageTag("/a.b/file"), null);
});
