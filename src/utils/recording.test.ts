import assert from 'node:assert/strict';
import { test } from 'node:test';

import { formatElapsed, gifRemaining, initialFormat } from './recording.ts';

void test('elapsed time is mm:ss, h:mm:ss past an hour', () => {
  assert.equal(formatElapsed(12_000), '00:12');
  assert.equal(formatElapsed(3_723_000), '1:02:03');
});

void test('gif remaining rounds up and never goes below zero', () => {
  assert.deepEqual(gifRemaining(12_000, 30_000), { seconds: 18, share: 0.4 });
  assert.equal(gifRemaining(31_000, 30_000).seconds, 0);
  assert.equal(gifRemaining(31_000, 30_000).share, 1);
});

void test('macOS without FFmpeg falls back to mp4 for webm and gif', () => {
  assert.equal(initialFormat('gif', false, true), 'mp4');
  assert.equal(initialFormat('gif', true, true), 'gif');
  assert.equal(initialFormat('webm', false, false), 'webm');
  assert.equal(initialFormat('mov', false, true), 'mov');
});
