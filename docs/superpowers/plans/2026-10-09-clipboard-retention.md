# 클립보드 보관 기간 · 카드 고정 · 앱 제외 · 서식 없이 복사 Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** 설정에서 클립보드 보관 기간(제한 없음/7/30/90일)을 정해 오래된 기록을 자동 정리하고, 카드 고정·앱 제외·서식 없이 복사를 추가한다.

**Architecture:** SQLite 스키마 v4에 `clips.pinned`, `apps.excluded`를 추가하고, "고정 안 된 행 삭제 + 이미지 파일 정리"를 `Store`의 공용 헬퍼 하나로 모아 보관 기간 정리·앱 제외·전체 삭제가 같이 쓴다. 정리는 시작 시·1시간마다·설정 변경 직후 백엔드에서 돌고, 프런트엔드는 기존 `clips://changed` 이벤트로 갱신된다. 설정 화면의 기록 관련 UI는 새 파일 `HistorySection.tsx`로 분리한다.

**Tech Stack:** Tauri 2 (Rust, rusqlite), React 19 + TypeScript, `node --test`.

**Spec:** `docs/superpowers/specs/2026-10-09-clipboard-retention-design.md`

## Global Constraints

- 새 의존성 없음 (Rust·npm 모두).
- 고정 카드는 자동으로 지우지 않는다: 보관 기간 정리, 앱 제외 정리, 전체 삭제 모두 `pinned = 0`인 행만 지운다. 고정 카드는 사용자가 직접 `Delete`할 때만 지워진다.
- 설정 키 `retention`, 값 `off` | `7` | `30` | `90`, 기본 `off`. 기준 시각은 `last_used_at`.
- 로그에는 정리된 개수만 남기고 내용은 남기지 않는다.
- 고정 단축키는 `⌘P` / `Ctrl+P`(그냥 `p`는 검색 입력). 서식 없이 복사는 `⇧Enter`와 `⇧`+더블클릭.
- 문구는 ko/en 둘 다 추가한다. 토큰은 `src/theme.css` 변수만 쓴다.
- 작업 브랜치: `feature/clipboard-retention`. 커밋 메시지 끝에 아래 두 줄을 붙인다:
  ```
  Co-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>
  Claude-Session: https://claude.ai/code/session_01A1kDMALrN6saiV5bTkY12W
  ```

## Review Focus

- 고정했던 카드와 같은 내용을 다시 복사하면(upsert의 bump 경로) 고정이 풀리지 않아야 한다 → Task 1 테스트.
- 기간이 지났더라도 그 사이 다시 복사·사용해 `last_used_at`이 갱신된 항목은 정리되지 않아야 한다 → Task 2 테스트.
- 한 번도 본 적 없는 앱(`apps`에 행이 없음)에서 복사하면 제외 검사가 오류 없이 "제외 아님"이어야 한다 → Task 3 테스트.
- `retention = off`에서는 어떤 정리도 일어나지 않아야 하고, 기간을 늘릴 때는 지울 항목이 0개라 확인 없이 적용돼야 한다 → Task 4 테스트.
- `⌘P`를 누른 채 있으면(auto-repeat) 고정이 깜빡이며 토글되지 않아야 하고, 카드가 하나도 없을 때 `⌘P`는 아무것도 하지 않아야 한다 → Task 6 테스트 + Task 7 가드.

---

### Task 1: 스키마 v4 + 카드 고정 저장소

**Files:**
- Modify: `src-tauri/src/store.rs` (스키마 상수, `init`, `ClipDto`, `list`, 새 `set_pinned`, 테스트)
- Modify: `src-tauri/src/lib.rs:52-70` (`list_clips`가 새 `list` 시그니처를 쓰도록)

**Interfaces:**
- Produces:
  - `Store::list(&self, query: &str, kind: Option<Kind>, app_id: Option<i64>, pinned_only: bool, offset: i64, limit: i64) -> Result<Vec<ClipDto>>`
  - `ClipDto.pinned: bool` (JSON `pinned`)
  - `Store::set_pinned(&self, id: i64, pinned: bool) -> Result<()>`
  - `list_clips` 명령: `kind == "pinned"`이면 `kind = None, pinned_only = true`

- [ ] **Step 1: 기존 테스트의 `list` 호출에 `false` 인자 추가**

모든 호출이 `..., <offset>, <limit>).unwrap` 꼴이라 한 줄로 바꾼다.

```bash
perl -pi -e 's/(\.list\(.*?), (\d+), (\d+)\)\.unwrap/$1, false, $2, $3).unwrap/g' src-tauri/src/store.rs
grep -c ', false, [0-9]*, [0-9]*).unwrap' src-tauri/src/store.rs
```
Expected: `32`

- [ ] **Step 2: 실패하는 테스트 작성**

`store.rs` 테스트 모듈 끝(마지막 `}` 앞)에 추가:

```rust
    #[test]
    fn pinned_clips_filter_and_survive_a_bump() {
        let (s, _d) = store();
        let a = s.upsert(text("keep"), None, 1).unwrap();
        s.upsert(text("other"), None, 2).unwrap();
        s.set_pinned(a, true).unwrap();
        assert_eq!(texts(&s.list("", None, None, true, 0, 50).unwrap()), vec!["keep"]);
        // Copying the same content again bumps it but must not unpin it.
        s.upsert(text("keep"), None, 3).unwrap();
        let all = s.list("", None, None, false, 0, 50).unwrap();
        assert_eq!(texts(&all), vec!["keep", "other"]);
        assert!(all[0].pinned);
        assert!(!all[1].pinned);
        s.set_pinned(a, false).unwrap();
        assert!(s.list("", None, None, true, 0, 50).unwrap().is_empty());
    }
```

그리고 `v1_database_migrates_to_latest`의 마지막 부분을 바꾼다:

```rust
        let s = Store::open(&db, &dir.path().join("images")).unwrap();
        let clip = &s.list("", None, None, false, 0, 50).unwrap()[0];
        assert!(clip.stack.is_empty());
        assert!(!clip.pinned);
        let version: i64 = s.conn.query_row("PRAGMA user_version", [], |r| r.get(0)).unwrap();
        assert_eq!(version, 4);
```

- [ ] **Step 3: 테스트가 실패하는지 확인**

Run: `cd src-tauri && cargo test --lib store::`
Expected: 컴파일 실패 — `list`의 인자 개수 불일치, `set_pinned`/`pinned` 없음.

- [ ] **Step 4: 구현**

`SCHEMA_V3` 아래에 추가:

```rust
/// Pinned clips are never removed automatically; excluded apps are never recorded.
const SCHEMA_V4: &str = "
BEGIN;
ALTER TABLE clips ADD COLUMN pinned INTEGER NOT NULL DEFAULT 0;
ALTER TABLE apps ADD COLUMN excluded INTEGER NOT NULL DEFAULT 0;
PRAGMA user_version = 4;
COMMIT;
";
```

`init()`의 `if version < 3 { ... }` 다음에:

```rust
        if version < 4 {
            conn.execute_batch(SCHEMA_V4)?;
        }
```

`ClipDto`의 `stack` 필드 앞에:

```rust
    pub pinned: bool,
```

`list` 시그니처와 본문:

```rust
    pub fn list(
        &self,
        query: &str,
        kind: Option<Kind>,
        app_id: Option<i64>,
        pinned_only: bool,
        offset: i64,
        limit: i64,
    ) -> Result<Vec<ClipDto>> {
        let mut sql = String::from(
            "SELECT c.id, c.kind, substr(c.text, 1, ?), COALESCE(length(c.text), 0), c.thumb_png, c.meta,
                    a.name, a.icon_png, c.last_used_at,
                    CASE c.kind WHEN 'files' THEN c.text WHEN 'image' THEN c.image_path END,
                    c.pinned
             FROM clips c LEFT JOIN apps a ON a.id = c.app_id
             WHERE 1 = 1",
        );
```

