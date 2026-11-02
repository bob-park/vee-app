# agent-desktop UI Testing Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Agents verify UI changes by driving the running Vee with `agent-desktop`; the rule lives in AGENTS.md and is proven by one real run.

**Architecture:** Docs-only change. Run the real test first so only commands that worked go into AGENTS.md.

**Tech Stack:** agent-desktop 1.0.2 (global npm CLI, already installed under mise Node), Tauri 2 dev build.

**Spec:** `docs/superpowers/specs/2026-11-02-agent-desktop-ui-testing-design.md`

## Global Constraints

- Branch: `feature/agent-desktop`. No app code, no dependency changes.
- Dev app runs as `com.bobpark.vee.dev`; never touch `com.bobpark.vee` data.
- No `--headed` without the user's explicit OK.
- AGENTS.md text in English.

## Review Focus

- Installed Vee running at the same time: `clipboard-set` lands in both; dev shortcut must not clash.
- Webview AX tree may be empty/flat for Tauri WKWebView → note the fallback (`screenshot`) if so.
- Tray-only app: no window until opened; `snapshot --app` returns `WINDOW_NOT_FOUND` before that.
- Stale refs after theme/locale switch re-render → always re-snapshot.
- Leftover dev process blocks the next `yarn tauri dev` → quit via tray / `close-app` at the end.

---

### Task 1: Real run against the dev app

**Files:** none (scratchpad screenshots only)

- [ ] **Step 1:** `agent-desktop permissions` → accessibility + screen_recording `granted`. Read `agent-desktop skills get desktop`.
- [ ] **Step 2:** Start `yarn tauri dev --config '{"identifier":"com.bobpark.vee.dev"}'` in background; wait until `agent-desktop list-apps` shows it.
- [ ] **Step 3:** Seed 2–3 text items with `agent-desktop clipboard-set "…"`.
- [ ] **Step 4:** Open panel headless via `snapshot --surface menubar -i` → tray item → open panel. If impossible, stop and ask the user for `--headed`.
- [ ] **Step 5:** `snapshot --app <name> -i` → seeded items visible; select one; re-snapshot/`get` to confirm.
- [ ] **Step 6:** Open Settings → General; for each of light/dark × 한국어/English: switch, re-snapshot, `screenshot` to scratchpad, confirm labels/theme changed.
- [ ] **Step 7:** Restore original theme/locale, quit the dev app. Record which commands worked and which failed.

### Task 2: AGENTS.md section

**Files:** Modify `AGENTS.md` (Always list + new `## UI testing with agent-desktop` section before `## Map`)

- [ ] **Step 1:** Add Always bullet: "**UI/UX testing:** drive the running app with `agent-desktop` yourself — see [UI testing with agent-desktop](#ui-testing-with-agent-desktop)."
- [ ] **Step 2:** Write the section with only Task 1-verified commands: setup/permissions, `skills get`, dev identifier launch, snapshot→act→verify loop, opening the panel, clipboard caveat, headed rule, the check list (panel flow, light/dark × ko/en), quit at end.
- [ ] **Step 3:** Verify: `yarn test`, `yarn typecheck`, `yarn lint`, `cargo test --manifest-path src-tauri/Cargo.toml --lib` all pass (no code changed, sanity only).
- [ ] **Step 4:** Commit `docs: agent-desktop UI testing in AGENTS.md` with spec + plan.
