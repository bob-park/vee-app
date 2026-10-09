# 고정 카드 왼쪽 영역 분리 Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** 고정한 카드(최대 3개)를 패널 왼쪽 별도 영역에 항상 보이게 하고, 히스토리 줄과 세로 구분선으로 나눈다.

**Architecture:** 백엔드가 고정 목록(`list_pinned`, 고정한 순서)과 히스토리(`list`, 고정 제외)를 따로 내주고 한도 3을 단독으로 검사한다. 패널은 두 목록을 화면 순서대로 이어 붙인 `items`를 하나의 선택 인덱스로 다루고, 고정 영역은 검색·필터와 무관하게 그린다.

**Tech Stack:** Tauri 2 (Rust, rusqlite), React 19 + TypeScript, `node --test`, `cargo test`.

**Spec:** `docs/superpowers/specs/2026-10-09-pinned-sidebar-design.md`

## Global Constraints

- 새 의존성 없음.
- 고정 한도는 `MAX_PINNED = 3`, 백엔드(`store.rs`) 한 곳에서만 검사.
- 한도 초과 시 `set_pinned` 커맨드 에러 문자열은 정확히 `"pin_limit"`.
- 알림 문구: ko `"최대 3개까지 고정할 수 있어요"`, en `"You can pin up to 3 cards"`, 2초 표시.
- 고정 영역 정렬: `pinned_at ASC, id ASC`(먼저 고정한 카드가 왼쪽).
- 디자인 토큰은 `src/theme.css`의 변수만 사용(`--border`, `--card`, `--radius` 등).
- 버전 0.6.2 → 0.6.3: `src-tauri/tauri.conf.json`, `package.json`, `src-tauri/Cargo.toml`(+`Cargo.lock`).
- yarn은 4.x로 실행(`yarn --version`이 4.x가 아니면 `corepack yarn@4.18.0 <명령>`).

## Review Focus

1. **선택한 카드를 고정/해제**하면 그 카드가 다른 영역으로 옮겨 가며 인덱스가 바뀐다 — 선택은 범위 안으로 클램프되어 크래시·빈 선택이 없어야 한다(Task 3 `clampSelection` 테스트).
2. **검색 중 고정**: 검색 결과 카드를 고정하면 히스토리 결과에서 사라지고 고정 영역에 나타나야 한다(Task 1 `list_excludes_pinned`가 검색어 포함 조회로 보장).
3. **이미 고정된 카드 재고정 / 해제된 카드 재해제**는 에러 없이 no-op(Task 1 `pin_limit_rejects_fourth`).
4. **히스토리 0개 + 고정만 있음**: 첫 고정 카드가 선택되고 오른쪽에 빈 문구가 보여야 한다(Task 3 `defaultSelection` 테스트 + Task 4 수동 확인).
5. **고정 카드 삭제·드래그·복사**가 고정 영역에서도 히스토리와 똑같이 동작(Task 4는 같은 `Card`/핸들러를 재사용, 수동 확인 항목에 포함).

---

### Task 1: 저장소 — 스키마 v5, 한도, 고정 목록 분리

**Files:**
- Modify: `src-tauri/src/store.rs` (스키마 상수 ~L94, `init` ~L221, `list` ~L377-451, `set_pinned` ~L523, 테스트 모듈)
- Modify: `src-tauri/src/lib.rs` (`list_clips` ~L52-71, `set_pinned` ~L102-107, `generate_handler!` ~L224)

**Interfaces:**
- Produces:
  - `pub const MAX_PINNED: i64 = 3;`
  - `Store::list(&self, query: &str, kind: Option<Kind>, app_id: Option<i64>, offset: i64, limit: i64) -> Result<Vec<ClipDto>>` — `pinned_only` 제거, 항상 미고정만.
  - `Store::list_pinned(&self) -> Result<Vec<ClipDto>>`
  - `Store::set_pinned(&self, id: i64, pinned: bool, now: i64) -> Result<bool>` — `false`면 한도 초과로 아무것도 안 바뀜.
  - Tauri 커맨드 `list_pinned() -> Vec<ClipDto>`; `set_pinned`는 한도 초과 시 `Err("pin_limit")`; `list_clips`는 `"pinned"` kind를 더 이상 받지 않음.

- [ ] **Step 1: 기존 테스트를 새 시그니처로 바꾸고 새 테스트 추가 (실패 상태)**

