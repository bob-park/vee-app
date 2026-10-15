# 설정 탭 · 호버 선택 · 텍스트 드래그 · 복사 디바운스 · 새 소식 창 구현 계획

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** 설정 화면을 4개 탭으로 나누고, 패널 호버 선택·텍스트 카드 드래그·클립보드 0.5초 디바운스·업데이트 후 "새 소식" 창을 추가한다.

**Architecture:** Tauri 2(Rust) 백엔드 + React 프런트. 디바운스는 `watcher.rs`에서 감시 스레드 → 채널 → 기록 워커 스레드로 나눈다. 텍스트 드래그는 벤더링한 `drag` 크레이트에 `DragItem::Text`를 추가한다. 새 소식은 `latest.json`의 `notes` → 업데이터가 받을 때 설정 저장소에 저장 → 다음 실행 때 새 `whats-new` 창이 보여준다.

**Tech Stack:** Rust(tauri 2, clipboard-rs 0.3.5, 벤더 `drag`), React 19 + TypeScript, `node --test`(TS 직접 실행), `cargo test`.

**Spec:** `docs/superpowers/specs/2026-10-15-settings-tabs-whats-new-design.md`

## Global Constraints

- 새 npm/cargo 의존성 없음. 벤더 `drag` 크레이트 수정은 허용(변경 사항은 `src-tauri/Cargo.toml`의 기존 벤더 주석에 한 줄 추가).
- macOS·Windows 모두 동작.
- 새 UI 문자열은 `src/i18n/ko.ts`와 `src/i18n/en.ts` 둘 다(`Dict` 타입이 강제). 새 소식 본문은 릴리스 본문 그대로.
- 원클릭 복사 설정은 만들지 않는다. 복사는 더블클릭·`Enter`.
- 디바운스 0.5초, macOS 폴링 200ms — 상수, 설정 없음.
- 디자인 토큰은 `src/theme.css`의 CSS 변수만 사용. 버튼 라운드 12px.
- 버전 0.7.1 → 0.8.0.
- 테스트: 프런트 `yarn test`, 타입 `yarn typecheck`, 백엔드 `cd src-tauri && cargo test`. yarn은 4.x(`yarn --version` 확인, 아니면 `corepack yarn@4.18.0`).

## Review Focus

1. 패널이 커서 바로 아래에서 열리거나, `←` `→`/휠로 카드가 가만있는 커서 밑을 지나갈 때 선택이 튀면 안 된다 → Task 2의 `pointerMoved` 테스트.
2. Vee가 직접 클립보드에 쓴 내용(카드 복사)이 디바운스 대기 중이던 변화와 겹쳐도 자기 쓰기가 기록되면 안 된다 → Task 1에서 기록 시점에도 억제 확인.
3. 릴리스 본문에 HTML·`<script>`가 섞여 있어도 텍스트로만 보여야 한다 → Task 7의 `parseNotes` 테스트.
4. `notes`가 없는 `latest.json`(옛 릴리스)이나 빈 본문이면 새 소식 창이 뜨지 않아야 한다 → Task 5·6 테스트.
5. 삭제 확인 중 마우스가 다른 카드를 스쳐도 확인이 취소되지 않아야 한다 → Task 2 Step 3(호버 경로에서 `setConfirmingId` 호출 안 함), 수동 확인.

---

### Task 1: 클립보드 0.5초 디바운스

**Files:**
- Modify: `src-tauri/src/watcher.rs` (`Handler`, `impl ClipboardHandler`, `spawn`, tests)
- Modify: `src-tauri/src/lib.rs:40` (주석: 폴링 200ms), `lib.rs:293` 테스트 주석

**Interfaces:**
- Produces: `const QUIET: Duration = Duration::from_millis(500);`, `const POLL: Duration = Duration::from_millis(200);`, `fn wait_quiet(rx: &std::sync::mpsc::Receiver<()>, quiet: Duration) -> bool`

