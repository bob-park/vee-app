import assert from 'node:assert/strict';
import { test } from 'node:test';

import { normalizeRect, overlayShapes, windowAt } from './captureSelection.ts';

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

void test('window mode tints the hovered window and keeps the planned box without dimming', () => {
  assert.deepEqual(overlayShapes(true, r(1), null, r(2)), {
    dim: false,
    hole: null,
    tint: r(1),
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
