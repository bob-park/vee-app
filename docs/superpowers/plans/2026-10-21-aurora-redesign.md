# Aurora Redesign Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Restyle all five Vee windows (panel, settings, toast, convert donut, what's-new) in kiri-app's Aurora Glass language. Every window gets a solid background, a soft aurora glow at the top, glass controls and one shared spring curve. No behaviour changes.

**Architecture:** Everything is CSS plus small component edits; there are no Rust changes. Task 1 adds the tokens, the `.glass`/`.aurora` classes, the keyframes and a shared `<Aurora />` component. Every later task uses them. Pure decisions are pulled into `src/utils/*.ts` with `node:test` tests: link splitting, radio arrow keys and donut geometry. Everything else is verified in `yarn tauri dev`.

**Tech Stack:** Tauri 2, React 19, Tailwind 4 + daisyUI 5 (`themes:false`), CSS keyframes, WAAPI (`motion.ts`), `pretendard` 1.3.9 (new, approved).

**Spec:** `docs/superpowers/specs/2026-10-21-aurora-redesign-design.md`. The chosen mockups are under `.superpowers/brainstorm/60429-1792590347/content/`. Open them in a browser for reference:
- `panel-cards-v2.html` (link option ②)
- `settings-v1.html` (B)
- `small-windows-v1.html`
- `donut-dark-v5.html` (③)

The kiri reference implementation is at `/Users/hwpark/Documents/rust-workspace/kiri-app`:
- `src/app/globals.css`
- `src/shared/components/settings/Segmented.tsx`
- `src/app/settings/_components/ThemePicker.tsx`
- `src/utils/radio.ts`
- `docs/design/aurora-glass.md`

## Global Constraints

### Colours, copy and conventions
- Colours live only in `src/app/globals.css`. No hex or rgb in components. Every new token has a light and a dark value (`docs/agents/design.md`).
- Keep these DOM hooks: `.card` + `data-id`, `data-slot`, `.update`, `.ghost`. Keep the ARIA roles: `listbox`/`option`, `tablist`/`tab`, `radiogroup`/`radio`, `switch`. Keep the `:focus-visible` ring.
- All user-visible text goes in `src/shared/i18n/{en,ko}.ts`, with no hardcoded strings. Follow the component section order in `docs/agents/conventions/react-sections.md`.

### Motion
- One spring curve, `--ease-spring: cubic-bezier(0.2, 0.9, 0.3, 1.2)`. Enter animations run 450–650 ms; exit animations run 150–250 ms ease-in.
- Every animation and transition is disabled under `prefers-reduced-motion: reduce`.
- Don't change animation durations that Rust waits on:
  - `TOAST_EXIT_MS = 180` (toast-out stays 180 ms)
  - `PANEL_CLOSE_MS = 200` (panel close stays 200 ms ease-in)
  - Both are in `src-tauri/src/app/window.rs`.

### Windows and dependencies
- Window sizes stay as they are:
  - panel 1200×300 (height is a ratio of the screen)
  - toast 420×132
  - convert 260×260
  - settings 520×600
  - what's-new 400×480
- The only dependency added is `pretendard` (exact `1.3.9`, like kiri). Ask the user before adding anything else.

### Scope, docs and checks
- No functional change. These stay exactly as they work today:
  - pin, resize, drag (tilt/ghost/slot), FLIP, conversion overlays
  - the delete confirmation and the card footer
  - toast timing, shortcuts
- Code and comments are written in English. Design docs may be in Korean.
- Before a task is done, run:
  - `cargo test --manifest-path src-tauri/Cargo.toml --lib`
  - `yarn test`
  - `yarn typecheck`
  - `yarn lint`
- UI tasks are also checked in `yarn tauri dev` in light and dark, ko and en. If an installed Vee is present, use a separate identifier (see `docs/agents/build-run.md`).

## Review Focus

