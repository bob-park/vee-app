# 패널 고정 · 크기 조절 · 버그 2건 Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** 이 계획은 네 가지를 구현한다.
- macOS에서 단축키 변경이 안 되는 버그를 고친다.
- 패널을 열 때와 새 클립이 들어올 때 최신 카드를 선택하게 한다.
- 패널 고정 토글을 추가한다.
- 설정에서 켜는 패널 높이 드래그 조절과 초기화 기능을 추가한다.

**Architecture:**
- **패널 고정:** 고정 상태는 백엔드 `AppState.panel_pinned` 하나가 진실이다. `windows::hide_panel`의 맨 앞에서 숨기기를 막고, 포커스 복원만 남긴다.
- **높이 계산:** 패널 높이는 `windows.rs`의 순수 함수(`panel_height_for`, `clamp_panel_height`)로 계산한다. 저장은 작업 영역 대비 비율(`panelHeightRatio` 설정)로 한다. 프론트는 드래그 중 원하는 높이(design pt)를 `resize_panel`로 보내기만 한다.
- **선택 버그:** 새 클립 도착을 별도 이벤트 `clips://added`로 분리해서 해결한다.

**Tech Stack:** Tauri 2 (Rust), React 19 + TypeScript, `node --test`(`yarn test`), `cargo test`(`src-tauri`).

**Spec:** `docs/superpowers/specs/2026-10-12-panel-pin-resize-design.md`

## Global Constraints

- **의존성:** 새 의존성 없음.
- **플랫폼:** macOS·Windows 모두 동작해야 한다.
- **문자열:** 새 문자열은 `src/i18n/en.ts`(`Dict` 타입 기준)와 `src/i18n/ko.ts` 둘 다에 넣는다.
- **용어:** UI에서 새 기능은 "패널 고정"(Pin panel)이다. 📌는 카드 고정 전용이므로 패널 고정에 쓰지 않는다.
- **높이 범위:**
  - 최소: `panel_height(work_h)`(기존 30%, 300~360pt)
  - 최대: `floor(work_h × 0.5)`. 단, 최소보다 작으면 최소를 쓴다.
- **설정 키:**
  - `panelResize`: `"on"`/`"off"`, 기본 `"off"`
  - `panelHeightRatio`: 빈 문자열이면 기본 크기. 값은 소수 4자리 문자열.
- **디자인 토큰:** `src/theme.css` 변수만 쓴다(`--accent`, `--accent-subtle`, `--muted`, `--subtle`, `--danger`, `--border`).
- **버전:** 0.6.3 → 0.7.0. 바꿀 곳은 `src-tauri/tauri.conf.json`, `package.json`, `src-tauri/Cargo.toml`이고 `Cargo.lock`도 따라 바뀐다.
- **yarn:** 4.x로 실행한다. `yarn --version`이 4.x가 아니면 `corepack yarn@4.18.0 <명령>`을 쓴다.
- **검증 명령:**
  - `yarn test`
  - `yarn typecheck`
  - `cd src-tauri && cargo test`
  - 수동 확인: `yarn tauri dev`

## Review Focus

1. **고정 중 카드를 패널 밖으로 드래그해 놓기:** 지금은 `panel://closed`가 와야 `dragging`이 풀린다. 고정 중에는 닫히지 않으므로 패널이 비켜난 채로 남는다. 놓은 뒤 원위치로 돌아와야 한다(Task 4 Step 5).
2. **고정 중 `Esc`:** `hide_panel` 커맨드는 `restore_focus = true`로 부른다. 막지 않으면 `Esc`가 직전 앱을 앞으로 가져온다. `Esc`는 아무 일도 하지 않아야 한다(Task 5 Step 3).
3. **고정 중 다른 앱 → 설정 창 → 패널 순서로 오가기:** `Focused(false)` 때 기록한 frontmost가 Vee 자신(설정 창)이면 그 값을 쓰지 않는다. 복사 후 포커스는 마지막으로 쓴 다른 앱으로 가야 한다(Task 4 Step 3).
4. **높이를 늘린 뒤 작은 모니터에서 열기:** 저장된 비율이 그 모니터의 최대치를 넘으면 최대치로 클램프한다(Task 3 테스트 `stored_ratio_is_clamped_per_monitor`).
5. **패널이 숨겨진 동안 보관 기간 정리나 카드 삭제가 일어난 경우:** 다음에 열 때 최신 카드가 선택되어야 한다(Task 2 테스트 `closed_panel_never_keeps_selection`).