`if let Some(a) = app_id { ... }` 블록 다음에:

```rust
        if pinned_only {
            sql.push_str(" AND c.pinned = 1");
        }
```

`ClipDto { ... }` 생성부의 `stack: Vec::new(),` 앞에:

```rust
                pinned: r.get::<_, i64>(10)? != 0,
```

`touch` 아래에:

```rust
    pub fn set_pinned(&self, id: i64, pinned: bool) -> Result<()> {
        self.conn.execute("UPDATE clips SET pinned = ?1 WHERE id = ?2", params![pinned, id])?;
        Ok(())
    }
```

`lib.rs`의 `list_clips` 본문:

```rust
    let (kind, pinned_only) = match kind.as_str() {
        "all" => (None, false),
        "pinned" => (None, true),
        k => (Some(Kind::parse(k).ok_or_else(|| format!("unknown kind: {k}"))?), false),
    };
    state
        .store
        .lock()
        .unwrap()
        .list(&query, kind, app_id, pinned_only, offset.max(0), limit.clamp(1, 200))
        .map_err(|e| e.to_string())
```

- [ ] **Step 5: 테스트 통과 확인**

Run: `cd src-tauri && cargo test`
Expected: 모두 PASS (기존 `history_is_not_trimmed` 포함).

- [ ] **Step 6: 커밋**

```bash
git add src-tauri/src/store.rs src-tauri/src/lib.rs
git commit -m "feat: store pinned clips in schema v4"   # + attribution lines
```

---

### Task 2: 고정 안 된 행 정리 — 보관 기간 정리, 전체 삭제, 저장 현황

**Files:**
- Modify: `src-tauri/src/store.rs` (새 헬퍼 `delete_unpinned`/`count_unpinned`, `prune`, `prunable_count`, `clear` 변경, `StatsDto`, `stats`, 테스트)

**Interfaces:**
- Consumes: Task 1의 `pinned` 열, `set_pinned`, `list(..., pinned_only, ...)`
- Produces:
  - `Store::prunable_count(&self, cutoff: i64) -> Result<i64>`
  - `Store::prune(&self, cutoff: i64) -> Result<usize>`
  - `Store::clear(&self) -> Result<usize>` (반환형이 `()`에서 바뀜, 고정 카드는 남김)
  - `Store::stats(&self) -> Result<StatsDto>`, `StatsDto { count: i64, pinned: i64, image_bytes: u64 }` (JSON `count`, `pinned`, `imageBytes`)
  - 내부 헬퍼 `delete_unpinned(cond, args)`, `count_unpinned(cond, args)` — Task 3이 재사용

- [ ] **Step 1: 실패하는 테스트 작성**

테스트 모듈 끝에 추가:

```rust
    #[test]
    fn prune_removes_only_unpinned_clips_older_than_the_cutoff() {
        let (s, _d) = store();
        let old_img = s.upsert(image(1), None, 10).unwrap();
        let old_img_path = image_path(&s, old_img);
        s.upsert(text("old"), None, 20).unwrap();
        let pinned = s.upsert(text("old but pinned"), None, 30).unwrap();
        s.set_pinned(pinned, true).unwrap();
        s.upsert(text("at cutoff"), None, 100).unwrap();
        s.upsert(text("new"), None, 200).unwrap();
        assert_eq!(s.prunable_count(100).unwrap(), 2);
        assert_eq!(s.prune(100).unwrap(), 2);
        assert_eq!(texts(&s.list("", None, None, false, 0, 50).unwrap()), vec!["new", "at cutoff", "old but pinned"]);
        assert!(!old_img_path.exists());
        assert_eq!(s.prunable_count(100).unwrap(), 0);
    }

    #[test]
    fn using_a_clip_again_keeps_it_from_being_pruned() {
        let (s, _d) = store();
        let id = s.upsert(text("reused"), None, 10).unwrap();
        s.touch(id, 500).unwrap();
        s.upsert(text("recopied"), None, 20).unwrap();
        s.upsert(text("recopied"), None, 600).unwrap();
        assert_eq!(s.prune(100).unwrap(), 0);
        assert_eq!(s.list("", None, None, false, 0, 50).unwrap().len(), 2);
    }

    #[test]
    fn clear_keeps_pinned_clips_and_their_images() {
        let (s, _d) = store();
        let kept = s.upsert(image(1), None, 1).unwrap();
        let kept_path = image_path(&s, kept);
        s.set_pinned(kept, true).unwrap();
        let gone = s.upsert(image(2), None, 2).unwrap();
        let gone_path = image_path(&s, gone);
        s.upsert(text("gone"), None, 3).unwrap();
        assert_eq!(s.clear().unwrap(), 2);
        let left = s.list("", None, None, false, 0, 50).unwrap();
        assert_eq!(left.len(), 1);
        assert_eq!(left[0].id, kept);
        assert!(kept_path.exists());
        assert!(!gone_path.exists());
    }

    #[test]
    fn stats_count_clips_pins_and_image_bytes() {
        let (s, _d) = store();
        let id = s.upsert(image(1), None, 1).unwrap(); // 8-byte png
        s.upsert(text("a"), None, 2).unwrap();
        s.set_pinned(id, true).unwrap();
        let st = s.stats().unwrap();
        assert_eq!((st.count, st.pinned, st.image_bytes), (2, 1, 8));
    }
```

- [ ] **Step 2: 테스트가 실패하는지 확인**

Run: `cd src-tauri && cargo test --lib store::`
Expected: 컴파일 실패 — `prunable_count`, `prune`, `stats` 없음, `clear()` 반환형 불일치.

- [ ] **Step 3: 구현**

`AppDto` 아래에:

```rust
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct StatsDto {
    pub count: i64,
    pub pinned: i64,
    pub image_bytes: u64,
}
```

`delete` 아래에 헬퍼와 공개 함수를 추가하고, 기존 `clear`를 교체:

```rust
    /// Deletes unpinned clips matching `cond`, then their image files. Returns how many went.
    fn delete_unpinned(&self, cond: &str, args: &[Value]) -> Result<usize> {
        let tx = self.conn.unchecked_transaction()?;
        let paths: Vec<String> = {
            let mut stmt = tx.prepare(&format!(
                "SELECT image_path FROM clips WHERE pinned = 0 AND ({cond}) AND image_path IS NOT NULL"
            ))?;
            stmt.query_map(params_from_iter(args.iter()), |r| r.get(0))?.collect::<rusqlite::Result<_>>()?
        };
        let n = tx.execute(&format!("DELETE FROM clips WHERE pinned = 0 AND ({cond})"), params_from_iter(args.iter()))?;
        tx.commit()?;
        for path in paths {
            let _ = std::fs::remove_file(path);
        }
        Ok(n)
    }

    fn count_unpinned(&self, cond: &str, args: &[Value]) -> Result<i64> {
        Ok(self.conn.query_row(
            &format!("SELECT COUNT(*) FROM clips WHERE pinned = 0 AND ({cond})"),
            params_from_iter(args.iter()),
            |r| r.get(0),
        )?)
    }

    /// Unpinned clips last used before `cutoff` (ms), i.e. what `prune` would remove.
    pub fn prunable_count(&self, cutoff: i64) -> Result<i64> {
        self.count_unpinned("last_used_at < ?", &[Value::Integer(cutoff)])
    }

    pub fn prune(&self, cutoff: i64) -> Result<usize> {
        self.delete_unpinned("last_used_at < ?", &[Value::Integer(cutoff)])
    }

    /// Deletes every clip except pinned ones.
    pub fn clear(&self) -> Result<usize> {
        self.delete_unpinned("1 = 1", &[])
    }

    pub fn stats(&self) -> Result<StatsDto> {
        let (count, pinned) = self.conn.query_row(
            "SELECT COUNT(*), COALESCE(SUM(pinned), 0) FROM clips",
            [],
            |r| Ok((r.get(0)?, r.get(1)?)),
        )?;
        let mut image_bytes = 0;
        for entry in std::fs::read_dir(&self.images_dir)? {
            image_bytes += entry?.metadata()?.len();
        }
        Ok(StatsDto { count, pinned, image_bytes })
    }
```

