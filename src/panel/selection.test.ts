import { test } from "node:test";
import assert from "node:assert/strict";
import { clampSelection, defaultSelection } from "./selection.ts";

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