---

### Task 1: macOS 단축키 녹화 버그

**Files:**
- Modify: `src/settings/ShortcutRecorder.tsx`

**Interfaces:**
- Consumes: 없음
- Produces: 없음(동작 수정)

- [ ] **Step 1: 재현.** `yarn tauri dev`를 실행한다. 설정 → 단축키 버튼을 누르고 `⌥⇧K`를 누른다. 기대하는 실패: 문구가 "녹화 중"에 멈추고 단축키가 바뀌지 않는다.
- [ ] **Step 2: 원인 확인.** `onKeyDown`이 불리는지 `console.log`로 찍어 보고, `document.activeElement`가 버튼인지 확인한다. 가설은 WebKit이 클릭한 `<button>`에 포커스를 주지 않는다는 것이다. 가설이 틀리면 `superpowers:systematic-debugging`으로 진짜 원인을 찾고, 이 Task를 그 원인에 맞게 고친다.
- [ ] **Step 3: 수정.** `onClick`에서 `setRecording(true)` 직후 `e.currentTarget.focus()`를 호출한다. 디버그 로그는 지운다.
- [ ] **Step 4: 수동 검증.**
  - `⌥⇧K` 녹화가 성공하고, 새 단축키로 패널이 열린다.
  - `Esc`를 누르면 녹화가 취소된다.
  - 녹화 중에 현재 단축키 `⌘⇧V`를 누르면 패널이 열리지 않는다. 열린다면 Step 2로 돌아가 원인을 기록하고, 녹화를 시작할 때 전역 단축키를 잠시 해제하는 커맨드가 필요한지 판단한다.
  - 원래 단축키로 되돌려 둔다.
- [ ] **Step 5: Commit**

```bash
git add src/settings/ShortcutRecorder.tsx
git commit -m "fix(settings): focus the shortcut recorder on click so macOS records keys"
```

### Task 2: 패널을 열 때·새 클립이 들어올 때 최신 카드 선택

**Files:**
- Modify: `src/panel/selection.ts`, `src/panel/selection.test.ts`, `src/panel/Panel.tsx:124-150`, `src-tauri/src/watcher.rs:142`

**Interfaces:**
- Produces:
  - `keepSelectionOnReload(panelOpen: boolean, added: boolean): boolean` (`selection.ts`)
  - 이벤트 `clips://added`(페이로드 없음). 새 클립이 저장될 때 `clips://changed` **대신** 보낸다.
  - `Panel.tsx` 내부 함수 `resetView()`. Task 5가 재사용한다.

- [ ] **Step 1: 실패하는 테스트**

```ts
test("a new clip moves the selection to the newest card", () => {
  assert.equal(keepSelectionOnReload(true, true), false);
});
test("other changes keep the selected card while the panel is open", () => {
  assert.equal(keepSelectionOnReload(true, false), true);
});
test("closed_panel_never_keeps_selection", () => {
  assert.equal(keepSelectionOnReload(false, false), false);
  assert.equal(keepSelectionOnReload(false, true), false);
});
```

- [ ] **Step 2:** `yarn test`를 실행한다. 기대 결과: `keepSelectionOnReload` 미정의로 FAIL.
- [ ] **Step 3:** `selection.ts`에 `keepSelectionOnReload`를 구현한다(`panelOpen && !added`).
- [ ] **Step 4:** `yarn test`를 실행한다. 기대 결과: PASS.
- [ ] **Step 5:** `watcher.rs:142`의 `emit("clips://changed")`를 `emit("clips://added")`로 바꾼다.
- [ ] **Step 6:** `Panel.tsx`를 고친다.
  - `clips://changed` → `reloadRef.current(keepSelectionOnReload(openRef.current, false), true)`
  - 새 리스너 `clips://added` → `reloadRef.current(keepSelectionOnReload(openRef.current, true), true)`
  - `panel://closed`의 상태 초기화 묶음(`setConfirmingId(null)`부터 `reload(false)`까지)을 `resetView()`로 뽑아 재사용한다.
