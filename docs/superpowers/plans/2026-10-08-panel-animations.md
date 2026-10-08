# 패널 카드 UI · 앱 태그 검색 · 애니메이션 Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** 카드 헤더에 앱 아이콘 배경을 깔고, `@앱` 태그 검색을 추가하고, 토스트·카드 추가·드래그에 애니메이션을 넣는다.

**Architecture:** Tauri 2 앱. Rust(`src-tauri/src`)가 SQLite 저장소와 윈도우를 관리하고, React 19(`src/`)가 패널·토스트 UI를 그린다. 앱 태그 검색은 Rust에 `list_apps` 커맨드와 `list_clips`의 `app_id` 필터를 추가하고, 애니메이션은 새 의존성 없이 CSS 키프레임과 Web Animations API(`element.animate`)로 구현한다. 드래그 플로팅과 공통 모션 상수는 새 파일 `src/panel/motion.ts` 한 곳에 둔다.

**Tech Stack:** Rust (rusqlite, tauri 2, `drag` 2.1), React 19 + TypeScript 7, Vite 8, 테스트는 `cargo test`와 `node --test`(`yarn test`).

**Spec:** `docs/superpowers/specs/2026-10-08-panel-animations-design.md`

## Global Constraints

- 현재 브랜치 `feature/modify-animations`에서 작업. 새 npm/cargo 의존성 추가 금지.
- 모든 애니메이션은 `prefers-reduced-motion: reduce`에서 비활성(즉시 최종 상태).
- 이동 이징 `cubic-bezier(0.32, 0.72, 0, 1)`, 스프링/팝 이징 `cubic-bezier(0.34, 1.56, 0.64, 1)`.
- 색은 `src/theme.css` 토큰(`--accent`, `--accent-subtle`, `--muted`, `--border`, `--card`, `--divider`, `--subtle`) 사용. 버튼/카드 radius 최대 12px(`var(--radius)`).
- UI 문구는 `src/i18n/en.ts`와 `src/i18n/ko.ts` 양쪽에 추가(`ko`는 `Dict` 타입이라 누락 시 타입 에러).
- 커밋 메시지는 기존 스타일(`feat:`, `fix:`, `docs:` + 영어 소문자 요약), 끝에 아래 두 줄:
  ```
  Co-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>
  Claude-Session: https://claude.ai/code/session_01KxDXBUHNSrTuwFvui65C1G
  ```
- 검증 명령: Rust `cd src-tauri && cargo test`, 프론트 `yarn test`, 타입 `yarn typecheck`, 수동 `yarn tauri dev`.

## Review Focus

1. **한글 앱 이름 / 특수문자 검색:** `@메`는 "메모"를 찾아야 하고 `@%`는 모든 앱이 아니라 빈 목록이어야 한다 → Task 3 테스트 `apps_match_name_and_rank_by_clip_count`에 포함.
2. **Backspace를 누르고 있을 때:** `@sa`를 지우려고 Backspace를 누르고 있다가 태그까지 지워지거나 카드가 삭제되면 안 된다(반복 입력은 무시) → Task 4 테스트 `backspace in an empty box removes the app tag before any card`.
3. **퇴장 애니메이션 중 새 토스트:** 180ms 퇴장 중 새 복사가 오면 새 토스트가 처음부터 팝으로 떠야 하고, 이전 타이머가 숨기면 안 된다 → Task 6 수동 확인 Step.
4. **드래그 중 목록 변경:** 카드를 띄운 채 다른 앱에서 복사해 목록이 다시 렌더링돼도 슬롯 표시가 사라지거나 고스트가 남으면 안 된다(React가 `className`을 다시 쓰므로 슬롯은 `data-slot` 속성으로 표시) → Task 8 수동 확인 Step.
5. **태그 선택 후 결과 없음:** 태그로 거른 결과가 0개면 "아직 복사한 항목 없음"이 아니라 `t.noResults`를 보여야 한다 → Task 5 수동 확인 Step.

---

## File Structure

| 파일 | 역할 | 작업 |
|------|------|------|
| `src-tauri/src/store.rs` | `AppDto`, `Store::apps`, `Store::list`에 `app_id` | 수정 |
| `src-tauri/src/lib.rs` | `list_apps` 커맨드, `list_clips`에 `app_id` | 수정 |
| `src-tauri/src/windows.rs` | 토스트 높이, 숨김 전 `toast://hide` | 수정 |
| `src-tauri/tauri.conf.json` | 토스트 윈도우 기본 높이 | 수정 |
| `src/api.ts` | `App` 타입, `listApps`, `listClips(appId)` | 수정 |
| `src/panel/keys.ts` (+ `keys.test.ts`) | 자동완성/태그 키 동작 | 수정 |
| `src/panel/Toolbar.tsx` | 태그, 자동완성 팝오버 | 수정 |
| `src/panel/Panel.tsx` | 앱 태그 상태, FLIP, 드래그 연결 | 수정 |
| `src/panel/Card.tsx` | 아이콘 배경, `data-id`, 드래그 콜백 시그니처 | 수정 |
| `src/panel/motion.ts` (+ `motion.test.ts`) | 이징 상수, reduced-motion, 기울기/경계 계산, `floatCard` | 생성 |
| `src/panel/panel.css` | 카드 배경, 태그, 팝오버, 슬롯/고스트 | 수정 |
| `src/toast/Toast.tsx`, `src/toast/toast.css` | 팝 애니메이션, 퇴장, 그림자 | 수정 |
| `src/theme.css` | `--toast-shadow` 제거 | 수정 |
| `src/i18n/en.ts`, `src/i18n/ko.ts` | `noApps`, `suggestHint`, `clearApp` | 수정 |

---

### Task 1: 스파이크 — 윈도우 밖에서 네이티브 드래그 시작 가능 여부 (버리는 코드)

스펙 §5의 위험 확인. **이 작업의 코드는 커밋하지 않는다.** 산출물은 `HANDOFF_EDGE` 값(0 또는 20) 하나다.

**Files:**
- 임시 수정: `src/panel/Card.tsx:235-245` (onMouseMove)

**Interfaces:**
- Produces: `HANDOFF_EDGE` 결정 — 0이면 "커서가 윈도우 밖으로 나간 뒤" 시작, 20이면 "윈도우 상단 20px 안" 또는 그 밖에서 시작. Task 8의 `src/panel/motion.ts`에 이 값을 적는다.

- [ ] **Step 1: 임계값 도달 시 바로 드래그하는 대신, 윈도우를 벗어날 때 드래그하도록 임시 변경**

`Card.tsx`의 `onMouseMove` 안 마지막 두 줄(`press.current = null; onDragOut();`)을 다음으로 바꾼다:

```tsx
        press.current = null;
        // SPIKE: start the native drag only once the cursor has left the window.
        const move = (ev: MouseEvent) => {
          console.log("spike move", ev.clientX, ev.clientY, ev.buttons);
          if ((ev.buttons & 1) === 0) return done();
          if (ev.clientX < 0 || ev.clientY < 0 || ev.clientX >= innerWidth || ev.clientY >= innerHeight) {
            done();
            onDragOut();
          }
        };
        const leave = () => {
          console.log("spike mouseleave");
          done();
          onDragOut();
        };
        const done = () => {
          window.removeEventListener("mousemove", move);
          document.documentElement.removeEventListener("mouseleave", leave);
        };
        window.addEventListener("mousemove", move);
        document.documentElement.addEventListener("mouseleave", leave);
```

- [ ] **Step 2: 실제 앱에서 확인**

