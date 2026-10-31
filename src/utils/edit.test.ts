import assert from 'node:assert/strict';
import { test } from 'node:test';

import {
  brushRadius,
  clampPan,
  cropFromDrag,
  dropLastMark,
  previewScale,
  resizeKeep,
  sizeAfter,
  thinPoints,
  toImagePx,
  viewportRect,
} from './edit.ts';

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

void test('previewScale is 1 up to the preview cap, then shrinks to it', () => {
  assert.equal(previewScale({ w: 2048, h: 1000 }), 1);
  assert.equal(previewScale({ w: 1000, h: 4096 }), 0.5);
});

void test('clampPan keeps the view inside the zoomed image', () => {
  const zoomed = { w: 1000, h: 600 };
  const view = { w: 400, h: 300 };
  assert.deepEqual(clampPan({ x: -50, y: -10 }, zoomed, view), { x: 0, y: 0 });
  assert.deepEqual(clampPan({ x: 900, y: 500 }, zoomed, view), { x: 600, y: 300 });
  assert.deepEqual(clampPan({ x: 120, y: 80 }, zoomed, view), { x: 120, y: 80 });
  // Fit: the image is no larger than the view, so there is nothing to pan.
  assert.deepEqual(clampPan({ x: 30, y: 40 }, { w: 400, h: 240 }, view), { x: 0, y: 0 });
});

void test('viewportRect maps the view onto the mini-map', () => {
  const map = { w: 80, h: 48 };
  // Half the image in each direction → half the map, offset by the pan.
  assert.deepEqual(viewportRect({ x: 500, y: 300 }, { w: 1000, h: 600 }, { w: 500, h: 300 }, map), {
    x: 40,
    y: 24,
    w: 40,
    h: 24,
  });
  // A view larger than the image covers the whole map.
  assert.deepEqual(viewportRect({ x: 0, y: 0 }, { w: 200, h: 120 }, { w: 500, h: 300 }, map), {
    x: 0,
    y: 0,
    w: 80,
    h: 48,
  });
});

void test('toImagePx maps view to image px, rounded and clamped', () => {
  const view = { w: 100, h: 50 };
  const image = { w: 6000, h: 3000 };
  assert.deepEqual(toImagePx({ x: 50, y: 25 }, view, image), { x: 3000, y: 1500 });
  assert.deepEqual(toImagePx({ x: -5, y: 999 }, view, image), { x: 0, y: 2999 });
});

void test('brushRadius scales the view radius to image px, at least 1', () => {
  assert.equal(brushRadius(12, { w: 1000, h: 500 }, { w: 6000, h: 3000 }), 72);
  assert.equal(brushRadius(0.1, { w: 1000, h: 500 }, { w: 6000, h: 3000 }), 1);
});

void test('thinPoints drops points too close to the last kept one, keeps both ends', () => {
  assert.deepEqual(
    thinPoints(
      [
        [0, 0],
        [1, 0],
        [2, 0],
        [10, 0],
        [11, 0],
      ],
      4,
    ),
    [
      [0, 0],
      [10, 0],
      [11, 0],
    ],
  );
});

void test('sizeAfter leaves the size alone for brush and subject', () => {
  assert.deepEqual(
    sizeAfter({ w: 10, h: 10 }, [
      { type: 'brush', strokes: [] },
      { type: 'subject', points: [] },
    ]),
    { w: 10, h: 10 },
  );
});

void test('dropLastMark takes back one click or stroke, null when none is left', () => {
  const p = (x: number) => ({ x, y: 0, keep: true });
  assert.deepEqual(dropLastMark({ type: 'subject', points: [p(1), p(2)] }), { type: 'subject', points: [p(1)] });
  assert.equal(dropLastMark({ type: 'subject', points: [p(1)] }), null);
  const s = (r: number) => ({ mode: 'erase' as const, radius: r, points: [[0, 0]] as [number, number][] });
  assert.deepEqual(dropLastMark({ type: 'brush', strokes: [s(1), s(2)] }), { type: 'brush', strokes: [s(1)] });
  assert.equal(dropLastMark({ type: 'brush', strokes: [s(1)] }), null);
});