- [ ] **Step 7: 수동 검증.**
  - 패널을 열고 닫는다 → 다른 앱에서 복사한다 → 다시 연다. 새 카드가 선택되어 있어야 한다. 5회 반복한다.
  - 패널이 떠 있는 동안 카드를 고정/해제한다. 선택이 그 카드를 따라가야 한다.
- [ ] **Step 8: Commit**

```bash
git add src/panel/selection.ts src/panel/selection.test.ts src/panel/Panel.tsx src-tauri/src/watcher.rs
git commit -m "fix(panel): always select the newest card on open and when a clip arrives"
```

### Task 3: 패널 높이·토스트 위치 계산

**Files:**
- Modify: `src-tauri/src/windows.rs`(상수, `panel_height` 근처, `panel_rect`, `toast_rect`, tests)

**Interfaces:**
- Produces(`windows.rs`, 모듈 내부):
  - `const PANEL_MAX_SHARE: f64 = 0.5;`
  - `fn panel_max(work_h: f64) -> f64`: `floor(work_h × 0.5)`와 `panel_height(work_h)` 중 큰 값
  - `fn clamp_panel_height(work_h: f64, h: f64) -> f64`: `h.round()`를 `[panel_height, panel_max]`로 클램프
  - `fn panel_height_for(work_h: f64, ratio: Option<f64>) -> f64`: `None`이면 `panel_height(work_h)`, 있으면 `clamp_panel_height(work_h, ratio * work_h)`
  - `fn panel_rect(work: Rect, unit: f64, h: f64) -> Rect`: `h`는 design pt. 기존 2인자 버전을 대체한다.
  - `fn toast_rect(work: Rect, unit: f64, lift: f64) -> Rect`: `lift`(design pt)만큼 위로 올린다.
  - `fn stored_ratio(app: &AppHandle) -> Option<f64>`: 설정 `panelHeightRatio`를 파싱한다. 빈 값이나 파싱 실패면 `None`.
  - 모든 높이 인자는 design pt(= `work.h / unit`)이다.

- [ ] **Step 1: 실패하는 테스트**(`mod tests`)

```rust
#[test]
fn panel_can_grow_to_half_the_work_area() {
    assert_eq!(panel_max(1117.0), 558.0);
    assert_eq!(clamp_panel_height(1117.0, 447.2), 447.0);
    assert_eq!(clamp_panel_height(1117.0, 900.0), 558.0);
    assert_eq!(clamp_panel_height(1117.0, 100.0), 335.0); // never below the default
    assert_eq!(panel_max(560.0), 300.0); // tiny screen: the default wins over 50%
}

#[test]
fn stored_ratio_is_clamped_per_monitor() {
    assert_eq!(panel_height_for(1117.0, None), 335.0);
    assert_eq!(panel_height_for(1117.0, Some(0.4)), 447.0);
    assert_eq!(panel_height_for(956.0, Some(0.6)), 478.0); // over 50% on a smaller screen
    assert_eq!(panel_height_for(1440.0, Some(0.1)), 360.0);
}

#[test]
fn pinned_toast_sits_above_the_panel() {
    let work = Rect { x: -2560.0, y: -241.0, w: 2560.0, h: 1410.0 };
    assert_eq!(toast_rect(work, 1.0, 368.0), Rect { x: -1490.0, y: 655.0, w: 420.0, h: 132.0 });
}
```

기존 테스트 `panel_and_toast_sit_at_the_bottom_of_the_work_area`도 새 시그니처에 맞춘다. 값은 바꾸지 않는다.
- `panel_rect(work, 1.0, 360.0)`
- `toast_rect(work, 1.0, 0.0)`
- `panel_rect(px, 1.5, 300.0)`