기존 `list(..., false, offset, limit)` 호출에서 `false, `를 뺀다:

```bash
cd src-tauri
sed -i '' 's/, false, \([0-9]\)/, \1/g' src/store.rs
grep -n ', false, [0-9]\|, true, [0-9]' src/store.rs   # 출력: list(...true...) 두 줄만 남아야 함(아래에서 교체)
```

기존 `set_pinned(x, true)` / `set_pinned(x, false)` 호출에 시각 인자를 붙인다:

```bash
sed -i '' 's/set_pinned(\([a-z_]*\), true)/set_pinned(\1, true, 1)/g; s/set_pinned(\([a-z_]*\), false)/set_pinned(\1, false, 1)/g' src/store.rs
grep -n 'set_pinned(' src/store.rs   # 모든 호출이 인자 3개여야 함
```

`Store::set_pinned`는 스펙의 `PinLimit` 에러 대신 `Ok(false)`로 한도 초과를 알린다(저장소 에러 타입이 `Box<dyn Error>`라 구분용 타입을 새로 만드는 것보다 단순). 커맨드에서 `"pin_limit"`으로 바꾸므로 외부 동작은 스펙과 같다.

`pinned_clips_filter_and_survive_a_bump` 테스트 전체를 교체:

```rust
    #[test]
    fn pinned_clips_leave_the_history_and_survive_a_bump() {
        let (s, _d) = store();
        let a = s.upsert(text("keep"), None, 1).unwrap();
        s.upsert(text("other"), None, 2).unwrap();
        assert!(s.set_pinned(a, true, 10).unwrap());
        assert_eq!(texts(&s.list_pinned().unwrap()), vec!["keep"]);
        assert_eq!(texts(&s.list("", None, None, 0, 50).unwrap()), vec!["other"]);
        // Copying the same content again bumps it but must not unpin it.
        s.upsert(text("keep"), None, 3).unwrap();
        let pins = s.list_pinned().unwrap();
        assert_eq!(texts(&pins), vec!["keep"]);
        assert!(pins[0].pinned);
        assert!(s.set_pinned(a, false, 11).unwrap());
        assert!(s.list_pinned().unwrap().is_empty());
        assert_eq!(texts(&s.list("", None, None, 0, 50).unwrap()), vec!["keep", "other"]);
    }
```

이제 고정 카드는 `list()`에 없으므로 세 테스트의 기대값을 바꾼다.

`prune_removes_only_unpinned_clips_older_than_the_cutoff`:
```rust
        assert_eq!(texts(&s.list("", None, None, 0, 50).unwrap()), vec!["new", "at cutoff"]);
        assert_eq!(texts(&s.list_pinned().unwrap()), vec!["old but pinned"]);
```

`clear_keeps_pinned_clips_and_their_images`:
```rust
        assert_eq!(s.clear().unwrap(), 2);
        assert!(s.list("", None, None, 0, 50).unwrap().is_empty());
        let left = s.list_pinned().unwrap();
        assert_eq!(left.len(), 1);
        assert_eq!(left[0].id, kept);
```

`excluding_an_app_deletes_its_unpinned_clips_and_marks_it`:
```rust
        assert_eq!(texts(&s.list("", None, None, 0, 50).unwrap()), vec!["note"]);
        assert_eq!(texts(&s.list_pinned().unwrap()), vec!["pinned secret"]);
```

테스트 모듈 끝에 새 테스트 4개 추가:

