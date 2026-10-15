import assert from 'node:assert/strict';
import { test } from 'node:test';

import { type KeyLike, acceleratorFromEvent, formatAccelerator } from './accelerator.ts';

const ev = (over: Partial<KeyLike>): KeyLike => ({
  key: 'v',
  code: 'KeyV',
  metaKey: false,
  ctrlKey: false,
  altKey: false,
  shiftKey: false,
  ...over,
});

void test('Cmd on mac and Ctrl on windows both map to CommandOrControl', () => {
  assert.equal(acceleratorFromEvent(ev({ metaKey: true, shiftKey: true }), true), 'CommandOrControl+Shift+KeyV');
  assert.equal(acceleratorFromEvent(ev({ ctrlKey: true, shiftKey: true }), false), 'CommandOrControl+Shift+KeyV');
  assert.equal(
    acceleratorFromEvent(ev({ ctrlKey: true, altKey: true, code: 'Digit1', key: '1' }), true),
    'Control+Alt+Digit1',
  );
});

void test('waits while only modifier keys are held', () => {
  assert.equal(acceleratorFromEvent(ev({ key: 'Meta', code: 'MetaLeft', metaKey: true }), true), null);
  assert.equal(acceleratorFromEvent(ev({ key: 'Shift', code: 'ShiftLeft', shiftKey: true }), false), null);
});

void test('rejects shortcuts without a real modifier', () => {
  assert.equal(acceleratorFromEvent(ev({}), true), null);
  assert.equal(acceleratorFromEvent(ev({ shiftKey: true }), true), null);
});

void test('formats for display', () => {
  assert.equal(formatAccelerator('CommandOrControl+Shift+V', true), '⌘ ⇧ V');
  assert.equal(formatAccelerator('CommandOrControl+Shift+KeyV', false), 'Ctrl + Shift + V');
  assert.equal(formatAccelerator('Control+Alt+Digit1', true), '⌃ ⌥ 1');
  assert.equal(formatAccelerator('CommandOrControl+Comma', false), 'Ctrl + ,');
});