- [ ] **Step 2:** `cd src-tauri && cargo test windows::`를 실행한다. 기대 결과: 컴파일 에러(미정의)로 FAIL.
- [ ] **Step 3:** 위 함수들을 구현한다. 호출부도 바꾼다.
  - `place_and_show`: `panel_rect(work, unit, panel_height_for(work.h / unit, stored_ratio(app)))`
  - `show_toast`: `lift = 0.0`. Task 4가 고정 시 값을 채운다.
- [ ] **Step 4:** `cargo test`를 실행한다. 기대 결과: 전체 PASS.
- [ ] **Step 5: Commit**

```bash
git add src-tauri/src/windows.rs
git commit -m "feat(panel): compute panel height from a saved ratio, clamped to 50% of the screen"
```

### Task 4: 패널 고정 — 백엔드

**Files:**
- Modify: `src-tauri/src/lib.rs`(`AppState`, `on_window_event`, 커맨드 등록), `src-tauri/src/windows.rs`(`toggle_panel`, `show_panel`, `hide_panel`, `show_toast`, `finish_drag`), `src-tauri/src/source_app.rs`(Windows `activate`)

**Interfaces:**
- Consumes: Task 3의 `toast_rect(work, unit, lift)`, `panel_height_for`, `stored_ratio`
- Produces:
  - `AppState.panel_pinned: AtomicBool`(pub)
  - 커맨드 `set_panel_pinned(pinned: bool)`. 프론트에서는 `invoke("set_panel_pinned", { pinned })`로 부른다.
  - 이벤트 `panel://refocused`(페이로드 없음). 고정된 패널이 단축키나 트레이 "열기"로 포커스를 다시 받을 때 보낸다.
  - 고정 중 드롭이 끝나면 `panel://drag-cancelled`를 보낸다(기존 이벤트 재사용).
  - `pub fn remember_front_app(app: &AppHandle)`(`windows.rs`)

- [ ] **Step 1:** `AppState`에 `panel_pinned`를 추가한다. `set_panel_pinned` 커맨드는 값만 저장한다. 고정을 풀 때 패널을 닫지 않는다.
- [ ] **Step 2:** `hide_panel` 맨 앞에서 고정이면 다음만 하고 `return`한다.
  - `restore_focus`가 참이면 `prev_app_pid`를 **`take()` 없이** 읽어 `source_app::activate`를 호출한다.
- [ ] **Step 3:** `remember_front_app`를 만든다.
  - `source_app::frontmost()`의 pid가 `std::process::id()`와 다를 때만 `prev_app_pid`에 기록한다.
  - `lib.rs`의 `(PANEL, Focused(false))` 분기에서, 고정이면 숨기기 대신 이 함수를 호출한다.
- [ ] **Step 4:** `toggle_panel`과 `show_panel`을 고친다.
  - 패널이 보이고 고정 상태라면 다음과 같이 동작한다.
    - `panel.is_focused()`가 참이면 아무것도 하지 않는다.
    - 아니면 `prev_app_pid = frontmost`, `set_focus()`, `emit_to(PANEL, "panel://refocused")` 순서로 처리한다.
  - `toggle_panel`은 고정 중에 `hide_panel`을 부르지 않는다.
- [ ] **Step 5:** `finish_drag(Dropped)`에서 고정이면 `hide_panel` 대신 `emit_to(PANEL, "panel://drag-cancelled")`를 보낸다. `confirm_copy`는 그대로 둔다.
- [ ] **Step 6:** `show_toast`를 고친다. 고정이면 모니터를 `panel.current_monitor()`로 고르고, `lift = panel_height_for(work.h / unit, stored_ratio(app)) + PANEL_MARGIN`으로 둔다.
- [ ] **Step 7:** Windows `activate(pid)`를 구현한다.
  - `EnumWindows`로 그 pid의 보이는 최상위 창을 찾아 `SetForegroundWindow`를 호출한다. 기존 `windows` 크레이트 피처(`Win32_UI_WindowsAndMessaging`)로 충분하다.
  - 위 doc 주석은 "고정 중에는 패널이 숨지 않으므로 직접 돌려준다"로 바꾼다.
  - macOS에서는 `cargo check --target x86_64-pc-windows-msvc`가 안 될 수 있다. 그러면 컴파일 확인을 Windows PC 수동 검증 항목으로 넘기고 커밋 메시지 본문에 적는다.
