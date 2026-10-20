import assert from 'node:assert/strict';
import { test } from 'node:test';

import { dropPoint, sliceAt, slicePath } from './donut.ts';

void test('slices run clockwise from the top', () => {
  assert.equal(sliceAt(130, 130 - 80, 6), 0); // straight up
  assert.equal(sliceAt(130 + 80, 130, 6), 1); // right: 90° falls in slice 1 of 6 (60°–120°)
  assert.equal(sliceAt(130, 130 + 80, 6), 3); // down
});

void test('centre, outside and empty donuts hit nothing', () => {
  assert.equal(sliceAt(130, 130, 6), null);
  assert.equal(sliceAt(130 + 120, 130, 6), null);
  assert.equal(sliceAt(130, 50, 0), null);
});

void test('just left of 12 o’clock is the last slice', () => {
  assert.equal(sliceAt(129, 50, 6), 5);
});

void test('a slice path starts on the outer ring and closes', () => {
  const d = slicePath(0, 4, 0);
  assert.match(d, /^M0 -112A112 112 0 0 1 /);
  assert.ok(d.endsWith('Z'));
});

void test('drop positions are points on macOS and device pixels on Windows', () => {
  assert.deepEqual(dropPoint({ x: 159, y: 133 }, 2, true), { x: 159, y: 133 });
  assert.deepEqual(dropPoint({ x: 300, y: 250 }, 1.5, false), { x: 200, y: 166.66666666666666 });
});
