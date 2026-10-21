import assert from 'node:assert/strict';
import { test } from 'node:test';

import { linkParts } from './link.ts';

void test('splits host and keeps the full url', () => {
  assert.deepEqual(linkParts('https://www.tauri.app/reference/config/#x'), {
    host: 'tauri.app',
    rest: 'https://www.tauri.app/reference/config/#x',
  });
});

void test('bare hosts and ports parse', () => {
  assert.equal(linkParts('localhost:3000/x').host, 'localhost');
  assert.equal(linkParts('github.com/bob-park').host, 'github.com');
});

void test('host-less schemes fall back to the scheme', () => {
  assert.equal(linkParts('mailto:a@b.c').host, 'mailto');
});

void test('only the first line counts, trimmed', () => {
  assert.deepEqual(linkParts('  https://a.com/x \nmore'), { host: 'a.com', rest: 'https://a.com/x' });
});

void test('unparseable text never gives an empty host', () => {
  assert.equal(linkParts('not a url/with path').host, 'not a url');
});

void test('very long urls are kept whole (the tile truncates)', () => {
  const long = 'https://x.io/' + 'a'.repeat(2000);
  assert.equal(linkParts(long).rest, long);
});