- [ ] **Step 8:** `cargo test && yarn typecheck`를 실행한다. 기대 결과: PASS.
- [ ] **Step 9: Commit**

```bash
git add src-tauri/src
git commit -m "feat(panel): keep a pinned panel open and hand focus back after copying"
```

### Task 5: 패널 고정 — UI

**Files:**
- Modify: `src/api.ts`, `src/panel/Panel.tsx`, `src/panel/Toolbar.tsx`, `src/panel/panel.css`, `src/i18n/en.ts`, `src/i18n/ko.ts`

**Interfaces:**
- Consumes: Task 4의 `set_panel_pinned`, `panel://refocused`. Task 2의 `resetView()`.
- Produces:
  - `api.setPanelPinned(pinned: boolean): Promise<void>`
  - `Toolbar`에 추가하는 props: `panelPinned: boolean`, `onTogglePanelPin: () => void`
  - i18n 키:

| 키 | en | ko |
|---|---|---|
| `pinPanel` | `"Pin panel"` | `"패널 고정"` |
| `unpinPanel` | `"Unpin panel"` | `"패널 고정 해제"` |
| `panelPinned` | `"Pinned"` | `"고정됨"` |

- [ ] **Step 1:** `Panel`에 `panelPinned` state를 두고(기본 `false`) 토글하면 `api.setPanelPinned`를 호출한다.
- [ ] **Step 2:** `Toolbar`의 ⚙︎ 바로 왼쪽에 `icon-btn` 버튼을 추가한다.
  - 속성: `aria-pressed={panelPinned}`, `aria-label`·`title`은 상태에 따라 `pinPanel`/`unpinPanel`, `tabIndex={-1}`
  - 켜짐 클래스 `on`: `--accent-subtle` 배경, `--accent` 색
  - 글리프는 📌가 아닌 것을 쓴다. 인라인 SVG 압정 실루엣이나 `⊼`이고, 선택은 구현자 판단이다.
- [ ] **Step 3:** `onKeyDown`의 `"hide"` 액션은 `panelPinned`면 아무것도 하지 않는다.
- [ ] **Step 4:** 고정 중 표시를 추가한다.
  - `.panel.pinned`: `border-color: var(--accent)`, `box-shadow: inset 0 0 0 0.5px var(--accent)`. 창 크기가 패널과 같아서 바깥 그림자는 잘리므로 안쪽에 그린다.
  - 배지 `<span className="panel-pin-badge">{t.panelPinned}</span>`
    - 위치: 패널 위 테두리에 매달린 탭(`position: absolute; top: 0; right: 56px; border-radius: 0 0 6px 6px`)
    - 스타일: `--accent` 배경, 흰 글자, 10px. `.panel`에 `position: relative`를 준다.
- [ ] **Step 5:** `panel://refocused` 리스너를 추가한다: `resetView()` 후 `inputRef.current?.focus()`.
- [ ] **Step 6:** `yarn typecheck && yarn test`를 실행한다. 기대 결과: PASS.
- [ ] **Step 7: 수동 검증**(`yarn tauri dev`, macOS)
  - 고정 → 다른 앱 클릭: 패널이 그대로 있다.
  - `Enter` 복사: 패널이 그대로 있고, 토스트가 패널 위에 뜨며, 바로 `⌘V`로 직전 앱에 붙여넣어진다.
  - `Esc`: 아무 일도 없다(직전 앱으로 포커스가 가지도 않는다).
  - `⌘⇧V`:
    - 다른 앱에 있을 때: 패널에 포커스가 오고, 검색이 비워지고, 최신 카드가 선택된다.
    - 패널에 있을 때: 아무 일도 없다.
  - 트레이 "열기"는 `⌘⇧V`와 같다.
  - 이미지 카드를 Finder로 드래그해 놓기: 패널이 원위치로 돌아온다.
  - `⌘,`: 설정이 열리고 패널은 남는다.
  - 고정 해제: 패널은 열린 채이고, 다른 앱을 클릭하면 내려간다.
