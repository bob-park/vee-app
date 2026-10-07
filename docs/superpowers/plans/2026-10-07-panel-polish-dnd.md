# Panel Polish & File Drag-Out Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Monitor-proportional panel/cards, a flash-free macOS entrance, file drag-out (copy), image-file thumbnails (single + stacked), no history cap, an optional delete confirmation, and a left-click tray menu.

**Architecture:** Tauri 2 app — Rust (`src-tauri/src`) owns windows, clipboard, SQLite store; React (`src/`) renders the panel/settings/toast webviews. Each task changes one behaviour end to end (Rust + TS where needed) and leaves the app building and tests green.

**Tech Stack:** Rust 2024, Tauri 2.12, rusqlite, clipboard-rs 0.3.5 (`RustImageData`), new crate `drag = "2.1"`; React 19 + TypeScript, `node:test`.

**Spec:** `docs/superpowers/specs/2026-10-07-panel-polish-dnd-design.md`

## Global Constraints

- Work on the current branch (`master`). Do not touch the pre-existing uncommitted `yarn.lock` change; stage files explicitly (never `git add -A`/`-a`).
- Every commit message ends with:
  ```
  Co-Authored-By: Claude Opus 5.5 (1M context) <noreply@anthropic.com>
  Claude-Session: https://claude.ai/code/session_01UzZJRwqVMiYpw19ZUBFhjg
  ```
- Test commands (from repo root):
  - Rust: `cargo test --manifest-path src-tauri/Cargo.toml --lib`
  - Frontend: `yarn test` and `yarn typecheck`
- Panel height = `clamp(round(work-area height in pt × 0.30), 300, 480)` pt.
- Image extensions: png, jpg, jpeg, gif, webp, bmp, tif, tiff (case-insensitive). Thumbnails only for files ≤ 50MB, at most the first 3 files (`STACK_LAYERS = 3`).
- Setting key `confirmDelete` = `"on" | "off"`, default `"off"`.
- Strings (ko / en) are given verbatim in the tasks; don't invent others.
- Design tokens live in `src/theme.css` (`--accent`, `--danger`, `--card`, `--subtle`, `--border`, `--muted`, `--radius`); use them, no new colours except the tag scrim `rgba(16, 17, 20, 0.65)`.

**Deviation from spec (deliberate):** card width uses `calc((100vh - 72px) * 0.95)` instead of `aspect-ratio`, because the panel window height equals the panel height, so `vh` is exact and avoids WebKit's flex/aspect-ratio edge cases.

---

### Task 1: Remove the history cap

**Files:**
- Modify: `src-tauri/src/store.rs` (const `MAX_CLIPS` line 11, `upsert` tail, `fn trim`, test `trims_oldest_beyond_max_and_removes_image_file`)

**Interfaces:** Produces: `Store::upsert` no longer deletes old rows. `MAX_CLIPS` and `trim` are gone.

- [ ] **Step 1: Replace the trim test with a no-trim test**

In `store.rs` tests, delete `trims_oldest_beyond_max_and_removes_image_file` and add:

```rust
    #[test]
    fn history_is_not_trimmed() {
        let (s, _d) = store();
        for i in 0..1100 {
            s.upsert(text(&format!("clip {i}")), None, i).unwrap();
        }
        assert_eq!(s.list("", None, 0, 200).unwrap().len(), 200);
        assert_eq!(s.list("", None, 1000, 200).unwrap().len(), 100);
    }
```

(Image-file removal on delete stays covered by `delete_and_clear_remove_image_files`.)

- [ ] **Step 2: Run it to see it fail**

Run: `cargo test --manifest-path src-tauri/Cargo.toml --lib history_is_not_trimmed`
Expected: FAIL — second assertion gets 0 (rows beyond 1000 were trimmed).

- [ ] **Step 3: Remove the cap**

Delete `pub const MAX_CLIPS: i64 = 1000;`, delete the whole `fn trim(&self)`, and change the end of `upsert` from

```rust
        )?;
        self.trim()
    }
```
to
```rust
        )?;
        Ok(())
    }
```

- [ ] **Step 4: Run all Rust tests**

Run: `cargo test --manifest-path src-tauri/Cargo.toml --lib`
Expected: all PASS, no `MAX_CLIPS` references left (`grep -rn MAX_CLIPS src-tauri/src` prints nothing).

- [ ] **Step 5: Commit**

```bash
git add src-tauri/src/store.rs
git commit -m "feat: keep unlimited clipboard history"
```

---

### Task 2: `confirmDelete` setting (backend + settings screen)

**Files:**
- Modify: `src-tauri/src/settings.rs` (`SettingsDto`, `get`, `validate`, `get_settings`, tests)
- Modify: `src/api.ts` (`Settings`, `setSetting`)
- Modify: `src/i18n/en.ts`, `src/i18n/ko.ts`
- Modify: `src/settings/Settings.tsx` (row after copy sound)

**Interfaces:**
- Produces: `Settings.confirmDelete: "on" | "off"` in TS; `api.setSetting("confirmDelete", "on" | "off")`.
- Produces i18n keys used by Task 9: `t.deleteConfirm`, `t.cancel`, `t.delete`, `t.deleteHint`.

- [ ] **Step 1: Write the failing Rust test** (in `settings.rs` tests)

```rust
    #[test]
    fn confirm_delete_defaults_off_and_accepts_on_off() {
        let dir = tempfile::tempdir().unwrap();
        let store = Store::open_in_memory(dir.path()).unwrap();
        assert_eq!(get(&store, "confirmDelete"), "off");
        assert!(validate("confirmDelete", "on").is_ok());
        assert!(validate("confirmDelete", "off").is_ok());
        assert!(validate("confirmDelete", "yes").is_err());
    }
```

- [ ] **Step 2: Run it to see it fail**

Run: `cargo test --manifest-path src-tauri/Cargo.toml --lib confirm_delete_defaults`
Expected: FAIL — `get` returns `""`.

- [ ] **Step 3: Implement in `settings.rs`**

- In `SettingsDto` add field after `sound: String,`: `confirm_delete: String,` (serde camelCase → `confirmDelete`).
- In `get`'s default match: `"sound" => "on",` then add `"confirmDelete" => "off",`.
- In `validate`: add `"confirmDelete" => matches!(value, "on" | "off"),`.
- In `get_settings` after `sound: get(&store, "sound"),` add `confirm_delete: get(&store, "confirmDelete"),`.

- [ ] **Step 4: Run Rust tests**

Run: `cargo test --manifest-path src-tauri/Cargo.toml --lib`
Expected: PASS.

- [ ] **Step 5: Frontend types and strings**

