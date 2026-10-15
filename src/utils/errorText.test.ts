import assert from 'node:assert/strict';
import { test } from 'node:test';

import { errorText } from './errorText.ts';

void test('an AppError shows its message', () => {
  assert.equal(errorText({ code: 'INVALID_INPUT', message: 'invalid setting a=b' }), 'invalid setting a=b');
});

void test('a plain string shows as is', () => {
  assert.equal(errorText('x'), 'x');
});
