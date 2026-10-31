import type { PointerEvent } from 'react';

import assert from 'node:assert/strict';
import { test } from 'node:test';

import { pointIn } from './pointer.ts';

void test('pointIn is relative to the element top-left', () => {
  const e = {
    clientX: 30,
    clientY: 50,
    currentTarget: { getBoundingClientRect: () => ({ left: 10, top: 20 }) },
  } as unknown as PointerEvent<Element>;
  assert.deepEqual(pointIn(e), { x: 20, y: 30 });
});