`src/api.ts`:
```ts
export interface Settings {
  theme: "system" | "light" | "dark";
  locale: "system" | "ko" | "en";
  shortcut: string;
  sound: "on" | "off";
  confirmDelete: "on" | "off";
  autostart: boolean;
  version: string;
}
```
and
```ts
  setSetting: (key: "theme" | "locale" | "sound" | "confirmDelete", value: string) =>
    invoke<void>("set_setting", { key, value }),
```

`src/i18n/en.ts` — after `copyFailed: ...,` add:
```ts
  deleteConfirm: "Delete this item?",
  cancel: "Cancel",
  delete: "Delete",
  deleteHint: "⏎ Delete · esc Cancel",
```
and inside `settings` after `copySound: ...,`:
```ts
    confirmDelete: "Confirm before deleting",
    confirmDeleteHint: "Ask once more when deleting an item from the panel",
```

`src/i18n/ko.ts` — same positions:
```ts
  deleteConfirm: "이 항목을 삭제할까요?",
  cancel: "취소",
  delete: "삭제",
  deleteHint: "⏎ 삭제 · esc 취소",
```
```ts
    confirmDelete: "개별 삭제 시 확인",
    confirmDeleteHint: "패널에서 항목을 지울 때 한 번 더 확인합니다",
```

- [ ] **Step 6: Settings row** — in `Settings.tsx`, directly after the `copySound` `<Row>`:

```tsx
        <Row label={s.confirmDelete} hint={s.confirmDeleteHint}>
          <input
            type="checkbox"
            role="switch"
            className="switch"
            aria-label={s.confirmDelete}
            checked={settings.confirmDelete === "on"}
            onChange={(e) => run(api.setSetting("confirmDelete", e.target.checked ? "on" : "off"))}
          />
        </Row>
```

- [ ] **Step 7: Typecheck + tests**

Run: `yarn typecheck && yarn test`
Expected: PASS.

- [ ] **Step 8: Commit**

```bash
git add src-tauri/src/settings.rs src/api.ts src/i18n/en.ts src/i18n/ko.ts src/settings/Settings.tsx
git commit -m "feat: add confirm-before-delete setting"
```

---

### Task 3: Store — per-file thumbnails (schema V2, `stack`, preview)

**Files:**
- Modify: `src-tauri/src/store.rs`
- Modify: `src-tauri/src/watcher.rs:32` (constructor only — real thumbnails come in Task 4)

**Interfaces:**
- Produces:
  - `pub const STACK_LAYERS: usize = 3;`
  - `NewClip::Files { paths: Vec<String>, thumbs: Vec<(usize, Vec<u8>)> }` — `(file index, PNG bytes)`.
  - `ClipDto.stack: Vec<Option<String>>` (JSON `stack`): for files clips with ≥1 thumbnail, `min(3, file count)` layers, `Some(data URL)` for image layers, `None` otherwise; empty for everything else.
  - `Store::preview_png(&self, id: i64) -> Result<Option<Vec<u8>>>` — first file thumbnail, else the source app's icon, else `None`.

- [ ] **Step 1: Write failing tests** (in `store.rs` tests)

Add a helper next to `text()`:
```rust
    fn files(paths: &[&str]) -> NewClip {
        NewClip::Files { paths: paths.iter().map(|p| p.to_string()).collect(), thumbs: vec![] }
    }
```
Replace every `NewClip::Files(vec![...])` in existing tests with `files(&[...])`, e.g. `files(&["/tmp/x"])`, `files(&[&path(&file)])`, `files(&[&path(d.path())])`, `files(&["/definitely/not/here.txt", &path(&file)])`, `files(&["/a", "/b"])`.

Add:
```rust
    #[test]
    fn file_clips_carry_a_stack_of_up_to_three_layers() {
        let (s, _d) = store();
        let paths = |p: &[&str]| p.iter().map(|p| p.to_string()).collect::<Vec<_>>();
        s.upsert(NewClip::Files { paths: paths(&["/a.png"]), thumbs: vec![(0, vec![1])] }, None, 1).unwrap();
        s.upsert(
            NewClip::Files { paths: paths(&["/b.png", "/c.txt", "/d.png", "/e.png"]), thumbs: vec![(0, vec![2]), (2, vec![3])] },
            None,
            2,
        )
        .unwrap();
        s.upsert(files(&["/f.txt", "/g.txt"]), None, 3).unwrap();
        let all = s.list("", None, 0, 50).unwrap();
        assert!(all[0].stack.is_empty());
        assert_eq!(all[1].stack.iter().map(Option::is_some).collect::<Vec<_>>(), vec![true, false, true]);
        assert_eq!(all[2].stack.len(), 1);
        assert!(all[2].stack[0].as_ref().unwrap().starts_with("data:image/png;base64,"));
        assert_eq!(s.preview_png(all[1].id).unwrap(), Some(vec![2]));
    }

    #[test]
    fn preview_falls_back_to_the_app_icon() {
        let (s, _d) = store();
        let app = s.upsert_app("com.a", "A", Some(&[7, 7])).unwrap();
        s.upsert(files(&["/x.txt"]), Some(app), 1).unwrap();
        s.upsert(text("no app"), None, 2).unwrap();
        let all = s.list("", None, 0, 50).unwrap();
        assert_eq!(s.preview_png(all[1].id).unwrap(), Some(vec![7, 7]));
        assert_eq!(s.preview_png(all[0].id).unwrap(), None);
    }

    #[test]
    fn delete_and_clear_remove_file_thumbnails() {
        let (s, _d) = store();
        let thumbs_left = |s: &Store| s.conn.query_row("SELECT count(*) FROM clip_thumbs", [], |r| r.get::<_, i64>(0)).unwrap();
        let one = |p: &str| NewClip::Files { paths: vec![p.into()], thumbs: vec![(0, vec![1])] };
        s.upsert(one("/a.png"), None, 1).unwrap();
        s.delete(s.list("", None, 0, 1).unwrap()[0].id).unwrap();
        assert_eq!(thumbs_left(&s), 0);
        s.upsert(one("/b.png"), None, 2).unwrap();
        s.clear().unwrap();
        assert_eq!(thumbs_left(&s), 0);
    }

    #[test]
    fn v1_database_migrates_to_v2() {
        let dir = tempfile::tempdir().unwrap();
        let db = dir.path().join("vee.db");
        {
            let conn = Connection::open(&db).unwrap();
            conn.execute_batch(SCHEMA_V1).unwrap();
            conn.execute(
                "INSERT INTO clips (kind, hash, text, meta, created_at, last_used_at) VALUES ('files', 'h', '/x.png', '1', 1, 1)",
                [],
            )
            .unwrap();
        }
        let s = Store::open(&db, &dir.path().join("images")).unwrap();
        assert!(s.list("", None, 0, 50).unwrap()[0].stack.is_empty());
        let version: i64 = s.conn.query_row("PRAGMA user_version", [], |r| r.get(0)).unwrap();
        assert_eq!(version, 2);
    }
```

- [ ] **Step 2: Run to see failures**

