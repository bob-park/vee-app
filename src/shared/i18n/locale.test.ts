import assert from 'node:assert/strict';
import { test } from 'node:test';

import { resolveLocale } from './locale.ts';

void test('an explicit language wins over the system one', () => {
  assert.equal(resolveLocale('ko', 'en-US'), 'ko');
  assert.equal(resolveLocale('en', 'ko-KR'), 'en');
});

void test('system follows the OS language, Korean or else English', () => {
  assert.equal(resolveLocale('system', 'ko-KR'), 'ko');
  assert.equal(resolveLocale('system', 'KO'), 'ko');
  assert.equal(resolveLocale('system', 'fr-FR'), 'en');
});