`lib.rs`의 `clear_history`는 바꾸지 않는다. `clear()?;`의 반환값을 쓰지 않으므로 반환형이 `usize`로 바뀌어도 그대로 컴파일된다.

- [ ] **Step 4: 테스트 통과 확인**

Run: `cd src-tauri && cargo test`
Expected: 모두 PASS. 기존 `clip_thumbs` 테스트의 `s.clear().unwrap();`도 그대로 통과(반환값 무시).

- [ ] **Step 5: 커밋**

```bash
git add src-tauri/src/store.rs
git commit -m "feat: prune unpinned clips by age and keep pins on clear"   # + attribution lines
```

---

### Task 3: 앱 제외 저장소

**Files:**
- Modify: `src-tauri/src/store.rs` (`is_excluded`, `set_excluded`, `app_clip_count`, `excluded_apps`, 테스트)

**Interfaces:**
- Consumes: Task 2의 `delete_unpinned`, `count_unpinned`
- Produces:
  - `Store::is_excluded(&self, bundle_id: &str) -> Result<bool>`
  - `Store::set_excluded(&self, app_id: i64, excluded: bool) -> Result<usize>` (제외할 때 지운 개수)
  - `Store::app_clip_count(&self, app_id: i64) -> Result<i64>` (고정 제외)
  - `Store::excluded_apps(&self) -> Result<Vec<AppDto>>` (이름순, `count`는 남은 클립 수)

- [ ] **Step 1: 실패하는 테스트 작성**

```rust
    #[test]
    fn excluding_an_app_deletes_its_unpinned_clips_and_marks_it() {
        let (s, _d) = store();
        let a = s.upsert_app("com.agilebits.onepassword", "1Password", None).unwrap();
        let b = s.upsert_app("com.apple.Notes", "Notes", None).unwrap();
        s.upsert(text("secret"), Some(a), 1).unwrap();
        let kept = s.upsert(text("pinned secret"), Some(a), 2).unwrap();
        s.set_pinned(kept, true).unwrap();
        s.upsert(text("note"), Some(b), 3).unwrap();
        assert!(!s.is_excluded("com.agilebits.onepassword").unwrap());
        assert!(!s.is_excluded("never.seen.app").unwrap());
        assert_eq!(s.app_clip_count(a).unwrap(), 1);
        assert_eq!(s.set_excluded(a, true).unwrap(), 1);
        assert!(s.is_excluded("com.agilebits.onepassword").unwrap());
        assert_eq!(texts(&s.list("", None, None, false, 0, 50).unwrap()), vec!["note", "pinned secret"]);
        let ex = s.excluded_apps().unwrap();
        assert_eq!((ex.len(), ex[0].name.as_str(), ex[0].count), (1, "1Password", 1));
        assert_eq!(s.set_excluded(a, false).unwrap(), 0);
        assert!(!s.is_excluded("com.agilebits.onepassword").unwrap());
        assert!(s.excluded_apps().unwrap().is_empty());
    }
```

- [ ] **Step 2: 테스트가 실패하는지 확인**

Run: `cd src-tauri && cargo test --lib store::excluding`
Expected: 컴파일 실패 — 네 함수 없음.

- [ ] **Step 3: 구현**

`upsert_app` 아래에:

```rust
    /// Unknown apps are not excluded.
    pub fn is_excluded(&self, bundle_id: &str) -> Result<bool> {
        let excluded: Option<bool> = self
            .conn
            .query_row("SELECT excluded FROM apps WHERE bundle_id = ?1", [bundle_id], |r| r.get(0))
            .optional()?;
        Ok(excluded.unwrap_or(false))
    }

    /// Excluding an app also deletes its unpinned clips; returns how many.
    pub fn set_excluded(&self, app_id: i64, excluded: bool) -> Result<usize> {
        self.conn.execute("UPDATE apps SET excluded = ?1 WHERE id = ?2", params![excluded, app_id])?;
        if !excluded {
            return Ok(0);
        }
        self.delete_unpinned("app_id = ?", &[Value::Integer(app_id)])
    }

    /// Unpinned clips from this app, i.e. what excluding it would delete.
    pub fn app_clip_count(&self, app_id: i64) -> Result<i64> {
        self.count_unpinned("app_id = ?", &[Value::Integer(app_id)])
    }

    pub fn excluded_apps(&self) -> Result<Vec<AppDto>> {
        let mut stmt = self.conn.prepare(
            "SELECT a.id, a.name, a.icon_png, COUNT(c.id)
             FROM apps a LEFT JOIN clips c ON c.app_id = a.id
             WHERE a.excluded = 1
             GROUP BY a.id
             ORDER BY a.name",
        )?;
        let rows = stmt.query_map([], |r| {
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

- [ ] **Step 4: 테스트 통과 확인**

Run: `cd src-tauri && cargo test`
Expected: 모두 PASS.

- [ ] **Step 5: 커밋**

```bash
git add src-tauri/src/store.rs
git commit -m "feat: exclude apps from history in the store"   # + attribution lines
```

---

### Task 4: 보관 기간 설정 + 자동 정리

**Files:**
- Modify: `src-tauri/src/settings.rs` (기본값, 검증, `SettingsDto`, `cutoff_for`, `prune_now`, `spawn_pruner`, `count_prunable` 명령, `set_setting` 훅, 테스트)
- Modify: `src-tauri/src/lib.rs` (`setup`에서 `spawn_pruner`, 명령 등록)

**Interfaces:**
- Consumes: Task 2의 `Store::prune`, `Store::prunable_count`
- Produces:
  - `settings::cutoff_for(value: &str, now: i64) -> Option<i64>`
  - `settings::prune_now(app: &AppHandle) -> Result<usize, String>`
  - `settings::spawn_pruner(app: AppHandle)`
  - 명령 `count_prunable(value: String) -> Result<i64, String>`
  - `SettingsDto.retention: String` (JSON `retention`)

- [ ] **Step 1: 실패하는 테스트 작성**

`settings.rs` 테스트 모듈에 추가:

```rust
    #[test]
    fn retention_defaults_off_and_accepts_known_periods() {
        let dir = tempfile::tempdir().unwrap();
        let store = Store::open_in_memory(dir.path()).unwrap();
        assert_eq!(get(&store, "retention"), "off");
        for v in ["off", "7", "30", "90"] {
            assert!(validate("retention", v).is_ok(), "{v}");
        }
        assert!(validate("retention", "14").is_err());
        assert!(validate("retention", "").is_err());
    }

    #[test]
    fn cutoff_is_none_when_off_and_days_back_otherwise() {
        const DAY: i64 = 86_400_000;
        let now = 100 * DAY;
        assert_eq!(cutoff_for("off", now), None);
        assert_eq!(cutoff_for("7", now), Some(93 * DAY));
        assert_eq!(cutoff_for("90", now), Some(10 * DAY));
        assert_eq!(cutoff_for("bogus", now), None);
    }