- [ ] **Step 1: 실패하는 테스트 작성** (`watcher.rs` tests)

```rust
#[test]
fn waits_until_changes_stop_for_the_quiet_period() {
    let (tx, rx) = std::sync::mpsc::channel();
    let start = std::time::Instant::now();
    std::thread::spawn(move || {
        for _ in 0..4 {
            std::thread::sleep(Duration::from_millis(150));
            tx.send(()).unwrap();
        }
        std::thread::sleep(Duration::from_secs(2)); // keep the sender alive
    });
    assert!(wait_quiet(&rx, Duration::from_millis(300)));
    let waited = start.elapsed();
    assert!(waited >= Duration::from_millis(900), "{waited:?}"); // 4×150 + 300
    assert!(waited < Duration::from_millis(1500), "{waited:?}");
}

#[test]
fn stops_waiting_when_the_watcher_is_gone() {
    let (tx, rx) = std::sync::mpsc::channel::<()>();
    drop(tx);
    assert!(!wait_quiet(&rx, Duration::from_millis(300)));
}
```

- [ ] **Step 2: 실패 확인** — `cd src-tauri && cargo test watcher::` → `cannot find function wait_quiet`로 컴파일 실패.

- [ ] **Step 3: 구현**
  - `wait_quiet`: `recv_timeout(quiet)`을 반복. `Ok` → 계속, `Timeout` → `true`, `Disconnected` → `false`.
  - `Handler`는 `{ app: AppHandle, tx: Sender<()> }`만 가진다. `on_clipboard_change`: `app.state::<AppState>().watcher_suppressed()`면 무시(변화 감지 시점 억제), 아니면 `tx.send(())`.
  - 기록 워커 스레드(`spawn` 안에서 생성): 자기 `ClipboardContext`를 만들고 `loop { rx.recv()` 실패 시 종료 → `wait_quiet(&rx, QUIET)`가 false면 종료 → `capture()` }. `capture`는 지금 `Handler::capture` 본문을 옮긴 것(맨 앞의 `watcher_suppressed()` 확인 유지 = 기록 시점 억제). macOS `autoreleasepool` 감싸기와 에러 로그도 워커로 옮긴다.
  - 감시기는 `ClipboardWatcherContext::new_with_interval(POLL)`로 만든다(Windows는 인자 무시).
  - `lib.rs`의 `suppress_watcher` 주석을 "200ms 폴링 + 0.5초 디바운스"로 고친다. 1200ms 값과 기존 테스트는 그대로.

- [ ] **Step 4: 통과 확인** — `cd src-tauri && cargo test` → 모두 PASS.

- [ ] **Step 5: 수동 확인** — `yarn tauri dev`, 터미널(iTerm2 "selection copies" 켜기 등)에서 긴 텍스트를 천천히 드래그 선택 → 패널에 완성된 선택 하나만 카드로 생긴다. 카드 더블클릭 복사 후 그 내용이 새 카드로 다시 생기지 않는다.

- [ ] **Step 6: 커밋** — `git commit -m "feat(watcher): debounce clipboard changes by 0.5s"`

---

### Task 2: 패널 호버 선택

**Files:**
- Modify: `src/panel/selection.ts`, `src/panel/selection.test.ts`
- Modify: `src/panel/Card.tsx` (props, `onMouseMove`)
- Modify: `src/panel/Panel.tsx` (`renderCard`, 선택 effect `:192-198`, 패널 열림/닫힘 리스너)

**Interfaces:**
- Produces: `export type Point = { x: number; y: number }`, `export function pointerMoved(last: Point | null, x: number, y: number): boolean` (selection.ts); `Card` prop `onHover: (screenX: number, screenY: number) => void`

- [ ] **Step 1: 실패하는 테스트 작성** (`selection.test.ts`)

