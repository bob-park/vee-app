import { test } from "node:test";
import assert from "node:assert/strict";
import { panelKeyAction, type KeyInput } from "./keys.ts";

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
    ...extra,
  });

test("ignores every key while an IME composition is active", () => {
  for (const key of ["Enter", "ArrowLeft", "ArrowRight", "Backspace", "Tab", "Escape"]) {
    assert.equal(press(key, { isComposing: true }), null, key);
  }
});

test("enter copies, escape hides, arrows move", () => {
  assert.deepEqual(press("Enter"), { type: "copy" });
  assert.deepEqual(press("Escape"), { type: "hide" });
  assert.deepEqual(press("ArrowRight"), { type: "move", delta: 1 });
  assert.deepEqual(press("ArrowLeft"), { type: "move", delta: -1 });
});

test("backspace deletes only when the search box is empty", () => {
  assert.deepEqual(press("Backspace"), { type: "delete" });
  assert.equal(press("Backspace", { queryEmpty: false }), null);
  assert.deepEqual(press("Delete", { queryEmpty: false }), { type: "delete" });
});

test("tab cycles filters forward, shift+tab backward", () => {
  assert.deepEqual(press("Tab"), { type: "cycleFilter", delta: 1 });
  assert.deepEqual(press("Tab", { shiftKey: true }), { type: "cycleFilter", delta: -1 });
});

test("other keys are left to the search box", () => {
  assert.equal(press("a"), null);
});

test("holding backspace or delete never deletes more than one card", () => {
  assert.equal(press("Backspace", { repeat: true }), null);
  assert.equal(press("Delete", { repeat: true }), null);
  assert.deepEqual(press("ArrowRight", { repeat: true }), { type: "move", delta: 1 });
});

test("while confirming a delete, enter confirms and other keys cancel", () => {
  assert.deepEqual(press("Enter", { confirming: true }), { type: "confirmDelete" });
  for (const key of ["Escape", "ArrowRight", "Backspace", "Delete", "Tab", "a"]) {
    assert.deepEqual(press(key, { confirming: true }), { type: "cancelDelete" }, key);
  }
});

test("modifiers and the held delete key don't cancel a confirmation", () => {
  for (const key of ["Shift", "Meta", "Control", "Alt"]) {
    assert.equal(press(key, { confirming: true }), null, key);
  }
  assert.equal(press("Delete", { confirming: true, repeat: true }), null);
  assert.equal(press("Backspace", { confirming: true, repeat: true }), null);
});

test("while suggesting apps, arrows, enter and escape drive the list", () => {
  const s = { suggesting: true, queryEmpty: false };
  assert.deepEqual(press("ArrowDown", s), { type: "suggestMove", delta: 1 });
  assert.deepEqual(press("ArrowUp", s), { type: "suggestMove", delta: -1 });
  assert.deepEqual(press("Enter", s), { type: "suggestPick" });
  assert.deepEqual(press("Escape", s), { type: "suggestClose" });
  assert.equal(press("a", s), null);
  // Everything else keeps its usual meaning, so focus never leaves the search box.
  assert.deepEqual(press("Tab", s), { type: "cycleFilter", delta: 1 });
});

test("an IME composition wins over the suggestion list", () => {
  for (const key of ["Enter", "ArrowDown", "Escape"]) {
    assert.equal(press(key, { suggesting: true, isComposing: true }), null, key);
  }
});

test("backspace in an empty box removes the app tag before any card", () => {
  assert.deepEqual(press("Backspace", { hasTag: true }), { type: "clearTag" });
  assert.equal(press("Backspace", { hasTag: true, repeat: true }), null);
  assert.equal(press("Backspace", { hasTag: true, queryEmpty: false }), null);
  assert.deepEqual(press("Delete", { hasTag: true }), { type: "delete" });
});

test("escape with a tag but no open list still hides the panel", () => {
  assert.deepEqual(press("Escape", { hasTag: true }), { type: "hide" });
});