```rust
    #[test]
    fn pin_limit_rejects_fourth() {
        let (s, _d) = store();
        let ids: Vec<i64> = (1..=4).map(|n| s.upsert(text(&format!("c{n}")), None, n).unwrap()).collect();
        for &id in &ids[..3] {
            assert!(s.set_pinned(id, true, 100).unwrap());
        }
        assert!(!s.set_pinned(ids[3], true, 101).unwrap());
        assert_eq!(s.list_pinned().unwrap().len(), 3);
        // Re-pinning a pinned clip at the limit is a no-op success, not a rejection.
        assert!(s.set_pinned(ids[0], true, 102).unwrap());
        // Unpinning an unpinned clip is a no-op success too.
        assert!(s.set_pinned(ids[3], false, 103).unwrap());
        // Freeing a slot lets the fourth in.
        assert!(s.set_pinned(ids[1], false, 104).unwrap());
        assert!(s.set_pinned(ids[3], true, 105).unwrap());
        assert_eq!(texts(&s.list_pinned().unwrap()), vec!["c1", "c3", "c4"]);
    }

    #[test]
    fn list_excludes_pinned() {
        let (s, _d) = store();
        let a = s.upsert(text("alpha pinned"), None, 1).unwrap();
        s.upsert(text("alpha loose"), None, 2).unwrap();
        s.set_pinned(a, true, 3).unwrap();
        // Searches and kind filters only ever see the history.
        assert_eq!(texts(&s.list("alpha", None, None, 0, 50).unwrap()), vec!["alpha loose"]);
        assert_eq!(texts(&s.list("", Some(Kind::Text), None, 0, 50).unwrap()), vec!["alpha loose"]);
        assert_eq!(texts(&s.list_pinned().unwrap()), vec!["alpha pinned"]);
    }

    #[test]
    fn list_pinned_keeps_pin_order() {
        let (s, _d) = store();
        let first = s.upsert(text("first"), None, 1).unwrap();
        let second = s.upsert(text("second"), None, 2).unwrap();
        s.set_pinned(second, true, 10).unwrap();
        s.set_pinned(first, true, 20).unwrap();
        s.touch(second, 30).unwrap(); // using a pin must not reorder the pinned area
        assert_eq!(texts(&s.list_pinned().unwrap()), vec!["second", "first"]);
        // Re-pinning keeps the original slot.
        s.set_pinned(second, true, 40).unwrap();
        assert_eq!(texts(&s.list_pinned().unwrap()), vec!["second", "first"]);
    }

    #[test]
    fn v5_migration_keeps_three_most_recent() {
        let dir = tempfile::tempdir().unwrap();
        let db = dir.path().join("vee.db");
        {
            let conn = Connection::open(&db).unwrap();
            for schema in [SCHEMA_V1, SCHEMA_V2, SCHEMA_V3, SCHEMA_V4] {
                conn.execute_batch(schema).unwrap();
            }
            for n in 1..=5i64 {
                conn.execute(
                    "INSERT INTO clips (kind, hash, text, created_at, last_used_at, pinned)
                     VALUES ('text', ?1, ?2, ?3, ?3, 1)",
                    params![format!("h{n}"), format!("pin {n}"), n],
                )
                .unwrap();
            }
        }
        let s = Store::open(&db, &dir.path().join("images")).unwrap();
        assert_eq!(texts(&s.list_pinned().unwrap()), vec!["pin 3", "pin 4", "pin 5"]);
        // The rest are unpinned, not deleted.
        assert_eq!(texts(&s.list("", None, None, 0, 50).unwrap()), vec!["pin 2", "pin 1"]);
    }
```

- [ ] **Step 2: 실패 확인**

Run: `cd src-tauri && cargo test --lib store::`
Expected: 컴파일 실패 — `list` 인자 개수 불일치, `list_pinned` 없음, `set_pinned` 인자 개수 불일치.

- [ ] **Step 3: 구현**

`SCHEMA_V4` 아래에 추가:

```rust
/// Pins are capped at `MAX_PINNED` and keep the order they were pinned in.
/// Existing users keep their most recently used pins; the rest are unpinned, not deleted.
const SCHEMA_V5: &str = "
BEGIN;
ALTER TABLE clips ADD COLUMN pinned_at INTEGER;
UPDATE clips SET pinned = 0
 WHERE pinned = 1
   AND id NOT IN (SELECT id FROM clips WHERE pinned = 1 ORDER BY last_used_at DESC, id DESC LIMIT 3);
UPDATE clips SET pinned_at = last_used_at WHERE pinned = 1;
PRAGMA user_version = 5;
COMMIT;
";

/// How many clips can be pinned at once.
pub const MAX_PINNED: i64 = 3;
```

`init()`의 v4 블록 아래:

```rust
        if version < 5 {
            conn.execute_batch(SCHEMA_V5)?;
        }
```

`list()`를 쿼리 조립 + 공용 매핑으로 나눈다. 기존 `list` 전체를 다음으로 교체:

```rust
    const CLIP_COLUMNS: &str = "SELECT c.id, c.kind, substr(c.text, 1, ?), COALESCE(length(c.text), 0), c.thumb_png, c.meta,
                    a.name, a.icon_png, c.last_used_at,
                    CASE c.kind WHEN 'files' THEN c.text WHEN 'image' THEN c.image_path END,
                    c.pinned
             FROM clips c LEFT JOIN apps a ON a.id = c.app_id";

    /// The history row: unpinned clips only, newest first. Pins live in `list_pinned`.
    pub fn list(
        &self,
        query: &str,
        kind: Option<Kind>,
        app_id: Option<i64>,
        offset: i64,
        limit: i64,
    ) -> Result<Vec<ClipDto>> {
        let mut sql = format!("{} WHERE c.pinned = 0", Self::CLIP_COLUMNS);
        let mut args = vec![Value::Integer(PREVIEW_CHARS)];
        if let Some(k) = kind {
            sql.push_str(" AND c.kind = ?");
            args.push(Value::Text(k.as_str().into()));
        }
        if let Some(a) = app_id {
            sql.push_str(" AND c.app_id = ?");
            args.push(Value::Integer(a));
        }
        let q = query.trim();
        match q.chars().count() {
            0 => {}
            1 | 2 => {
                sql.push_str(" AND c.text LIKE ? ESCAPE '\\'");
                args.push(Value::Text(format!("%{}%", escape_like(q))));
            }
            _ => {
                sql.push_str(" AND c.id IN (SELECT rowid FROM clips_fts WHERE clips_fts MATCH ?)");
                args.push(Value::Text(fts_phrase(q)));
            }
        }
        sql.push_str(" ORDER BY c.last_used_at DESC, c.id DESC LIMIT ? OFFSET ?");
        args.push(Value::Integer(limit));
        args.push(Value::Integer(offset));
        self.query_clips(&sql, args)
    }

    /// Pinned clips in the order they were pinned, regardless of any search or filter.
    pub fn list_pinned(&self) -> Result<Vec<ClipDto>> {
        let sql = format!("{} WHERE c.pinned = 1 ORDER BY c.pinned_at ASC, c.id ASC LIMIT ?", Self::CLIP_COLUMNS);
        self.query_clips(&sql, vec![Value::Integer(PREVIEW_CHARS), Value::Integer(MAX_PINNED)])
    }

    fn query_clips(&self, sql: &str, args: Vec<Value>) -> Result<Vec<ClipDto>> {
        let mut stmt = self.conn.prepare(sql)?;
        let rows = stmt.query_map(params_from_iter(args), |r| {
            let kind = Kind::parse(&r.get::<_, String>(1)?).unwrap_or(Kind::Text);
            let paths: Option<String> = r.get(9)?;
            let paths: Vec<&Path> = paths.as_deref().map(|p| p.lines().map(Path::new).collect()).unwrap_or_default();
            Ok(ClipDto {
                id: r.get(0)?,
                kind,
                text_preview: r.get(2)?,
                char_count: r.get(3)?,
                thumb: r.get::<_, Option<Vec<u8>>>(4)?.map(|b| data_url(&b)),
                meta: r.get(5)?,
                app_name: r.get(6)?,
                app_icon: r.get::<_, Option<Vec<u8>>>(7)?.map(|b| data_url(&b)),
                last_used_at: r.get(8)?,
                missing: paths.iter().any(|p| !p.exists()),
                is_dir: kind == Kind::Files && paths.len() == 1 && paths[0].is_dir(),
                pinned: r.get::<_, i64>(10)? != 0,
                stack: Vec::new(),
            })
        })?;
        let mut clips: Vec<ClipDto> = rows.collect::<rusqlite::Result<_>>()?;
        drop(stmt);
        for clip in clips.iter_mut().filter(|c| c.kind == Kind::Files) {
            let count = clip.meta.as_deref().and_then(|m| m.parse().ok()).unwrap_or(1);
            clip.stack = self.stack(clip.id, count)?;
        }
        Ok(clips)
    }
```

`set_pinned` 교체:

```rust
    /// Pins or unpins a clip. Returns false, changing nothing, when pinning would go past `MAX_PINNED`.
    pub fn set_pinned(&self, id: i64, pinned: bool, now: i64) -> Result<bool> {
        if !pinned {
            self.conn.execute("UPDATE clips SET pinned = 0, pinned_at = NULL WHERE id = ?1", [id])?;
            return Ok(true);
        }
        let (already, count): (bool, i64) = self.conn.query_row(
            "SELECT COALESCE(MAX(id = ?1), 0), COUNT(*) FROM clips WHERE pinned = 1",
            [id],
            |r| Ok((r.get::<_, i64>(0)? != 0, r.get(1)?)),
        )?;
        if already {
            return Ok(true);
        }
        if count >= MAX_PINNED {
            return Ok(false);
        }
        self.conn.execute("UPDATE clips SET pinned = 1, pinned_at = ?1 WHERE id = ?2", params![now, id])?;
        Ok(true)
    }
```