```ts
test("the first pointer event is only a baseline", () => {
  assert.equal(pointerMoved(null, 10, 10), false);
});
test("a mousemove at the same spot (content scrolled under the cursor) is not a move", () => {
  assert.equal(pointerMoved({ x: 10, y: 10 }, 10, 10), false);
});
test("a real move selects", () => {
  assert.equal(pointerMoved({ x: 10, y: 10 }, 11, 10), true);
});
```

- [ ] **Step 2: 실패 확인** — `yarn test` → `pointerMoved` 없음으로 FAIL.

- [ ] **Step 3: 구현**
  - `pointerMoved`: `last !== null && (last.x !== x || last.y !== y)`.
  - `Card`: 기존 `onMouseMove` 첫 줄에서 `onHover(e.screenX, e.screenY)` 호출(드래그 처리 로직은 그대로).
  - `Panel`: `lastPointer = useRef<Point | null>(null)`, `hoverSelected = useRef(false)`.
    `onHover={(x, y) => { const moved = pointerMoved(lastPointer.current, x, y); lastPointer.current = { x, y }; if (!moved || dragging || i === selected) return; hoverSelected.current = true; setSelected(i); }}`
    — 호버 경로는 `setConfirmingId`를 호출하지 않는다.
  - 선택 effect: `hoverSelected.current`가 true면 false로 되돌리고 `scrollIntoView`만 건너뛴다(`loadMore` 확인은 유지).
  - 패널이 열리거나 닫힐 때(`panel://closed` 등 기존 리스너) `lastPointer.current = null`.

- [ ] **Step 4: 통과 확인** — `yarn test && yarn typecheck` → PASS.

- [ ] **Step 5: 수동 확인** — `yarn tauri dev`: 마우스로 카드를 훑으면 선택이 따라오고 `Enter`로 그 카드가 복사됨 / 커서를 카드 위에 둔 채 단축키로 패널을 열면 최신 카드가 선택된 채 유지 / 커서를 둔 채 `→` 연타·휠 스크롤 시 선택이 커서 쪽으로 튀지 않음 / 반쯤 보이는 끝 카드에 올려도 목록이 스크롤되지 않음 / 삭제 확인 중 다른 카드를 스쳐도 확인 유지.

- [ ] **Step 6: 커밋** — `git commit -m "feat(panel): select cards on hover"`

---

### Task 3: 텍스트·링크 카드 드래그 앤 드롭

**Files:**
- Modify: `src-tauri/vendor/drag/src/lib.rs` (`DragItem`), `platform_impl/macos/mod.rs`, `platform_impl/windows/mod.rs`, `platform_impl/gtk/mod.rs`
- Modify: `src-tauri/vendor/drag/Cargo.toml` (Windows `windows` 크레이트에 필요한 feature가 있으면 추가: `Win32_System_DataExchange` 등)
- Modify: `src-tauri/Cargo.toml:46` 벤더 주석
- Modify: `src-tauri/src/windows.rs` (`start_drag` `ClipContent::Text` 분기, 문서 주석)
- Modify: `src/panel/Panel.tsx` (`onDragOut` 조건, `dragOut`)

**Interfaces:**
- Produces: `DragItem::Text(String)` — 놓은 곳에 일반 텍스트로 들어간다.
- Consumes: 기존 `finish_drag` → `confirm_copy` 흐름(변경 없음, 스펙: 파일 드롭과 동일 처리).

- [ ] **Step 1: 벤더 크레이트에 `DragItem::Text(String)` 추가**
  - macOS: `NSPasteboardItem`에 `setString_forType(text, NSPasteboardTypeString)`로 넣고 `NSDraggingItem`을 만든다(기존 `Data` 분기와 같은 프레임·이미지 처리).
  - Windows: `SHCreateDataObject(None, None, None)`로 빈 셸 데이터 객체를 만들고, `CF_UNICODETEXT`(NUL 끝 UTF-16)를 `GlobalAlloc(GMEM_MOVEABLE)` HGLOBAL로 `SetData(..., true)`. 드롭 소스·드래그 이미지·`DoDragDrop`·콜백은 `Files` 분기와 같은 코드를 재사용(함수로 뽑아 공유해도 됨). `options.mode` 존중.
  - gtk: `Data`처럼 지원 안 함(빈 드래그).
  - `src-tauri/Cargo.toml` 벤더 주석에 "DragItem::Text 추가(macOS·Windows)" 한 줄.