```

- [ ] **Step 2: 테스트가 실패하는지 확인**

Run: `cd src-tauri && cargo test --lib settings::`
Expected: 컴파일 실패 — `cutoff_for` 없음 (그리고 `retention` 기본값 테스트 실패).

- [ ] **Step 3: 구현**

`get`의 기본값 match에 추가:

```rust
        "retention" => "off",
```

`validate`의 match에 추가:

```rust
        "retention" => matches!(value, "off" | "7" | "30" | "90"),
```

`SettingsDto`에 `confirm_delete` 다음 필드 `retention: String,`을 넣고, `get_settings`에 `retention: get(&store, "retention"),`를 넣는다.

`copy_sound` 아래에:

```rust
const DAY_MS: i64 = 86_400_000;
const PRUNE_EVERY: std::time::Duration = std::time::Duration::from_secs(60 * 60);

/// Clips last used before this (ms) are pruned; `None` keeps everything.
pub fn cutoff_for(value: &str, now: i64) -> Option<i64> {
    let days: i64 = value.parse().ok()?;
    Some(now - days * DAY_MS)
}

/// Prunes by the saved retention and tells the panel when anything went.
pub fn prune_now(app: &AppHandle) -> Result<usize, String> {
    let n = {
        let store = app.state::<AppState>().store.lock().unwrap();
        match cutoff_for(&get(&store, "retention"), crate::now_ms()) {
            Some(cutoff) => store.prune(cutoff).map_err(|e| e.to_string())?,
            None => 0,
        }
    };
    if n > 0 {
        log::info!("pruned {n} clips");
        let _ = app.emit("clips://changed", ());
    }
    Ok(n)
}

/// Prunes now and then every hour.
pub fn spawn_pruner(app: AppHandle) {
    std::thread::spawn(move || loop {
        if let Err(e) = prune_now(&app) {
            log::warn!("pruning failed: {e}");
        }
        std::thread::sleep(PRUNE_EVERY);
    });
}

/// How many clips a not-yet-saved retention value would delete right away.
#[tauri::command]
pub fn count_prunable(state: State<AppState>, value: String) -> Result<i64, String> {
    validate("retention", &value)?;
    match cutoff_for(&value, crate::now_ms()) {
        Some(cutoff) => state.store.lock().unwrap().prunable_count(cutoff).map_err(|e| e.to_string()),
        None => Ok(0),
    }
}
```

`set_setting`의 `if key == "locale" { ... }` 다음에:

```rust
    if key == "retention" {
        prune_now(&app)?;
    }
```

`lib.rs` `setup`에서 `watcher::spawn(...)` 바로 앞에:

```rust
            settings::spawn_pruner(app.handle().clone());
```

`generate_handler!`에 `settings::count_prunable,`을 `settings::set_shortcut,` 다음에 추가.

- [ ] **Step 4: 테스트 통과 확인**

Run: `cd src-tauri && cargo test`
Expected: 모두 PASS.

- [ ] **Step 5: 커밋**

```bash
git add src-tauri/src/settings.rs src-tauri/src/lib.rs
git commit -m "feat: prune history by the retention setting at start, hourly and on change"   # + attribution lines
```

---

### Task 5: 백엔드 명령 — 고정, 앱 제외, 저장 현황, 서식 없이 복사, watcher 제외

**Files:**
- Modify: `src-tauri/src/lib.rs` (새 명령 5개, `list_apps`에 `limit`, `copy_clip`에 `plain`, 등록)
- Modify: `src-tauri/src/watcher.rs:121-129` (제외된 앱 건너뛰기)
- Modify: `src-tauri/src/windows.rs` (`ToastPayload.plain`, `copy_clip(app, id, plain)`, `confirm_copy`)

**Interfaces:**
- Consumes: Task 1~3의 `set_pinned`, `is_excluded`, `set_excluded`, `app_clip_count`, `excluded_apps`, `stats`, `StatsDto`
- Produces (Tauri 명령, JS 인자명은 camelCase):
  - `set_pinned(id: i64, pinned: bool)` → emit `clips://changed`
  - `count_app_clips(app_id: i64) -> i64`
  - `set_app_excluded(app_id: i64, excluded: bool)` → emit `clips://changed`, `settings://changed`
  - `list_excluded_apps() -> Vec<AppDto>`
  - `get_stats() -> StatsDto`
  - `list_apps(query: String, limit: Option<i64>)` — 기본 8, `clamp(1, 500)`
  - `copy_clip(id: i64, plain: bool)`
  - `ToastPayload.plain: bool` (JSON `plain`)

- [ ] **Step 1: `lib.rs` 명령 추가·변경**

`list_apps` 교체:

```rust
#[tauri::command]
fn list_apps(state: State<AppState>, query: String, limit: Option<i64>) -> Result<Vec<AppDto>, String> {
    state.store.lock().unwrap().apps(&query, limit.unwrap_or(8).clamp(1, 500)).map_err(|e| e.to_string())
}
```

`copy_clip` 교체:

```rust
#[tauri::command]
fn copy_clip(app: AppHandle, id: i64, plain: bool) -> Result<(), String> {
    windows::copy_clip(&app, id, plain)
}
```

`clear_history` 아래에:

```rust
#[tauri::command]
fn set_pinned(app: AppHandle, state: State<AppState>, id: i64, pinned: bool) -> Result<(), String> {
    state.store.lock().unwrap().set_pinned(id, pinned).map_err(|e| e.to_string())?;
    let _ = app.emit("clips://changed", ());
    Ok(())
}

#[tauri::command]
fn count_app_clips(state: State<AppState>, app_id: i64) -> Result<i64, String> {
    state.store.lock().unwrap().app_clip_count(app_id).map_err(|e| e.to_string())
}

#[tauri::command]
fn set_app_excluded(app: AppHandle, state: State<AppState>, app_id: i64, excluded: bool) -> Result<(), String> {
    let n = state.store.lock().unwrap().set_excluded(app_id, excluded).map_err(|e| e.to_string())?;
    if n > 0 {
        log::info!("deleted {n} clips from an excluded app");
    }
    let _ = app.emit("clips://changed", ());
    let _ = app.emit("settings://changed", ());
    Ok(())
}

#[tauri::command]
fn list_excluded_apps(state: State<AppState>) -> Result<Vec<AppDto>, String> {
    state.store.lock().unwrap().excluded_apps().map_err(|e| e.to_string())
}

#[tauri::command]
fn get_stats(state: State<AppState>) -> Result<StatsDto, String> {
    state.store.lock().unwrap().stats().map_err(|e| e.to_string())
}
```

`use store::{AppDto, ClipDto, Kind, Store};` → `use store::{AppDto, ClipDto, Kind, StatsDto, Store};`

`generate_handler!`의 `clear_history,` 다음에:

```rust
            set_pinned,
            count_app_clips,
            set_app_excluded,
            list_excluded_apps,
            get_stats,
```

- [ ] **Step 2: watcher에서 제외된 앱 건너뛰기**

`watcher.rs` `Handler::capture`의 `Some(front) => {` 블록 맨 앞에:

```rust
            Some(front) => {
                if store.is_excluded(&front.bundle_id)? {
                    return Ok(());
                }
                let icon = if store.app_known(&front.bundle_id)? { None } else { source_app::icon_png(&front) };
```

- [ ] **Step 3: 서식 없이 복사 + 토스트 플래그**

`windows.rs` `ToastPayload`에 필드 추가:

```rust
    pub files: usize,
    /// Copied as plain text, without the source app's formatting.
    pub plain: bool,
}

impl ToastPayload {
    fn copied(content: &ClipContent, plain: bool) -> Self {
        match content {
            ClipContent::Text(t) => {
                Self { ok: true, kind: classify_text(t), text: Some(toast_preview(t)), files: 0, plain }
            }
            ClipContent::Image(_) => Self { ok: true, kind: Kind::Image, text: None, files: 0, plain: false },
            ClipContent::Files(f) => Self { ok: true, kind: Kind::Files, text: None, files: f.len(), plain: false },
        }
    }

    fn failed() -> Self {
        Self { ok: false, kind: Kind::Text, text: None, files: 0, plain: false }
    }
}
```

`copy_clip`:

```rust
/// Copies a clip back to the clipboard, closes the panel and confirms with a toast.
/// `plain` skips the saved raw formats so only the text goes back.
pub fn copy_clip(app: &AppHandle, id: i64, plain: bool) -> Result<(), String> {
    let state = app.state::<AppState>();
    let (content, formats) = {
        let store = state.store.lock().unwrap();
        let formats = if plain { Vec::new() } else { store.formats(id).map_err(|e| e.to_string())? };
        (store.content(id).map_err(|e| e.to_string())?, formats)
    };
```

같은 함수의 `confirm_copy(app, id, &content);` → `confirm_copy(app, id, &content, plain);`

`confirm_copy` 시그니처와 마지막 줄:

```rust
fn confirm_copy(app: &AppHandle, id: i64, content: &ClipContent, plain: bool) {
    ...
    show_toast(app, ToastPayload::copied(content, plain));
}
```

`grep -n "ToastPayload::copied\|confirm_copy(" src-tauri/src/*.rs`로 다른 호출부가 없는지 확인하고, 있으면 `false`를 넘긴다.

- [ ] **Step 4: 빌드·테스트 확인**

Run: `cd src-tauri && cargo clippy --all-targets -- -D warnings && cargo test`
Expected: 경고 없이 빌드, 모두 PASS.

- [ ] **Step 5: 커밋**

```bash
git add src-tauri/src/lib.rs src-tauri/src/watcher.rs src-tauri/src/windows.rs
git commit -m "feat: commands for pins, excluded apps, stats and plain-text copy"   # + attribution lines
```

---

### Task 6: 프런트엔드 API 타입 + 패널 키

**Files:**
- Modify: `src/api.ts`
- Modify: `src/panel/keys.ts`
- Test: `src/panel/keys.test.ts`

**Interfaces:**
- Consumes: Task 5의 명령들
- Produces:
  - `type Filter = Kind | "all" | "pinned"`, `Clip.pinned: boolean`, `type Retention = "off" | "7" | "30" | "90"`, `Settings.retention: Retention`, `ToastPayload.plain: boolean`, `interface Stats { count: number; pinned: number; imageBytes: number }`
  - `api.copyClip(id, plain = false)`, `api.listApps(query, limit?)`, `api.setPinned(id, pinned)`, `api.countPrunable(value)`, `api.countAppClips(appId)`, `api.setAppExcluded(appId, excluded)`, `api.listExcludedApps()`, `api.getStats()`, `api.setSetting`이 `"retention"` 키 허용
  - `KeyInput.metaOrCtrl: boolean`, `KeyAction`에 `{ type: "copy"; plain: boolean }`, `{ type: "togglePin" }`

- [ ] **Step 1: 실패하는 테스트 작성**

`keys.test.ts`의 `press` 기본값에 `metaOrCtrl: false,`를 추가하고, `"enter copies, escape hides, arrows move"`의 첫 줄을 바꾼다:

```ts
  assert.deepEqual(press("Enter"), { type: "copy", plain: false });
```

파일 끝에 추가:

```ts
test("shift+enter copies as plain text", () => {
  assert.deepEqual(press("Enter", { shiftKey: true }), { type: "copy", plain: true });
  // In the app list, shift+enter still picks the app.
  assert.deepEqual(press("Enter", { shiftKey: true, suggesting: true }), { type: "suggestPick" });
});

test("cmd/ctrl+p toggles the pin; a plain p is typed into search", () => {
  assert.deepEqual(press("p", { metaOrCtrl: true }), { type: "togglePin" });
  assert.deepEqual(press("P", { metaOrCtrl: true, shiftKey: true }), { type: "togglePin" });
  assert.equal(press("p"), null);
  // Holding the shortcut must not flip the pin back and forth.
  assert.equal(press("p", { metaOrCtrl: true, repeat: true }), null);
});

test("pin and plain copy keys are ignored while composing or confirming", () => {
  assert.equal(press("p", { metaOrCtrl: true, isComposing: true }), null);
  assert.equal(press("Enter", { shiftKey: true, isComposing: true }), null);
  assert.deepEqual(press("p", { metaOrCtrl: true, confirming: true }), { type: "cancelDelete" });
});
```

- [ ] **Step 2: 테스트가 실패하는지 확인**

Run: `yarn test`
Expected: FAIL — `copy`에 `plain` 없음, `togglePin` 없음.

- [ ] **Step 3: `keys.ts` 구현**

`KeyAction`의 `| { type: "copy" }` → `| { type: "copy"; plain: boolean }`, 그리고 `| { type: "togglePin" }`를 추가.

`KeyInput`에 추가:

```ts
  /** Cmd on macOS or Ctrl on Windows is held. */
  metaOrCtrl: boolean;
```

`if (e.suggesting) { ... }` 블록 다음, `switch (e.key)` 앞에:

```ts
  if (e.metaOrCtrl && e.key.toLowerCase() === "p") return e.repeat ? null : { type: "togglePin" };
```

`case "Enter": return { type: "copy" };` →

```ts
    case "Enter":
      return { type: "copy", plain: e.shiftKey };
```

- [ ] **Step 4: `api.ts` 구현**

```ts
export type Kind = "text" | "link" | "image" | "files";
export type Filter = Kind | "all" | "pinned";
export type Retention = "off" | "7" | "30" | "90";
```

`Clip`의 `stack` 앞에 `pinned: boolean;`. `Settings`의 `confirmDelete` 다음에 `retention: Retention;`. `ToastPayload`의 `files` 다음에:

```ts
  /** Copied as plain text, without formatting. */
  plain: boolean;
```

`App` 아래에:

```ts
export interface Stats {
  count: number;
  pinned: number;
  imageBytes: number;
}
```

`api` 객체에서 바꾸거나 추가:

```ts
  listApps: (query: string, limit?: number) => invoke<App[]>("list_apps", { query, limit }),
  copyClip: (id: number, plain = false) => invoke<void>("copy_clip", { id, plain }),
  setPinned: (id: number, pinned: boolean) => invoke<void>("set_pinned", { id, pinned }),
  countPrunable: (value: Retention) => invoke<number>("count_prunable", { value }),
  countAppClips: (appId: number) => invoke<number>("count_app_clips", { appId }),
  setAppExcluded: (appId: number, excluded: boolean) => invoke<void>("set_app_excluded", { appId, excluded }),
  listExcludedApps: () => invoke<App[]>("list_excluded_apps"),
  getStats: () => invoke<Stats>("get_stats"),
  setSetting: (key: "theme" | "locale" | "sound" | "soundName" | "confirmDelete" | "retention", value: string) =>
    invoke<void>("set_setting", { key, value }),
```

- [ ] **Step 5: 테스트 통과 확인**

Run: `yarn test`
Expected: PASS. (`yarn typecheck`는 Task 7에서 `Panel.tsx`가 새 필드를 넘긴 뒤에 통과한다.)