Run: `yarn tauri dev`
1. 아무 이미지를 복사(⌘C)해 이미지 클립을 만든다. Finder에서 파일 하나도 복사해 파일 클립을 만든다.
2. 단축키로 패널을 연다. 패널에서 우클릭 → Inspect로 콘솔을 연다.
3. 이미지 카드를 누른 채 위로 천천히 끌어 패널 윈도우 위쪽 밖으로 나간다.
4. 확인: (a) 콘솔에 `spike move`가 윈도우 밖 좌표(음수 y)로 찍히거나 `spike mouseleave`가 찍히는지, (b) 그 뒤 OS 드래그 이미지가 커서에 붙고 패널이 내려가는지, (c) 바탕화면/Finder에 놓으면 파일이 생기는지.
5. 파일 카드로 반복.

Expected: (b)와 (c)가 둘 다 되면 **통과 → `HANDOFF_EDGE = 0`**. 드래그 이미지가 안 붙거나 아무 일도 없으면 **실패 → `HANDOFF_EDGE = 20`**(윈도우 안에서 시작하는 현재 방식이라 동작이 보장됨).

- [ ] **Step 3: 임시 코드 되돌리고 결과 기록**

```bash
git checkout src/panel/Card.tsx
git status --short   # 출력 없음이어야 함
```

결과(통과/실패, 정한 `HANDOFF_EDGE`)를 사용자에게 보고하고 Task 8에서 쓴다.

---

### Task 2: 카드 헤더 — 앱 아이콘 배경 (목업 D)

**Files:**
- Modify: `src/panel/Card.tsx:250-258` (card-head)
- Modify: `src/panel/panel.css` (`.card-head`, `.app-icon*`, `.card-body`, `.card-foot`, `.confirm`)

**Interfaces:**
- Consumes: `Clip.appIcon: string | null`, `Clip.appName: string | null` (기존)
- Produces: CSS 클래스 `.card-bg`, `.app-name` (Task 8 고스트가 그대로 복제함)

- [ ] **Step 1: Card 마크업 변경**

`Card.tsx`에서 `<div className="card-head">` 블록 전체를 다음으로 바꾼다(아이콘 `<img>`/빈 `<span>` 제거, 배경 이미지는 카드 최상위 첫 자식):

```tsx
      {clip.appIcon && <img className="card-bg" src={clip.appIcon} alt="" aria-hidden />}
      <div className="card-head">
        <span className="app-name">{clip.appName ?? ""}</span>
        <span className={clip.kind === "files" ? "badge badge-file" : "badge"}>{badgeLabel(clip, t)}</span>
        <span className="time">{relativeTime(clip.lastUsedAt, locale)}</span>
      </div>
```

- [ ] **Step 2: CSS 변경**

`panel.css`에서 `.card-head`, `.app-icon`, `.app-icon-empty` 규칙을 지우고 그 자리에 넣는다:

```css
.card-head {
  position: relative;
  z-index: 1;
  display: flex;
  align-items: center;
  gap: 6px;
  min-height: 40px;
  /* The left 46px is where the backdrop icon shows through. */
  padding: 8px 8px 8px 46px;
}

/* The source app's icon, faded in from the top-left corner like a watermark. */
.card-bg {
  position: absolute;
  top: -14px;
  left: -18px;
  z-index: 0;
  width: 72px;
  height: 72px;
  pointer-events: none;
  opacity: 0.16;
  -webkit-mask-image: linear-gradient(135deg, #000 30%, transparent 85%);
  mask-image: linear-gradient(135deg, #000 30%, transparent 85%);
}

:root[data-theme="dark"] .card-bg {
  opacity: 0.22;
}

.app-name {
  min-width: 0;
  overflow: hidden;
  color: var(--muted);
  font-size: 11px;
  font-weight: 600;
  white-space: nowrap;
  text-overflow: ellipsis;
}
```

`.card-body` 규칙에 `z-index: 1;`을 추가한다(이미 `position: relative`). `.card-foot` 규칙에 `position: relative; z-index: 1;`을 추가한다. `.confirm` 규칙에 `z-index: 2;`를 추가한다(삭제 확인창이 헤더/본문 위에 오도록).

- [ ] **Step 3: 타입 검사**

Run: `yarn typecheck`
Expected: 에러 없이 종료

- [ ] **Step 4: 수동 확인**

Run: `yarn tauri dev`. 설정에서 테마를 라이트/다크로 바꿔가며 패널을 열고 확인: 아이콘이 좌상단에 은은하게 페이드되어 보이고, 앱 이름·배지·시간이 한 줄에 읽히고, 아이콘 없는 클립(앱 정보 없음)은 이름 자리만 비어 있고, 선택 테두리와 삭제 확인창(Delete 키)이 정상인지.

- [ ] **Step 5: Commit**

```bash
git add src/panel/Card.tsx src/panel/panel.css
git commit -m "feat: show the source app icon as a faded card backdrop

Co-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>
Claude-Session: https://claude.ai/code/session_01KxDXBUHNSrTuwFvui65C1G"
```

---

### Task 3: 백엔드 — 앱 필터와 `list_apps`

**Files:**
- Modify: `src-tauri/src/store.rs` (`ClipDto` 아래에 `AppDto`, `list` 시그니처, 새 `apps`, 테스트 모듈)
- Modify: `src-tauri/src/lib.rs:14` (import), `:52-58` (`list_clips`), `:166` (handler 등록)
- Test: `src-tauri/src/store.rs` 내부 `mod tests`

**Interfaces:**
- Produces (Rust): `pub struct AppDto { pub id: i64, pub name: String, pub icon: Option<String>, pub count: i64 }` (serde camelCase), `Store::apps(&self, query: &str, limit: i64) -> Result<Vec<AppDto>>`, `Store::list(&self, query: &str, kind: Option<Kind>, app_id: Option<i64>, offset: i64, limit: i64)`
- Produces (IPC): `list_apps { query: string } -> App[]`, `list_clips { query, kind, appId: number | null, offset, limit }` — Task 5가 사용

- [ ] **Step 1: 기존 `list` 호출에 `None` 인자 추가 (기계적 변경)**

테스트가 먼저 컴파일되도록 store 테스트의 모든 `.list(a, b, …)` 호출에 세 번째 인자 `None`을 넣는다:

```bash
sed -i '' -E 's/\.list\(([^,]*), ([^,]*), /.list(\1, \2, None, /g' src-tauri/src/store.rs
grep -c '\.list(' src-tauri/src/store.rs
grep '\.list(' src-tauri/src/store.rs | grep -vc ', None, ' 
```

Expected: 첫 grep은 30, 두 번째는 `.list(` 중 `, None, `이 없는 줄 수 = 0. (`Some(Kind::Link)` 같은 두 번째 인자도 콤마가 없으므로 그대로 보존된다.)

- [ ] **Step 2: 실패하는 테스트 작성**

`store.rs`의 `mod tests` 안, `upsert_app_keeps_existing_icon` 테스트 바로 아래에 추가:

```rust
    #[test]
    fn list_filters_by_app() {
        let (s, _d) = store();
        let a = s.upsert_app("com.a", "A", None).unwrap();
        let b = s.upsert_app("com.b", "B", None).unwrap();
        s.upsert(text("from a"), Some(a), 1).unwrap();
        s.upsert(text("from b"), Some(b), 2).unwrap();
        s.upsert(text("also a"), Some(a), 3).unwrap();
        assert_eq!(texts(&s.list("", None, Some(a), 0, 50).unwrap()), vec!["also a", "from a"]);
        assert_eq!(texts(&s.list("also", None, Some(a), 0, 50).unwrap()), vec!["also a"]);
        assert!(s.list("", None, Some(999), 0, 50).unwrap().is_empty());
    }

    #[test]
    fn apps_match_name_and_rank_by_clip_count() {
        let (s, _d) = store();
        let safari = s.upsert_app("com.apple.Safari", "Safari", Some(&[1])).unwrap();
        let notes = s.upsert_app("com.apple.Notes", "메모", None).unwrap();
        let slack = s.upsert_app("com.tinyspeck.slackmacgap", "Slack", None).unwrap();
        s.upsert_app("com.unused", "Sapling", None).unwrap(); // no clips → never suggested
        s.upsert(text("a"), Some(safari), 1).unwrap();
        s.upsert(text("b"), Some(slack), 2).unwrap();
        s.upsert(text("c"), Some(slack), 3).unwrap();
        s.upsert(text("d"), Some(notes), 4).unwrap();
        let names = |q: &str, limit: i64| -> Vec<(String, i64)> {
            s.apps(q, limit).unwrap().into_iter().map(|a| (a.name, a.count)).collect()
        };
        assert_eq!(names("s", 8), vec![("Slack".to_string(), 2), ("Safari".to_string(), 1)]);
        assert_eq!(names("SA", 8), vec![("Safari".to_string(), 1)]);
        assert_eq!(names("메", 8), vec![("메모".to_string(), 1)]);
        assert_eq!(names("", 8), vec![("Slack".to_string(), 2), ("Safari".to_string(), 1), ("메모".to_string(), 1)]);
        assert_eq!(names("", 1), vec![("Slack".to_string(), 2)]);
        assert!(names("%", 8).is_empty());
        assert!(s.apps("safari", 8).unwrap()[0].icon.as_deref().unwrap().starts_with("data:image/png;base64,"));
    }
```

- [ ] **Step 3: 실패 확인**

Run: `cd src-tauri && cargo test --lib store::tests 2>&1 | tail -20`
Expected: 컴파일 에러 — `list` 인자 개수 불일치, `no method named apps`.

- [ ] **Step 4: 구현**

`store.rs`의 `ClipDto` 구조체 바로 아래에 추가:

```rust
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AppDto {
    pub id: i64,
    pub name: String,
    pub icon: Option<String>,
    /// How many clips came from this app.
    pub count: i64,
}
```

`pub fn list(&self, query: &str, kind: Option<Kind>, offset: i64, limit: i64)` 시그니처를 다음으로 바꾼다:

```rust
    pub fn list(&self, query: &str, kind: Option<Kind>, app_id: Option<i64>, offset: i64, limit: i64) -> Result<Vec<ClipDto>> {
```

같은 함수의 `if let Some(k) = kind { … }` 블록 바로 아래에 추가:

```rust
        if let Some(a) = app_id {
            sql.push_str(" AND c.app_id = ?");
            args.push(Value::Integer(a));
        }
```

`list` 함수 끝(`fn stack` 위)에 새 메서드 추가:

```rust
    /// Apps that have clips, whose name contains `query` (ASCII case-insensitive), most clips first.
    pub fn apps(&self, query: &str, limit: i64) -> Result<Vec<AppDto>> {
        let mut stmt = self.conn.prepare(
            "SELECT a.id, a.name, a.icon_png, COUNT(c.id) AS n
             FROM apps a JOIN clips c ON c.app_id = a.id
             WHERE a.name LIKE ?1 ESCAPE '\\'
             GROUP BY a.id
             ORDER BY n DESC, a.name ASC
             LIMIT ?2",
        )?;
        let pattern = format!("%{}%", escape_like(query.trim()));
        let rows = stmt.query_map(params![pattern, limit], |r| {
            Ok(AppDto {
                id: r.get(0)?,
                name: r.get(1)?,
                icon: r.get::<_, Option<Vec<u8>>>(2)?.map(|b| data_url(&b)),
                count: r.get(3)?,
            })
        })?;
        Ok(rows.collect::<rusqlite::Result<_>>()?)
    }
```

`lib.rs:14`를 `use store::{AppDto, ClipDto, Kind, Store};`로 바꾼다. `list_clips`를 다음으로 바꾸고 바로 아래에 `list_apps`를 추가:

```rust
#[tauri::command]
fn list_clips(
    state: State<AppState>,
    query: String,
    kind: String,
    app_id: Option<i64>,
    offset: i64,
    limit: i64,
) -> Result<Vec<ClipDto>, String> {
    let kind = match kind.as_str() {
        "all" => None,
        k => Some(Kind::parse(k).ok_or_else(|| format!("unknown kind: {k}"))?),
    };
    state
        .store
        .lock()
        .unwrap()
        .list(&query, kind, app_id, offset.max(0), limit.clamp(1, 200))
        .map_err(|e| e.to_string())
}

#[tauri::command]
fn list_apps(state: State<AppState>, query: String) -> Result<Vec<AppDto>, String> {
    state.store.lock().unwrap().apps(&query, 8).map_err(|e| e.to_string())
}
```

`generate_handler![` 목록의 `list_clips,` 다음 줄에 `list_apps,`를 추가한다.

- [ ] **Step 5: 통과 확인**

Run: `cd src-tauri && cargo test 2>&1 | tail -5`
Expected: `test result: ok.` (실패 0)

- [ ] **Step 6: Commit**

```bash
git add src-tauri/src/store.rs src-tauri/src/lib.rs
git commit -m "feat: filter clips by source app and list apps for search

Co-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>
Claude-Session: https://claude.ai/code/session_01KxDXBUHNSrTuwFvui65C1G"
```

---

### Task 4: 키 처리 — 자동완성과 태그 지우기

**Files:**
- Modify: `src/panel/keys.ts`
- Test: `src/panel/keys.test.ts`

**Interfaces:**
- Produces: `KeyInput`에 `suggesting: boolean`, `hasTag: boolean` 추가. `KeyAction`에 `{ type: "suggestMove"; delta: 1 | -1 }`, `{ type: "suggestPick" }`, `{ type: "suggestClose" }`, `{ type: "clearTag" }` 추가 — Task 5의 `Panel.onKeyDown`이 사용.

- [ ] **Step 1: 실패하는 테스트 작성**

`keys.test.ts`의 `press` 헬퍼를 새 필드 기본값을 포함하도록 바꾼다:

```ts
const press = (key: string, extra: Partial<KeyInput> = {}) =>
  panelKeyAction({
    key,
    shiftKey: false,
    isComposing: false,
    queryEmpty: true,
    repeat: false,
    confirming: false,
    suggesting: false,
    hasTag: false,
    ...extra,
  });
```

파일 끝에 추가:

```ts
test("while suggesting apps, arrows, enter and escape drive the list", () => {
  const s = { suggesting: true, queryEmpty: false };
  assert.deepEqual(press("ArrowDown", s), { type: "suggestMove", delta: 1 });
  assert.deepEqual(press("ArrowUp", s), { type: "suggestMove", delta: -1 });
  assert.deepEqual(press("Enter", s), { type: "suggestPick" });
  assert.deepEqual(press("Escape", s), { type: "suggestClose" });
  assert.equal(press("a", s), null);
  // Everything else keeps its usual meaning, so focus never leaves the search box.
  assert.deepEqual(press("Tab", s), { type: "cycleFilter", delta: 1 });
});

test("an IME composition wins over the suggestion list", () => {
  for (const key of ["Enter", "ArrowDown", "Escape"]) {
    assert.equal(press(key, { suggesting: true, isComposing: true }), null, key);
  }
});

test("backspace in an empty box removes the app tag before any card", () => {
  assert.deepEqual(press("Backspace", { hasTag: true }), { type: "clearTag" });
  assert.equal(press("Backspace", { hasTag: true, repeat: true }), null);
  assert.equal(press("Backspace", { hasTag: true, queryEmpty: false }), null);
  assert.deepEqual(press("Delete", { hasTag: true }), { type: "delete" });
});

test("escape with a tag but no open list still hides the panel", () => {
  assert.deepEqual(press("Escape", { hasTag: true }), { type: "hide" });
});
```