Run: `cargo test --manifest-path src-tauri/Cargo.toml --lib store::`
Expected: compile errors (`NewClip::Files` shape, `stack`, `preview_png`, `clip_thumbs`).

- [ ] **Step 3: Implement**

After `SCHEMA_V1` add:
```rust
/// Thumbnails of image files inside a files clip, keyed by the file's position.
const SCHEMA_V2: &str = "
BEGIN;
CREATE TABLE clip_thumbs (
  clip_id INTEGER NOT NULL,
  idx     INTEGER NOT NULL,
  png     BLOB NOT NULL,
  PRIMARY KEY (clip_id, idx)
);
CREATE TRIGGER clip_thumbs_ad AFTER DELETE ON clips BEGIN
  DELETE FROM clip_thumbs WHERE clip_id = old.id;
END;
PRAGMA user_version = 2;
COMMIT;
";
```
and near `PREVIEW_CHARS`:
```rust
/// How many files of a files clip get a layer in the card's preview stack.
pub const STACK_LAYERS: usize = 3;
```

In `init`, after the V1 block:
```rust
        if version < 2 {
            conn.execute_batch(SCHEMA_V2)?;
        }
```

`NewClip`:
```rust
pub enum NewClip {
    Text(String),
    Image { png: Vec<u8>, thumb_png: Vec<u8>, width: u32, height: u32 },
    /// `thumbs` holds `(file index, PNG)` for image files among the first `STACK_LAYERS`.
    Files { paths: Vec<String>, thumbs: Vec<(usize, Vec<u8>)> },
}
```

`ClipDto` — add after `is_dir`:
```rust
    /// Preview layers of a files clip; empty when none of its files has a thumbnail.
    pub stack: Vec<Option<String>>,
```

`upsert` — hash arm becomes `NewClip::Files { paths, .. } => sha256_hex(&[b"files\0", paths.join("\n").as_bytes()]),` and the insert part becomes:
```rust
        let (kind, text, image_path, thumb, meta, file_thumbs) = match clip {
            NewClip::Text(t) => (classify_text(&t), Some(t), None, None, None, Vec::new()),
            NewClip::Image { png, thumb_png, width, height } => {
                let path = self.images_dir.join(format!("{hash}.png"));
                std::fs::write(&path, png)?;
                let path = path.to_string_lossy().into_owned();
                (Kind::Image, None, Some(path), Some(thumb_png), Some(format!("{width}×{height}")), Vec::new())
            }
            NewClip::Files { paths, thumbs } => {
                let count = paths.len().to_string();
                (Kind::Files, Some(paths.join("\n")), None, None, Some(count), thumbs)
            }
        };
        self.conn.execute(
            "INSERT INTO clips (kind, hash, text, image_path, thumb_png, meta, app_id, created_at, last_used_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?8)",
            params![kind.as_str(), hash, text, image_path, thumb, meta, app_id, now],
        )?;
        let id = self.conn.last_insert_rowid();
        for (idx, png) in file_thumbs {
            self.conn.execute(
                "INSERT INTO clip_thumbs (clip_id, idx, png) VALUES (?1, ?2, ?3)",
                params![id, idx as i64, png],
            )?;
        }
        Ok(())
```

`list` — in the `ClipDto { .. }` literal add `stack: Vec::new(),` and replace the final `Ok(rows.collect::<rusqlite::Result<_>>()?)` with:
```rust
        let mut clips: Vec<ClipDto> = rows.collect::<rusqlite::Result<_>>()?;
        drop(stmt);
        for clip in clips.iter_mut().filter(|c| c.kind == Kind::Files) {
            let count = clip.meta.as_deref().and_then(|m| m.parse().ok()).unwrap_or(1);
            clip.stack = self.stack(clip.id, count)?;
        }
        Ok(clips)
```

New methods in `impl Store` (after `list`):
```rust
    fn stack(&self, id: i64, files: usize) -> Result<Vec<Option<String>>> {
        let mut stmt = self.conn.prepare("SELECT idx, png FROM clip_thumbs WHERE clip_id = ?1")?;
        let thumbs: std::collections::HashMap<i64, Vec<u8>> =
            stmt.query_map([id], |r| Ok((r.get(0)?, r.get(1)?)))?.collect::<rusqlite::Result<_>>()?;
        if thumbs.is_empty() {
            return Ok(Vec::new());
        }
        Ok((0..files.min(STACK_LAYERS) as i64).map(|i| thumbs.get(&i).map(|png| data_url(png))).collect())
    }

    /// Image for dragging a clip out: its first file thumbnail, else its source app's icon.
    pub fn preview_png(&self, id: i64) -> Result<Option<Vec<u8>>> {
        let thumb = self
            .conn
            .query_row("SELECT png FROM clip_thumbs WHERE clip_id = ?1 ORDER BY idx LIMIT 1", [id], |r| r.get(0))
            .optional()?;
        if thumb.is_some() {
            return Ok(thumb);
        }
        Ok(self
            .conn
            .query_row(
                "SELECT a.icon_png FROM clips c JOIN apps a ON a.id = c.app_id WHERE c.id = ?1",
                [id],
                |r| r.get::<_, Option<Vec<u8>>>(0),
            )
            .optional()?
            .flatten())
    }
```

`watcher.rs:32` — temporarily: `return Ok(Some(NewClip::Files { paths: files, thumbs: Vec::new() }));`

- [ ] **Step 4: Run Rust tests**

Run: `cargo test --manifest-path src-tauri/Cargo.toml --lib`
Expected: all PASS.

- [ ] **Step 5: Frontend type** — in `src/api.ts` `Clip` add after `isDir: boolean;`:
```ts
  /** Preview layers of a files clip: data URL per image file, null for other files. */
  stack: (string | null)[];
```
Run: `yarn typecheck` → PASS.

- [ ] **Step 6: Commit**

```bash
git add src-tauri/src/store.rs src-tauri/src/watcher.rs src/api.ts
git commit -m "feat: store per-file thumbnails for file clips"
```

---

### Task 4: Watcher — generate thumbnails for copied image files

**Files:**
- Modify: `src-tauri/src/watcher.rs`
- Modify: `src-tauri/Cargo.toml` (image formats)

**Interfaces:**
- Consumes: `store::STACK_LAYERS`, `NewClip::Files { paths, thumbs }` (Task 3).
- Produces: `pub fn is_image_path(path: &str) -> bool`, `fn file_thumbs(paths: &[String]) -> Vec<(usize, Vec<u8>)>`.

- [ ] **Step 1: Enable gif/webp/bmp decoding** (clipboard-rs only enables png/jpeg/tiff; Cargo feature unification adds ours)

`src-tauri/Cargo.toml` — macOS deps section, add:
```toml
image = { version = "0.25", default-features = false, features = ["gif", "webp", "bmp"] }
```
Windows section — change the existing `image` line to:
```toml
image = { version = "0.25", default-features = false, features = ["png", "jpeg", "gif", "webp", "bmp", "tiff"] }
```

