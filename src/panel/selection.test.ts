import { test } from "node:test";
import assert from "node:assert/strict";
import { clampSelection, defaultSelection, followSelection } from "./selection.ts";

test("opening selects the newest history card, right after the pins", () => {
  assert.equal(defaultSelection(0, 5), 0);
  assert.equal(defaultSelection(2, 5), 2);
  assert.equal(defaultSelection(3, 1), 3);
});

test("with no history the first pin is selected", () => {
  assert.equal(defaultSelection(2, 0), 0);
  assert.equal(defaultSelection(0, 0), 0);
});

test("selection stays inside the combined pinned + history list", () => {
  assert.equal(clampSelection(-1, 4), 0);
  assert.equal(clampSelection(4, 4), 3);
  assert.equal(clampSelection(2, 4), 2);
  // A card moving between areas can shrink the list under the selection.
  assert.equal(clampSelection(5, 3), 2);
  assert.equal(clampSelection(1, 0), 0);
});

test("a reload keeps the same card selected when it moves between areas", () => {
  // History card 12 at index 3 (after pins 1, 2) gets pinned: it is now the third pin.
  assert.equal(followSelection(12, 3, [1, 2, 12, 11, 13]), 2);
  // Pin 1 at index 0 gets unpinned and returns to the front of the history.
  assert.equal(followSelection(1, 0, [2, 1, 11]), 1);
});

test("when the selected card is gone the index is kept, so the next card is selected", () => {
  assert.equal(followSelection(12, 1, [11, 13]), 1);
  assert.equal(followSelection(12, 5, [11, 13]), 1);
  assert.equal(followSelection(undefined, 0, [11]), 0);
});