- [ ] **Step 2: `start_drag`에서 텍스트 연결** — `ClipContent::Text(text)` → `DragItem::Text(text.clone())`. 파일 존재 확인·이미지 스테이징은 파일/이미지 분기에만. 함수 문서 주석의 "files or image clip"을 "any clip"으로.

- [ ] **Step 3: 프런트 연결** — `Panel.tsx`의 `onDragOut` 조건을 `!clip.missing`으로(모든 종류). `dragOut`의 `dragFileName(...)`은 이미지에만 의미가 있지만 그대로 넘겨도 무방.

- [ ] **Step 4: 빌드 확인** — `cd src-tauri && cargo check && cargo test`, `yarn typecheck` → 성공. Windows 쪽은 `cargo check --target x86_64-pc-windows-msvc`가 가능하면 실행, 불가하면 Windows PC에서 확인하도록 커밋 메시지·PR에 명시.

- [ ] **Step 5: 수동 확인 (macOS)** — `yarn tauri dev`: 텍스트 카드를 TextEdit/VS Code/브라우저 입력란에 끌어 놓으면 놓은 자리에 글자가 들어감 / 링크 카드는 URL이 들어감 / 클립보드 내용은 그대로 / "복사됨" 알림·효과음·카드 맨 앞으로 / 패널 안에 놓으면 제자리. Windows는 메모장·브라우저 입력란에서 같은 확인(Windows PC).

- [ ] **Step 6: 커밋** — `git commit -m "feat(panel): drag text and link cards into other apps"`

---

### Task 4: 설정 화면 탭 (macOS 툴바 스타일)

**Files:**
- Create: `src/settings/Tabs.tsx`
- Modify: `src/settings/Settings.tsx`, `src/settings/HistorySection.tsx`, `src/settings/settings.css`
- Modify: `src/i18n/en.ts`, `src/i18n/ko.ts`

**Interfaces:**
- Produces: `export type SettingsTab = "general" | "panel" | "shortcuts" | "history"`, `export function Tabs({ value, onChange }: { value: SettingsTab; onChange: (tab: SettingsTab) => void })`; i18n `settings.tabs: Record<SettingsTab, string>` — ko: 일반·패널·단축키·기록, en: General·Panel·Shortcuts·History.

- [ ] **Step 1: `Tabs` 컴포넌트** — 아이콘(유니코드/인라인 SVG, 빌더 판단) + 라벨 버튼 4개, 가운데 정렬. `role="tablist"`/`role="tab"`/`aria-selected`. 선택 탭: `background: var(--accent-subtle); color: var(--accent); font-weight: 600; border-radius: 10px`. 비선택: `color: var(--muted)`. 탭 줄 아래 `1px solid var(--divider)`. 목업 `settings-tabs.html` C안.

- [ ] **Step 2: `Settings` 재구성** — `const [tab, setTab] = useState<SettingsTab>("general")`. `<h1>` 삭제. `role="tabpanel"` 안에 탭별 섹션:
  - general: 테마, 언어, 로그인 시 자동 실행, `UpdateRow`
  - panel: 복사할 때 소리 재생, 효과음, 개별 삭제 시 확인, 패널 크기 조절, 패널 크기 초기화
  - shortcuts: 단축키, 패널 고정 단축키
  - history: `HistorySection`(+ 맨 위에 `DiskAccessRow`). `HistorySection`의 `sec-title` 제목은 탭이 대신하므로 삭제.
  - `.error`와 패널 크기 초기화 `ConfirmModal`은 탭 바깥에 그대로.
  - 탭 줄은 스크롤되지 않게 고정(내용 영역만 스크롤). 창 크기 520×600 유지.