`lib.rs` — `list_clips` 본문의 kind 해석과 호출:

```rust
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
```

`list_clips` 아래에 커맨드 추가:

```rust
#[tauri::command]
fn list_pinned(state: State<AppState>) -> Result<Vec<ClipDto>, String> {
    state.store.lock().unwrap().list_pinned().map_err(|e| e.to_string())
}
```

`set_pinned` 커맨드 교체:

```rust
#[tauri::command]
fn set_pinned(app: AppHandle, state: State<AppState>, id: i64, pinned: bool) -> Result<(), String> {
    let ok = state.store.lock().unwrap().set_pinned(id, pinned, now_ms()).map_err(|e| e.to_string())?;
    if !ok {
        return Err("pin_limit".into());
    }
    let _ = app.emit("clips://changed", ());
    Ok(())
}
```

`generate_handler!` 목록의 `list_clips,` 다음 줄에 `list_pinned,` 추가.

- [ ] **Step 4: 통과 확인**

Run: `cd src-tauri && cargo test --lib && cargo clippy --all-targets -- -D warnings`
Expected: 모든 테스트 PASS(새 4개 포함), clippy 경고 없음.

- [ ] **Step 5: Commit**

```bash
git add src-tauri/src/store.rs src-tauri/src/lib.rs
git commit -m "feat(store): cap pins at 3, keep pin order, split pinned from history"
```

---

### Task 2: 프론트 API·필터·문구 정리

**Files:**
- Modify: `src/api.ts:4,77`
- Modify: `src/panel/Toolbar.tsx:5`
- Modify: `src/i18n/en.ts:3`, `src/i18n/ko.ts:5`

**Interfaces:**
- Consumes: Task 1의 `list_pinned` 커맨드, `set_pinned`의 `"pin_limit"` 에러.
- Produces:
  - `type Filter = Kind | "all"`
  - `api.listPinned(): Promise<Clip[]>`
  - `FILTERS = ["all", "text", "image", "files", "link"]`
  - `t.pinLimit: string`

- [ ] **Step 1: `api.ts` 수정**

```ts
export type Filter = Kind | "all";
```

`listClips` 아래에 추가:

```ts
  /** Pinned clips in pin order; searches and filters never apply. */
  listPinned: () => invoke<Clip[]>("list_pinned"),
  /** Rejects with "pin_limit" when 3 clips are already pinned. */
  setPinned: (id: number, pinned: boolean) => invoke<void>("set_pinned", { id, pinned }),
```

(기존 `setPinned` 줄은 위 주석 달린 줄로 대체.)

- [ ] **Step 2: 필터 칩에서 고정 제거**

`src/panel/Toolbar.tsx`:

```ts
export const FILTERS: Filter[] = ["all", "text", "image", "files", "link"];
```

- [ ] **Step 3: i18n**

`src/i18n/en.ts` — `filters`에서 `pinned` 제거, `unpin` 아래에 추가:

```ts
  filters: { all: "All", text: "Text", image: "Images", files: "Files", link: "Links" },
```
```ts
  pinLimit: "You can pin up to 3 cards",
```

`src/i18n/ko.ts`:

```ts
  filters: { all: "전체", text: "텍스트", image: "이미지", files: "파일", link: "링크" },
```
```ts
  pinLimit: "최대 3개까지 고정할 수 있어요",
```

- [ ] **Step 4: 확인**

Run: `yarn typecheck && yarn test`
Expected: PASS. (`Panel.tsx`는 아직 `listPinned`를 쓰지 않지만 타입 오류는 없어야 함.)

- [ ] **Step 5: Commit**

```bash
git add src/api.ts src/panel/Toolbar.tsx src/i18n/en.ts src/i18n/ko.ts
git commit -m "feat(panel): drop the pinned filter chip, add listPinned and pin limit copy"
```

---

### Task 3: 합쳐진 선택 인덱스 헬퍼

**Files:**
- Create: `src/panel/selection.ts`
- Test: `src/panel/selection.test.ts`

**Interfaces:**
- Produces:
  - `defaultSelection(pinnedCount: number, historyCount: number): number`
  - `clampSelection(index: number, total: number): number`

