import { test } from "node:test";
import assert from "node:assert/strict";
import { cursorCentredFrame, dragImageLayout, ghostLeaves, nextTilt } from "./motion.ts";

test("tilt leans with horizontal motion and is capped at 10 degrees", () => {
  assert.equal(nextTilt(0, 5), 3);
  assert.equal(nextTilt(0, 100), 10);
  assert.equal(nextTilt(0, -100), -10);
});

test("tilt settles back when the cursor stops moving sideways", () => {
  assert.equal(nextTilt(10, 0), 7);
  assert.ok(Math.abs(nextTilt(nextTilt(nextTilt(10, 0), 0), 0)) < 4);
});

test("the card hands off to the OS drag once it starts to cross the top or sides of the window", () => {
  const box = (left: number, top: number) => ({ left, top, right: left + 200, bottom: top + 220 });
  const lifted = box(100, 40);
  assert.equal(ghostLeaves(box(300, 40), lifted, 1200), false);
  assert.equal(ghostLeaves(box(100, -1), lifted, 1200), true);
  assert.equal(ghostLeaves(box(-1, 40), lifted, 1200), true);
  assert.equal(ghostLeaves(box(1001, 40), lifted, 1200), true);
});

test("a card that already overhangs an edge when lifted only hands off once it goes further", () => {
  // The panel sits on the screen's bottom edge: a lifted card overhangs it, and moving down is never a drag out.
  assert.equal(ghostLeaves({ left: 100, top: 90, right: 300, bottom: 310 }, { left: 100, top: 82, right: 300, bottom: 302 }, 1200), false);
  // The last, partly scrolled-in card sticks out on the right from the start.
  const partly = { left: 1100, top: 40, right: 1300, bottom: 260 };
  assert.equal(ghostLeaves(partly, partly, 1200), false);
  assert.equal(ghostLeaves({ ...partly, left: 1110, right: 1310 }, partly, 1200), true);
});

test("the drag image is padded so the grabbed point sits at its centre", () => {
  // Grabbed 30 from the left and 50 from the top of a 200x220 card.
  assert.deepEqual(cursorCentredFrame(200, 220, 30, 50), { width: 340, height: 340, x: 140, y: 120 });
  // Grabbed dead centre: no padding.
  assert.deepEqual(cursorCentredFrame(200, 220, 100, 110), { width: 200, height: 220, x: 0, y: 0 });
});

test("on macOS the drag image is drawn at 2x and centred on the grabbed point", () => {
  assert.deepEqual(dragImageLayout(true, 200, 220, 30, 50, 1), {
    px: 2,
    frame: { width: 340, height: 340, x: 140, y: 120 },
  });
});

test("on Windows the drag image is drawn at device pixels, also centred on the grabbed point", () => {
  // Windows draws the bitmap 1:1 in device pixels; the patched drag crate holds it by its centre.
  assert.deepEqual(dragImageLayout(false, 200, 220, 30, 50, 1.5), {
    px: 1.5,
    frame: { width: 340, height: 340, x: 140, y: 120 },
  });
});