- [ ] **Step 2: Write failing tests** (in `watcher.rs` tests)

```rust
    /// A valid 1×1 PNG.
    const PIXEL_PNG: &str = "iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAYAAAAfFcSJAAAADUlEQVR42mNkYPhfDwAChwGA60e6kgAAAABJRU5ErkJggg==";

    #[test]
    fn recognises_image_extensions_case_insensitively() {
        assert!(is_image_path("/a/Shot.PNG"));
        assert!(is_image_path("C:\\pics\\a.jpeg"));
        assert!(is_image_path("/a/b.webp"));
        assert!(!is_image_path("/a/report.pdf"));
        assert!(!is_image_path("/a/README"));
    }

    #[test]
    fn thumbnails_only_decodable_images_among_the_first_three() {
        use base64::{Engine, engine::general_purpose::STANDARD};
        let dir = tempfile::tempdir().unwrap();
        let png = dir.path().join("a.PNG");
        std::fs::write(&png, STANDARD.decode(PIXEL_PNG).unwrap()).unwrap();
        let txt = dir.path().join("b.txt");
        std::fs::write(&txt, "x").unwrap();
        let broken = dir.path().join("c.jpg");
        std::fs::write(&broken, "not an image").unwrap();
        let fourth = dir.path().join("d.png");
        std::fs::copy(&png, &fourth).unwrap();
        let p = |p: &std::path::Path| p.to_string_lossy().into_owned();
        let thumbs = file_thumbs(&[p(&png), p(&txt), p(&broken), p(&fourth)]);
        assert_eq!(thumbs.iter().map(|(i, _)| *i).collect::<Vec<_>>(), vec![0]);
        assert!(thumbs[0].1.starts_with(b"\x89PNG"));
    }
```

- [ ] **Step 3: Run to see failure**

Run: `cargo test --manifest-path src-tauri/Cargo.toml --lib watcher::`
Expected: compile error — `is_image_path`, `file_thumbs` not found.

- [ ] **Step 4: Implement** in `watcher.rs`

Imports: `use crate::{AppState, now_ms, source_app, store::{NewClip, STACK_LAYERS}};` and extend the clipboard-rs import with `RustImageData`.

Add constants/functions above `read`:
```rust
const IMAGE_EXTS: [&str; 8] = ["png", "jpg", "jpeg", "gif", "webp", "bmp", "tif", "tiff"];

pub fn is_image_path(path: &str) -> bool {
    std::path::Path::new(path)
        .extension()
        .and_then(|e| e.to_str())
        .is_some_and(|e| IMAGE_EXTS.contains(&e.to_ascii_lowercase().as_str()))
}

fn thumb_png(path: &str) -> Option<Vec<u8>> {
    let image = RustImageData::from_path(path).ok()?;
    Some(image.thumbnail(THUMB_SIZE, THUMB_SIZE).ok()?.to_png().ok()?.get_bytes().to_vec())
}

/// Thumbnails of image files among the first `STACK_LAYERS`; undecodable files are skipped.
fn file_thumbs(paths: &[String]) -> Vec<(usize, Vec<u8>)> {
    paths
        .iter()
        .take(STACK_LAYERS)
        .enumerate()
        .filter(|(_, p)| is_image_path(p))
        .filter(|(_, p)| std::fs::metadata(p).is_ok_and(|m| m.is_file() && m.len() <= MAX_IMAGE_BYTES as u64))
        .filter_map(|(i, p)| thumb_png(p).map(|png| (i, png)))
        .collect()
}
```
And in `read`:
```rust
        if !files.is_empty() {
            let thumbs = file_thumbs(&files);
            return Ok(Some(NewClip::Files { paths: files, thumbs }));
        }
```

- [ ] **Step 5: Run Rust tests**

Run: `cargo test --manifest-path src-tauri/Cargo.toml --lib`
Expected: all PASS.

- [ ] **Step 6: Commit**

```bash
git add src-tauri/Cargo.toml src-tauri/Cargo.lock src-tauri/src/watcher.rs
git commit -m "feat: thumbnail image files when files are copied"
```

---

### Task 5: Card — image-file preview (single + stack)

**Files:**
- Create: `src/panel/fileThumb.ts`, `src/panel/fileThumb.test.ts`
- Modify: `src/panel/Card.tsx`, `src/panel/panel.css`

**Interfaces:**
- Consumes: `Clip.stack` (Task 3).
- Produces: `imageTag(path: string): string | null`.

- [ ] **Step 1: Failing test** `src/panel/fileThumb.test.ts`

```ts
import { test } from "node:test";
import assert from "node:assert/strict";
import { imageTag } from "./fileThumb.ts";

test("tags image files with their upper-cased extension", () => {
  assert.equal(imageTag("/Users/me/Desktop/Shot.png"), "PNG");
  assert.equal(imageTag("C:\\pics\\a.JPEG"), "JPEG");
});

test("non-image or extension-less paths get no tag", () => {
  assert.equal(imageTag("/a/report.pdf"), null);
  assert.equal(imageTag("/a/README"), null);
  assert.equal(imageTag("/a.b/file"), null);
});
```

- [ ] **Step 2: Run to see failure**

Run: `yarn test`
Expected: FAIL — cannot find `./fileThumb.ts`.

- [ ] **Step 3: Implement** `src/panel/fileThumb.ts`

```ts
// Keep in sync with IMAGE_EXTS in src-tauri/src/watcher.rs.
const IMAGE_EXTS = ["png", "jpg", "jpeg", "gif", "webp", "bmp", "tif", "tiff"];

/** "PNG" for an image file path, null otherwise. */
export function imageTag(path: string): string | null {
  const name = path.split(/[\\/]/).pop() ?? "";
  const dot = name.lastIndexOf(".");
  const ext = dot > 0 ? name.slice(dot + 1).toLowerCase() : "";
  return IMAGE_EXTS.includes(ext) ? ext.toUpperCase() : null;
}
```

- [ ] **Step 4: Run tests** — `yarn test` → PASS.

- [ ] **Step 5: Render it in `Card.tsx`**

Add import `import { imageTag } from "./fileThumb.ts";`. Add helper and reuse it in `footLabel`:
```tsx
function firstPath(clip: Clip): string {
  return (clip.textPreview ?? "").split("\n")[0];
}
```
In `footLabel`'s files case replace `const first = (clip.textPreview ?? "").split("\n")[0];` with `const first = firstPath(clip);`.

