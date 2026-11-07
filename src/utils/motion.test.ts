import assert from 'node:assert/strict';
import { test } from 'node:test';

import { cubicBezier, cursorCentredFrame, dragImageLayout, easeMove, ghostLeaves, lerpRect, nextTilt } from './motion.ts';

void test('tilt leans with horizontal motion and is capped at 10 degrees', () => {
  assert.equal(nextTilt(0, 5), 3);
  assert.equal(nextTilt(0, 100), 10);
  assert.equal(nextTilt(0, -100), -10);
});

void test('tilt settles back when the cursor stops moving sideways', () => {
  assert.equal(nextTilt(10, 0), 7);
  assert.ok(Math.abs(nextTilt(nextTilt(nextTilt(10, 0), 0), 0)) < 4);
});

void test('the card hands off to the OS drag once it starts to cross any edge of the window', () => {
  const lifted = { left: 100, top: 40, right: 300, bottom: 260 };
  assert.equal(ghostLeaves(lifted, 200, 0, 1200, 800), false);
  assert.equal(ghostLeaves(lifted, 0, -41, 1200, 800), true);
  assert.equal(ghostLeaves(lifted, -101, 0, 1200, 800), true);
  assert.equal(ghostLeaves(lifted, 901, 0, 1200, 800), true);
  // Dragging down out of a top-docked panel.
  assert.equal(ghostLeaves(lifted, 0, 540, 1200, 800), false);
  assert.equal(ghostLeaves(lifted, 0, 541, 1200, 800), true);
});

void test('a card that already overhangs an edge when lifted only hands off once it goes further', () => {
  const low = { left: 100, top: 82, right: 300, bottom: 302 };
  assert.equal(ghostLeaves(low, 0, -2, 1200, 300), false);
  assert.equal(ghostLeaves(low, 0, 8, 1200, 300), true);
  // The last, partly scrolled-in card sticks out on the right from the start.
  const partly = { left: 1100, top: 40, right: 1300, bottom: 260 };
  assert.equal(ghostLeaves(partly, 0, 0, 1200, 800), false);
  assert.equal(ghostLeaves(partly, 10, 0, 1200, 800), true);
});

void test('the drag image is padded so the grabbed point sits at its centre', () => {
  // Grabbed 30 from the left and 50 from the top of a 200x220 card.
  assert.deepEqual(cursorCentredFrame(200, 220, 30, 50), { width: 340, height: 340, x: 140, y: 120 });
  // Grabbed dead centre: no padding.
  assert.deepEqual(cursorCentredFrame(200, 220, 100, 110), { width: 200, height: 220, x: 0, y: 0 });
});

void test('on macOS the drag image is drawn at 2x and centred on the grabbed point', () => {
  assert.deepEqual(dragImageLayout(true, 200, 220, 30, 50, 1), {
    px: 2,
    frame: { width: 340, height: 340, x: 140, y: 120 },
  });
});

void test('on Windows the drag image is drawn at device pixels, also centred on the grabbed point', () => {
  // Windows draws the bitmap 1:1 in device pixels; the patched drag crate holds it by its centre.
  assert.deepEqual(dragImageLayout(false, 200, 220, 30, 50, 1.5), {
    px: 1.5,
    frame: { width: 340, height: 340, x: 140, y: 120 },
  });
});

void test('cubicBezier starts at 0, ends at 1 and clamps outside them', () => {
  assert.equal(easeMove(0), 0);
  assert.equal(easeMove(1), 1);
  assert.equal(easeMove(-1), 0);
  assert.equal(easeMove(2), 1);
});

void test('cubicBezier with linear control points is linear', () => {
  assert.ok(Math.abs(cubicBezier(0, 0, 1, 1)(0.3) - 0.3) < 1e-4);
});

void test('easeMove front-loads the move and only ever rises', () => {
  assert.ok(easeMove(0.5) > 0.85);
  const steps = [0.1, 0.2, 0.3, 0.4, 0.5, 0.6, 0.7, 0.8, 0.9].map(easeMove);
  steps.slice(1).forEach((v, i) => assert.ok(v > steps[i]));
});

void test('lerpRect moves every field the same fraction of the way', () => {
  const a = { x: 0, y: 0, width: 10, height: 20 };
  const b = { x: 10, y: 10, width: 30, height: 40 };
  assert.deepEqual(lerpRect(a, b, 0.5), { x: 5, y: 5, width: 20, height: 30 });
  assert.deepEqual(lerpRect(a, b, 0), a);
  assert.deepEqual(lerpRect(a, b, 1), b);
});