- [ ] **Step 1: 실패하는 테스트 작성**

`src/panel/selection.test.ts`:

```ts
import { test } from "node:test";
import assert from "node:assert/strict";
import { clampSelection, defaultSelection } from "./selection.ts";

test("opening selects the newest history card, right after the pins", () => {
  assert.equal(defaultSelection(0, 5), 0);
  assert.equal(defaultSelection(2, 5), 2);
  assert.equal(defaultSelection(3, 1), 3);
});

test("with no history the first pin is selected", () => {
  assert.equal(defaultSelection(2, 0), 0);
  assert.equal(defaultSelection(0, 0), 0);
});

test("selection stays inside the combined pinned + history list", () => {
  assert.equal(clampSelection(-1, 4), 0);
  assert.equal(clampSelection(4, 4), 3);
  assert.equal(clampSelection(2, 4), 2);
  // A card moving between areas can shrink the list under the selection.
  assert.equal(clampSelection(5, 3), 2);
  assert.equal(clampSelection(1, 0), 0);
});
```

- [ ] **Step 2: 실패 확인**

Run: `yarn test`
Expected: FAIL — `Cannot find module './selection.ts'`.

- [ ] **Step 3: 구현**

`src/panel/selection.ts`:

```ts
/** Selection is an index into the panel's cards in screen order: pins first, then history. */

/** The newest history card, or the first pin when the history is empty. */
export function defaultSelection(pinnedCount: number, historyCount: number): number {
  return historyCount > 0 ? pinnedCount : 0;
}

export function clampSelection(index: number, total: number): number {
  return Math.max(0, Math.min(index, total - 1));
}
```

- [ ] **Step 4: 통과 확인**

