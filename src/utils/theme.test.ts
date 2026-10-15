import assert from 'node:assert/strict';
import { test } from 'node:test';

import { resolveTheme } from './theme.ts';

void test('resolveTheme', () => {
  assert.equal(resolveTheme('light', true), 'light');
  assert.equal(resolveTheme('dark', false), 'dark');
  assert.equal(resolveTheme('system', true), 'dark');
  assert.equal(resolveTheme('system', false), 'light');
});
