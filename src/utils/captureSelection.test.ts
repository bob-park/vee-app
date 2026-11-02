import assert from 'node:assert/strict';
import { test } from 'node:test';

import { normalizeRect, windowAt } from './captureSelection.ts';

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