- [ ] **Step 3: 확인** — `yarn typecheck && yarn test` PASS. `yarn tauri dev`로 설정 창: 탭 4개 전환, 모든 기존 설정 동작, 라이트/다크 테마, 기록 탭에서 앱 선택 팝업이 창 밖으로 잘리지 않음.

- [ ] **Step 4: 커밋** — `git commit -m "feat(settings): group settings into toolbar tabs"`

---

### Task 5: `latest.json`에 릴리스 본문 넣기

**Files:**
- Modify: `scripts/latest-json.mjs`, `scripts/latest-json.test.mjs`
- Modify: `docs/release.md` (절차 4 앞에 한 줄)

**Interfaces:**
- Produces: `buildManifest(tag, sigs, now = new Date(), notes = "")` — `notes`가 공백이 아니면 매니페스트에 `notes` 필드(트림한 본문)를 넣고, 비면 필드 자체를 넣지 않는다.

- [ ] **Step 1: 실패하는 테스트**

```js
test("carries the release body as notes", () => {
  const m = buildManifest("v0.2.0", [{ name: "Vee_0.2.0_x64-setup.exe.sig", body: "s" }], now, "## 변경 사항\n- a\n");
  assert.equal(m.notes, "## 변경 사항\n- a");
});
test("omits notes when the body is blank", () => {
  const m = buildManifest("v0.2.0", [{ name: "Vee_0.2.0_x64-setup.exe.sig", body: "s" }], now, "  \n");
  assert.equal("notes" in m, false);
});
```

- [ ] **Step 2: 실패 확인** — `yarn test` → FAIL.
- [ ] **Step 3: 구현** — `buildManifest`에 `notes` 인자. 메인 블록에서 `gh release view <tag> -R REPO --json body -q .body`로 본문을 읽어 넘기고, 본문이 비면 경고만 출력. `docs/release.md`: "`latest-json.mjs` 전에 릴리스 본문을 완성한다. 본문이 그대로 앱의 새 소식 창에 나온다."
- [ ] **Step 4: 통과 확인** — `yarn test` PASS.
- [ ] **Step 5: 커밋** — `git commit -m "feat(release): include release notes in latest.json"`

---

### Task 6: 새 소식 — 백엔드 (저장·창 띄우기)

**Files:**
- Create: `src-tauri/src/whats_new.rs`
- Modify: `src-tauri/src/updater.rs` (`check`의 `Ok(Some(...))` 분기), `src-tauri/src/lib.rs` (mod, setup, `on_window_event`, `generate_handler!`), `src-tauri/src/windows.rs` (`pub const WHATS_NEW: &str = "whats-new"`), `src-tauri/tauri.conf.json` (창 정의)

**Interfaces:**
- Produces:
  - 저장 키(설정 테이블, `set_setting` 검증 목록에는 넣지 않음 → 프런트가 못 씀): `whatsNewVersion`, `whatsNewNotes`
  - `pub fn save(store: &Store, version: &str, notes: Option<&str>)` — 노트가 비거나 없으면 저장하지 않음
  - `pub fn due(store: &Store, current: &str) -> Option<WhatsNew>` — 저장된 버전 == `current`이고 노트가 비지 않을 때만 `Some`
  - `#[derive(Serialize)] #[serde(rename_all = "camelCase")] pub struct WhatsNew { version: String, notes: String }`
  - 커맨드 `get_whats_new(app) -> Option<WhatsNew>`(현재 앱 버전 기준 `due`), `dismiss_whats_new(app)` — 두 키를 `""`로 쓰고 창 숨김
  - 창 `whats-new`: `create: false`, `visible: false`, `title: "Vee"`, `width: 400`, `height: 480`, `resizable: false`, `center: true`

- [ ] **Step 1: 실패하는 테스트** (`whats_new.rs`, `Store::open_in_memory` 사용)

