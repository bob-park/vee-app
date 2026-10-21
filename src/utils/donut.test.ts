import assert from 'node:assert/strict';
import { test } from 'node:test';

import { dropPoint, labelAt, restPath, sliceAt, slicePath } from './donut.ts';

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

void test('a slice path starts just inside the outer ring, rounds its corners and closes', () => {
  const d = slicePath(0, 6, 0);
  assert.match(d, /^M0 -88Q0 -98 /); // outer 98 minus the 10px corner, at 12 o'clock
  assert.equal(d.match(/Q/g)?.length, 4);
  assert.ok(d.endsWith('Z'));
});

/** Perpendicular distance of `p` from the ray at `deg` clockwise from 12 o'clock. */
function offRay(p: [number, number], deg: number) {
  const b = (deg * Math.PI) / 180;
  return Math.abs(p[0] * Math.cos(b) + p[1] * Math.sin(b));
}

void test('the gap between slices is the same width at the inner and outer ring', () => {
  // Slice 0 of 6 spans 0°–60°; with a 10px gap every point on its radial edges sits 5px off the boundary ray.
  const d = slicePath(0, 6, 10);
  const first = d.match(/^M(-?[\d.]+) (-?[\d.]+)/)!;
  assert.ok(Math.abs(offRay([Number(first[1]), Number(first[2])], 0) - 5) < 0.01);
  // The path's last point is the inner end of the same edge.
  const last = d.match(/ (-?[\d.]+) (-?[\d.]+)Z$/)!;
  assert.ok(Math.abs(offRay([Number(last[1]), Number(last[2])], 0) - 5) < 0.01);
  // The `L` point is the inner end of the other edge, on the 60° boundary.
  const l = d.match(/L(-?[\d.]+) (-?[\d.]+)/)!;
  assert.ok(Math.abs(offRay([Number(l[1]), Number(l[2])], 60) - 5) < 0.01);
});

void test('fewer than six formats keep 60° slices, centred on 12 o’clock', () => {
  // Two slices span -60°..0° and 0°..60°.
  assert.match(slicePath(0, 2, 0), /^M-76\.21 -44Q/); // radius 88 at -60°
  assert.equal(sliceAt(129, 50, 2), 0); // just left of 12 o'clock
  assert.equal(sliceAt(131, 50, 2), 1); // just right
  assert.deepEqual(labelAt(0, 1), { x: 0, y: -76 }); // a lone slice sits on top
});

void test('the empty rest of a short donut hits nothing and is drawn as one faint arc', () => {
  assert.equal(sliceAt(130 + 80, 130, 2), null); // 3 o'clock lies past the second slice
  assert.equal(sliceAt(130, 130 + 80, 5), null); // 6 o'clock is outside five 60° slices
  assert.equal(sliceAt(130, 130 + 80, 6), 3); // a full donut still covers it
  assert.ok(restPath(2)?.endsWith('Z'));
  assert.equal(restPath(6), null);
});

void test('drop positions are points on macOS and device pixels on Windows', () => {
  assert.deepEqual(dropPoint({ x: 159, y: 133 }, 2, true), { x: 159, y: 133 });
  assert.deepEqual(dropPoint({ x: 300, y: 250 }, 1.5, false), { x: 200, y: 166.66666666666666 });
});
