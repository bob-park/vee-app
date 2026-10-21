import assert from 'node:assert/strict';
import { test } from 'node:test';

import { arrowStep } from './radio.ts';

void test('arrows step with wrap-around', () => {
  assert.equal(arrowStep('ArrowRight', 2, 3), 0);
  assert.equal(arrowStep('ArrowDown', 0, 3), 1);
  assert.equal(arrowStep('ArrowLeft', 0, 3), 2);
  assert.equal(arrowStep('ArrowUp', 1, 3), 0);
});

void test('Home and End jump to the ends; other keys do nothing', () => {
  assert.equal(arrowStep('Home', 2, 3), 0);
  assert.equal(arrowStep('End', 0, 3), 2);
  assert.equal(arrowStep('a', 1, 3), null);
});
