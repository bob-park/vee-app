import assert from 'node:assert/strict';
import { test } from 'node:test';

import { formatLabel } from './formatLabel.ts';

void test('720p keeps its lower-case p', () => {
  assert.equal(formatLabel('720p'), '720p');
});

void test('other formats are upper case', () => {
  assert.equal(formatLabel('mp4'), 'MP4');
  assert.equal(formatLabel('webp'), 'WEBP');
  assert.equal(formatLabel('hwpx'), 'HWPX');
});