Add above `Body`:
```tsx
/** Tilt per layer, front (first file) first. */
const LAYER_TILT = [0, 5, -7];

function FilesBody({ clip }: { clip: Clip }) {
  const { stack } = clip;
  if (stack.length === 0) return <div className={clip.isDir ? "doc folder" : "doc"} aria-hidden />;
  if (stack.length === 1) {
    const tag = imageTag(firstPath(clip));
    return (
      <>
        <img className="thumb" src={stack[0] ?? ""} alt="" />
        {tag && <span className="ext-tag">{tag}</span>}
      </>
    );
  }
  return (
    <div className="stack" aria-hidden>
      {stack.map((src, i) => {
        const style = { transform: `rotate(${LAYER_TILT[i]}deg)`, zIndex: LAYER_TILT.length - i };
        return src ? <img key={i} src={src} alt="" style={style} /> : <div key={i} className="stack-doc" style={style} />;
      })}
    </div>
  );
}
```
In `Body` replace the files line with `if (clip.kind === "files") return <FilesBody clip={clip} />;`.

- [ ] **Step 6: CSS** — append to `src/panel/panel.css` (before the entrance-animation comment block):

```css
.card-body {
  position: relative;
}

.ext-tag {
  position: absolute;
  left: 14px;
  bottom: 6px;
  padding: 2px 6px;
  border-radius: 6px;
  background: rgba(16, 17, 20, 0.65);
  color: #fff;
  font-size: 11px;
  font-weight: 600;
}

.stack {
  position: relative;
  height: 100%;
}

.stack > * {
  position: absolute;
  top: 10px;
  left: 18px;
  width: calc(100% - 36px);
  height: calc(100% - 14px);
  object-fit: cover;
  border-radius: 6px;
  border: 2px solid var(--card);
  box-shadow: 0 2px 8px rgba(16, 24, 40, 0.18);
}

.stack-doc {
  background: var(--divider);
}

/* Thumbnails must not start WebKit's own HTML drag. */
.panel img {
  -webkit-user-drag: none;
}
```
(`.card-body` already exists — add `position: relative;` to the existing rule instead of a duplicate selector.)

- [ ] **Step 7: Verify** — `yarn typecheck && yarn test` → PASS.

- [ ] **Step 8: Commit**

```bash
git add src/panel/fileThumb.ts src/panel/fileThumb.test.ts src/panel/Card.tsx src/panel/panel.css
git commit -m "feat: preview image files on file cards"
```

---

### Task 6: Panel and cards scale with the monitor

**Files:**
- Modify: `src-tauri/src/windows.rs` (constants, `panel_rect`, tests)
- Modify: `src/panel/panel.css` (`.card`, `.preview`)

**Interfaces:** Produces `fn panel_height(work_h_points: f64) -> f64`.

- [ ] **Step 1: Failing tests** in `windows.rs` tests

Add:
```rust
    #[test]
    fn panel_height_is_thirty_percent_of_the_work_area_clamped() {
        assert_eq!(panel_height(956.0), 300.0); // MacBook Air 13" → 287, floored to 300
        assert_eq!(panel_height(1117.0), 335.0); // MacBook Pro 16"
        assert_eq!(panel_height(1440.0), 432.0); // 27" QHD
        assert_eq!(panel_height(1692.0), 480.0); // 32" 4K scaled → 508, capped
    }
```
Update `panel_and_toast_sit_at_the_bottom_of_the_work_area`: the mac assertion becomes
```rust
        assert_eq!(panel_rect(work, 1.0), Rect { x: -2552.0, y: 738.0, w: 2544.0, h: 423.0 });
```
(1410 × 0.3 = 423; y = −241 + 1410 − 423 − 8). The Windows assertion (`h: 450.0`, `y: 578.0`) stays — 1040 px / 1.5 = 693 pt → 208 → clamped to 300 pt → 450 px.

- [ ] **Step 2: Run to see failure**

Run: `cargo test --manifest-path src-tauri/Cargo.toml --lib windows::`
Expected: compile error (`panel_height` missing).

- [ ] **Step 3: Implement**

Replace `const PANEL_HEIGHT: f64 = 300.0;` with:
```rust
const PANEL_RATIO: f64 = 0.30;
const PANEL_MIN: f64 = 300.0;
const PANEL_MAX: f64 = 480.0;
```
Add above `panel_rect`:
```rust
/// Panel height in design points: a share of the work area, clamped.
fn panel_height(work_h_points: f64) -> f64 {
    (work_h_points * PANEL_RATIO).round().clamp(PANEL_MIN, PANEL_MAX)
}
```
In `panel_rect`: `let (margin, h) = (PANEL_MARGIN * unit, panel_height(work.h / unit) * unit);`

- [ ] **Step 4: Run Rust tests** — `cargo test --manifest-path src-tauri/Cargo.toml --lib` → PASS.

- [ ] **Step 5: CSS** — in `panel.css`:

`.card`: replace `flex: 0 0 180px;` with
```css
  /* The window is exactly the panel's height; 72px is the toolbar, paddings and borders. */
  flex: 0 0 calc((100vh - 72px) * 0.95);
  position: relative;
```
`.preview`: replace `font-size: 12px;` with `font-size: clamp(12px, calc(100vh / 26), 15px);` and `-webkit-line-clamp: 8;` with `-webkit-line-clamp: 14;` (the card body's `overflow: hidden` cuts whatever doesn't fit).

- [ ] **Step 6: Commit**

```bash
git add src-tauri/src/windows.rs src/panel/panel.css
git commit -m "feat: size the panel and cards to the monitor"
```

---

### Task 7: Fix the macOS entrance flash

**Files:**
- Modify: `src-tauri/src/windows.rs` (`place_and_show`, new `set_panel_alpha`, `reveal_panel`)
- Modify: `src-tauri/src/lib.rs` (command + registration)
- Modify: `src/api.ts`, `src/panel/Panel.tsx`

**Interfaces:** Produces Tauri command `reveal_panel` / `api.revealPanel(): Promise<void>`.

Cause (for the implementer): a hidden WKWebView doesn't paint, so `show()` first displays the last painted frame — the panel in its *open* position — then the parked frame, then the slide. Fix: show at alpha 0, let the webview paint the parked frame, then alpha 1 and slide.

This is native window behaviour with no unit-testable logic; verification is manual (Step 5).

- [ ] **Step 1: Rust** — in `windows.rs`, add:

```rust
/// macOS only: window opacity, used to hide the stale frame a hidden webview shows on `show()`.
fn set_panel_alpha(panel: &WebviewWindow, alpha: f64) {
    #[cfg(target_os = "macos")]
    match panel.ns_window() {
        // SAFETY: Tauri hands out the live NSWindow; callers run on the main thread.
        Ok(ptr) => unsafe { (*(ptr as *const objc2_app_kit::NSWindow)).setAlphaValue(alpha) },
        Err(e) => log::warn!("couldn't set panel alpha: {e}"),
    }
    #[cfg(not(target_os = "macos"))]
    let _ = (panel, alpha);
}

/// Called by the panel once its parked frame has been painted.
pub fn reveal_panel(app: &AppHandle) {
    if let Some(panel) = app.get_webview_window(PANEL) {
        set_panel_alpha(&panel, 1.0);
    }
}
```
In `place_and_show`, insert `set_panel_alpha(panel, 0.0);` immediately before `panel.show()?;`.

