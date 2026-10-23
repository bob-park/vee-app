import assert from 'node:assert/strict';
import { test } from 'node:test';

import { dragSize, isSide, pinsThatFit } from './panelEdge.ts';

void test('dragging the inner edge outward grows the panel on every edge', () => {
  const s = { size: 300, x: 500, y: 500 };
  assert.equal(dragSize('bottom', s, 500, 450), 350);
  assert.equal(dragSize('top', s, 500, 550), 350);
  assert.equal(dragSize('left', s, 550, 500), 350);
  assert.equal(dragSize('right', s, 450, 500), 350);
});

void test('only left and right are side positions', () => {
  assert.deepEqual((['bottom', 'top', 'left', 'right'] as const).filter(isSide), ['left', 'right']);
});

void test('a 432pt-wide side panel on a MacBook Pro 16" fits one pin with history, a 340pt one two', () => {
  assert.equal(pinsThatFit(432, 1101), 1); // cards ~400pt
  assert.equal(pinsThatFit(340, 1101), 2);
});

void test('a tall side panel fits all three pins with history', () => {
  assert.equal(pinsThatFit(340, 1904), 3); // portrait screen
});

void test('a panel too short for even one card beside the toolbar fits no pins', () => {
  assert.equal(pinsThatFit(1280, 684), 0);
});
