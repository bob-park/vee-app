import assert from 'node:assert/strict';
import { test } from 'node:test';

import { cropFromDrag, resizeKeep, sizeAfter } from './edit.ts';

void test('cropFromDrag maps view to image px and clamps', () => {
  assert.deepEqual(cropFromDrag({ x: 10, y: 10 }, { x: 60, y: 35 }, { w: 100, h: 50 }, { w: 1000, h: 500 }), {
    x: 100,
    y: 100,
    w: 500,
    h: 250,
  });
  assert.deepEqual(cropFromDrag({ x: 90, y: 40 }, { x: 200, y: 99 }, { w: 100, h: 50 }, { w: 1000, h: 500 }), {
    x: 900,
    y: 400,
    w: 100,
    h: 100,
  });
  // Dragging up-left normalises to the same box.
  assert.deepEqual(cropFromDrag({ x: 60, y: 35 }, { x: 10, y: 10 }, { w: 100, h: 50 }, { w: 1000, h: 500 }), {
    x: 100,
    y: 100,
    w: 500,
    h: 250,
  });
});

void test('resizeKeep follows the ratio when locked', () => {
  assert.deepEqual(resizeKeep({ w: 1280, h: 720 }, { w: 800 }, true), { w: 800, h: 450 });
  assert.deepEqual(resizeKeep({ w: 1280, h: 720 }, { w: 800 }, false), { w: 800, h: 720 });
  assert.deepEqual(resizeKeep({ w: 1280, h: 720 }, { h: 360 }, true), { w: 640, h: 360 });
  assert.deepEqual(resizeKeep({ w: 4000, h: 10 }, { w: 100 }, true), { w: 100, h: 1 });
});

void test('sizeAfter mirrors rotate and the upscale cap', () => {
  assert.deepEqual(sizeAfter({ w: 420, h: 180 }, [{ type: 'upscale', scale: 2 }]), { w: 840, h: 360 });
  assert.deepEqual(sizeAfter({ w: 3000, h: 1000 }, [{ type: 'upscale', scale: 4 }]), { w: 4096, h: 1365 });
  assert.deepEqual(sizeAfter({ w: 1000, h: 3000 }, [{ type: 'upscale', scale: 4 }]), { w: 1365, h: 4096 });
  assert.deepEqual(sizeAfter({ w: 100, h: 50 }, [{ type: 'rotate', quarter: 1 }]), { w: 50, h: 100 });
});

void test('sizeAfter applies crop (clamped like Rust) and resize in order', () => {
  const ops = [
    { type: 'crop', x: 80, y: 40, w: 50, h: 50 },
    { type: 'removeBg' },
    { type: 'resize', w: 40, h: 20 },
  ] as const;
  assert.deepEqual(sizeAfter({ w: 100, h: 50 }, [ops[0]]), { w: 20, h: 10 });
  assert.deepEqual(sizeAfter({ w: 100, h: 50 }, [...ops]), { w: 40, h: 20 });
});