- [ ] **Step 6: 커밋**

```bash
git add src/api.ts src/panel/keys.ts src/panel/keys.test.ts
git commit -m "feat: panel keys for pinning and plain-text copy"   # + attribution lines
```

---

### Task 7: 패널 UI — 고정 칩·배지·버튼, ⌘P, ⇧Enter, 토스트

**Files:**
- Modify: `src/panel/Toolbar.tsx:5` (`FILTERS`)
- Modify: `src/panel/Panel.tsx` (키 입력, 복사, 고정 토글, Card props)
- Modify: `src/panel/Card.tsx` (pinned 클래스, 📌 버튼, `onCopy(plain)`)
- Modify: `src/panel/panel.css` (`.card.pinned`, `.pin`)
- Modify: `src/toast/Toast.tsx` (plain 문구)
- Modify: `src/i18n/en.ts`, `src/i18n/ko.ts` (패널 문구)

**Interfaces:**
- Consumes: Task 6의 `Filter`, `Clip.pinned`, `api.setPinned`, `api.copyClip(id, plain)`, `KeyInput.metaOrCtrl`, `togglePin`/`copy.plain` 액션, `ToastPayload.plain`
- Produces: `Card` props `onCopy: (plain: boolean) => void`, `onTogglePin: () => void`; i18n 키 `filters.pinned`, `copiedPlain`, `pin`, `unpin`

- [ ] **Step 1: i18n 문구**

`en.ts`:

```ts
  filters: { all: "All", pinned: "📌 Pinned", text: "Text", image: "Images", files: "Files", link: "Links" },
```
`copied: "Copied",` 다음에:
```ts
  copiedPlain: "Copied as plain text",
  pin: "Pin",
  unpin: "Unpin",
```

`ko.ts`:

```ts
  filters: { all: "전체", pinned: "📌 고정", text: "텍스트", image: "이미지", files: "파일", link: "링크" },
```
`copied: "복사됨",` 다음에:
```ts
  copiedPlain: "서식 없이 복사됨",
  pin: "고정",
  unpin: "고정 해제",
```

- [ ] **Step 2: 필터 칩**

`Toolbar.tsx`:

```ts
export const FILTERS: Filter[] = ["all", "pinned", "text", "image", "files", "link"];
```

- [ ] **Step 3: `Card.tsx`**

Props에서 `onCopy: () => void;` → 

```ts
  /** `plain` copies text without its formatting (shift held). */
  onCopy: (plain: boolean) => void;
  onTogglePin: () => void;
```

구조분해에 `onTogglePin`을 추가하고, 루트 `div`의 className과 더블클릭을 바꾼다:

```tsx
      className={["card", selected && "selected", confirming && "confirming", clip.pinned && "pinned"].filter(Boolean).join(" ")}
      ...
      onDoubleClick={(e) => onCopy(e.shiftKey)}
```

`card-head` 안 `time` 다음에:

```tsx
        <button
          className="pin"
          aria-label={clip.pinned ? t.unpin : t.pin}
          title={clip.pinned ? t.unpin : t.pin}
          tabIndex={-1}
          // Don't start a drag, select the card or copy it.
          onMouseDown={(e) => {
            e.preventDefault();
            e.stopPropagation();
          }}
          onClick={(e) => {
            e.stopPropagation();
            onTogglePin();
          }}
          onDoubleClick={(e) => e.stopPropagation()}
        >
          📌
        </button>
```

- [ ] **Step 4: `panel.css`**

`.card.selected` 규칙 바로 위에(선택 테두리가 이기도록):

```css
.card.pinned {
  border-color: color-mix(in srgb, var(--accent) 45%, transparent);
}
```

`.app-name` 규칙 아래에:

```css
.pin {
  flex: none;
  padding: 0 2px;
  font-size: 12px;
  line-height: 1;
  opacity: 0;
  transition: opacity 120ms;
}

.card:hover .pin {
  opacity: 0.5;
}

.card.pinned .pin,
.card .pin:hover {
  opacity: 1;
}
```

- [ ] **Step 5: `Panel.tsx`**

`copy`:

```ts
  const copy = (clip: Clip | undefined, plain = false) => {
    if (clip) void api.copyClip(clip.id, plain).catch(() => {});
  };

  const togglePin = (clip: Clip | undefined) => {
    if (clip) void api.setPinned(clip.id, !clip.pinned);
  };
```

`panelKeyAction({...})` 인자에 `metaOrCtrl: e.metaKey || e.ctrlKey,`를 추가하고, switch를 바꾼다:

```ts
      case "copy":
        copy(clips[selected], action.plain);
        break;
      case "togglePin":
        togglePin(clips[selected]);
        break;
```

(`e.preventDefault()`가 이미 액션이 있을 때 호출되므로 `⌘P` 인쇄는 막힌다.)

`<Card ... />`의 `onCopy={() => copy(clip)}` →

```tsx
              onCopy={(plain) => copy(clip, plain)}
              onTogglePin={() => togglePin(clip)}
```

- [ ] **Step 6: 토스트**

`Toast.tsx`의 `<b>{p.ok ? t.copied : t.copyFailed}</b>` →

```tsx
        <b>{p.ok ? (p.plain ? t.copiedPlain : t.copied) : t.copyFailed}</b>
```

- [ ] **Step 7: 타입·테스트 확인**

Run: `yarn typecheck && yarn test`
Expected: 오류 없음, PASS. (`Settings.tsx`가 아직 `retention`을 안 써도 타입은 통과한다.)

- [ ] **Step 8: 커밋**

```bash
git add src/panel src/toast/Toast.tsx src/i18n
git commit -m "feat: pin cards and copy as plain text from the panel"   # + attribution lines
```

---

### Task 8: 설정 UI — "클립보드 기록" 섹션

**Files:**
- Create: `src/settings/HistorySection.tsx` (`HistorySection`, `ConfirmBox`, `formatBytes`)
- Modify: `src/settings/Settings.tsx` (`Segmented` export + `className`/`pending` props, 섹션 교체, `clear` 이동)
- Modify: `src/settings/settings.css`
- Modify: `src/i18n/en.ts`, `src/i18n/ko.ts` (설정 문구)

**Interfaces:**
- Consumes: Task 6의 `api.countPrunable`, `api.setSetting("retention", …)`, `api.listApps(query, limit)`, `api.listExcludedApps`, `api.countAppClips`, `api.setAppExcluded`, `api.getStats`, `api.clearHistory`, `Retention`, `Stats`, `App`
- Produces: `export function HistorySection({ run }: { run: (p: Promise<unknown>) => void })`; `Settings.tsx`에서 `export function Segmented<T>({ value, options, onChange, className?, pending? })`

- [ ] **Step 1: i18n 설정 문구**

`en.ts`의 `settings` 안에서 `history`, `clearHistory`, `clearConfirm`을 지우고 다음으로 바꾼다:

```ts
    historySection: "Clipboard history",
    retention: "Keep history",
    retentionHint: "Items not used for longer than this are removed automatically",
    retentionOptions: { off: "Forever", "7": "7 days", "30": "30 days", "90": "90 days" },
    retentionConfirm: (label: string, n: number) =>
      `Switching to "${label}" deletes ${n.toLocaleString("en")} older ${n === 1 ? "item" : "items"} right away. This can't be undone.`,
    deleteAndApply: "Delete and apply",
    excludedApps: "Don't record these apps",
    excludedAppsHint: "Anything copied in these apps isn't saved",
    addApp: "Add app",
    includeApp: (name: string) => `Record ${name} again`,
    excludeConfirm: (name: string, n: number) =>
      `${n.toLocaleString("en")} ${n === 1 ? "item" : "items"} from "${name}" will also be deleted. Pinned cards stay.`,
    deleteAndExclude: "Delete and exclude",
    stats: (count: number, size: string) => `${count.toLocaleString("en")} items · images ${size}`,
    clearHistory: "Delete all",
    clearConfirm: "Delete all clipboard history? This can't be undone.",
    clearConfirmKeepPinned: (n: number) =>
      `Delete everything except ${n} pinned ${n === 1 ? "card" : "cards"}? This can't be undone.`,
