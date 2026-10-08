import { test } from "node:test";
import assert from "node:assert/strict";
import { cancelFloat, floatCard, leftWindow, nextTilt } from "./motion.ts";

test("tilt leans with horizontal motion and is capped at 10 degrees", () => {
  assert.equal(nextTilt(0, 5), 3);
  assert.equal(nextTilt(0, 100), 10);
  assert.equal(nextTilt(0, -100), -10);
});

test("tilt settles back when the cursor stops moving sideways", () => {
  assert.equal(nextTilt(10, 0), 7);
  assert.ok(Math.abs(nextTilt(nextTilt(nextTilt(10, 0), 0), 0)) < 4);
});

test("the card hands off to the OS drag once the cursor leaves the window", () => {
  const w = 1200;
  const h = 300;
  assert.equal(leftWindow(600, 150, w, h, 0), false);
  assert.equal(leftWindow(600, -1, w, h, 0), true);
  assert.equal(leftWindow(-1, 150, w, h, 0), true);
  assert.equal(leftWindow(1200, 150, w, h, 0), true);
  assert.equal(leftWindow(600, 300, w, h, 0), true);
});

test("a top edge margin hands off before the cursor reaches the window edge", () => {
  assert.equal(leftWindow(600, 19, 1200, 300, 20), true);
  assert.equal(leftWindow(600, 20, 1200, 300, 20), false);
});

/** Just enough DOM for floatCard: one card, its clone, and the listeners it registers. */
function fakeDom() {
  const listeners = new Map<string, Set<(e?: unknown) => void>>();
  const target = {
    addEventListener: (type: string, fn: (e?: unknown) => void) => {
      if (!listeners.has(type)) listeners.set(type, new Set());
      listeners.get(type)!.add(fn);
    },
    removeEventListener: (type: string, fn: (e?: unknown) => void) => listeners.get(type)?.delete(fn),
  };
  const body: unknown[] = [];
  const el = (): any => ({
    dataset: {} as Record<string, string>,
    style: {} as Record<string, string>,
    classList: { add() {} },
    getBoundingClientRect: () => ({ left: 10, top: 40, width: 200, height: 220 }),
    cloneNode() {
      const copy = el();
      copy.dataset = { ...this.dataset };
      return copy;
    },
    remove() {
      body.splice(body.indexOf(this), 1);
    },
    animate: () => ({ cancel() {}, onfinish: null }),
  });
  Object.assign(globalThis, {
    window: target,
    document: { body: { appendChild: (n: unknown) => body.push(n) }, documentElement: target },
    innerWidth: 1200,
    innerHeight: 300,
  });
  const count = () => [...listeners.values()].reduce((n, s) => n + s.size, 0);
  const fire = (type: string, e?: unknown) => [...(listeners.get(type) ?? [])].forEach((fn) => fn(e));
  return { card: el(), body, count, fire };
}

test("cancelling a float removes the ghost, the slot and every listener", () => {
  const dom = fakeDom();
  floatCard(dom.card, 50, 100, () => assert.fail("must not start the OS drag"));
  assert.equal(dom.body.length, 1);
  cancelFloat();
  assert.equal(dom.body.length, 0);
  assert.equal("slot" in dom.card.dataset, false);
  assert.equal(dom.count(), 0);
});

test("grabbing a card again while it springs back starts from a clean card", () => {
  const dom = fakeDom();
  floatCard(dom.card, 50, 100, () => {});
  dom.fire("mouseup");
  floatCard(dom.card, 50, 100, () => {});
  assert.equal(dom.body.length, 1);
  assert.equal("slot" in (dom.body[0] as { dataset: object }).dataset, false);
  assert.equal("slot" in dom.card.dataset, true);
  cancelFloat();
});