```rust
#[test]
fn shows_notes_saved_for_the_running_version_only() {
    let dir = tempfile::tempdir().unwrap();
    let store = Store::open_in_memory(dir.path()).unwrap();
    assert!(due(&store, "0.9.0").is_none()); // fresh install
    save(&store, "0.9.0", Some("## 새 기능\n- a"));
    assert!(due(&store, "0.8.0").is_none()); // not installed yet
    let w = due(&store, "0.9.0").unwrap();
    assert_eq!((w.version.as_str(), w.notes.as_str()), ("0.9.0", "## 새 기능\n- a"));
}

#[test]
fn ignores_updates_without_notes() {
    let dir = tempfile::tempdir().unwrap();
    let store = Store::open_in_memory(dir.path()).unwrap();
    save(&store, "0.9.0", None);
    save(&store, "0.9.0", Some("   "));
    assert!(due(&store, "0.9.0").is_none());
}
```

- [ ] **Step 2: 실패 확인** — `cd src-tauri && cargo test whats_new` → 컴파일 실패.
- [ ] **Step 3: 구현**
  - `updater::check`: 다운로드 성공 시 `whats_new::save(&store, &update.version, update.body.as_deref())`.
  - setup(창 생성 뒤): `due(...)`가 `Some`이면 `whats-new` 창 `show()` + `set_focus()`(Accessory 앱이라 포커스 필요).
  - `on_window_event`: `whats-new`의 `CloseRequested` → `prevent_close` + `dismiss` 동작(키 비우고 숨김).
  - 커맨드 두 개를 `generate_handler!`에 등록.
- [ ] **Step 4: 통과 확인** — `cd src-tauri && cargo test` PASS.
- [ ] **Step 5: 커밋** — `git commit -m "feat(whats-new): keep release notes from the update and show them once"`

---

### Task 7: 새 소식 — 창 UI와 마크다운 렌더링

**Files:**
- Create: `src/whatsnew/notes.ts`, `src/whatsnew/notes.test.ts`, `src/whatsnew/WhatsNew.tsx`, `src/whatsnew/whatsnew.css`
- Modify: `src/main.tsx` (라벨 분기), `src/api.ts`, `src/i18n/en.ts`, `src/i18n/ko.ts`

**Interfaces:**
- Consumes: Task 6의 `get_whats_new`, `dismiss_whats_new`
- Produces:
  - `api.getWhatsNew: () => invoke<{ version: string; notes: string } | null>("get_whats_new")`, `api.dismissWhatsNew: () => invoke<void>("dismiss_whats_new")`
  - `export type Inline = { kind: "text" | "bold" | "code"; text: string }`
  - `export type Block = { kind: "heading"; level: 2 | 3; inlines: Inline[] } | { kind: "item"; depth: number; inlines: Inline[] } | { kind: "para"; inlines: Inline[] }`
  - `export function parseNotes(md: string): Block[]`
  - i18n `whatsNew: { title: string; ok: string }` — ko: "새로워진 Vee"/"확인", en: "What's new in Vee"/"OK"

- [ ] **Step 1: 실패하는 테스트** (`notes.test.ts`, 현재 v0.7.1 릴리스 본문 형식 기준)

```ts
test("parses headings, nested items, bold and code", () => {
  const md = "## 변경 사항\n\n### 새 기능\n- **패널 고정 단축키:** `⌘⇧P`로 켜요.\n  - 설정에서 바꿔요.";
  assert.deepEqual(parseNotes(md), [
    { kind: "heading", level: 2, inlines: [{ kind: "text", text: "변경 사항" }] },
    { kind: "heading", level: 3, inlines: [{ kind: "text", text: "새 기능" }] },
    { kind: "item", depth: 0, inlines: [
      { kind: "bold", text: "패널 고정 단축키:" }, { kind: "text", text: " " },
      { kind: "code", text: "⌘⇧P" }, { kind: "text", text: "로 켜요." } ] },
    { kind: "item", depth: 1, inlines: [{ kind: "text", text: "설정에서 바꿔요." }] },
  ]);
});
test("keeps HTML as plain text", () => {
  assert.deepEqual(parseNotes("<script>x</script>"), [
    { kind: "para", inlines: [{ kind: "text", text: "<script>x</script>" }] },
  ]);
});
test("handles CRLF and an unclosed marker", () => {
  assert.deepEqual(parseNotes("- a **b\r\n"), [
    { kind: "item", depth: 0, inlines: [{ kind: "text", text: "a **b" }] },
  ]);
});
```

