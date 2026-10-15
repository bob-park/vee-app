import { test } from "node:test";
import assert from "node:assert/strict";
import { errorText } from "./errorText.ts";

test("an AppError shows its message", () => {
  assert.equal(errorText({ code: "INVALID_INPUT", message: "invalid setting a=b" }), "invalid setting a=b");
});

test("a plain string shows as is", () => {
  assert.equal(errorText("x"), "x");
});
