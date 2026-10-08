import { test } from "node:test";
import assert from "node:assert/strict";
import { leftWindow, nextTilt } from "./motion.ts";

test("tilt leans with horizontal motion and is capped at 10 degrees", () => {
  assert.equal(nextTilt(0, 5), 3);
  assert.equal(nextTilt(0, 100), 10);
  assert.equal(nextTilt(0, -100), -10);
});

test("tilt settles back when the cursor stops moving sideways", () => {
  assert.equal(nextTilt(10, 0), 7);
  assert.ok(Math.abs(nextTilt(nextTilt(nextTilt(10, 0), 0), 0)) < 4);
});

test("the card hands off to the OS drag once the cursor leaves the window", () => {
  const w = 1200;
  const h = 300;
  assert.equal(leftWindow(600, 150, w, h, 0), false);
  assert.equal(leftWindow(600, -1, w, h, 0), true);
  assert.equal(leftWindow(-1, 150, w, h, 0), true);
  assert.equal(leftWindow(1200, 150, w, h, 0), true);
  assert.equal(leftWindow(600, 300, w, h, 0), true);
});

test("a top edge margin hands off before the cursor reaches the window edge", () => {
  assert.equal(leftWindow(600, 19, 1200, 300, 20), true);
  assert.equal(leftWindow(600, 20, 1200, 300, 20), false);
});