- [ ] **Step 8: Commit**

```bash
git add src
git commit -m "feat(panel): pin-panel toggle with pinned ring and badge"
```

### Task 6: 크기 조절 — 백엔드

**Files:**
- Modify: `src-tauri/src/windows.rs`, `src-tauri/src/settings.rs`(`get`, `validate`, `SettingsDto`, `get_settings`, tests), `src-tauri/src/lib.rs`(커맨드 등록)

**Interfaces:**
- Consumes: Task 3의 `clamp_panel_height`, `panel_rect`, `panel_height_for`
- Produces:
  - 커맨드 `resize_panel(height: f64, save: bool) -> Result<(), String>`
    - `height`는 design pt로 받는다.
    - 패널의 현재 모니터(`current_monitor()`)에서 클램프한 뒤, **아래쪽을 고정한 채** `place(panel_rect(...))`로 배치한다.
    - `save`가 참이면 `panelHeightRatio = format!("{:.4}", h / work_h)`로 저장하고 `settings://changed`를 보낸다.
    - `panelResize`가 `"off"`면 `Err("resize_off")`를 돌려준다.
  - 커맨드 `reset_panel_height() -> Result<(), String>`
    - `panelHeightRatio`를 `""`로 저장한다.
    - 패널이 보이면 기본 높이로 다시 배치한다.
    - `settings://changed`를 보낸다.
  - `SettingsDto`에 `panel_resize: String`, `panel_height_ratio: String`을 추가한다(camelCase 직렬화).
  - `validate`
    - `"panelResize"`: `on`/`off`만 허용한다.
    - `"panelHeightRatio"`: 거부한다. 이 값은 커맨드로만 쓴다.

- [ ] **Step 1: 실패하는 테스트**(`settings.rs`)

```rust
#[test]
fn panel_resize_is_a_switch_and_the_ratio_is_command_only() {
    assert!(validate("panelResize", "on").is_ok());
    assert!(validate("panelResize", "maybe").is_err());
    assert!(validate("panelHeightRatio", "0.4").is_err());
}
```

`get` 기본값 테스트에 다음을 추가한다.

```rust
assert_eq!(get(&store, "panelResize"), "off");
assert_eq!(get(&store, "panelHeightRatio"), "");
```

- [ ] **Step 2:** `cargo test settings::`를 실행한다. 기대 결과: FAIL.
- [ ] **Step 3:** 설정 키, DTO, 커맨드 2개를 구현하고 `invoke_handler`에 등록한다.
- [ ] **Step 4:** `cargo test`를 실행한다. 기대 결과: PASS.
- [ ] **Step 5: Commit**

```bash
git add src-tauri/src
git commit -m "feat(panel): resize_panel and reset_panel_height commands"
```

### Task 7: 크기 조절 — 패널 손잡이

**Files:**
- Create: `src/panel/ResizeHandle.tsx`
- Modify: `src/api.ts`(`Settings`, `api`), `src/panel/Panel.tsx`, `src/panel/panel.css`

**Interfaces:**
- Consumes: Task 6의 커맨드와 `Settings.panelResize`
- Produces:
  - `api.resizePanel(height: number, save: boolean): Promise<void>`
  - `api.resetPanelHeight(): Promise<void>`
  - `Settings`에 추가하는 필드: `panelResize: "on" | "off"`, `panelHeightRatio: string`
  - `api.setSetting`의 key 유니온에 `"panelResize"` 추가
  - `<ResizeHandle />`(props 없음). `settings.panelResize === "on"`일 때만 렌더한다.

