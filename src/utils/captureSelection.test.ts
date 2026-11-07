import assert from 'node:assert/strict';
import { test } from 'node:test';

import { coveredParts, normalizeRect, overlayShapes, rectPath, windowAt } from './captureSelection.ts';

void test('a drag in any direction becomes a top-left rect', () => {
  assert.deepEqual(normalizeRect({ x: 50, y: 50 }, { x: 10, y: 20 }), { x: 10, y: 20, width: 40, height: 30 });
});

void test('a drag thinner than 4 px is no selection', () => {
  assert.equal(normalizeRect({ x: 0, y: 0 }, { x: 3, y: 100 }), null);
});

const win = (id: number, x: number, y: number, width: number, height: number) => ({
  id,
  pid: id,
  app: `app${id}`,
  x,
  y,
  width,
  height,
});

void test('overlapping windows pick the front one', () => {
  const windows = [win(1, 100, 100, 200, 200), win(2, 0, 0, 400, 400)];
  assert.equal(windowAt(windows, { x: 150, y: 150 })?.id, 1);
  assert.equal(windowAt(windows, { x: 50, y: 50 })?.id, 2);
});

void test('a point outside every window hits nothing', () => {
  assert.equal(windowAt([win(1, 100, 100, 200, 200)], { x: 500, y: 500 }), null);
});

const r = (x: number) => ({ x, y: 0, width: 10, height: 10 });

void test('window mode tints the hovered window and dims nothing until a window is picked', () => {
  assert.deepEqual(overlayShapes(true, r(1), null, null), {
    dim: false,
    hole: null,
    tint: r(1),
    outline: null,
    planned: null,
  });
});

void test('a window picked for recording dims the rest, like a dragged area', () => {
  assert.deepEqual(overlayShapes(true, null, null, r(2)), {
    dim: true,
    hole: r(2),
    tint: null,
    outline: null,
    planned: r(2),
  });
});

void test('a drag replaces the planned box in region mode', () => {
  assert.deepEqual(overlayShapes(false, null, r(3), r(2)), {
    dim: true,
    hole: r(3),
    tint: null,
    outline: r(3),
    planned: null,
  });
  assert.deepEqual(overlayShapes(false, null, null, r(2)), {
    dim: true,
    hole: r(2),
    tint: null,
    outline: null,
    planned: r(2),
  });
});

void test('only the overlapping windows in front cover a window, whole so their corners round', () => {
  const front = win(1, 150, 150, 100, 100);
  const back = win(2, 0, 0, 200, 200);
  const behind = win(3, 0, 0, 500, 500);
  const apart = win(4, 300, 300, 10, 10);
  assert.deepEqual(coveredParts([apart, front, back, behind], back), [{ x: 150, y: 150, width: 100, height: 100 }]);
  assert.deepEqual(coveredParts([front, back], front), []);
});

void test('rectPath is square at radius 0 and rounds the corners otherwise', () => {
  const rect = { x: 10, y: 20, width: 100, height: 50 };
  assert.equal(rectPath(rect, 0), 'M10 20h100v50h-100Z');
  assert.equal(
    rectPath(rect, 12),
    'M22 20h76a12 12 0 0 1 12 12v26a12 12 0 0 1 -12 12h-76a12 12 0 0 1 -12 -12v-26a12 12 0 0 1 12 -12Z',
  );
  // A radius bigger than half the shorter side is capped there.
  assert.ok(rectPath({ x: 0, y: 0, width: 10, height: 6 }, 12).startsWith('M3 0h4a3 3'));
});
