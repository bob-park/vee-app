import assert from 'node:assert/strict';
import { test } from 'node:test';

import { type KeyInput, panelKeyAction } from './keys.ts';

const press = (key: string, extra: Partial<KeyInput> = {}) =>
  panelKeyAction({
    key,
    shiftKey: false,
    isComposing: false,
    queryEmpty: true,
    repeat: false,
    confirming: false,
    suggesting: false,
    hasTag: false,
    metaOrCtrl: false,
    code: '',
    panelPinCombo: false,
    resetCombo: false,
    pinnedCopy: null,
    justCancelledDrag: false,
    side: false,
    ...extra,
  });

void test('ignores every key while an IME composition is active', () => {
  for (const key of ['Enter', 'ArrowLeft', 'ArrowRight', 'Backspace', 'Tab', 'Escape']) {
    assert.equal(press(key, { isComposing: true }), null, key);
  }
});

void test('enter copies, escape hides, arrows move', () => {
  assert.deepEqual(press('Enter'), { type: 'copy', plain: false });
  assert.deepEqual(press('Escape'), { type: 'hide' });
  assert.deepEqual(press('ArrowRight'), { type: 'move', delta: 1 });
  assert.deepEqual(press('ArrowLeft'), { type: 'move', delta: -1 });
});

void test('backspace deletes only when the search box is empty', () => {
  assert.deepEqual(press('Backspace'), { type: 'delete' });
  assert.equal(press('Backspace', { queryEmpty: false }), null);
  assert.deepEqual(press('Delete', { queryEmpty: false }), { type: 'delete' });
});

void test('tab cycles filters forward, shift+tab backward', () => {
  assert.deepEqual(press('Tab'), { type: 'cycleFilter', delta: 1 });
  assert.deepEqual(press('Tab', { shiftKey: true }), { type: 'cycleFilter', delta: -1 });
});

void test('other keys are left to the search box', () => {
  assert.equal(press('a'), null);
});

void test('holding backspace or delete never deletes more than one card', () => {
  assert.equal(press('Backspace', { repeat: true }), null);
  assert.equal(press('Delete', { repeat: true }), null);
  assert.deepEqual(press('ArrowRight', { repeat: true }), { type: 'move', delta: 1 });
});

void test('while confirming a delete, enter confirms and other keys cancel', () => {
  assert.deepEqual(press('Enter', { confirming: true }), { type: 'confirmDelete' });
  for (const key of ['Escape', 'ArrowRight', 'Backspace', 'Delete', 'Tab', 'a']) {
    assert.deepEqual(press(key, { confirming: true }), { type: 'cancelDelete' }, key);
  }
});

void test("modifiers and the held delete key don't cancel a confirmation", () => {
  for (const key of ['Shift', 'Meta', 'Control', 'Alt']) {
    assert.equal(press(key, { confirming: true }), null, key);
  }
  assert.equal(press('Delete', { confirming: true, repeat: true }), null);
  assert.equal(press('Backspace', { confirming: true, repeat: true }), null);
});

void test('while suggesting apps, arrows, enter and escape drive the list', () => {
  const s = { suggesting: true, queryEmpty: false };
  assert.deepEqual(press('ArrowDown', s), { type: 'suggestMove', delta: 1 });
  assert.deepEqual(press('ArrowUp', s), { type: 'suggestMove', delta: -1 });
  assert.deepEqual(press('Enter', s), { type: 'suggestPick' });
  assert.deepEqual(press('Escape', s), { type: 'suggestClose' });
  assert.equal(press('a', s), null);
  // Everything else keeps its usual meaning, so focus never leaves the search box.
  assert.deepEqual(press('Tab', s), { type: 'cycleFilter', delta: 1 });
});

void test('an IME composition wins over the suggestion list', () => {
  for (const key of ['Enter', 'ArrowDown', 'Escape']) {
    assert.equal(press(key, { suggesting: true, isComposing: true }), null, key);
  }
});

void test('backspace in an empty box removes the app tag before any card', () => {
  assert.deepEqual(press('Backspace', { hasTag: true }), { type: 'clearTag' });
  assert.equal(press('Backspace', { hasTag: true, repeat: true }), null);
  assert.equal(press('Backspace', { hasTag: true, queryEmpty: false }), null);
  assert.deepEqual(press('Delete', { hasTag: true }), { type: 'delete' });
});

void test('escape with a tag but no open list still hides the panel', () => {
  assert.deepEqual(press('Escape', { hasTag: true }), { type: 'hide' });
});

void test('shift+enter copies as plain text', () => {
  assert.deepEqual(press('Enter', { shiftKey: true }), { type: 'copy', plain: true });
  // In the app list, shift+enter still picks the app.
  assert.deepEqual(press('Enter', { shiftKey: true, suggesting: true }), { type: 'suggestPick' });
});

