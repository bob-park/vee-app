import assert from 'node:assert/strict';
import { test } from 'node:test';

import { cancelFloat, floatCard } from './motion.ts';

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

void test('cancelling a float removes the ghost, the slot and every listener', () => {
  const dom = fakeDom();
  floatCard(dom.card, 50, 100, () => assert.fail('must not start the OS drag'));
  assert.equal(dom.body.length, 1);
  cancelFloat();
  assert.equal(dom.body.length, 0);
  assert.equal('slot' in dom.card.dataset, false);
  assert.equal(dom.count(), 0);
});

void test('grabbing a card again while it springs back starts from a clean card', () => {
  const dom = fakeDom();
  floatCard(dom.card, 50, 100, () => {});
  dom.fire('mouseup');
  floatCard(dom.card, 50, 100, () => {});
  assert.equal(dom.body.length, 1);
  assert.equal('slot' in (dom.body[0] as { dataset: object }).dataset, false);
  assert.equal('slot' in dom.card.dataset, true);
  cancelFloat();
});