- [ ] **Step 2: 실패 확인**

Run: `yarn test 2>&1 | tail -15`
Expected: 새 테스트 4개 FAIL (`suggestMove` 대신 `null`, `clearTag` 대신 `delete` 등)

- [ ] **Step 3: 구현**

`keys.ts`의 `KeyAction` 유니온에서 `| null;` 앞에 추가:

```ts
  | { type: "suggestMove"; delta: 1 | -1 }
  | { type: "suggestPick" }
  | { type: "suggestClose" }
  | { type: "clearTag" }
```

`KeyInput`의 `confirming` 아래에 추가:

```ts
  /** The `@app` suggestion list is open. */
  suggesting: boolean;
  /** An app tag is set in the search box. */
  hasTag: boolean;
```

`panelKeyAction`에서 `if (e.confirming) { … }` 블록 바로 아래에 추가:

```ts
  if (e.suggesting) {
    switch (e.key) {
      case "ArrowDown":
        return { type: "suggestMove", delta: 1 };
      case "ArrowUp":
        return { type: "suggestMove", delta: -1 };
      case "Enter":
        return { type: "suggestPick" };
      case "Escape":
        return { type: "suggestClose" };
    }
  }
```

`case "Backspace":` 줄의 반환문을 다음으로 바꾼다:

```ts
    case "Backspace":
      if (!e.queryEmpty || e.repeat) return null;
      return e.hasTag ? { type: "clearTag" } : { type: "delete" };
```

- [ ] **Step 4: 통과 확인**

Run: `yarn test 2>&1 | tail -5`
Expected: `# fail 0`

(`Panel.tsx`가 아직 새 필드를 넘기지 않아 `yarn typecheck`는 Task 5 전까지 실패한다 — 정상. Task 5에서 함께 커밋한다.)

- [ ] **Step 5: 이 작업은 Task 5와 함께 커밋** (타입 검사가 통과하는 단위로 묶기 위해 여기서는 커밋하지 않는다)

---

### Task 5: 프론트 — `@` 앱 태그 검색 UI (목업 A)

**Files:**
- Modify: `src/api.ts`
- Modify: `src/panel/Panel.tsx`
- Modify: `src/panel/Toolbar.tsx`
- Modify: `src/panel/panel.css`
- Modify: `src/i18n/en.ts`, `src/i18n/ko.ts`

**Interfaces:**
- Consumes: IPC `list_apps`, `list_clips(appId)` (Task 3), `KeyAction`의 새 타입들 (Task 4)
- Produces: `export interface App { id: number; name: string; icon: string | null; count: number }`, `api.listApps(query: string): Promise<App[]>`, `api.listClips(query, kind, appId: number | null, offset, limit)`

- [ ] **Step 1: API 타입과 함수**

`api.ts`의 `Clip` 인터페이스 아래에 추가:

```ts
export interface App {
  id: number;
  name: string;
  icon: string | null;
  /** How many clips came from this app. */
  count: number;
}
```

`api` 객체의 `listClips`를 바꾸고 바로 아래에 `listApps` 추가:

```ts
  listClips: (query: string, kind: Filter, appId: number | null, offset: number, limit: number) =>
    invoke<Clip[]>("list_clips", { query, kind, appId, offset, limit }),
  listApps: (query: string) => invoke<App[]>("list_apps", { query }),
```

- [ ] **Step 2: 문구 추가**

`en.ts`의 `noResults: "No results",` 아래에:

```ts
  noApps: "No matching apps",
  suggestHint: "↑↓ Move · ⏎ Select · esc Close",
  clearApp: "Remove app filter",
```

`ko.ts`의 `noResults: "검색 결과가 없어요",` 아래에:

```ts
  noApps: "일치하는 앱이 없어요",
  suggestHint: "↑↓ 이동 · ⏎ 선택 · esc 닫기",
  clearApp: "앱 필터 해제",
```

- [ ] **Step 3: Toolbar — 태그와 팝오버**

`Toolbar.tsx` 상단 import를 다음으로 바꾼다:

```ts
import type { RefObject } from "react";
import type { App, Filter } from "../api.ts";
import { usePrefs } from "../prefs.tsx";
```

`Props` 인터페이스의 `inputRef` 아래에 추가:

```ts
  /** The app tag in the search box, or null. */
  app: App | null;
  onClearApp: () => void;
  /** The `@app` suggestion list is open. */
  suggesting: boolean;
  suggestions: App[];
  suggestIndex: number;
  onPickApp: (app: App) => void;
```

`FILTERS` 선언 아래에 일치 부분 강조 함수 추가:

```tsx
/** The app name with the part matching `q` in bold. */
function highlight(name: string, q: string) {
  const i = q ? name.toLowerCase().indexOf(q.toLowerCase()) : -1;
  if (i < 0) return name;
  return (
    <>
      {name.slice(0, i)}
      <b>{name.slice(i, i + q.length)}</b>
      {name.slice(i + q.length)}
    </>
  );
}
```

함수 인자 구조분해에 `app, onClearApp, suggesting, suggestions, suggestIndex, onPickApp,`를 `inputRef,` 다음에 추가한다. `<label className="search"> … </label>` 전체를 다음으로 바꾼다(`<label>` → `<div>`: label 안에 버튼이 생기면 첫 버튼이 label 대상이 되어 아무 데나 클릭해도 태그가 지워진다. 포커스는 `Panel`의 `onMouseDown`이 이미 입력창에 붙잡아 둔다):

```tsx
      <div className="search">
        <span aria-hidden>🔍</span>
        {app && (
          <span className="app-tag">
            {app.icon && <img src={app.icon} alt="" />}
            <span className="app-tag-name">{app.name}</span>
            <button className="app-tag-x" onClick={onClearApp} aria-label={t.clearApp} tabIndex={-1}>
              ×
            </button>
          </span>
        )}
        <input
          ref={inputRef}
          value={query}
          placeholder={app ? "" : t.search}
          aria-label={t.search}
          onChange={(e) => onQuery(e.target.value)}
          autoFocus
          spellCheck={false}
        />
        {suggesting && (
          <div className="suggest" role="listbox">
            {suggestions.length === 0 ? (
              <p className="suggest-empty">{t.noApps}</p>
            ) : (
              suggestions.map((a, i) => (
                <div
                  key={a.id}
                  role="option"
                  aria-selected={i === suggestIndex}
                  className={i === suggestIndex ? "suggest-item on" : "suggest-item"}
                  onClick={() => onPickApp(a)}
                >
                  {a.icon ? <img src={a.icon} alt="" /> : <span className="suggest-noicon" />}
                  <span className="suggest-name">{highlight(a.name, query.slice(1))}</span>
                  <span className="suggest-count">{a.count}</span>
                </div>
              ))
            )}
            <p className="suggest-hint">{t.suggestHint}</p>
          </div>
        )}
      </div>
```

- [ ] **Step 4: Panel — 상태, 조회, 키 연결**