void test('cmd/ctrl+p toggles the pin; a plain p is typed into search', () => {
  assert.deepEqual(press('p', { code: 'KeyP', metaOrCtrl: true }), { type: 'togglePin' });
  assert.deepEqual(press('P', { code: 'KeyP', metaOrCtrl: true, shiftKey: true }), { type: 'togglePin' });
  assert.equal(press('p'), null);
  // Holding the shortcut must not flip the pin back and forth.
  assert.equal(press('p', { code: 'KeyP', metaOrCtrl: true, repeat: true }), null);
});

void test('pin and plain copy keys are ignored while composing or confirming', () => {
  assert.equal(press('p', { code: 'KeyP', metaOrCtrl: true, isComposing: true }), null);
  assert.equal(press('Enter', { shiftKey: true, isComposing: true }), null);
  assert.deepEqual(press('p', { code: 'KeyP', metaOrCtrl: true, confirming: true }), { type: 'cancelDelete' });
});

void test('cmd/ctrl+p pins by physical key, so the Korean input source works too', () => {
  assert.deepEqual(press('ㅔ', { code: 'KeyP', metaOrCtrl: true }), { type: 'togglePin' });
  assert.equal(press('ㅔ', { code: 'KeyP' }), null);
});

void test('cmd/ctrl+comma opens settings once per press', () => {
  assert.deepEqual(press(',', { code: 'Comma', metaOrCtrl: true }), { type: 'openSettings' });
  assert.equal(press(',', { code: 'Comma', metaOrCtrl: true, repeat: true }), null);
  assert.equal(press(',', { code: 'Comma' }), null);
});

void test('the panel pin shortcut wins over card pin and ignores auto-repeat', () => {
  const combo = { metaOrCtrl: true, shiftKey: true, code: 'KeyP', panelPinCombo: true };
  assert.deepEqual(press('P', combo), { type: 'togglePanelPin' });
  assert.equal(press('P', { ...combo, repeat: true }), null);
  assert.deepEqual(press('p', { metaOrCtrl: true, code: 'KeyP' }), { type: 'togglePin' });
});

void test('the reset shortcut clears the search once per press', () => {
  const combo = { metaOrCtrl: true, code: 'KeyK', resetCombo: true };
  assert.deepEqual(press('k', combo), { type: 'resetSearch' });
  assert.equal(press('k', { ...combo, repeat: true }), null);
  assert.equal(press('k', { ...combo, isComposing: true }), null);
  assert.deepEqual(press('k', { ...combo, suggesting: true }), { type: 'resetSearch' });
});

void test('a pinned-copy shortcut copies that pinned card once per press', () => {
  const combo = { metaOrCtrl: true, code: 'Digit2', pinnedCopy: 1 as const };
  assert.deepEqual(press('2', combo), { type: 'copyPinned', index: 1 });
  assert.equal(press('2', { ...combo, repeat: true }), null);
});

void test('a pinned-copy shortcut is returned even when no card is pinned there', () => {
  // keys.ts doesn't know the pin count; returning the action keeps the digit out of the search box.
  assert.deepEqual(press('3', { metaOrCtrl: true, code: 'Digit3', pinnedCopy: 2 }), { type: 'copyPinned', index: 2 });
});

void test('a pinned-copy shortcut loses to composition and delete confirmation', () => {
  const combo = { metaOrCtrl: true, code: 'Digit1', pinnedCopy: 0 as const };
  assert.equal(press('1', { ...combo, isComposing: true }), null);
  assert.deepEqual(press('1', { ...combo, confirming: true }), { type: 'cancelDelete' });
});

void test('a pinned-copy shortcut set to cmd+p beats the fixed card pin combo', () => {
  assert.deepEqual(press('p', { metaOrCtrl: true, code: 'KeyP', pinnedCopy: 0 }), { type: 'copyPinned', index: 0 });
});

void test('the escape that cancelled a drag does not also hide the panel', () => {
  assert.equal(press('Escape', { justCancelledDrag: true }), null);
  assert.deepEqual(press('Escape'), { type: 'hide' });
  // Only escape is swallowed; other keys right after a cancel still work.
  assert.deepEqual(press('Enter', { justCancelledDrag: true }), { type: 'copy', plain: false });
});

void test('side panel moves between cards with up and down', () => {
  assert.deepEqual(press('ArrowDown', { side: true }), { type: 'move', delta: 1 });
  assert.deepEqual(press('ArrowUp', { side: true }), { type: 'move', delta: -1 });
  assert.deepEqual(press('ArrowRight', { side: true }), { type: 'move', delta: 1 });
  assert.equal(press('ArrowDown'), null);
});

void test('side panel up and down still drive an open suggestion list', () => {
  assert.deepEqual(press('ArrowDown', { side: true, suggesting: true }), { type: 'suggestMove', delta: 1 });
});