```

`ko.ts`의 같은 자리:

```ts
    historySection: "클립보드 기록",
    retention: "보관 기간",
    retentionHint: "마지막으로 쓴 지 기간이 지난 항목은 자동으로 정리돼요",
    retentionOptions: { off: "제한 없음", "7": "최근 7일", "30": "최근 30일", "90": "최근 90일" },
    retentionConfirm: (label: string, n: number) =>
      `'${label}'로 바꾸면 오래된 항목 ${n.toLocaleString("ko")}개가 지금 바로 삭제돼요. 되돌릴 수 없어요.`,
    deleteAndApply: "삭제하고 적용",
    excludedApps: "기록하지 않을 앱",
    excludedAppsHint: "이 앱에서 복사한 내용은 저장하지 않아요",
    addApp: "앱 추가",
    includeApp: (name: string) => `${name} 다시 기록`,
    excludeConfirm: (name: string, n: number) =>
      `'${name}'의 기록 ${n.toLocaleString("ko")}개도 삭제돼요. 고정한 카드는 남아요.`,
    deleteAndExclude: "삭제하고 제외",
    stats: (count: number, size: string) => `저장된 항목 ${count.toLocaleString("ko")}개 · 이미지 ${size}`,
    clearHistory: "전체 삭제",
    clearConfirm: "클립보드 히스토리를 모두 삭제할까요? 되돌릴 수 없어요.",
    clearConfirmKeepPinned: (n: number) => `고정한 카드 ${n}개를 제외하고 모두 삭제할까요? 되돌릴 수 없어요.`,
```

`grep -rn "s.history\b\|settings.history\b" src`로 옛 키를 쓰는 곳이 `Settings.tsx`뿐인지 확인한다(Step 3에서 지운다).

- [ ] **Step 2: `HistorySection.tsx` 작성**

```tsx
import { useCallback, useEffect, useState } from "react";
import { listen } from "@tauri-apps/api/event";
import { ask } from "@tauri-apps/plugin-dialog";
import { api, type App, type Retention, type Stats } from "../api.ts";
import { usePrefs } from "../prefs.tsx";
import { Segmented } from "./Settings.tsx";

const RETENTIONS: Retention[] = ["off", "7", "30", "90"];

function formatBytes(n: number): string {
  if (n < 1024 * 1024) return `${Math.round(n / 1024)}KB`;
  if (n < 1024 * 1024 * 1024) return `${Math.round(n / 1024 / 1024)}MB`;
  return `${(n / 1024 / 1024 / 1024).toFixed(1)}GB`;
}

function ConfirmBox({
  message,
  confirmLabel,
  onConfirm,
  onCancel,
}: {
  message: string;
  confirmLabel: string;
  onConfirm: () => void;
  onCancel: () => void;
}) {
  const { t } = usePrefs();
  return (
    <div className="confirm-box" role="alertdialog">
      <p>{message}</p>
      <div className="confirm-acts">
        <button className="btn gray" onClick={onCancel}>
          {t.cancel}
        </button>
        <button className="btn danger" onClick={onConfirm}>
          {confirmLabel}
        </button>
      </div>
    </div>
  );
}

/** One confirmation at a time: picking anything else cancels the open one. */
type Pending = { kind: "retention"; value: Retention; count: number } | { kind: "exclude"; app: App; count: number } | null;

export function HistorySection({ run }: { run: (p: Promise<unknown>) => void }) {
  const { settings, t } = usePrefs();
  const s = t.settings;
  const [pending, setPending] = useState<Pending>(null);
  const [excluded, setExcluded] = useState<App[]>([]);
  const [apps, setApps] = useState<App[]>([]);
  const [stats, setStats] = useState<Stats | null>(null);

  const refresh = useCallback(() => {
    void Promise.all([api.listExcludedApps(), api.listApps("", 500), api.getStats()]).then(([ex, all, st]) => {
      setExcluded(ex);
      setApps(all);
      setStats(st);
    });
  }, []);

  useEffect(() => {
    refresh();
    const offClips = listen("clips://changed", refresh);
    const offSettings = listen("settings://changed", refresh);
    return () => {
      void offClips.then((off) => off());
      void offSettings.then((off) => off());
    };
  }, [refresh]);

  const pickRetention = async (value: Retention) => {
    setPending(null);
    if (value === settings.retention) return;
    const count = await api.countPrunable(value);
    if (count === 0) run(api.setSetting("retention", value));
    else setPending({ kind: "retention", value, count });
  };

  const pickApp = async (id: number) => {
    setPending(null);
    const app = apps.find((a) => a.id === id);
    if (!app) return;
    const count = await api.countAppClips(id);
    if (count === 0) run(api.setAppExcluded(id, true));
    else setPending({ kind: "exclude", app, count });
  };

  const confirm = () => {
    if (pending?.kind === "retention") run(api.setSetting("retention", pending.value));
    if (pending?.kind === "exclude") run(api.setAppExcluded(pending.app.id, true));
    setPending(null);
  };

  const clear = async () => {
    setPending(null);
    const pinned = stats?.pinned ?? 0;
    const message = pinned > 0 ? s.clearConfirmKeepPinned(pinned) : s.clearConfirm;
    if (await ask(message, { title: "Vee", kind: "warning" })) run(api.clearHistory());
  };

  const excludedIds = new Set(excluded.map((a) => a.id));
  const choices = apps.filter((a) => !excludedIds.has(a.id));

  return (
    <>
      <h2 className="sec-title">{s.historySection}</h2>
      <section>
        <div className="setting-row col">
          <div>
            {s.retention}
            <small>{s.retentionHint}</small>
          </div>
          <Segmented
            className="wide"
            value={settings.retention}
            pending={pending?.kind === "retention" ? pending.value : undefined}
            onChange={(v) => void pickRetention(v)}
            options={RETENTIONS.map((r) => ({ value: r, label: s.retentionOptions[r] }))}
          />
          {pending?.kind === "retention" && (
            <ConfirmBox
              message={s.retentionConfirm(s.retentionOptions[pending.value], pending.count)}
              confirmLabel={s.deleteAndApply}
              onConfirm={confirm}
              onCancel={() => setPending(null)}
            />
          )}
        </div>
        <div className="setting-row col">
          <div>
            {s.excludedApps}
            <small>{s.excludedAppsHint}</small>
          </div>
          <div className="app-tags">
            {excluded.map((a) => (
              <span key={a.id} className="app-chip">
                {a.icon && <img src={a.icon} alt="" />}
                {a.name}
                <button aria-label={s.includeApp(a.name)} onClick={() => run(api.setAppExcluded(a.id, false))}>
                  ×
                </button>
              </span>
            ))}
            <select
              className="select add-app"
              aria-label={s.addApp}
              value=""
              disabled={choices.length === 0}
              onChange={(e) => void pickApp(Number(e.target.value))}
            >
              <option value="" disabled>
                {s.addApp}
              </option>
              {choices.map((a) => (
                <option key={a.id} value={a.id}>
                  {a.name}
                </option>
              ))}
            </select>
          </div>
          {pending?.kind === "exclude" && (
            <ConfirmBox
              message={s.excludeConfirm(pending.app.name, pending.count)}
              confirmLabel={s.deleteAndExclude}
              onConfirm={confirm}
              onCancel={() => setPending(null)}
            />
          )}
        </div>
        <div className="setting-row">
          <span className="stat">{stats ? s.stats(stats.count, formatBytes(stats.imageBytes)) : ""}</span>
          <button className="btn danger" onClick={() => void clear()}>
            {s.clearHistory}
          </button>
        </div>
      </section>
    </>
  );
}
```

- [ ] **Step 3: `Settings.tsx` 변경**

`Segmented`를 export하고 props 두 개를 추가한다:

```tsx
export function Segmented<T extends string>({
  value,
  options,
  onChange,
  className,
  pending,
}: {
  value: T;
  options: { value: T; label: string }[];
  onChange: (value: T) => void;
  className?: string;
  /** An option waiting for confirmation, outlined in red. */
  pending?: T;
}) {
  return (
    <div className={className ? `seg ${className}` : "seg"} role="radiogroup">
      {options.map((o) => (
        <button
          key={o.value}
          role="radio"
          aria-checked={o.value === value}
          className={[o.value === value && "on", o.value === pending && "pending"].filter(Boolean).join(" ")}
          onClick={() => onChange(o.value)}
        >
          {o.label}
        </button>
      ))}
    </div>
  );
}
```

`import { ask } ...` 줄과 `Settings` 안의 `clear` 함수를 지우고, `import { HistorySection } from "./HistorySection.tsx";`를 추가한다. 두 번째 `<section>` 전체를 다음으로 바꾼다(`UpdateRow`는 첫 섹션 끝, `ShortcutRecorder` 행 다음으로 옮긴다):

```tsx
        <Row label={s.shortcut} hint={s.shortcutHint}>
          <ShortcutRecorder value={settings.shortcut} />
        </Row>
        <UpdateRow />
      </section>
      <HistorySection run={run} />