- [ ] **Step 1:** `ResizeHandle`을 구현한다.
  - 구성: 패널 위 가장자리 전체에 걸친 6px 히트 영역(`cursor: ns-resize`), 가운데에 28×4px 회색 바(`--muted` 50%). hover나 드래그 중이면 바를 `--accent`로 칠한다.
  - `pointerdown`: `setPointerCapture`를 하고 `startY = e.screenY`, `startH = window.innerHeight`를 기록한다.
  - `pointermove`: `h = startH + (startY - e.screenY)`를 `requestAnimationFrame`당 한 번만 `api.resizePanel(h, false)`로 보낸다.
  - `pointerup`: `api.resizePanel(h, true)`
  - `screenY`는 CSS px이고, CSS px는 macOS·Windows 모두 design pt와 같다.
  - 패널의 `onMouseDown`이 `preventDefault`를 하지만 pointer 이벤트에는 영향이 없다.
- [ ] **Step 2:** `Panel.tsx`에서 `.panel` 첫 자식으로 렌더한다. 패널 `padding-top`은 그대로 두고, 손잡이는 `position: absolute`로 둔다.
- [ ] **Step 3:** `yarn typecheck && yarn test`를 실행한다. 기대 결과: PASS.
- [ ] **Step 4: 수동 검증**
  - 설정에서 `panelResize`를 켠다(Task 8 전이면 임시로 `api.setSetting` 콘솔 호출).
  - 위로 끌면 아래쪽은 고정된 채 위로 커지고, 카드·글꼴·파일 아이콘이 비율대로 커진다.
  - 화면의 50%에서 멈추고, 기본 높이 아래로 줄지 않는다.
  - 재시작해도 높이가 유지된다.
  - 끄면 손잡이가 사라지고 높이는 유지된다.
  - 고정 중에도 동작한다.
- [ ] **Step 5: Commit**

```bash
git add src/panel src/api.ts
git commit -m "feat(panel): drag the top edge to resize the panel"
```

### Task 8: 설정 화면 — 토글·초기화·모달

**Files:**
- Create: `src/settings/ConfirmModal.tsx`
- Modify: `src/settings/Settings.tsx`(단축키 Row 위), `src/settings/settings.css`, `src/i18n/en.ts`, `src/i18n/ko.ts`

**Interfaces:**
- Consumes: Task 7의 `api.resetPanelHeight`, `Settings.panelResize`, `Settings.panelHeightRatio`
- Produces:
  - `ConfirmModal({ open, title, body, confirmLabel, onConfirm, onCancel })`
  - `settings.*` i18n 키:

| 키 | en | ko |
|---|---|---|
| `panelResize` | `"Resizable panel"` | `"패널 크기 조절"` |
| `panelResizeHint` | `"Drag the panel's top edge to change its height (up to 50% of the screen)"` | `"패널 위 가장자리를 끌어 높이를 바꿉니다 (최대 화면의 50%)"` |
| `panelSizeReset` | `"Reset panel size"` | `"패널 크기 초기화"` |
| `panelSizeResetBody` | `"Return the panel to its default height. Continue?"` | `"패널 높이를 기본값으로 되돌립니다. 계속할까요?"` |
| `reset` | `"Reset"` | `"초기화"` |

- [ ] **Step 1:** `ConfirmModal`을 네이티브 `<dialog>`로 만든다.
  - `open`이 바뀌면 `showModal()`/`close()`를 호출한다. 네이티브가 `Esc`, 포커스 가두기, `aria-modal`을 처리한다.
  - `cancel` 이벤트와 backdrop 클릭은 `onCancel`로 보낸다.
  - 열릴 때 취소 버튼에 `autoFocus`를 준다.
  - 스타일: 카드 `--card` 배경, radius 12px, 너비 280px. `::backdrop`은 `rgba(0,0,0,.45)`.
  - 버튼: [취소(`t.cancel`)] [초기화(`--danger` 배경, 흰 글자)]