1. **Dragging a card out.** The `.ghost` clone in `floatCard` copies the card's classes. It must not replay the entrance animation, lift on hover, or blur. Check by dragging a card mid-entrance and after; Task 3 Step 5.
2. **Reopening the panel with ⌘V many times in a row.** The card entrance plays on each open. It must not replay when you type in search, change the filter, or a new clip arrives (that is FLIP's job). Arrow keys and Enter must work during the entrance. Task 3 Step 5.
3. **Reduced motion on.** With *System Settings → Accessibility → Display → Reduce motion*, all of these must be still:
   - aurora drift
   - sliding pills
   - card/section entrances
   - donut halo spin
   - the ↓ bob
   - toast blur

   Task 1 Step 4 adds the CSS; each UI task re-checks it.
4. **Odd "link" text.** The link tile must never crash or show an empty domain. Examples: `localhost:3000/x`, `mailto:a@b.c`, a URL with spaces or a newline, or a 2,000-char URL. Task 4 Step 1 covers this with tests.
5. **Donut slices that need an engine.** These have a two-line label: format plus "needs FFmpeg/LibreOffice/rhwp". The label must still fit inside the thinner, more rounded slice, at 45% opacity, in both themes. Task 6 Step 6.

---

### Task 1: Foundation — font, tokens, glass/aurora classes, motion, design docs

**Files:**
- Modify: `package.json`, `yarn.lock` (via `yarn add pretendard@1.3.9`)
- Modify: `src/app/globals.css`
- Create: `src/shared/components/Aurora.tsx`
- Create: `docs/design/aurora-glass.md`
- Modify: `docs/agents/design.md:13` (the pixel-identical rule), and wherever `docs/agents/design.md` lists `docs/design/` contents

**Interfaces:**
- Produces these CSS tokens as Tailwind colours (light/dark in `[data-theme]` blocks, exposed via `@theme inline`, Vee's existing pattern):

  | Token | Light | Dark |
  | ----- | ----- | ---- |
  | `--color-glass` | `rgba(255,255,255,.58)` | `rgba(30,31,40,.55)` |
  | `--color-glass-edge` | `rgba(255,255,255,.85)` | `rgba(255,255,255,.08)` |
  | `--color-aurora-1` | `#a98bff` | `#5b3bd6` |
  | `--color-aurora-2` | `#7fd4ff` | `#1f6f9e` |
  | `--color-aurora-3` | `#ffb3d9` | `#7a2d63` |
  | `--color-sky` | `#5fc3ff` | `#5fc3ff` |

  `--color-sky` is the second stop of the CTA/link gradient.
- Produces the `--ease-spring` theme variable (`ease-spring` utility) and these animation utilities:
  - `animate-spring-in`: 650ms; from `translateY(-16px) scale(.94)`, `blur(4px)`, `opacity 0`.
  - `animate-pop`: 450ms; scale .3→1.
  - `animate-ring`: 900ms ease-out, 200ms delay. The box-shadow ring is in `--ring-color`, default `var(--color-primary)`, and spreads to 14px.
  - `animate-drift`: 12s ease-in-out infinite alternate.
  - `animate-bob`: 1.6s ease-in-out infinite; translateY 3px.
- Produces the `.glass` component class: glass background and 1px glass-edge border, plus `backdrop-filter: blur(18px) saturate(1.4)`.
- Produces `.aurora`, an absolutely positioned, pointer-events-none, overflow-hidden band, with up to three `.aurora-blob` children. Blob colours are aurora-1/2/3, with `filter: blur(46px)`, `opacity: .4` and `animate-drift` with -4s/-8s delays. The band hugs the top of its positioned parent; height comes from the caller.
- Produces `<Aurora className?: string />` in `src/shared/components/Aurora.tsx`. It renders `<div className={cx('aurora', className)} aria-hidden>` with three `<i className="aurora-blob" />`.

- [ ] **Step 1: Add the font**

  Run `yarn add pretendard@1.3.9`. Then add `@import 'pretendard/dist/web/variable/pretendardvariable.css';` right after `@import 'tailwindcss';` in `src/app/globals.css`. Set `--font-sans: 'Pretendard Variable', Pretendard, -apple-system, 'Helvetica Neue', Arial, sans-serif;` and keep the mono stack.

- [ ] **Step 2: Move the base palette to kiri's Aurora values**

  In both daisyUI themes in `globals.css`:

  | Variable | Light | Dark |
  | -------- | ----- | ---- |
  | `--color-base-100` | `#f6f5fb` | `#121318` |
  | `--color-base-200` | (unchanged) | `#1d1e25` |
  | `--color-base-300` | (unchanged) | `#2c2e38` |
  | `--color-base-content` | (unchanged) | `#ececf2` |
  | `--color-primary` | (unchanged) | `#8b5cff` |
  | `--radius-box` | `0.875rem` (14px) | `0.875rem` (14px) |

  Leave every other token alone.

  `--vee-card` stays solid. Settings sections, dropdowns and modals still use it. Glass is used only where the tasks below say so.

- [ ] **Step 3: Add the tokens, utilities, `.glass`, `.aurora`/`.aurora-blob` and keyframes from the Interfaces block**

  Put the classes in `@layer components`, next to `.ghost`. Update the header comment of `globals.css` so it names `docs/design/aurora-glass.md` as the reference.

- [ ] **Step 4: Add one `@media (prefers-reduced-motion: reduce)` block**

  It disables these:
  - `.aurora-blob` animation
  - `animate-spring-in`, `animate-pop`, `animate-ring` and `animate-bob` (set `animation: none`)
  - transitions on any element using `ease-spring`

  Under reduced motion, elements stay at their final state (opacity 1, no blur). Use kiri's reduced-motion block in `kiri-app/src/app/globals.css` (around line 316) as the model.

- [ ] **Step 5: Create `src/shared/components/Aurora.tsx`** as described in Interfaces.

- [ ] **Step 6: Write `docs/design/aurora-glass.md`** (Korean is fine)

  Base it on kiri's `docs/design/aurora-glass.md`, adapted to Vee. Cover:
  - the palette table (light/dark)
  - the glass recipe, and where glass is used versus solid `bg-card`
  - the "soft aurora": a solid window background, with the aurora only behind the top bar
  - a motion table: enter, exit, the sliding pill (450ms spring), and FLIP and drag, which keep `EASE_MOVE`/`EASE_SPRING` from `motion.ts`
  - the panel open slide, which keeps `EASE_MOVE`
  - the per-window layout from the spec

  Explain why the panel open slide keeps `EASE_MOVE`. The panel window is exactly the panel's size, so the spring's 1.2 overshoot would push the sheet past the window's top edge and clip it.

  Then edit `docs/agents/design.md`:
  - list the new doc next to `kraken-design.md`, and say `aurora-glass.md` wins where the two disagree
  - change "A refactor must not change a single pixel" to say this still holds for refactors, and that visual changes follow `docs/design/aurora-glass.md`

- [ ] **Step 7: Verify**

  Run `yarn typecheck && yarn lint && yarn test`. All must pass. Then run `yarn tauri dev` and open the settings window. Text must render in Pretendard, both Korean and English; compare the glyph shapes against kiri.

- [ ] **Step 8: Commit**

  ```bash
  git add package.json yarn.lock src/app/globals.css src/shared/components/Aurora.tsx docs/design/aurora-glass.md docs/agents/design.md
  git commit -m "feat: add aurora glass tokens, motion and pretendard font"
  ```

---

### Task 2: Radio arrow keys helper

**Files:**
- Create: `src/utils/radio.ts`
- Test: `src/utils/radio.test.ts`

**Interfaces:**
- Produces `arrowStep(key: string, index: number, length: number): number | null`. It behaves like kiri's: Right/Down go to `+1`, Left/Up go to `-1` (both wrap), Home goes to `0`, End goes to `length-1`, and any other key gives `null`.

- [ ] **Step 1: Write the failing tests**

  ```ts
  void test('arrows step with wrap-around', () => {
    assert.equal(arrowStep('ArrowRight', 2, 3), 0);
    assert.equal(arrowStep('ArrowDown', 0, 3), 1);
    assert.equal(arrowStep('ArrowLeft', 0, 3), 2);
    assert.equal(arrowStep('ArrowUp', 1, 3), 0);
  });
  void test('Home and End jump to the ends; other keys do nothing', () => {
    assert.equal(arrowStep('Home', 2, 3), 0);
    assert.equal(arrowStep('End', 0, 3), 2);
    assert.equal(arrowStep('a', 1, 3), null);
  });
  ```

- [ ] **Step 2:** Run `yarn test`. It should FAIL because the module isn't found.

- [ ] **Step 3: Implement `arrowStep`** in `src/utils/radio.ts`.

- [ ] **Step 4:** Run `yarn test`. It should PASS.

- [ ] **Step 5: Commit** with `feat(settings): add radio arrow-key helper`.

---

### Task 3: Panel shell, toolbar and card motion

**Files:**
- Modify: `src/app/panel/PanelApp.tsx` (the root `div` and the list containers, around lines 440–535)
- Modify: `src/app/panel/_components/Toolbar.tsx`
- Modify: `src/app/panel/_components/Card.tsx` (only the root `className`; the body comes in Task 4)
- Modify: `src/app/panel/_components/motion.ts` (`playFlip`'s new-card frames)
- Modify: `src/app/globals.css` (`.ghost`)

**Interfaces:**
- Consumes these from Task 1:
  - `<Aurora />` and `.glass`
  - `ease-spring`, `animate-spring-in`, `animate-ring`

- [ ] **Step 1: Panel root**

  - Keep `bg-base-100`, the border and every translate/transition class exactly as they are. Open keeps `duration-300` + `EASE_MOVE`; close/drag keeps `duration-200 ease-in`.
  - Change `rounded-2xl` to `rounded-[18px]`.
  - Insert `<Aurora className="inset-x-0 top-0 h-[72px]" />` as the first child, so the glow sits behind the toolbar only.
  - Give the toolbar and the card body `relative z-1` so they paint above the aurora.

- [ ] **Step 2: Toolbar glass**

  - Search box: replace `bg-subtle` with `glass`.
  - Icon buttons:
    - Pin and settings: idle `glass text-muted`. The active pin keeps `bg-accent-subtle text-primary`.
    - The settings button's ⚙︎ glyph rotates 45° on hover, via `motion-safe:group-hover:rotate-45 transition-transform duration-300`. Wrap the glyph in a `span` so the `Tip` doesn't rotate.

- [ ] **Step 3: Filter chips become a sliding pill**

  - The `tablist` becomes `glass relative inline-grid auto-cols-fr grid-flow-col gap-1 rounded-box p-[3px]`.
  - Add one `aria-hidden` thumb span `absolute inset-y-[3px] left-[3px] rounded-[9px] bg-primary transition-transform duration-450 ease-spring`.
    - Its width is `calc((100% - 6px - ${(n - 1) * 4}px) / ${n})`.
    - It moves with `transform: translateX(calc(${i} * (100% + 4px)))`, where `i = FILTERS.indexOf(filter)` and `n = FILTERS.length`.
  - Tabs are `relative px-2.5 py-1 text-[12px]`. The selected tab gets `text-primary-content font-semibold` and the rest get `text-muted`.
  - Keep `role="tab"`, `aria-selected` and `tabIndex={-1}`.

- [ ] **Step 4: Cards become glass, lift on hover, and play the open entrance**

  In `Card.tsx`, the root:
  - Replace `bg-card` with `glass`.
  - Keep every border-state branch (confirming, failed, done, selected, pinned) and the `data-slot:*` classes. The state borders override the glass edge.
  - Add a hover lift that skips the slot: `transition-[translate,box-shadow] duration-250 ease-spring not-data-slot:hover:-translate-y-0.5`.

  In `PanelApp.tsx`:
  - Add the state `entering`.
    - Set it to `true` in the same place that flips `open` to true for a new show.
    - Clear it with a 900 ms `setTimeout`, and also when the panel closes.
  - While `entering` is true, put `data-entering` on the panel root.
  - `Card` gets a new prop `enterIndex: number`; `renderCard` passes `Math.min(i, 8)`. The card root sets `style={{ '--i': enterIndex } as React.CSSProperties}`.

  In `globals.css`, add:
  - `[data-entering] .card { animation: spring-in 650ms var(--ease-spring) both; animation-delay: calc(var(--i, 0) * 25ms); }`
  - `.ghost { animation: none !important; translate: none !important; filter: none !important; }`

  The entrance is pure CSS, so it never blocks the keyboard. Don't replay it on search, filter or new clips.

- [ ] **Step 5: Change `playFlip`'s new-card frames**

  - The pop-in becomes the spring-in look: `{ transform: 'translateY(-16px) scale(0.94)', filter: 'blur(4px)', opacity: 0 }` → `{ transform: 'none', filter: 'none', opacity: 1 }`, over 650 ms with `easing: 'cubic-bezier(0.2, 0.9, 0.3, 1.2)'`.
  - Export that easing as `EASE_SPRING_IN` from `motion.ts`, next to `EASE_MOVE`/`EASE_SPRING`.
  - The 1400 ms ring stays as it is.
  - Leave the FLIP slide and `floatCard` untouched.

- [ ] **Step 6: Verify**

  Run `yarn typecheck && yarn lint && yarn test`. Then, in `yarn tauri dev`, check each of these:
  - **Opening:** open the panel five times quickly with the shortcut. The cards de-blur in one after another each time. Arrow keys and Enter work immediately.
  - **No replay:** typing in search and switching filters doesn't replay the entrance. The filter pill slides.
  - **New clip:** copying text elsewhere while the panel is pinned → the new card springs in with a ring.
  - **Dragging (Review Focus 1):** drag a card out during the entrance and after it. The ghost is sharp, doesn't lift, and doesn't replay. The slot stays dashed. The drop springs back.
  - **Resize and pin:** resize the handle; panel pin on and off.
  - **Themes and languages:** check light and dark, ko and en, over a white desktop and a black one.
  - **Reduced motion:** turn it on. Nothing animates, and the cards are fully visible.

- [ ] **Step 7: Commit** with `feat(panel): aurora glass panel, sliding filter and card entrance`.

---

### Task 4: Card content tiles (text and link)

**Files:**
- Create: `src/utils/link.ts`
- Test: `src/utils/link.test.ts`
- Modify: `src/app/panel/_components/Card.tsx` (`Body`)
- Modify: `src/app/globals.css` (link hero gradient token)

**Interfaces:**
- Produces `linkParts(text: string): { host: string; rest: string }`.
  - It takes the first line, trimmed, and parses it with `new URL`.
    - If that fails and there is no scheme, it retries with `https://` in front.
  - `host` is the hostname without a leading `www.`.
    - For schemes with no host (`mailto:`), `host` is the scheme without the colon (`mailto`).
  - `rest` is the full first line as written.
  - When nothing parses, `host` is the first line cut at its first `/`, and `rest` is the whole line. `host` is never `''` for non-empty input.

- [ ] **Step 1: Write the failing tests** (these cover Review Focus 4)

  ```ts
  void test('splits host and keeps the full url', () => {
    assert.deepEqual(linkParts('https://www.tauri.app/reference/config/#x'), {
      host: 'tauri.app',
      rest: 'https://www.tauri.app/reference/config/#x',
    });
  });
  void test('bare hosts and ports parse', () => {
    assert.equal(linkParts('localhost:3000/x').host, 'localhost');
    assert.equal(linkParts('github.com/bob-park').host, 'github.com');
  });
  void test('host-less schemes fall back to the scheme', () => {
    assert.equal(linkParts('mailto:a@b.c').host, 'mailto');
  });
  void test('only the first line counts, trimmed', () => {
    assert.deepEqual(linkParts('  https://a.com/x \nmore'), { host: 'a.com', rest: 'https://a.com/x' });
  });
  void test('unparseable text never gives an empty host', () => {
    assert.equal(linkParts('not a url/with path').host, 'not a url');
  });
  void test('very long urls are kept whole (the tile truncates)', () => {
    const long = 'https://x.io/' + 'a'.repeat(2000);
    assert.equal(linkParts(long).rest, long);
  });
  ```

- [ ] **Step 2:** Run `yarn test`. It should FAIL because the module isn't found.

- [ ] **Step 3: Implement `linkParts`** in `src/utils/link.ts`.

- [ ] **Step 4:** Run `yarn test`. It should PASS.

- [ ] **Step 5: Change `Body` in `Card.tsx`**

  The card keeps its header → body → footer stack and every overlay.

  **Text** stays the current `bg-subtle` `<p>` tile. Add one `aria-hidden` decorative `"`, positioned `absolute right-2 -bottom-1.5` in `text-primary opacity-12 text-[44px] leading-none font-serif`. To do that, make the `<p>`'s wrapper `relative`.

  **Link** is a `bg-subtle rounded-lg overflow-hidden h-[calc(100%-8px)] flex flex-col` tile:
  - A hero, `flex-1 min-h-0 grid place-items-center bg-linear-to-br from-primary/18 to-sky/22 text-primary`, holding the chain icon.
    - The icon is `size-[30%] max-h-9`, using the link path from `ToastApp.tsx`'s `KIND_ICONS.link`.
    - Move that path into a shared `src/shared/components/KindIcon.tsx` exporting `KIND_PATHS` and `KindIcon({ kind, className })`, and switch `ToastApp` to import it.
  - A foot, `border-t border-divider px-2.5 py-1.5`, with:
    - `<b className="block truncate text-[12px]">{host}</b>`
    - `<span className="text-muted block truncate text-[11px]">{rest}</span>`

  `truncate` gives the `…` the spec asks for.

- [ ] **Step 6: Verify**

  Run `yarn typecheck && yarn lint && yarn test`. Then, in `yarn tauri dev`, copy each of these:
  - a long URL
  - `localhost:3000`
  - `mailto:` text
  - multi-line text
  - an image
  - several files

  Check that:
  - link cards are the same size as the other cards
  - the domain and URL end in `…`
  - the footer still shows the char count, and the copy hint when the card is selected
  - the conversion overlay still covers the body
  - both themes look right at a short (15%) and a tall (50%) panel height

- [ ] **Step 7: Commit** with `feat(panel): link card tile with domain and chain icon`.

---

### Task 5: Settings window

**Files:**
- Modify: `src/app/settings/SettingsApp.tsx`
- Modify: `src/app/settings/_components/Tabs.tsx`
- Modify: `src/app/settings/_components/Segmented.tsx`
- Modify: `src/app/settings/_components/Switch.tsx`
- Modify: `src/app/settings/_components/SettingSection.tsx`
- Create: `src/app/settings/_components/ThemePicker.tsx`
- Modify: `src/app/globals.css` (theme preview tokens)

**Interfaces:**
- Consumes `arrowStep` (Task 2), plus these from Task 1: `<Aurora />`, `.glass`, `ease-spring`, `animate-spring-in`.
- Produces `ThemePicker({ value, onChange, labelledBy }: { value: 'system' | 'light' | 'dark'; onChange: (v: 'system' | 'light' | 'dark') => void; labelledBy: string })`. Use the setting's existing theme type from the settings DTO rather than re-declaring the union.

- [ ] **Step 1: Window shell**

  In `SettingsApp`:
  - `<main>` becomes `relative`.
  - Insert `<Aurora className="inset-x-0 top-0 h-[110px]" />` as its first child.
  - Give the tab bar and the body `relative z-1`.
  - The tabpanel `div` gets `key={tab}`, so its sections remount on a tab change.

- [ ] **Step 2: Tabs**

  - Drop the bar's bottom border.
  - Wrap the buttons in a centred `glass relative flex gap-1 rounded-[14px] p-1` holder, with `mt-1.5 mb-3.5`.
  - Add an `aria-hidden` thumb, `absolute top-1 left-1 h-[calc(100%-8px)] w-[72px] rounded-[10px] bg-accent-subtle transition-transform duration-450 ease-spring`, with `transform: translateX(${index * 76}px)`.
    - `index` is the index in the filtered `TABS`, so Windows (no permissions tab) still lines up.
  - The tab buttons lose their own background. The selected tab keeps `text-primary font-semibold`, and its icon gets `scale-110 transition-transform duration-450 ease-spring`.
  - Keep the keyboard handler and the ids as they are.

- [ ] **Step 3: Segmented**

  - The root becomes `relative grid auto-cols-fr grid-flow-col` (`inline-grid` unless `wide`) on `bg-subtle rounded-[10px] p-0.5`.
  - Add a thumb span `bg-card shadow-seg absolute inset-y-0.5 left-0.5 rounded-lg transition-transform duration-450 ease-spring`.
    - Width: `calc((100% - 4px) / n)`.
    - Transform: `translateX(${index * 100}%)`.
    - It renders only when `index >= 0`.
  - The buttons become `relative` and lose `bg-card shadow-seg`. The `pending` dashed red outline, the text colours, `role="radio"` and `aria-checked` stay.
  - Add arrow keys with `arrowStep`: `onKeyDown` on the root, plus roving `tabIndex`, as in kiri's `Segmented.tsx`.

- [ ] **Step 4: Switch and section**

  - In `Switch`, change the knob transition to `[transition:translate_450ms_var(--ease-spring)]` and the background to `[transition:background_200ms]`.
  - In `SettingSection`, replace `border-base-300 bg-card border` with `glass shadow-[0_4px_24px_rgba(16,17,20,.05)]`, and add `animate-spring-in`. Put the shadow in a `shadow-glass` `@utility` in `globals.css`, not inline.
  - Stagger the sections by passing `style={{ animationDelay: \`${i * 60}ms\` }}`. To do that, `SettingSection` accepts an optional `index?: number`. The general tab's sections pass 0/1/2; other tabs pass their order.

- [ ] **Step 5: ThemePicker**

  - Build a `role="radiogroup"` of three 74px buttons (`role="radio"`): system, light, dark. Use roving `tabIndex` and `arrowStep`.
  - Each button holds a 48px-tall preview box and its label (`s.system` / `s.light` / `s.dark`).
  - The preview colours are fixed per depicted theme (one value serves both app themes, as in kiri). Add these tokens to `@theme`:

    | Token | Value |
    | ----- | ----- |
    | `--color-preview-light` | `#f6f5fb` |
    | `--color-preview-light-bar` | `#ffffff` |
    | `--color-preview-dark` | `#121318` |
    | `--color-preview-dark-bar` | `#262833` |

  - The system preview is split diagonally with `linear-gradient(135deg, light 50%, dark 50%)`, written as an arbitrary Tailwind value that references the tokens.
  - In each preview, a primary/35 bar sits at the bottom and a muted line at the top-left.
  - Selected state: `-translate-y-0.5`, a `border-primary` 2px box, and the label in `text-primary font-semibold`. All transitions use `duration-450 ease-spring`.
  - In `SettingsApp`, replace the theme `Segmented` with `ThemePicker`, and give the row's label element an id for `labelledBy`.
    - `Row` needs an optional `labelId?: string` prop, set on the label `div`.

- [ ] **Step 6: Verify**

  Run `yarn typecheck && yarn lint && yarn test`. Then, in `yarn tauri dev`, open settings and check:
  - **Tabs:** click and arrow through every tab. The pill slides and the sections spring in.
  - **Theme:** the picker changes the theme and works with the arrow keys.
  - **Segmented:** language and the other Segmented controls slide.
  - **Retention:** the history tab's retention-period change still shows the red dashed `pending` outline before you confirm.
  - **Switches:** they spring.
  - **Popups and modals:** the dropdown, app picker and confirm modal still look right over the glass sections.
  - **Themes and languages:** check light and dark, ko and en. Reduced motion stops all of the motion.

- [ ] **Step 7: Commit** with `feat(settings): aurora glass settings with sliding tabs and theme picker`.

---

### Task 6: Convert donut

**Files:**
- Modify: `src/utils/donut.ts`
- Test: `src/utils/donut.test.ts`
- Modify: `src/app/convert/ConvertApp.tsx`
- Modify: `src/app/globals.css` (donut tokens, halo)

**Interfaces:**
- `DONUT` becomes `{ size: 260, disc: 106, outer: 98, inner: 54, corner: 10, gap: 10 }`.
  - `gap` is in CSS px: the constant width of the space between slices.
  - These values keep the halo (disc + 4px + blur 8px) inside the 260 window.
- `slicePath(i: number, count: number, gap = DONUT.gap): string`. The gap is now in px, not radians. Each slice's radial edges sit `gap/2` px from the boundary angle, measured perpendicular, so they run parallel to their neighbour's edge. The corners are still rounded by `corner`.
  - Approach: every point on a radial edge at radius `ρ` (the `R-k`, `R`, `r`, `r+k` points) uses the angular trim `asin((gap/2) / ρ)` for its own `ρ`. The arc ends of each corner keep their `k/R`, `k/r` offsets on top of that.
  - Keep the four `Q` corners, built as in the current code.
- `sliceAt` and `labelAt` keep their signatures and use the new radii.

- [ ] **Step 1: Write the failing tests** (update `donut.test.ts`)

  ```ts
  void test('a slice path starts just inside the outer ring, rounds its corners and closes', () => {
    const d = slicePath(0, 4, 0);
    assert.match(d, /^M0 -88Q0 -98 /); // outer 98 minus the 10px corner, at 12 o'clock
    assert.equal(d.match(/Q/g)?.length, 4);
    assert.ok(d.endsWith('Z'));
  });
  void test('the gap between slices is the same width at the inner and outer ring', () => {
    // Slice 0 of 4 starts at 12 o'clock; with a 10px gap its first point (on the radial edge) sits 5px right of the axis.
    const d = slicePath(0, 4, 10);
    const [, x0] = d.match(/^M(-?[\d.]+) /)!;
    assert.ok(Math.abs(Number(x0) - 5) < 0.01);
  });
  ```

  Keep the hit tests. The coordinates they use still land inside 54..98, so leave them unchanged.

- [ ] **Step 2:** Run `yarn test`. It should FAIL on the two new or updated assertions.

- [ ] **Step 3: Implement the new `DONUT` values and `slicePath`** in `src/utils/donut.ts`.

- [ ] **Step 4:** Run `yarn test`. It should PASS.

- [ ] **Step 5: Change the donut window**

  **Plate.** Replace the SVG `<circle>` disc with an HTML plate behind the SVG:
  - a `div` at `absolute`, `size = 2*disc`, centred, `rounded-full bg-base-100 overflow-hidden`, with a 1px edge (`shadow-[0_0_0_1px_var(--color-donut-edge)]` + the existing `shadow-donut` filter), holding `<Aurora className="inset-0" />` at `opacity-45`
  - plus a halo sibling before it, `absolute rounded-full` at `inset = 130 - disc - 4`, with `bg-[conic-gradient(var(--color-primary),var(--color-sky),var(--color-aurora-3),var(--color-primary))] blur-[8px] opacity-50 animate-[spin_8s_linear_infinite] motion-reduce:animate-none`

  The halo is on in both themes.

  **Tokens.** Add these to `globals.css`:

  | Token | Light | Dark |
  | ----- | ----- | ---- |
  | `--color-donut-edge` | `rgba(16,17,20,.10)` | `rgba(255,255,255,.24)` |
  | `--vee-donut` | `#ffffff` | `#34364a` |

  `--vee-donut` is the slice colour.

  **Slices.**
  - Add a `slice-lit` `@utility`: `filter: drop-shadow(0 -1px 0 rgba(255,255,255,.14)) drop-shadow(0 3px 6px rgba(0,0,0,.35))` in dark, and `drop-shadow(0 2px 4px rgba(16,17,20,.10))` in light (via `[data-theme]`).
  - The slice entrance becomes `slice-in` from `scale(.5) rotate(-20deg)` with opacity 0, over 500ms `ease-spring`, with the same stagger (`40 + i*25` ms).
  - The donut entrance becomes `animate-pop`.
  - Hover is `fill-primary scale-[1.07]` with a `[transition:fill_180ms_ease,scale_350ms_var(--ease-spring)]`.

  **Centre.** Remove `open.name` from the centre. Replace it with a 30px round `bg-accent-subtle text-primary animate-bob` ↓ arrow (an SVG arrow, `aria-hidden`) above the existing `t.convert.dropHint`. Leave `OpenPayload.name` in the type, because Rust still sends it.

- [ ] **Step 6: Verify**

  Run `yarn typecheck && yarn lint && yarn test`. Then, in `yarn tauri dev`, drag a card and press the convert key. Check:
  - **Backgrounds:** over a white Finder window, a black desktop and a normal wallpaper, in both themes, the plate and halo are visible and nothing is clipped at the window edge.
  - **Gaps:** the slices don't touch.
  - **Hover:** it springs.
  - **Dropping:** dropping still converts.
  - **Hit areas:** `sliceAt` hits match what you see. Hover near the inner and outer edges.
  - **Engine slices (Review Focus 5):** with FFmpeg, LibreOffice or rhwp missing, the two-line slices still fit and read clearly.
  - **Reduced motion:** the halo, the bob and the entrance are still.

- [ ] **Step 7: Commit** with `feat(convert): rounded donut on an aurora plate with halo`.

---

### Task 7: Toast and what's-new

**Files:**
- Modify: `src/app/toast/ToastApp.tsx`
- Modify: `src/app/whats-new/WhatsNewApp.tsx`
- Modify: `src/app/globals.css` (`toast-in` keyframes, toast tokens)

**Interfaces:**
- Consumes these from Task 1: `<Aurora />`, `.glass`, `animate-pop`, `animate-ring`, `animate-spring-in`, `--color-sky`.
- Consumes `KindIcon` from Task 4.

- [ ] **Step 1: Toast**

  **Card.**
  - It keeps its position, `key` and role.
  - Change `bg-toast-bg` to `bg-base-100`, and `rounded-2xl` to `rounded-[18px]`. Add `overflow-hidden` and `relative`.
  - Insert `<Aurora className="inset-0 opacity-80" />` first.
  - Give the content `relative z-1`.

  **Icon tile.** It becomes `glass size-[44px] rounded-xl`:
  - `text-success` when `ok`, `text-error` when not.
  - It shows `<KindIcon kind={p.kind} />` or `!`.
  - Delete `--color-toast-icon` if nothing else uses it.

  **Check badge.** It becomes `bg-success animate-pop [animation-delay:250ms]` plus `animate-ring` with `--ring-color: var(--color-success)`, combined as in kiri's `JobRow` (two animations on one element via a comma list in an arbitrary `animate-[…]`, or the badge plus a sibling ring span — implementer's choice). Keep the `border-base-100` ring around it.

  **Entrance and exit.**
  - The `toast-in` keyframes become `from { transform: translateY(20px) scale(.96); filter: blur(4px); opacity: 0 }`, at 550 ms `var(--ease-spring)`.
  - `toast-out` stays at 180 ms (it is paired with `TOAST_EXIT_MS`).

- [ ] **Step 2: What's-new**

  **Shell.**
  - The root becomes `relative`.
  - Insert `<Aurora className="inset-x-0 top-0 h-[110px]" />` first.
  - Give the header, notes and footer `relative z-1`.

  **Header.**
  - The version badge gets `animate-pop`.
  - The `h1` gets `animate-spring-in [animation-delay:80ms]`.

  **Notes.** In `render`, each top-level `item` (depth 0) with its nested items becomes a `glass rounded-box px-3.5 py-3 my-2 animate-spring-in` block.
  - Use `list-none` and drop the `ul` bullet.
  - Add the stagger `animationDelay: 160 + n*60 ms`, where `n` counts depth-0 items rendered so far.
  - Keep nested items as indented bullet lines inside the card.
  - Headings and paragraphs stay as they are.

  **OK button.**
  - It becomes `bg-linear-to-br from-primary to-sky text-white active:scale-96 transition-transform duration-150`.
  - Keep `autoFocus`.

- [ ] **Step 3: Verify**

  Run `yarn typecheck && yarn lint && yarn test`. Then, in `yarn tauri dev`:

  **Toast.** Copy text, a link, an image, files, and a failed copy. Each toast rises from blur and the check pops with a green ring. Check it over white and black desktops, in both themes, and while the panel is pinned (the toast sits above it). It hides on time with no clipped shadow.

  **What's-new.** It has no shortcut. Temporarily change the stored last-seen version in a dev copy of the DB (never the installed `com.bobpark.vee` data folder), or use the existing dev path in `docs/agents/build-run.md` if there is one. Then check:
  - the cards stagger in
  - Enter and Esc dismiss it
  - ko and en, light and dark
  - reduced motion

- [ ] **Step 4: Commit** with `feat: aurora toast and what's-new`.

---

### Task 8: Whole-branch verification

- [ ] **Step 1: Run the four checks**

  Run `cargo test --manifest-path src-tauri/Cargo.toml --lib && yarn test && yarn typecheck && yarn lint`. All must pass. Paste the summary lines.

- [ ] **Step 2: Walk through all five windows**

  In `yarn tauri dev`, open each window in four combinations: light+ko, light+en, dark+ko, dark+en. Then repeat once with reduced motion on.

  Every element must use the same spring. Fix any element that still has an old easing (`cubic-bezier(0.34, 1.56, …)` outside `floatCard`, or `120ms` transitions), or any leftover hex value in a component (`grep -rnE "#[0-9a-fA-F]{3,8}\b" src/app src/shared --include=*.tsx`).

- [ ] **Step 3: Note pre-existing rule violations for the PR description**

  Add any rule violations you saw in code you didn't touch to the PR description as follow-ups. Don't fix them here.

- [ ] **Step 4: Commit any fixes** with `fix: aurora polish from full walkthrough`, then hand off to `superpowers:finishing-a-development-branch`.
