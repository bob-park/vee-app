# Top-Dock Drag-Out Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** A floating card hands off to the OS drag when it crosses the window's bottom edge too, so dragging down out of a top-docked panel works.

**Architecture:** `ghostLeaves` (pure, in `src/utils/motion.ts`) gains the window height and checks the bottom edge with the same "only past the lift-time overhang" rule as the other three edges. `floatCard` passes `innerHeight`.

**Tech Stack:** TypeScript, `node:test`.

**Spec:** `docs/superpowers/specs/2026-11-05-top-dock-drag-out-design.md`

## Global Constraints

- Same bottom rule for every dock position (user decision): `box.bottom > Math.max(h, start.bottom)`.
- No change to the drag image, panel slide-out, or Rust drag code.
- Comments in English.

## Review Focus

- Bottom dock: a lifted card already overhangs the bottom; it must not hand off until it moves further down than at lift time (covered by the updated overhang test).
- Top dock, card lifted fully inside: crossing `h` by 1px hands off (covered by the new assertion).

---

### Task 1: Check the bottom edge in `ghostLeaves`

**Files:**
- Modify: `src/utils/motion.ts:10-17`
- Modify: `src/app/panel/_components/motion.ts:138`
- Test: `src/utils/motion.test.ts:16-36`

**Interfaces:**
- Produces: `ghostLeaves(box: Box, start: Box, w: number, h: number): boolean`

- [ ] **Step 1: Update the tests**

In the first test (rename to `'the card hands off to the OS drag once it starts to cross any edge of the window'`), pass `h = 800` to every call and add:

```ts
assert.equal(ghostLeaves(box(100, 581), lifted, 1200, 800), true); // bottom 801 > 800: dragging down out of a top dock
assert.equal(ghostLeaves(box(100, 580), lifted, 1200, 800), false);
```

In the overhang test, pass `h = 300` (bottom dock: lifted bottom 302 already overhangs), replace the comment with "A bottom-docked card overhangs the bottom when lifted; it hands off only once it goes further down.", and change the case to:

```ts
const low = { left: 100, top: 82, right: 300, bottom: 302 };
assert.equal(ghostLeaves({ ...low, top: 80, bottom: 300 }, low, 1200, 300), false);
assert.equal(ghostLeaves({ ...low, top: 90, bottom: 310 }, low, 1200, 300), true);
```

Pass `h = 800` to the `partly` calls.

- [ ] **Step 2: Run to see it fail**

Run: `mise exec -- node --test src/utils/motion.test.ts`
Expected: FAIL on the new `bottom 801` assertion and the `bottom: 310` assertion.

- [ ] **Step 3: Implement**

Add `h: number` to `ghostLeaves`, OR in `box.bottom > Math.max(h, start.bottom)`, and rewrite the doc comment: no edge is exempt, an edge the card already overhung at lift time counts only past that overhang. In `floatCard`, call `ghostLeaves(ghost.getBoundingClientRect(), lifted, innerWidth, innerHeight)`.

- [ ] **Step 4: Run all checks**

Run: `mise exec -- yarn test && mise exec -- yarn typecheck && mise exec -- yarn lint && cargo test --manifest-path src-tauri/Cargo.toml --lib`
Expected: all pass.

- [ ] **Step 5: Manual check**

`mise exec -- yarn tauri dev` (separate identifier if Vee is installed, see `docs/agents/build-run.md`). For each dock (top, bottom, left, right), drag a file card straight down/out into Finder: the OS drag starts without an up-then-down motion. Bottom dock: releasing inside still springs the card back.

- [ ] **Step 6: Commit**

```bash
git add src/utils/motion.ts src/utils/motion.test.ts src/app/panel/_components/motion.ts
git commit -m "fix(panel): hand a card off to the OS drag across the bottom edge too"
```
