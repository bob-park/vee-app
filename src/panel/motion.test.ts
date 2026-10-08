import { test } from "node:test";
import assert from "node:assert/strict";
import { cancelFloat, cursorCentredFrame, dragImageLayout, floatCard, ghostLeaves, nextTilt } from "./motion.ts";

test("tilt leans with horizontal motion and is capped at 10 degrees", () => {
  assert.equal(nextTilt(0, 5), 3);
  assert.equal(nextTilt(0, 100), 10);
  assert.equal(nextTilt(0, -100), -10);
});

test("tilt settles back when the cursor stops moving sideways", () => {
  assert.equal(nextTilt(10, 0), 7);
  assert.ok(Math.abs(nextTilt(nextTilt(nextTilt(10, 0), 0), 0)) < 4);
});

test("the card hands off to the OS drag once it starts to cross the top or sides of the window", () => {
  const box = (left: number, top: number) => ({ left, top, right: left + 200, bottom: top + 220 });
  const lifted = box(100, 40);
  assert.equal(ghostLeaves(box(300, 40), lifted, 1200), false);
  assert.equal(ghostLeaves(box(100, -1), lifted, 1200), true);
  assert.equal(ghostLeaves(box(-1, 40), lifted, 1200), true);
  assert.equal(ghostLeaves(box(1001, 40), lifted, 1200), true);
});

test("a card that already overhangs an edge when lifted only hands off once it goes further", () => {
  // The panel sits on the screen's bottom edge: a lifted card overhangs it, and moving down is never a drag out.
  assert.equal(ghostLeaves({ left: 100, top: 90, right: 300, bottom: 310 }, { left: 100, top: 82, right: 300, bottom: 302 }, 1200), false);
  // The last, partly scrolled-in card sticks out on the right from the start.
  const partly = { left: 1100, top: 40, right: 1300, bottom: 260 };
  assert.equal(ghostLeaves(partly, partly, 1200), false);
  assert.equal(ghostLeaves({ ...partly, left: 1110, right: 1310 }, partly, 1200), true);
});

test("the drag image is padded so the grabbed point sits at its centre", () => {
  // Grabbed 30 from the left and 50 from the top of a 200x220 card.
  assert.deepEqual(cursorCentredFrame(200, 220, 30, 50), { width: 340, height: 340, x: 140, y: 120 });
  // Grabbed dead centre: no padding.
  assert.deepEqual(cursorCentredFrame(200, 220, 100, 110), { width: 200, height: 220, x: 0, y: 0 });
});

test("on macOS the drag image is drawn at 2x and centred on the grabbed point", () => {
  assert.deepEqual(dragImageLayout(true, 200, 220, 30, 50, 1), {
    px: 2,
    frame: { width: 340, height: 340, x: 140, y: 120 },
  });
});

test("on Windows the drag image is drawn at device pixels, also centred on the grabbed point", () => {
  // Windows draws the bitmap 1:1 in device pixels; the patched drag crate holds it by its centre.
  assert.deepEqual(dragImageLayout(false, 200, 220, 30, 50, 1.5), {
    px: 1.5,
    frame: { width: 340, height: 340, x: 140, y: 120 },
  });
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
