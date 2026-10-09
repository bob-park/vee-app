import { test } from "node:test";
import assert from "node:assert/strict";
import { nextIndex } from "./dropdownKeys.ts";

test("arrows move and clamp at the ends", () => {
  assert.equal(nextIndex("ArrowDown", 0, 6), 1);
  assert.equal(nextIndex("ArrowDown", 5, 6), 5);
  assert.equal(nextIndex("ArrowUp", 3, 6), 2);
  assert.equal(nextIndex("ArrowUp", 0, 6), 0);
});

test("Home and End jump to the ends", () => {
  assert.equal(nextIndex("Home", 3, 6), 0);
  assert.equal(nextIndex("End", 1, 6), 5);
});

test("no highlight starts from the first item", () => {
  assert.equal(nextIndex("ArrowDown", -1, 6), 0);
  assert.equal(nextIndex("ArrowUp", -1, 6), 0);
});

test("other keys and empty lists are ignored", () => {
  assert.equal(nextIndex("a", 2, 6), null);
  assert.equal(nextIndex("ArrowDown", -1, 0), null);
});