`Panel.tsx` import를 `import { api, type App, type Clip, type Filter, type UpdateStatus } from "../api.ts";`로 바꾼다.

`const [updateFailed, …]` 아래에 상태 추가:

```ts
  const [app, setApp] = useState<App | null>(null);
  const [suggestions, setSuggestions] = useState<App[]>([]);
  const [suggestIndex, setSuggestIndex] = useState(0);
```

`const rowRef = …` 아래에 파생 값 추가:

```ts
  // While `@…` is being typed it names an app, not text to search for.
  const suggesting = app === null && query.startsWith("@");
  const textQuery = suggesting ? "" : query;
  const appId = app?.id ?? null;
```

`reload` 안의 `api.listClips(query, filter, 0, PAGE)`를 `api.listClips(textQuery, filter, appId, 0, PAGE)`로, 의존성 배열 `[query, filter]`를 `[textQuery, filter, appId]`로 바꾼다. `loadMore` 안의 `api.listClips(query, filter, clips.length, PAGE)`를 `api.listClips(textQuery, filter, appId, clips.length, PAGE)`로, 의존성 `[query, filter, clips.length, hasMore]`를 `[textQuery, filter, appId, clips.length, hasMore]`로 바꾼다.

`useEffect(() => { void reload(false); }, [reload]);` 아래에 추가:

```ts
  useEffect(() => {
    if (!suggesting) return;
    let live = true;
    void api.listApps(query.slice(1)).then((apps) => {
      if (!live) return;
      setSuggestions(apps);
      setSuggestIndex(0);
    });
    return () => {
      live = false;
    };
  }, [suggesting, query]);
```

`panel://closed` 핸들러의 `setQuery("");` 아래에 `setApp(null);`을 추가한다.

`installUpdate` 위에 추가:

```ts
  const pickApp = (picked: App) => {
    setApp(picked);
    setQuery("");
  };
```

`onKeyDown`의 `panelKeyAction({ … })` 인자에 `confirming` 다음으로 추가:

```ts
      suggesting,
      hasTag: app !== null,
```

`switch (action.type)`의 `case "cancelDelete":` 블록 위에 추가:

```ts
      case "suggestMove":
        setSuggestIndex((i) => Math.max(0, Math.min(i + action.delta, suggestions.length - 1)));
        break;
      case "suggestPick": {
        const picked = suggestions[suggestIndex];
        if (picked) pickApp(picked);
        break;
      }
      case "suggestClose":
        setQuery("");
        break;
      case "clearTag":
        setApp(null);
        break;
```

`<Toolbar … />`에 `inputRef={inputRef}` 다음으로 props 추가:

```tsx
        app={app}
        onClearApp={() => setApp(null)}
        suggesting={suggesting}
        suggestions={suggestions}
        suggestIndex={suggestIndex}
        onPickApp={pickApp}
```

빈 상태 문구 조건 `{query || filter !== "all" ? t.noResults : t.empty}`를 `{query || filter !== "all" || app ? t.noResults : t.empty}`로 바꾼다.

- [ ] **Step 5: CSS**

`panel.css`의 `.search` 규칙에 `position: relative;`를 추가하고, `.search input` 규칙 아래에 추가:

```css
.app-tag {
  flex: none;
  max-width: 120px;
  display: flex;
  align-items: center;
  gap: 4px;
  padding: 1px 2px;
  border-radius: 6px;
  background: var(--accent-subtle);
  color: var(--accent);
  font-size: 12px;
  font-weight: 600;
  white-space: nowrap;
}

.app-tag img {
  width: 16px;
  height: 16px;
  border-radius: 4px;
}

.app-tag-name {
  min-width: 0;
  overflow: hidden;
  text-overflow: ellipsis;
}

.app-tag-x {
  padding: 0 2px;
  opacity: 0.6;
  line-height: 1;
}

.suggest {
  position: absolute;
  top: calc(100% + 6px);
  left: 0;
  z-index: 3;
  width: 240px;
  max-height: calc(100vh - 70px);
  overflow-y: auto;
  padding: 4px;
  border-radius: var(--radius);
  border: 1px solid var(--border);
  background: var(--card);
  color: var(--text);
  box-shadow: rgba(0, 0, 0, 0.12) 0 8px 24px;
}

.suggest-item {
  display: flex;
  align-items: center;
  gap: 8px;
  padding: 6px 8px;
  border-radius: 8px;
}

.suggest-item.on {
  background: var(--accent-subtle);
}

.suggest-item img,
.suggest-noicon {
  flex: none;
  width: 20px;
  height: 20px;
  border-radius: 5px;
}

.suggest-noicon {
  background: var(--subtle);
  border: 1px solid var(--border);
}

.suggest-name {
  flex: 1;
  min-width: 0;
  overflow: hidden;
  white-space: nowrap;
  text-overflow: ellipsis;
}

.suggest-name b {
  color: var(--accent);
}

.suggest-count {
  color: var(--muted);
  font-size: 11px;
}

.suggest-empty {
  padding: 6px 8px;
  color: var(--muted);
}

.suggest-hint {
  margin-top: 4px;
  padding: 6px 8px 4px;
  border-top: 1px solid var(--divider);
  color: var(--muted);
  font-size: 11px;
}
```

- [ ] **Step 6: 정적 검사와 테스트**

Run: `yarn typecheck && yarn test 2>&1 | tail -3`
Expected: 타입 에러 없음, `# fail 0`

- [ ] **Step 7: 수동 확인**

Run: `yarn tauri dev`. 여러 앱(Safari, 메모, 터미널 등)에서 복사해 둔 뒤 패널에서:
1. `@`만 입력 → 클립 많은 순 앱 목록. `@sa` → Safari가 강조되어 맨 위. ↑↓로 이동, ⏎로 선택 → 입력창 앞에 태그, 입력창 비워짐, Safari 클립만 보임.
2. 태그 상태에서 `release` 입력 → Safari 안에서 텍스트 검색.
3. 입력을 다 지운 뒤 Backspace 한 번 → 태그 해제(카드 삭제 안 됨). 다시 Backspace → 기존처럼 카드 삭제(또는 확인창).
4. `@` 입력 후 esc → `@…`만 지워지고 패널 유지. 다시 esc → 패널 닫힘.
5. `@zzz` → "일치하는 앱이 없어요".
6. **Review Focus 5:** 클립이 1개인 앱을 태그로 고른 뒤 그 카드를 삭제 → 빈 상태 문구가 "검색 결과가 없어요"(`t.noResults`)인지.
7. `@메`를 한글 IME로 입력 중 ⏎ → 조합만 확정되고 앱이 선택되지 않는지. 한 번 더 ⏎ → 선택.
8. 팝오버 항목 클릭으로도 선택되고, 태그의 × 클릭으로 해제되는지. 패널 닫았다 열면 태그가 사라져 있는지.

- [ ] **Step 8: Commit (Task 4 포함)**

```bash
git add src/api.ts src/panel/Panel.tsx src/panel/Toolbar.tsx src/panel/panel.css src/panel/keys.ts src/panel/keys.test.ts src/i18n/en.ts src/i18n/ko.ts
git commit -m "feat: filter the panel by app with an @app search tag

Co-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>
Claude-Session: https://claude.ai/code/session_01KxDXBUHNSrTuwFvui65C1G"
```

---

### Task 6: 토스트 — 팝 애니메이션, 퇴장, 그림자 (목업 B)