- [ ] **Step 2:** `Settings.tsx`에 두 Row를 추가한다.
  - `panelResize`: 기존 `.switch` 패턴, `api.setSetting("panelResize", …)`
  - `panelSizeReset`: 버튼. `settings.panelHeightRatio === ""`면 `disabled`. 클릭하면 모달을 열고, 확인하면 `run(api.resetPanelHeight())` 후 모달을 닫는다.
- [ ] **Step 3:** `yarn typecheck && yarn test`를 실행한다. 기대 결과: PASS.
- [ ] **Step 4: 수동 검증**
  - 토글을 켜고 끈다.
  - 크기를 바꾼 뒤 초기화 버튼이 활성화되는지 확인한다.
  - 모달에서 `Esc`, 배경 클릭, 취소를 누르면 그대로다.
  - 초기화를 누르면, 떠 있는 패널이 즉시 기본 높이로 돌아가고 버튼이 비활성화된다.
  - 라이트·다크 테마와 한국어·English 모두에서 확인한다.
- [ ] **Step 5: Commit**

```bash
git add src/settings src/i18n
git commit -m "feat(settings): panel resize switch and reset with confirmation"
```

### Task 9: 문서·버전

**Files:**
- Modify: `README.md`, `package.json`, `src-tauri/tauri.conf.json`, `src-tauri/Cargo.toml`(+`Cargo.lock`)

- [ ] **Step 1:** `README.md`를 고친다.
  - 주요 기능에 "패널 고정", "패널 크기 조절"을 각각 한 항목씩 추가한다.
  - "카드 패널" 항목의 "최대 360pt" 문장에 "설정에서 크기 조절을 켜면 화면 높이의 50%까지 늘릴 수 있습니다"를 덧붙인다.
  - 설정 목록에 "패널 크기 조절, 패널 크기 초기화"를 추가한다.
  - 사용법 표에 "패널 고정/해제 | 패널 오른쪽 위 고정 버튼" 행을 추가한다.
- [ ] **Step 2:** 세 파일의 버전을 `0.7.0`으로 올리고 `cd src-tauri && cargo check`를 실행해 `Cargo.lock`을 갱신한다.
- [ ] **Step 3:** 전체 검증 `yarn test && yarn typecheck && (cd src-tauri && cargo test)`를 실행한다. 기대 결과: 모두 PASS.
- [ ] **Step 4: Commit**

```bash
git add README.md package.json src-tauri/tauri.conf.json src-tauri/Cargo.toml src-tauri/Cargo.lock
git commit -m "chore: release 0.7.0"
```

### Task 10: Apache 2.0 라이선스 적용 (사용자 추가 요청, 2026-10-12)

**Files:**
- Create: `LICENSE`
- Modify: `package.json`, `src-tauri/Cargo.toml`, `README.md`

- [ ] **Step 1:** `LICENSE`에 Apache License 2.0 전문을 넣는다. 원문은 `curl -sL https://www.apache.org/licenses/LICENSE-2.0.txt`로 받아 수정 없이 쓴다. 맨 끝 APPENDIX의 저작권 줄 예시는 원문 그대로 둔다.
- [ ] **Step 2:** 라이선스 메타데이터를 넣는다.
  - `package.json`: `"license": "Apache-2.0"`를 `"version"` 다음 줄에 넣는다.
  - `src-tauri/Cargo.toml`: `[package]`에 `license = "Apache-2.0"`를 넣는다.
- [ ] **Step 3:** `README.md` 맨 끝에 `## 라이선스` 섹션을 추가한다. 문구: "[Apache License 2.0](LICENSE)을 따릅니다. Copyright 2026 Bob Park"
- [ ] **Step 4:** `cd src-tauri && cargo check`(Cargo.toml 파싱 확인)와 `yarn typecheck`를 실행한다. 기대 결과: PASS.
- [ ] **Step 5: Commit**

```bash
git add LICENSE package.json src-tauri/Cargo.toml src-tauri/Cargo.lock README.md
git commit -m "chore: license under Apache 2.0"
```