- [ ] **Step 2: Command** — in `lib.rs`:
```rust
#[tauri::command]
fn reveal_panel(app: AppHandle) {
    windows::reveal_panel(&app);
}
```
and add `reveal_panel,` to `generate_handler![...]` after `hide_panel,`. (Sync commands run on the main thread, which AppKit requires.)

- [ ] **Step 3: Frontend** — `src/api.ts`: add `revealPanel: () => invoke<void>("reveal_panel"),` after `hidePanel`.

`Panel.tsx` — replace the `panel://opened` listener body:
```tsx
    const offOpened = listen("panel://opened", () => {
      inputRef.current?.focus();
      // The window is shown transparent; reveal it once the parked frame is on screen, then slide.
      requestAnimationFrame(() =>
        requestAnimationFrame(() => void api.revealPanel().finally(() => setOpen(true))),
      );
    });
```

- [ ] **Step 4: Checks** — `cargo test --manifest-path src-tauri/Cargo.toml --lib && yarn typecheck && yarn test` → PASS.

- [ ] **Step 5: Manual check (macOS)** — `yarn tauri dev`; open/close the panel with the shortcut ~10 times, including right after copying something. Expected: never a frame of the panel at its final position before the slide; slide-up looks the same as before.

- [ ] **Step 6: Commit**

```bash
git add src-tauri/src/windows.rs src-tauri/src/lib.rs src/api.ts src/panel/Panel.tsx
git commit -m "fix: hide the stale frame when the panel opens on macOS"
```

---

### Task 8: Drag file cards out (always copy)

**Files:**
- Modify: `src-tauri/Cargo.toml` (`drag = "2.1"`)
- Modify: `src-tauri/src/lib.rs` (`AppState.dragging`, focus handler, `start_drag` command)
- Modify: `src-tauri/src/windows.rs` (`confirm_copy` extraction, `start_drag`, `finish_drag`, `drag_image`, test)
- Modify: `src/api.ts`, `src/panel/Card.tsx`, `src/panel/Panel.tsx`, `src/panel/panel.css`

**Interfaces:**
- Consumes: `Store::preview_png` (Task 3), `ClipContent::Files`.
- Produces: `AppState.dragging: AtomicBool`; command `start_drag(id)` / `api.startDrag(id: number): Promise<void>`; event `panel://drag-cancelled`; `Card` prop `onDragOut?: () => void`.

- [ ] **Step 1: Dependency** — `src-tauri/Cargo.toml` `[dependencies]`: add `drag = "2.1"`. Run `cargo build --manifest-path src-tauri/Cargo.toml` → builds.

- [ ] **Step 2: Failing test for the drag image** (in `windows.rs` tests)

```rust
    #[test]
    fn drag_image_shrinks_previews_and_falls_back_to_the_app_icon() {
        use base64::{Engine, engine::general_purpose::STANDARD};
        let pixel = STANDARD
            .decode("iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAYAAAAfFcSJAAAADUlEQVR42mNkYPhfDwAChwGA60e6kgAAAABJRU5ErkJggg==")
            .unwrap();
        assert!(drag_image(Some(pixel)).starts_with(b"\x89PNG"));
        assert_eq!(drag_image(None), DRAG_ICON);
        assert_eq!(drag_image(Some(vec![1, 2, 3])), DRAG_ICON);
    }
```
Run: `cargo test --manifest-path src-tauri/Cargo.toml --lib drag_image` → FAIL (not defined).

- [ ] **Step 3: `AppState.dragging`** — in `lib.rs`:
- `use std::sync::atomic::{AtomicBool, Ordering};`
- field in `AppState`: 
  ```rust
      /// A file drag out of the panel is in progress; losing focus must not hide it.
      pub dragging: AtomicBool,
  ```
- `AppState::new`: add `dragging: AtomicBool::new(false),`.
- Focus handler arm becomes:
  ```rust
              (windows::PANEL, WindowEvent::Focused(false)) => {
                  if !window.app_handle().state::<AppState>().dragging.load(Ordering::SeqCst) {
                      windows::hide_panel(window.app_handle(), false)
                  }
              }
  ```

- [ ] **Step 4: `windows.rs` — extract the post-copy confirmation**

Add:
```rust
/// Bumps the clip, plays the copy sound and confirms with a toast.
fn confirm_copy(app: &AppHandle, id: i64, content: &ClipContent) {
    let state = app.state::<AppState>();
    let sound_on = {
        let store = state.store.lock().unwrap();
        let _ = store.touch(id, now_ms());
        crate::settings::copy_sound_enabled(&store)
    };
    if sound_on {
        source_app::play_copy_sound();
    }
    let _ = app.emit("clips://changed", ());
    show_toast(app, ToastPayload::copied(content));
}
```
and make `copy_clip`'s `Ok(content)` arm:
```rust
        Ok(content) => {
            confirm_copy(app, id, &content);
            Ok(())
        }
```

- [ ] **Step 5: `windows.rs` — drag**

Imports: `use std::path::{Path, PathBuf};` and `use drag::{DragItem, DragMode, DragResult};`.

```rust
const DRAG_ICON: &[u8] = include_bytes!("../icons/128x128.png");
const DRAG_IMAGE_SIZE: u32 = 96;

/// A small PNG to show under the cursor while dragging.
fn drag_image(preview: Option<Vec<u8>>) -> Vec<u8> {
    use clipboard_rs::{RustImageData, common::RustImage};
    preview
        .and_then(|png| {
            let image = RustImageData::from_bytes(&png).ok()?;
            Some(image.thumbnail(DRAG_IMAGE_SIZE, DRAG_IMAGE_SIZE).ok()?.to_png().ok()?.get_bytes().to_vec())
        })
        .unwrap_or_else(|| DRAG_ICON.to_vec())
}

/// Starts dragging a files clip out of the panel. Drops always copy.
pub fn start_drag(app: &AppHandle, id: i64) -> Result<(), String> {
    let state = app.state::<AppState>();
    let (paths, preview) = {
        let store = state.store.lock().unwrap();
        let paths = match store.content(id).map_err(|e| e.to_string())? {
            Some(ClipContent::Files(paths)) => paths,
            _ => return Err("not a file clip".into()),
        };
        (paths, store.preview_png(id).map_err(|e| e.to_string())?)
    };
    if paths.iter().any(|p| !Path::new(p).exists()) {
        return Err("a copied file no longer exists".into());
    }
    let panel = app.get_webview_window(PANEL).ok_or("panel window is missing")?;
    state.dragging.store(true, Ordering::SeqCst);
    // Let drops land on whatever is behind the (slid-away) panel.
    let _ = panel.set_ignore_cursor_events(true);
    let items = DragItem::Files(paths.iter().map(PathBuf::from).collect());
    let content = ClipContent::Files(paths);
    let handle = app.clone();
    let started = drag::start_drag(
        &panel,
        items,
        drag::Image::Raw(drag_image(preview)),
        move |result, _cursor| finish_drag(&handle, id, &content, result),
        drag::Options { mode: DragMode::Copy, ..Default::default() },
    );
    if let Err(e) = started {
        end_drag(app);
        return Err(e.to_string());
    }
    Ok(())
}

fn end_drag(app: &AppHandle) -> Option<WebviewWindow> {
    app.state::<AppState>().dragging.store(false, Ordering::SeqCst);
    let panel = app.get_webview_window(PANEL)?;
    let _ = panel.set_ignore_cursor_events(false);
    Some(panel)
}

fn finish_drag(app: &AppHandle, id: i64, content: &ClipContent, result: DragResult) {
    let panel = end_drag(app);
    match result {
        DragResult::Dropped => {
            hide_panel(app, true);
            confirm_copy(app, id, content);
        }
        DragResult::Cancel => {
            if let Some(panel) = panel {
                let _ = panel.set_focus();
            }
            let _ = app.emit_to(PANEL, "panel://drag-cancelled", ());
        }
    }
}
```