**Files:**
- Modify: `src-tauri/src/windows.rs:20-23` (상수), `:254-276` (`show_toast`), `:508` (테스트)
- Modify: `src-tauri/tauri.conf.json` (toast `height`)
- Modify: `src/toast/Toast.tsx`, `src/toast/toast.css`, `src/theme.css`

**Interfaces:**
- Produces: 이벤트 `toast://hide` (payload 없음) — Rust가 숨기기 `TOAST_EXIT_MS`(180ms) 전에 토스트 윈도우로 보낸다.

- [ ] **Step 1: 실패하는 테스트로 기대값 갱신**

`windows.rs`의 `panel_and_toast_sit_at_the_bottom_of_the_work_area` 테스트에서 토스트 줄을 바꾼다(아래 끝 1169 − 높이 132 − 여백 14 = 1023):

```rust
        assert_eq!(toast_rect(work, 1.0), Rect { x: -1490.0, y: 1023.0, w: 420.0, h: 132.0 });
```

- [ ] **Step 2: 실패 확인**

Run: `cd src-tauri && cargo test panel_and_toast 2>&1 | tail -8`
Expected: FAIL — `left: Rect { … y: 1059.0, … h: 96.0 }`

- [ ] **Step 3: Rust 구현**

`windows.rs` 상수를 바꾸고 하나 추가:

```rust
// Tall enough that the card's shadow fades out inside the window instead of being cut off.
const TOAST_HEIGHT: f64 = 132.0;
```

```rust
const TOAST_MS: u64 = 1500;
/// Time the toast's exit animation gets before the window hides.
const TOAST_EXIT_MS: u64 = 180;
```

`show_toast`의 `std::thread::spawn(move || { … });` 블록을 다음으로 바꾼다:

```rust
    std::thread::spawn(move || {
        // A newer toast restarted the timer; let it run the exit and hide the window.
        let current = || TOAST_GENERATION.load(Ordering::SeqCst) == generation;
        std::thread::sleep(Duration::from_millis(TOAST_MS));
        if !current() {
            return;
        }
        let _ = app.emit_to(TOAST, "toast://hide", ());
        std::thread::sleep(Duration::from_millis(TOAST_EXIT_MS));
        if current() {
            if let Some(toast) = app.get_webview_window(TOAST) {
                let _ = toast.hide();
            }
        }
    });
```

`tauri.conf.json`의 `"label": "toast"` 윈도우에서 `"height": 96`을 `"height": 132`로 바꾼다.

- [ ] **Step 4: Rust 통과 확인**

Run: `cd src-tauri && cargo test 2>&1 | tail -3`
Expected: `test result: ok.`

- [ ] **Step 5: Toast 컴포넌트**

`Toast.tsx`의 상태와 effect를 다음으로 바꾼다:

```tsx
  const [toast, setToast] = useState<{ payload: ToastPayload; key: number; out: boolean } | null>(null);

  useEffect(() => {
    const offShow = listen<ToastPayload>("toast://show", (e) =>
      setToast({ payload: e.payload, key: Date.now(), out: false }),
    );
    const offHide = listen("toast://hide", () => setToast((cur) => cur && { ...cur, out: true }));
    return () => {
      void offShow.then((off) => off());
      void offHide.then((off) => off());
    };
  }, []);
```

루트 `<div key={toast.key} className={p.ok ? "toast" : "toast toast-error"} role="status">`를 다음으로 바꾼다:

```tsx
    <div
      key={toast.key}
      className={["toast", !p.ok && "toast-error", toast.out && "out"].filter(Boolean).join(" ")}
      role="status"
    >
```

- [ ] **Step 6: 토스트 CSS와 그림자**

`toast.css`의 `.toast` 규칙에서 `left: 20px; right: 20px; bottom: 18px;`을 `left: 36px; right: 36px; bottom: 36px;`로, `box-shadow: 0 12px 34px var(--toast-shadow);`와 `animation: toast-in 160ms ease-out;`를 다음으로 바꾼다:

```css
  /* Three short layers instead of one long blur, so it all fades out inside the window's margin. */
  box-shadow:
    0 1px 2px rgba(16, 24, 40, 0.06),
    0 4px 10px rgba(16, 24, 40, 0.08),
    0 10px 24px rgba(16, 24, 40, 0.12);
  animation: toast-in 220ms cubic-bezier(0.34, 1.56, 0.64, 1) both;
```

`.toast` 규칙 바로 아래에 추가:

```css
:root[data-theme="dark"] .toast {
  box-shadow:
    0 1px 2px rgba(0, 0, 0, 0.3),
    0 4px 10px rgba(0, 0, 0, 0.28),
    0 10px 24px rgba(0, 0, 0, 0.35);
}

.toast.out {
  animation: toast-out 180ms ease-in forwards;
}
```

`.toast-check` 규칙에 `animation: toast-check-in 300ms 80ms both;`를 추가한다.

파일 끝의 `@keyframes toast-in { … }`를 다음으로 바꾼다:

```css
@keyframes toast-in {
  from {
    transform: scale(0.9);
    opacity: 0;
  }
  to {
    transform: none;
    opacity: 1;
  }
}

@keyframes toast-out {
  to {
    transform: scale(0.96);
    opacity: 0;
  }
}

/* The check badge pops a beat after the card. */
@keyframes toast-check-in {
  from {
    transform: scale(0);
  }
  55% {
    transform: scale(1.25);
  }
  to {
    transform: none;
  }
}

@media (prefers-reduced-motion: reduce) {
  .toast,
  .toast.out,
  .toast-check {
    animation: none;
  }
  .toast.out {
    opacity: 0;
  }
}
```

`theme.css`에서 `--toast-shadow: rgba(16, 24, 40, 0.28);`와 `--toast-shadow: rgba(0, 0, 0, 0.6);` 두 줄을 지운다.

```bash
grep -rn "toast-shadow" src   # 출력 없음이어야 함
```

- [ ] **Step 7: 정적 검사**

Run: `yarn typecheck`
Expected: 에러 없음

- [ ] **Step 8: 수동 확인**

Run: `yarn tauri dev`
1. 패널에서 카드를 ⏎로 복사 → 토스트가 살짝 튀어오르듯 커지며 등장, 체크 배지가 한 박자 늦게 톡 튐, 1.5초 뒤 작아지며 사라짐.
2. 라이트/다크 둘 다 그림자가 위·좌우·아래 모서리에서 직선으로 잘리지 않는지(밝은 배경 위에서 확인).
3. 복사 실패 토스트(원본 파일을 지운 파일 클립 복사)도 같은 애니메이션인지.
4. **Review Focus 3:** 토스트가 사라지기 시작하는 순간(약 1.5초 후)에 다른 카드를 다시 복사 → 새 토스트가 처음부터 팝으로 뜨고 1.5초 동안 유지되는지(중간에 숨겨지지 않는지). 빠르게 3번 연속 복사해도 마지막 토스트가 끝까지 보이는지.
5. 시스템 설정 → 손쉬운 사용 → 디스플레이 → 동작 줄이기를 켜고 → 애니메이션 없이 나타나고 사라지는지.

- [ ] **Step 9: Commit**

```bash
git add src-tauri/src/windows.rs src-tauri/tauri.conf.json src/toast/Toast.tsx src/toast/toast.css src/theme.css
git commit -m "feat: pop the toast in and out and stop clipping its shadow

Co-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>
Claude-Session: https://claude.ai/code/session_01KxDXBUHNSrTuwFvui65C1G"
```

---

### Task 7: 클립 추가 애니메이션 — FLIP + 글로우 (목업 C)

