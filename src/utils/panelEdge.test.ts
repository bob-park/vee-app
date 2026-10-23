import assert from 'node:assert/strict';
import { test } from 'node:test';

import { dragSize, isSide, sidePinsThatFit } from './panelEdge.ts';

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

void test('a MacBook Pro 16" side panel shows one pin with history at the default width, two at the minimum', () => {
  assert.equal(sidePinsThatFit(1728, 1117, null), 1); // 25% = 432pt wide, cards ~400pt
  assert.equal(sidePinsThatFit(1728, 1117, 0.1), 2); // clamped to the 340pt floor
});

void test('a tall screen fits all three pins with history', () => {
  assert.equal(sidePinsThatFit(1080, 1920, null), 3); // portrait: 340pt floor, cards ~308pt
});

void test('a screen too short for even one card beside the toolbar fits no pins', () => {
  assert.equal(sidePinsThatFit(2560, 700, 0.5), 0);
});