`lib.rs` command (sync → main thread, which `drag` requires on macOS):
```rust
#[tauri::command]
fn start_drag(app: AppHandle, id: i64) -> Result<(), String> {
    windows::start_drag(&app, id)
}
```
Register `start_drag,` in `generate_handler!` after `copy_clip,`.

- [ ] **Step 6: Rust tests** — `cargo test --manifest-path src-tauri/Cargo.toml --lib` → PASS.

- [ ] **Step 7: Frontend**

`src/api.ts`: `startDrag: (id: number) => invoke<void>("start_drag", { id }),` after `copyClip`.

`Card.tsx` — props gain `onDragOut?: () => void;`; import `useRef` from react; inside `Card`:
```tsx
const DRAG_THRESHOLD = 5;
```
(module level), and in the component:
```tsx
  const press = useRef<{ x: number; y: number } | null>(null);
```
on the root `<div>` add:
```tsx
      onMouseDown={(e) => {
        press.current = onDragOut ? { x: e.clientX, y: e.clientY } : null;
      }}
      onMouseMove={(e) => {
        const p = press.current;
        if (!p || !onDragOut) return;
        if ((e.buttons & 1) === 0) {
          press.current = null;
          return;
        }
        if (Math.hypot(e.clientX - p.x, e.clientY - p.y) < DRAG_THRESHOLD) return;
        press.current = null;
        onDragOut();
      }}
      onMouseUp={() => {
        press.current = null;
      }}
```

`Panel.tsx`:
- state: `const [dragging, setDragging] = useState(false);`
- in the listener effect add
  ```tsx
      const offDragCancelled = listen("panel://drag-cancelled", () => setDragging(false));
  ```
  and its cleanup `void offDragCancelled.then((off) => off());`; in the `panel://closed` handler add `setDragging(false);`.
- helper:
  ```tsx
  const dragOut = (clip: Clip) => {
    setDragging(true);
    void api.startDrag(clip.id).catch(() => setDragging(false));
  };
  ```
- root className: `className={["panel", open && "open", dragging && "dragging"].filter(Boolean).join(" ")}`
- on `<Card>`: `onDragOut={clip.kind === "files" && !clip.missing ? () => dragOut(clip) : undefined}`

`panel.css` — after the `.panel.open` rule (must come later to win):
```css
/* Dragging a file out: the panel gets out of the way so the whole screen is a drop target. */
.panel.open.dragging {
  transform: translateY(calc(100% + 16px));
  transition: transform 200ms ease-in;
}
```
and inside the existing `prefers-reduced-motion` block add `.panel.open.dragging { transition: none; }`.

- [ ] **Step 8: Checks** — `yarn typecheck && yarn test && cargo test --manifest-path src-tauri/Cargo.toml --lib` → PASS.

- [ ] **Step 9: Manual check (macOS)** — `yarn tauri dev`, copy 1 and 3 files in Finder, open the panel:
  - Drag a file card a few px → panel slides down, drag image follows cursor with green +.
  - Drop on Desktop / a Finder window on the **same** volume → a copy appears, original stays; panel closed; "복사됨 · 파일 N개" toast; card moved to front.
  - Start a drag, press Esc / drop back on nothing → panel slides back up, keyboard works.
  - Text/link/screenshot cards and "파일 없음" cards don't drag.
  If `drag::start_drag` errors with no current event, log the error and report — don't work around it silently.

- [ ] **Step 10: Commit**

```bash
git add src-tauri/Cargo.toml src-tauri/Cargo.lock src-tauri/src/lib.rs src-tauri/src/windows.rs src/api.ts src/panel/Card.tsx src/panel/Panel.tsx src/panel/panel.css
git commit -m "feat: drag file cards out of the panel to copy them"
```

---

### Task 9: Delete confirmation overlay

**Files:**
- Modify: `src/panel/keys.ts`, `src/panel/keys.test.ts`
- Modify: `src/panel/Panel.tsx`, `src/panel/Card.tsx`, `src/panel/panel.css`

**Interfaces:**
- Consumes: `settings.confirmDelete`, `t.deleteConfirm`, `t.cancel`, `t.delete`, `t.deleteHint` (Task 2).
- Produces: `KeyInput.confirming: boolean`; actions `{ type: "confirmDelete" }`, `{ type: "cancelDelete" }`; `Card` props `confirming`, `onConfirmDelete`, `onCancelDelete`.

- [ ] **Step 1: Failing tests** — in `keys.test.ts` change the helper to
```ts
const press = (key: string, extra: Partial<KeyInput> = {}) =>
  panelKeyAction({ key, shiftKey: false, isComposing: false, queryEmpty: true, repeat: false, confirming: false, ...extra });
```
and add:
```ts
test("while confirming a delete, enter confirms and other keys cancel", () => {
  assert.deepEqual(press("Enter", { confirming: true }), { type: "confirmDelete" });
  for (const key of ["Escape", "ArrowRight", "Backspace", "Delete", "Tab", "a"]) {
    assert.deepEqual(press(key, { confirming: true }), { type: "cancelDelete" }, key);
  }
});

test("modifiers and the held delete key don't cancel a confirmation", () => {
  for (const key of ["Shift", "Meta", "Control", "Alt"]) {
    assert.equal(press(key, { confirming: true }), null, key);
  }
  assert.equal(press("Delete", { confirming: true, repeat: true }), null);
  assert.equal(press("Backspace", { confirming: true, repeat: true }), null);
});
```
Run: `yarn test` → FAIL (type error / wrong actions).