**Files:**
- Create: `src/panel/motion.ts`
- Modify: `src/panel/Card.tsx` (루트 div에 `data-id`)
- Modify: `src/panel/Panel.tsx`

**Interfaces:**
- Produces (`motion.ts`): `export const EASE_MOVE = "cubic-bezier(0.32, 0.72, 0, 1)"`, `export const EASE_SPRING = "cubic-bezier(0.34, 1.56, 0.64, 1)"`, `export function reducedMotion(): boolean`, `export function cardLefts(row: HTMLElement | null): Map<number, number>`, `export function playFlip(row: HTMLElement, before: Map<number, number>): void` — Task 8이 같은 파일에 드래그 함수를 추가한다.

- [ ] **Step 1: motion.ts 생성**

```ts
/** Shared motion for the panel. Everything here is a no-op under reduced motion. */

export const EASE_MOVE = "cubic-bezier(0.32, 0.72, 0, 1)";
export const EASE_SPRING = "cubic-bezier(0.34, 1.56, 0.64, 1)";

export function reducedMotion(): boolean {
  return matchMedia("(prefers-reduced-motion: reduce)").matches;
}

/** Each card's left edge on screen, by clip id. */
export function cardLefts(row: HTMLElement | null): Map<number, number> {
  const lefts = new Map<number, number>();
  for (const el of row?.children ?? []) {
    const card = el as HTMLElement;
    lefts.set(Number(card.dataset.id), card.getBoundingClientRect().left);
  }
  return lefts;
}

/**
 * Slides cards that moved from their old place (FLIP) and pops in cards that are new,
 * ringing them once in the accent colour so a copy made elsewhere is easy to spot.
 */
export function playFlip(row: HTMLElement, before: Map<number, number>): void {
  for (const el of row.children) {
    const card = el as HTMLElement;
    const prev = before.get(Number(card.dataset.id));
    if (prev === undefined) {
      card.animate([{ transform: "scale(0.86)", opacity: 0 }, { transform: "none", opacity: 1 }], {
        duration: 360,
        easing: EASE_MOVE,
      });
      card.animate(
        [
          { boxShadow: "0 0 0 0 rgba(113, 50, 245, 0.5)" },
          { boxShadow: "0 0 0 6px rgba(113, 50, 245, 0.28)", offset: 0.3 },
          { boxShadow: "0 0 0 0 rgba(113, 50, 245, 0)" },
        ],
        { duration: 1400, easing: "ease-out" },
      );
      continue;
    }
    const dx = prev - card.getBoundingClientRect().left;
    if (dx !== 0) {
      card.animate([{ transform: `translateX(${dx}px)` }, { transform: "none" }], { duration: 360, easing: EASE_MOVE });
    }
  }
}
```

(글로우를 CSS 클래스가 아니라 `animate`로 주는 이유: React가 카드를 다시 렌더링하며 `className`을 덮어써도 끊기지 않는다.)

- [ ] **Step 2: Card에 `data-id`**

`Card.tsx` 루트 `<div` 의 `role="option"` 위에 `data-id={clip.id}`를 추가한다.

- [ ] **Step 3: Panel에 연결**

`Panel.tsx` 첫 줄 import에 `useLayoutEffect`를 추가하고, `import { cardLefts, playFlip, reducedMotion } from "./motion.ts";`를 추가한다.

`const rowRef = …` 아래에 추가:

```ts
  /** Card positions captured just before an animated reload, consumed by the next layout. */
  const flipFrom = useRef<Map<number, number> | null>(null);
  const openRef = useRef(open);
  openRef.current = open;
```

`reload`를 두 번째 인자를 받도록 바꾼다. 시그니처 `async (keepSelection: boolean) => {`를 `async (keepSelection: boolean, animate = false) => {`로 바꾸고, `if (id !== requestId.current) return;` 바로 아래(`setClips(page);` 위)에 추가:

```ts
      // Only copies arriving while the panel is on screen animate — not searches or filters.
      if (animate && openRef.current && !reducedMotion()) flipFrom.current = cardLefts(rowRef.current);
```

`clips://changed` 리스너를 `listen("clips://changed", () => void reloadRef.current(true, true))`로 바꾼다.

`useEffect(() => { rowRef.current?.children[selected]?.scrollIntoView…` effect 위에 추가:

```ts
  useLayoutEffect(() => {
    const before = flipFrom.current;
    flipFrom.current = null;
    if (before && rowRef.current) playFlip(rowRef.current, before);
  }, [clips]);
```

- [ ] **Step 4: 정적 검사와 테스트**

Run: `yarn typecheck && yarn test 2>&1 | tail -3`
Expected: 에러 없음, `# fail 0`

- [ ] **Step 5: 수동 확인**

Run: `yarn tauri dev`. 패널을 연 채로(포커스를 빼앗지 않는 패널이므로) 뒤의 다른 앱에서:
1. 새 텍스트를 복사 → 기존 카드가 오른쪽으로 미끄러지고, 새 카드가 작은 상태에서 커지며 나타나고 보라 링이 한 번 퍼졌다 사라짐.
2. 이미 목록에 있는 텍스트를 다시 복사 → 그 카드가 원래 자리에서 맨 앞으로 미끄러져 이동, 글로우 없음.
3. 패널에서 카드 삭제(Delete) → 빈자리를 나머지 카드가 미끄러지며 메움.
4. 검색어 입력, 필터 Tab 전환, 패널 닫고 다시 열기 → 애니메이션 없음.
5. 히스토리가 비어 있을 때 첫 복사 → 첫 카드가 팝 + 글로우.
6. 동작 줄이기 켬 → 애니메이션 없음.

- [ ] **Step 6: Commit**

```bash
git add src/panel/motion.ts src/panel/Card.tsx src/panel/Panel.tsx
git commit -m "feat: slide cards and ring the new one when a copy lands while the panel is open

Co-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>
Claude-Session: https://claude.ai/code/session_01KxDXBUHNSrTuwFvui65C1G"
```

---

### Task 8: 드래그 — 커서를 따라가는 카드 (목업 B)

**Files:**
- Modify: `src/panel/motion.ts` (드래그 함수 추가)
- Create: `src/panel/motion.test.ts`
- Modify: `src/panel/Card.tsx` (`onDragOut` 시그니처)
- Modify: `src/panel/Panel.tsx` (`onDragOut` 전달부)
- Modify: `src/panel/panel.css` (슬롯/고스트)

**Interfaces:**
- Consumes: Task 1의 `HANDOFF_EDGE` 값, Task 7의 `EASE_SPRING`, `reducedMotion`
- Produces: `export const HANDOFF_EDGE: number`, `export function nextTilt(prev: number, dx: number): number`, `export function leftWindow(x: number, y: number, w: number, h: number, edge: number): boolean`, `export function floatCard(card: HTMLElement, x0: number, y0: number, onLeave: () => void): void`. Card prop 변경: `onDragOut?: (card: HTMLElement, x: number, y: number) => void`.

- [ ] **Step 1: 실패하는 테스트 작성**

`src/panel/motion.test.ts` 생성:

```ts
import { test } from "node:test";
import assert from "node:assert/strict";
import { leftWindow, nextTilt } from "./motion.ts";

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
```

- [ ] **Step 2: 실패 확인**

Run: `yarn test 2>&1 | tail -10`
Expected: FAIL — `leftWindow`/`nextTilt` export 없음

- [ ] **Step 3: motion.ts에 드래그 함수 추가**

`motion.ts` 끝에 추가. **`HANDOFF_EDGE`는 Task 1 결과로 정한다: 통과면 `0`, 실패면 `20`.**