- [ ] **Step 2: 실패 확인** — `yarn test` → FAIL.
- [ ] **Step 3: `parseNotes` 구현** — 줄 단위. `## `/`### ` → heading, 앞 공백 2칸당 depth 1인 `- `/`* ` → item, 빈 줄 건너뜀, 나머지 → para. 인라인은 `**…**`, `` `…` ``만, 짝이 없으면 텍스트. `#` 하나·`####` 이상은 제목 3단계로 취급해도 됨(빌더 판단).
- [ ] **Step 4: 통과 확인** — `yarn test` PASS.
- [ ] **Step 5: `WhatsNew` 창** — 마운트 시 `api.getWhatsNew()`; `null`이면 아무것도 안 그림. 구성: 버전 배지(`v{version}`, `--ok-bg`/`--ok-fg`, 6px 라운드) → 제목 `t.whatsNew.title`(22px/700, letter-spacing -0.5px) → 본문(블록을 React 요소로; `dangerouslySetInnerHTML` 금지; code는 키캡 스타일 `kbd` 권장; 본문 영역만 스크롤) → 오른쪽 아래 확인 버튼(`--accent` 배경, 흰 글자, 12px 라운드). 확인 클릭·`Enter`·`Esc` → `api.dismissWhatsNew()`. `main.tsx`에 `label === "whats-new"` 분기. 목업 `whats-new-v2.html` A안(단, "x에서 업데이트됐어요" 줄은 없음).
- [ ] **Step 6: 수동 확인** — `yarn tauri dev` 전에 DB에 키를 직접 넣어(sqlite3로 `whatsNewVersion`=현재 버전, `whatsNewNotes`=v0.7.1 릴리스 본문) 실행 → 창이 가운데 뜨고 본문이 렌더링, 확인 후 재실행 시 안 뜸. 라이트/다크 둘 다.
- [ ] **Step 7: 커밋** — `git commit -m "feat(whats-new): show release notes window after an update"`

---

### Task 8: 문서와 버전

**Files:**
- Modify: `README.md` (주요 기능·사용법), `src-tauri/tauri.conf.json`, `package.json`, `src-tauri/Cargo.toml` (`version` 0.8.0; `Cargo.lock`은 빌드로 갱신)

- [ ] **Step 1: README** — 주요 기능에 호버 선택(마우스를 올리고 `Enter`), 텍스트·링크 카드 드래그(놓은 자리에 글자 입력), 복사 디바운스(0.5초 안에 연달아 바뀌면 마지막 것만 기록), 설정 탭(일반·패널·단축키·기록), 새 소식 창(자동 업데이트 후 첫 실행 시 릴리스 노트). 드래그 앤 드롭 항목의 "파일·이미지 카드" 문구 수정. 사용법 표에 "카드 선택: 마우스 올리기".
- [ ] **Step 2: 버전 0.8.0** — 세 파일. `cd src-tauri && cargo check`로 `Cargo.lock` 갱신.
- [ ] **Step 3: 전체 확인** — `yarn test && yarn typecheck && (cd src-tauri && cargo test)` 모두 PASS.
- [ ] **Step 4: 커밋** — `git commit -m "docs: describe hover select, text drag, settings tabs and what's new"` (버전 변경은 별도 `chore: release 0.8.0` 커밋으로 해도 됨 — 기존 히스토리 관례)