```

- [ ] **Step 4: `settings.css`**

파일 끝에 추가:

```css
.settings .sec-title {
  margin: 0 0 6px 4px;
  color: var(--muted);
  font-size: 11px;
  font-weight: 600;
}

.setting-row.col {
  flex-direction: column;
  align-items: stretch;
  gap: 8px;
}

.seg.wide {
  display: flex;
}

.seg.wide button {
  flex: 1;
}

.seg button.pending {
  outline: 1.5px dashed var(--danger);
  color: var(--danger);
}

.confirm-box {
  padding: 10px;
  border: 1px solid color-mix(in srgb, var(--danger) 20%, transparent);
  border-radius: 10px;
  background: color-mix(in srgb, var(--danger) 6%, transparent);
  font-size: 12px;
}

.confirm-acts {
  display: flex;
  justify-content: flex-end;
  gap: 6px;
  margin-top: 8px;
}

.btn.gray {
  background: var(--subtle);
  color: var(--text);
}

.app-tags {
  display: flex;
  flex-wrap: wrap;
  align-items: center;
  gap: 6px;
}

.app-chip {
  display: inline-flex;
  align-items: center;
  gap: 4px;
  padding: 3px 4px 3px 8px;
  border-radius: 8px;
  background: var(--badge-bg);
  color: var(--badge-fg);
  font-size: 12px;
}

.app-chip img {
  width: 14px;
  height: 14px;
}

.app-chip button {
  padding: 0 4px;
  color: var(--muted);
}

.select.add-app {
  margin-left: auto;
}

.stat {
  color: var(--muted);
  font-size: 11px;
}
```

- [ ] **Step 5: 타입·테스트 확인**

Run: `yarn typecheck && yarn test`
Expected: 오류 없음, PASS.

- [ ] **Step 6: 커밋**

```bash
git add src/settings src/i18n
git commit -m "feat: clipboard history settings for retention, excluded apps and stats"   # + attribution lines
```

---

### Task 9: 문서 + 실제 앱에서 확인

**Files:**
- Modify: `README.md` (주요 기능·사용법 표·설정 목록)

- [ ] **Step 1: README 갱신**

"주요 기능"의 **클립보드 히스토리** 항목에서 "개수 제한 없이 보관하며,"를 "기본은 기간 제한 없이 보관하고, 설정에서 최근 7일/30일/90일만 남기게 할 수 있습니다(고정한 카드는 남습니다)."로 바꾼다. 같은 목록에 추가:

```markdown
- **카드 고정**: `⌘P`(macOS) / `Ctrl+P`(Windows)나 카드의 📌으로 고정하면 보관 기간 정리·전체 삭제에서도 지워지지 않습니다. "📌 고정" 필터로 모아 볼 수 있습니다.
- **서식 없이 복사**: `⇧Enter` 또는 `⇧`+더블클릭으로 글꼴·링크 같은 서식을 뺀 텍스트만 복사합니다.
- **기록하지 않을 앱**: 설정에서 고른 앱에서 복사한 내용은 저장하지 않습니다.
```

**설정** 항목 끝의 "히스토리 전체 삭제"를 "보관 기간, 기록하지 않을 앱, 히스토리 전체 삭제(고정 카드 제외)"로 바꾼다.

"사용법" 표의 `복사 후 닫기` 행 아래에:

```markdown
| 서식 없이 복사 | `⇧Enter`, `⇧`+더블클릭 |
| 카드 고정/해제 | `Cmd+P` / `Ctrl+P`, 카드의 📌 |
```

"필터 전환" 행 설명에서 필터 목록이 언급되면 "고정"을 포함시킨다.

- [ ] **Step 2: 전체 검증**

Run: `cd src-tauri && cargo clippy --all-targets -- -D warnings && cargo test && cd .. && yarn typecheck && yarn test`
Expected: 모두 통과.

- [ ] **Step 3: 실제 앱 수동 확인**

Run: `yarn tauri dev`

확인 목록 (하나라도 다르면 해당 Task로 돌아가 고친다):
1. 설정 → "클립보드 기록" 섹션이 보이고, 업데이트 행은 위 섹션에 있다.
2. 보관 기간 "최근 7일" 선택 → 7일 넘은 항목이 있으면 빨간 점선 + 확인 박스, 없으면 바로 적용. "취소"하면 원래 값 유지. "삭제하고 적용" 후 패널이 즉시 갱신되고 저장 현황 숫자가 줄어든다.
3. 제한 없음으로 되돌리면 확인 없이 적용된다.
4. "앱 추가"에서 앱 선택 → 기록이 있으면 확인 박스, 적용 후 태그로 표시. 그 앱에서 복사해도 카드가 생기지 않는다. 태그 × 후 다시 복사하면 기록된다.
5. 패널에서 `⌘P` → 보라 테두리 + 📌, `Tab`으로 "📌 고정" 필터에 고정 카드만 보인다. hover 📌 클릭으로도 토글되고 카드가 복사·드래그되지 않는다.
6. 설정 "전체 삭제" → 고정이 있으면 "고정한 카드 N개를 제외하고…" 문구, 삭제 후 고정 카드만 남는다.
7. Notion/Word/웹에서 서식 텍스트 복사 → 패널에서 `⇧Enter` → 토스트 "서식 없이 복사됨", 문서에 붙이면 서식이 없다. `Enter`는 서식 유지.
8. 다크 테마에서 확인 박스·칩·고정 테두리가 읽힌다.

- [ ] **Step 4: 커밋**

```bash
git add README.md
git commit -m "docs: describe retention, pinned cards, excluded apps and plain-text copy"   # + attribution lines
```