```ts
/**
 * Distance from the window's top edge at which a floating card hands off to the OS drag.
 * 0 = only once the cursor has left the window (Task 1 spike decides this value).
 */
export const HANDOFF_EDGE = 0;

/** The floating card leans into horizontal motion and settles when it stops. */
export function nextTilt(prev: number, dx: number): number {
  return Math.max(-10, Math.min(10, prev * 0.7 + dx * 0.6));
}

export function leftWindow(x: number, y: number, w: number, h: number, edge: number): boolean {
  return x < 0 || y < edge || x >= w || y >= h;
}

/**
 * Lifts a copy of `card` that follows the cursor, leaving a dashed slot behind. When the
 * cursor leaves the window the copy is dropped and `onLeave` starts the OS drag; when the
 * button is released inside, the copy springs back into the slot.
 */
export function floatCard(card: HTMLElement, x0: number, y0: number, onLeave: () => void): void {
  const r = card.getBoundingClientRect();
  const ghost = card.cloneNode(true) as HTMLElement;
  ghost.classList.add("ghost");
  Object.assign(ghost.style, { left: `${r.left}px`, top: `${r.top}px`, width: `${r.width}px`, height: `${r.height}px` });
  document.body.appendChild(ghost);
  // An attribute, not a class: React rewrites className whenever the card re-renders.
  card.dataset.slot = "";

  let tilt = 0;
  let lastX = x0;
  const place = (x: number, y: number) => {
    ghost.style.transform = `translate(${x - x0}px, ${y - y0}px) rotate(${tilt}deg) scale(1.05)`;
  };
  const restore = () => {
    ghost.remove();
    delete card.dataset.slot;
  };
  const stop = () => {
    window.removeEventListener("mousemove", move);
    window.removeEventListener("mouseup", drop);
    document.documentElement.removeEventListener("mouseleave", leave);
  };
  function leave() {
    stop();
    restore();
    onLeave();
  }
  function drop() {
    stop();
    ghost.animate([{ transform: ghost.style.transform }, { transform: "none" }], {
      duration: 320,
      easing: EASE_SPRING,
    }).onfinish = restore;
  }
  function move(e: MouseEvent) {
    if ((e.buttons & 1) === 0) return drop();
    tilt = nextTilt(tilt, e.clientX - lastX);
    lastX = e.clientX;
    if (leftWindow(e.clientX, e.clientY, innerWidth, innerHeight, HANDOFF_EDGE)) return leave();
    place(e.clientX, e.clientY);
  }

  place(x0, y0);
  window.addEventListener("mousemove", move);
  window.addEventListener("mouseup", drop);
  document.documentElement.addEventListener("mouseleave", leave);
}
```

- [ ] **Step 4: 통과 확인**

Run: `yarn test 2>&1 | tail -3`
Expected: `# fail 0`

- [ ] **Step 5: Card — 드래그 콜백에 카드 요소와 좌표 전달**

`Card.tsx`의 `Props`에서 `onDragOut?: () => void;`를 다음으로 바꾼다:

```ts
  /** Starts dragging the card out, from the card element and the cursor position. */
  onDragOut?: (card: HTMLElement, x: number, y: number) => void;
```

`onMouseMove` 핸들러 마지막 줄 `onDragOut();`을 `onDragOut(e.currentTarget, e.clientX, e.clientY);`로 바꾼다.

- [ ] **Step 6: Panel — 플로팅 연결**

`Panel.tsx`의 motion import를 `import { cardLefts, floatCard, playFlip, reducedMotion } from "./motion.ts";`로 바꾼다. `dragOut` 함수 아래에 추가:

```ts
  /** Floats the card under the cursor first; the OS drag starts once it leaves the panel. */
  const liftCard = (clip: Clip, card: HTMLElement, x: number, y: number) => {
    if (reducedMotion()) dragOut(clip);
    else floatCard(card, x, y, () => dragOut(clip));
  };
```

`<Card … onDragOut={…}>`의 값을 다음으로 바꾼다:

```tsx
              onDragOut={
                (clip.kind === "files" || clip.kind === "image") && !clip.missing
                  ? (card, x, y) => liftCard(clip, card, x, y)
                  : undefined
              }
```

- [ ] **Step 7: CSS — 슬롯과 고스트**

`panel.css`의 `/* Thumbnails must not start WebKit's own HTML drag. */` 규칙 위에 추가:

```css
/* Where a floating card came from. */
.card[data-slot] {
  border: 2px dashed var(--accent-subtle);
  background: transparent;
  box-shadow: none;
}

.card[data-slot] > * {
  visibility: hidden;
}

/* The copy that follows the cursor while dragging a card. */
.card.ghost {
  position: fixed;
  z-index: 10;
  margin: 0;
  pointer-events: none;
  box-shadow: 0 14px 34px rgba(16, 24, 40, 0.25);
}
```

- [ ] **Step 8: 정적 검사**

Run: `yarn typecheck && yarn test 2>&1 | tail -3`
Expected: 에러 없음, `# fail 0`

- [ ] **Step 9: 수동 확인**

Run: `yarn tauri dev`
1. 이미지 카드를 끌기 → 원래 자리는 점선 슬롯, 카드 복제본이 커서를 따라오고 좌우로 움직이면 기울어졌다가 멈추면 바로 섬.
2. 패널 안에서 놓기 → 복제본이 스프링처럼 슬롯으로 돌아가고 카드가 원래대로.
3. 위로 끌어 윈도우 밖으로(HANDOFF_EDGE가 20이면 상단 근처로) → 복제본이 사라지고 OS 드래그 이미지가 붙고 패널이 내려감 → Finder에 놓으면 파일 생성. ESC/엉뚱한 곳에 놓아 취소 → 패널이 다시 올라옴.
4. 파일 카드(여러 파일 스택 포함)도 같은지. 텍스트/링크 카드는 끌어도 아무 변화 없는지.
5. **Review Focus 4:** 이미지 카드를 띄운 채 다른 손으로 다른 앱에서 텍스트 복사(또는 다른 기기 유니버설 클립보드) → 목록이 다시 그려져도 슬롯 점선이 유지되고, 놓으면 복제본이 남지 않고 사라지는지.
6. 동작 줄이기 켬 → 플로팅 없이 바로 OS 드래그(기존 동작).

- [ ] **Step 10: Commit**

```bash
git add src/panel/motion.ts src/panel/motion.test.ts src/panel/Card.tsx src/panel/Panel.tsx src/panel/panel.css
git commit -m "feat: float the dragged card under the cursor before the OS drag

Co-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>
Claude-Session: https://claude.ai/code/session_01KxDXBUHNSrTuwFvui65C1G"
```

---

### Task 9: 전체 검증

**Files:** 없음 (검증만)

- [ ] **Step 1: 전체 테스트와 빌드**

Run:
```bash
cd src-tauri && cargo test 2>&1 | tail -3 && cd .. && yarn test 2>&1 | tail -3 && yarn build 2>&1 | tail -3
```
Expected: `test result: ok.`, `# fail 0`, Vite 빌드 성공

- [ ] **Step 2: 스펙 성공 기준 5개를 실제 앱에서 한 번씩 확인**

Run: `yarn tauri dev` — 스펙 "성공 기준" 1~5를 라이트/다크 각각 확인하고 결과를 보고한다. 실패 항목이 있으면 해당 Task로 돌아간다.

- [ ] **Step 3: 남은 변경 없음 확인**

```bash
git status --short   # 출력 없음
git log --oneline master..HEAD
```