Run: `yarn test`
Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add src/panel/selection.ts src/panel/selection.test.ts
git commit -m "feat(panel): selection helpers for pins + history"
```

---

### Task 4: 패널 — 왼쪽 고정 영역, 키보드, 한도 알림, FLIP

**Files:**
- Modify: `src/panel/Panel.tsx`
- Modify: `src/panel/motion.ts:12-50` (`cardLefts`, `playFlip`)
- Modify: `src/panel/panel.css` (`.row` 근처에 새 규칙)

**Interfaces:**
- Consumes: `api.listPinned`, `api.setPinned`의 `"pin_limit"` 거절, `t.pinLimit`(Task 2); `defaultSelection`, `clampSelection`(Task 3).
- Produces: `cardLefts(container)`/`playFlip(container, before)`가 컨테이너 하위 모든 `.card[data-id]`를 대상으로 함.

- [ ] **Step 1: `motion.ts` — 컨테이너 전체 카드 대상**

```ts
/** Each card's left edge on screen, by clip id, across every card area inside `container`. */
export function cardLefts(container: HTMLElement | null): Map<number, number> {
  const lefts = new Map<number, number>();
  for (const card of container?.querySelectorAll<HTMLElement>(".card[data-id]") ?? []) {
    lefts.set(Number(card.dataset.id), card.getBoundingClientRect().left);
  }
  return lefts;
}
```

`playFlip` 시그니처와 루프 머리만 바꾼다(본문은 그대로):

```ts
export function playFlip(container: HTMLElement, before: Map<number, number>): void {
  for (const card of container.querySelectorAll<HTMLElement>(".card[data-id]")) {
    const prev = before.get(Number(card.dataset.id));
```

(기존의 `const card = el as HTMLElement;` 줄은 삭제.)

- [ ] **Step 2: `Panel.tsx` — 상태와 로딩**

import 추가:

```ts
import { clampSelection, defaultSelection } from "./selection.ts";
```

상태 추가(`clips` 상태 옆):

```ts
  const [pinned, setPinnedClips] = useState<Clip[]>([]);
  const [pinNotice, setPinNotice] = useState(false);
  const noticeTimer = useRef<number | undefined>(undefined);
  const bodyRef = useRef<HTMLDivElement>(null);
```

`openRef` 선언 아래:

```ts
  /** Every card in screen order; `selected` indexes this. */
  const items = [...pinned, ...clips];
```

`reload` 본문 교체:

```ts
      const id = ++requestId.current;
      const [page, pins] = await Promise.all([api.listClips(textQuery, filter, appId, 0, PAGE), api.listPinned()]);
      if (id !== requestId.current) return;
      // Only copies arriving while the panel is on screen animate — not searches or filters.
      if (animate && openRef.current && !reducedMotion()) flipFrom.current = cardLefts(bodyRef.current);
      setPinnedClips(pins);
      setClips(page);
      setHasMore(page.length === PAGE);
      setSelected((s) =>
        keepSelection ? clampSelection(s, pins.length + page.length) : defaultSelection(pins.length, page.length),
      );
      if (!keepSelection) {
        setConfirmingId(null);
        rowRef.current?.scrollTo({ left: 0 });
      }
```

`panel://closed` 리스너 안 `setUpdateFailed(false);` 아래:

```ts
      setPinNotice(false);
```

FLIP 레이아웃 이펙트 교체:

```ts
  useLayoutEffect(() => {
    const before = flipFrom.current;
    flipFrom.current = null;
    if (before && bodyRef.current) playFlip(bodyRef.current, before);
  }, [clips, pinned]);
```

선택 스크롤 이펙트 교체(고정 카드는 `.row` 밖이므로 id로 찾는다):

```ts
  const selectedId = items[selected]?.id;
  useEffect(() => {
    bodyRef.current
      ?.querySelector(`[data-id="${selectedId}"]`)
      ?.scrollIntoView({ block: "nearest", inline: "nearest" });
    if (selected >= items.length - 5) void loadMore();
  }, [selectedId, selected, items.length, loadMore]);
```

- [ ] **Step 3: `Panel.tsx` — 고정 토글과 알림**

`togglePin` 교체:

```ts
  const showPinNotice = () => {
    window.clearTimeout(noticeTimer.current);
    setPinNotice(true);
    noticeTimer.current = window.setTimeout(() => setPinNotice(false), 2000);
  };

  const togglePin = (clip: Clip | undefined) => {
    if (!clip) return;
    api.setPinned(clip.id, !clip.pinned).catch((e) => {
      if (e === "pin_limit") showPinNotice();
    });
  };
```

- [ ] **Step 4: `Panel.tsx` — 키 처리에서 `clips[...]` → `items[...]`**

`onKeyDown`의 switch에서:

```ts
      case "move":
        setSelected((s) => clampSelection(s + action.delta, items.length));
        break;
      case "copy":
        copy(items[selected], action.plain);
        break;
      case "togglePin":
        togglePin(items[selected]);
        break;
```

```ts
      case "delete": {
        const clip = items[selected];
```

```ts
        remove(items.find((c) => c.id === confirmingId));
```

- [ ] **Step 5: `Panel.tsx` — 렌더**

`return` 위에 카드 렌더 함수:

```tsx
  /** `i` is the card's index in `items`. */
  const renderCard = (clip: Clip, i: number) => (
    <Card
      key={clip.id}
      clip={clip}
      selected={i === selected}
      onSelect={() => {
        setSelected(i);
        setConfirmingId(null);
      }}
      onCopy={(plain) => copy(clip, plain)}
      onTogglePin={() => togglePin(clip)}
      onDragOut={
        (clip.kind === "files" || clip.kind === "image") && !clip.missing
          ? (card, x, y) => liftCard(clip, card, x, y)
          : undefined
      }
      confirming={clip.id === confirmingId}
      onConfirmDelete={() => {
        remove(clip);
        setConfirmingId(null);
      }}
      onCancelDelete={() => setConfirmingId(null)}
    />
  );
```

`<Toolbar ... />` 뒤의 `{clips.length === 0 ? (...) : (...)}` 블록 전체를 교체:

```tsx
      <div className="body" ref={bodyRef}>
        {pinned.length > 0 && (
          <>
            <div className="pinned-row" role="listbox">
              {pinned.map((clip, i) => renderCard(clip, i))}
              {pinNotice && (
                <div className="pin-notice" role="status">
                  {t.pinLimit}
                </div>
              )}
            </div>
            <div className="pin-divider" aria-hidden />
          </>
        )}
        {clips.length === 0 ? (
          <div className="empty">{query || filter !== "all" || app ? t.noResults : t.empty}</div>
        ) : (
          <div className="row" ref={rowRef} role="listbox" onWheel={onWheel} onScroll={onScroll}>
            {clips.map((clip, i) => renderCard(clip, pinned.length + i))}
          </div>
        )}
      </div>
```

- [ ] **Step 6: `panel.css`**

`.row` 규칙에 `min-width: 0;`을 추가하고(가로 flex 안에서 스크롤되려면 필요), `.row` 위에 추가:

```css
/* Pins on the left, history scrolling on the right. */
.body {
  flex: 1;
  min-height: 0;
  display: flex;
}

.pinned-row {
  position: relative;
  flex: none;
  display: flex;
  gap: 10px;
  padding: 4px;
}

.pin-divider {
  flex: none;
  width: 1px;
  margin: 8px 6px;
  background: var(--border);
}

.pin-notice {
  position: absolute;
  top: 12px;
  left: 50%;
  z-index: 3;
  transform: translateX(-50%);
  padding: 8px 12px;
  border-radius: var(--radius);
  border: 1px solid var(--border);
  background: var(--card);
  box-shadow: rgba(0, 0, 0, 0.12) 0 8px 24px;
  font-size: 12px;
  font-weight: 600;
  white-space: nowrap;
}
```

- [ ] **Step 7: 확인**

Run: `yarn typecheck && yarn test`
Expected: PASS.

Run: `yarn tauri dev` 후 수동 확인(라이트/다크 모두):
1. 고정 0개 → 고정 영역·구분선 없음, 기존과 동일.
2. `⌘P`로 1→3개 고정 → 왼쪽에 고정한 순서로 쌓이고 카드가 미끄러지듯 이동.
3. 4번째 `⌘P` / 📌 클릭 → "최대 3개까지 고정할 수 있어요" 2초 후 사라짐, 연타하면 타이머 재시작.
4. 검색어·필터·`@앱` 입력 → 오른쪽만 걸러지고 고정 영역 그대로.
5. `←/→`로 고정 영역 ↔ 히스토리 넘나들기; 패널 열면 히스토리 첫 카드 선택.
6. 고정 카드에서 `Enter` 복사, `Delete` 확인 삭제, 파일/이미지 드래그 아웃.
7. 고정 카드 `⌘P` 해제 → 히스토리로 돌아가고 선택이 범위 안에 남음.
8. 히스토리를 비우고(설정 → 전체 삭제) 고정만 남김 → 첫 고정 카드 선택, 오른쪽 빈 문구.

- [ ] **Step 8: Commit**

```bash
git add src/panel/Panel.tsx src/panel/motion.ts src/panel/panel.css
git commit -m "feat(panel): pinned cards on the left, separated from history"
```

---

### Task 5: README·버전 0.6.3

**Files:**
- Modify: `README.md` (주요 기능 "검색과 필터", "카드 고정" 항목)
- Modify: `src-tauri/tauri.conf.json:4`, `package.json:4`, `src-tauri/Cargo.toml:3`, `src-tauri/Cargo.lock`

- [ ] **Step 1: README**

"검색과 필터" 항목의 `전체 / 고정 / 텍스트 / 이미지 / 파일 / 링크로` → `전체 / 텍스트 / 이미지 / 파일 / 링크로`.

"카드 고정" 항목 전체를 교체:

```markdown
- **카드 고정**: `Cmd+P`(macOS) / `Ctrl+P`(Windows)나 카드의 📌으로 최대 3개까지 고정할 수 있습니다. 고정한 카드는 패널 왼쪽에 고정한 순서대로 따로 모여 검색·필터와 상관없이 항상 보이고, 보관 기간 정리·전체 삭제에서도 지워지지 않습니다.
```

- [ ] **Step 2: 버전 올리기**

```bash
sed -i '' 's/"version": "0.6.2"/"version": "0.6.3"/' src-tauri/tauri.conf.json package.json
sed -i '' 's/^version = "0.6.2"/version = "0.6.3"/' src-tauri/Cargo.toml
(cd src-tauri && cargo check)   # Cargo.lock의 vee 버전 갱신
grep -n '0\.6\.[23]' src-tauri/tauri.conf.json package.json src-tauri/Cargo.toml
```

Expected: 세 파일 모두 `0.6.3`, `0.6.2` 없음. `git diff src-tauri/Cargo.lock`은 앱 패키지 버전 한 줄만 바뀜.

- [ ] **Step 3: 전체 확인**

Run: `(cd src-tauri && cargo test --lib) && yarn typecheck && yarn test`
Expected: 모두 PASS.

- [ ] **Step 4: Commit**

```bash
git add README.md src-tauri/tauri.conf.json package.json src-tauri/Cargo.toml src-tauri/Cargo.lock
git commit -m "chore: release 0.6.3"
```