- [ ] **Step 2: Implement `keys.ts`**

Add to the `KeyAction` union: `| { type: "confirmDelete" } | { type: "cancelDelete" }`. Add to `KeyInput`:
```ts
  /** A delete confirmation is showing on the selected card. */
  confirming: boolean;
```
Add `const MODIFIERS = ["Shift", "Meta", "Control", "Alt"];` and, right after the `isComposing` check in `panelKeyAction`:
```ts
  if (e.confirming) {
    // Auto-repeat from the Delete that opened the confirmation must not dismiss it.
    if (e.repeat || MODIFIERS.includes(e.key)) return null;
    return e.key === "Enter" ? { type: "confirmDelete" } : { type: "cancelDelete" };
  }
```
Run: `yarn test` → PASS.

- [ ] **Step 3: Panel wiring** (`Panel.tsx`)

- `const { t, settings } = usePrefs();`
- `const [confirmingId, setConfirmingId] = useState<number | null>(null);`
- in `reload`, inside `if (!keepSelection) ...` turn it into a block that also calls `setConfirmingId(null);`:
  ```tsx
      if (!keepSelection) {
        setConfirmingId(null);
        rowRef.current?.scrollTo({ left: 0 });
      }
  ```
- `panel://closed` handler: add `setConfirmingId(null);`
- `panelKeyAction({...})` input: add `confirming: confirmingId !== null,`
- switch cases:
  ```tsx
      case "delete": {
        const clip = clips[selected];
        if (settings.confirmDelete === "on") setConfirmingId(clip?.id ?? null);
        else remove(clip);
        break;
      }
      case "confirmDelete":
        remove(clips.find((c) => c.id === confirmingId));
        setConfirmingId(null);
        break;
      case "cancelDelete":
        setConfirmingId(null);
        break;
  ```
- `<Card>` props:
  ```tsx
              confirming={clip.id === confirmingId}
              onConfirmDelete={() => {
                remove(clip);
                setConfirmingId(null);
              }}
              onCancelDelete={() => setConfirmingId(null)}
  ```
  and change `onSelect={() => setSelected(i)}` to `onSelect={() => { setSelected(i); setConfirmingId(null); }}`.

- [ ] **Step 4: Card overlay** (`Card.tsx`)

Props add `confirming: boolean; onConfirmDelete: () => void; onCancelDelete: () => void;`. Root className:
```tsx
      className={["card", selected && "selected", confirming && "confirming"].filter(Boolean).join(" ")}
```
Last child of the root `<div>`:
```tsx
      {confirming && (
        <div className="confirm" onClick={(e) => e.stopPropagation()} onDoubleClick={(e) => e.stopPropagation()}>
          <p>{t.deleteConfirm}</p>
          <div className="confirm-btns">
            <button className="confirm-btn" onClick={onCancelDelete}>
              {t.cancel}
            </button>
            <button className="confirm-btn danger" onClick={onConfirmDelete}>
              {t.delete}
            </button>
          </div>
          <span className="confirm-hint">{t.deleteHint}</span>
        </div>
      )}
```

- [ ] **Step 5: CSS** — append to `panel.css` (before the entrance block):
```css
.card.confirming {
  border: 2px solid var(--danger);
  box-shadow: 0 0 0 4px color-mix(in srgb, var(--danger) 14%, transparent);
}

.confirm {
  position: absolute;
  inset: 0;
  display: flex;
  flex-direction: column;
  align-items: center;
  justify-content: center;
  gap: 12px;
  padding: 12px;
  background: color-mix(in srgb, var(--card) 94%, transparent);
  text-align: center;
}

.confirm p {
  font-size: 14px;
  font-weight: 600;
}

.confirm-btns {
  display: flex;
  gap: 8px;
}

.confirm-btn {
  padding: 8px 14px;
  border-radius: var(--radius);
  background: var(--subtle);
  font-size: 12px;
  font-weight: 600;
}

.confirm-btn.danger {
  background: var(--danger);
  color: #fff;
}

.confirm-hint {
  color: var(--muted);
  font-size: 11px;
}
```

- [ ] **Step 6: Checks** — `yarn typecheck && yarn test` → PASS.

- [ ] **Step 7: Manual check** — setting off: Delete removes immediately (unchanged). Setting on: Delete shows overlay on the selected card; Enter deletes; Esc cancels and the panel stays open; arrow key cancels; clicking [취소]/[삭제] works; closing the panel clears it.

- [ ] **Step 8: Commit**

```bash
git add src/panel/keys.ts src/panel/keys.test.ts src/panel/Panel.tsx src/panel/Card.tsx src/panel/panel.css
git commit -m "feat: optional confirmation before deleting a clip"
```

---

### Task 10: Tray icon — left click opens the menu

**Files:**
- Modify: `src-tauri/src/tray.rs` (`create`, imports)

No unit-testable logic; verified manually.

- [ ] **Step 1: Implement** — in `create`:
  - replace
    ```rust
        // Left click is reserved for double-click → panel; right click opens the menu.
        .show_menu_on_left_click(false)
    ```
    with `.show_menu_on_left_click(true)`
  - delete the whole `.on_tray_icon_event(|tray, event| { ... })` call.
  - imports: `use tauri::tray::TrayIconBuilder;` (drop `TrayIconEvent`).

- [ ] **Step 2: Checks** — `cargo test --manifest-path src-tauri/Cargo.toml --lib` → PASS, no unused-import warnings from `tray.rs`.

- [ ] **Step 3: Manual check** — `yarn tauri dev`: left click on the menu-bar icon shows 열기 / 설정 / 업데이트 확인 / 종료; right click still shows it; 열기 opens the panel.

- [ ] **Step 4: Commit**

```bash
git add src-tauri/src/tray.rs
git commit -m "feat: open the tray menu on left click"
```

---

### Task 11: Final verification

- [ ] **Step 1: Full test suite**

Run:
```bash
cargo test --manifest-path src-tauri/Cargo.toml --lib
yarn test
yarn typecheck
yarn build
```
Expected: all PASS / build succeeds.

- [ ] **Step 2: End-to-end manual pass (macOS, `yarn tauri dev`)** — the spec's manual list:
  - No flash on open (Task 7).
  - Panel height on the built-in display vs. an external one (≈300–335 on a laptop, ≈432 on 27" QHD); text and thumbnails scale.
  - Copy one PNG in Finder → card shows the image + `PNG` tag. Copy 4 files incl. 2 images → tilted stack with document tile; badge "파일 4개".
  - File drag-out copy, cancel (Task 8).
  - Delete confirmation on/off (Task 9).
  - Tray left-click menu (Task 10).
  - History keeps more than 1000 entries (optional: trust Task 1 test).

- [ ] **Step 3: Report** results, including anything that failed manual checks, before calling the work done.
