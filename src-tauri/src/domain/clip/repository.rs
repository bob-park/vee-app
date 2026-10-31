//! SQL for the `clips`, `clip_thumbs` and `clip_formats` tables, plus the image files clips own.

use super::model::{ClipContent, ClipDto, Kind, MAX_PINNED, NewClip, STACK_LAYERS, StatsDto, classify_text};
use crate::shared::{db::escape_like, error::AppResult, png::data_url};
use rusqlite::{Connection, OptionalExtension, params, params_from_iter, types::Value};
use sha2::{Digest, Sha256};
use std::path::{Path, PathBuf};

const PREVIEW_CHARS: i64 = 500;

fn sha256_hex(parts: &[&[u8]]) -> String {
    let mut hasher = Sha256::new();
    for part in parts {
        hasher.update(part);
    }
    hasher.finalize().iter().map(|b| format!("{b:02x}")).collect()
}

/// Quote the whole query as one FTS5 phrase so operators like AND/NOT/* are literal.
fn fts_phrase(q: &str) -> String {
    format!("\"{}\"", q.replace('"', "\"\""))
}

/// Inserts a new clip, or moves an identical one to the front. Returns its id.
/// Image clips write their PNG into `images`.
pub fn upsert(conn: &Connection, images: &Path, clip: NewClip, app_id: Option<i64>, now: i64) -> AppResult<i64> {
    let hash = match &clip {
        NewClip::Text(t) => sha256_hex(&[b"text\0", t.as_bytes()]),
        NewClip::Image { png, .. } => sha256_hex(&[b"image\0", png]),
        NewClip::Files { paths, .. } => sha256_hex(&[b"files\0", paths.join("\n").as_bytes()]),
    };
    let bumped: Option<i64> = conn
        .query_row("UPDATE clips SET last_used_at = ?1, app_id = ?2 WHERE hash = ?3 RETURNING id", params![now, app_id, hash], |r| r.get(0))
        .optional()?;
    if let Some(id) = bumped {
        return Ok(id);
    }
    let (kind, text, image_path, thumb, meta, file_thumbs) = match clip {
        NewClip::Text(t) => (classify_text(&t), Some(t), None, None, None, Vec::new()),
        NewClip::Image { png, thumb_png, width, height } => {
            let path = images.join(format!("{hash}.png"));
            std::fs::write(&path, png)?;
            let path = path.to_string_lossy().into_owned();
            (Kind::Image, None, Some(path), Some(thumb_png), Some(format!("{width}×{height}")), Vec::new())
        }
        NewClip::Files { paths, thumbs } => {
            let count = paths.len().to_string();
            (Kind::Files, Some(paths.join("\n")), None, None, Some(count), thumbs)
        }
    };
    // Image and files clips start with `ocr_text` NULL (pending); text has nothing to recognize.
    let ocr_text = matches!(kind, Kind::Text | Kind::Link).then_some("");
    conn.execute(
        "INSERT INTO clips (kind, hash, text, image_path, thumb_png, meta, app_id, created_at, last_used_at, ocr_text)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?8, ?9)",
        params![kind.as_str(), hash, text, image_path, thumb, meta, app_id, now, ocr_text],
    )?;
    let id = conn.last_insert_rowid();
    for (idx, png) in file_thumbs {
        conn.execute("INSERT INTO clip_thumbs (clip_id, idx, png) VALUES (?1, ?2, ?3)", params![id, idx as i64, png])?;
    }
    Ok(id)
}

/// Replaces a clip's raw pasteboard formats; the latest copy of identical content wins.
pub fn set_formats(conn: &Connection, id: i64, formats: &[(String, Vec<u8>)]) -> AppResult<()> {
    let tx = conn.unchecked_transaction()?;
    tx.execute("DELETE FROM clip_formats WHERE clip_id = ?1", [id])?;
    for (format, data) in formats {
        tx.execute("INSERT OR REPLACE INTO clip_formats (clip_id, format, data) VALUES (?1, ?2, ?3)", params![id, format, data])?;
    }
    tx.commit()?;
    Ok(())
}

pub fn formats(conn: &Connection, id: i64) -> AppResult<Vec<(String, Vec<u8>)>> {
    let mut stmt = conn.prepare("SELECT format, data FROM clip_formats WHERE clip_id = ?1 ORDER BY rowid")?;
    let rows = stmt.query_map([id], |r| Ok((r.get(0)?, r.get(1)?)))?;
    Ok(rows.collect::<rusqlite::Result<_>>()?)
}

const CLIP_COLUMNS: &str = "SELECT c.id, c.kind, substr(c.text, 1, ?), COALESCE(length(c.text), 0), c.thumb_png, c.meta,
                    a.name, a.icon_png, c.last_used_at,
                    CASE c.kind WHEN 'files' THEN c.text WHEN 'image' THEN c.image_path END,
                    c.pinned
             FROM clips c LEFT JOIN apps a ON a.id = c.app_id";

/// The history row: unpinned clips only, newest first. Pins live in `list_pinned`.
pub fn list(conn: &Connection, query: &str, kind: Option<Kind>, app_id: Option<i64>, offset: i64, limit: i64) -> AppResult<Vec<ClipDto>> {
    let mut sql = format!("{CLIP_COLUMNS} WHERE c.pinned = 0");
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
            sql.push_str(" AND (c.text LIKE ? ESCAPE '\\' OR c.ocr_text LIKE ? ESCAPE '\\')");
            let pattern = Value::Text(format!("%{}%", escape_like(q)));
            args.extend([pattern.clone(), pattern]);
        }
        _ => {
            sql.push_str(" AND c.id IN (SELECT rowid FROM clips_fts WHERE clips_fts MATCH ?)");
            args.push(Value::Text(fts_phrase(q)));
        }
    }
    sql.push_str(" ORDER BY c.last_used_at DESC, c.id DESC LIMIT ? OFFSET ?");
    args.push(Value::Integer(limit));
    args.push(Value::Integer(offset));
    query_clips(conn, &sql, args)
}

/// Pinned clips in the order they were pinned, regardless of any search or filter.
pub fn list_pinned(conn: &Connection) -> AppResult<Vec<ClipDto>> {
    let sql = format!("{CLIP_COLUMNS} WHERE c.pinned = 1 ORDER BY c.pinned_at ASC, c.id ASC LIMIT ?");
    query_clips(conn, &sql, vec![Value::Integer(PREVIEW_CHARS), Value::Integer(MAX_PINNED)])
}

fn query_clips(conn: &Connection, sql: &str, args: Vec<Value>) -> AppResult<Vec<ClipDto>> {
    let mut stmt = conn.prepare(sql)?;
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
        clip.stack = stack(conn, clip.id, count)?;
    }
    Ok(clips)
}

fn stack(conn: &Connection, id: i64, files: usize) -> AppResult<Vec<Option<String>>> {
    let mut stmt = conn.prepare("SELECT idx, png FROM clip_thumbs WHERE clip_id = ?1")?;
    let thumbs: std::collections::HashMap<i64, Vec<u8>> =
        stmt.query_map([id], |r| Ok((r.get(0)?, r.get(1)?)))?.collect::<rusqlite::Result<_>>()?;
    if thumbs.is_empty() {
        return Ok(Vec::new());
    }
    Ok((0..files.min(STACK_LAYERS) as i64).map(|i| thumbs.get(&i).map(|png| data_url(png))).collect())
}

/// Image for dragging a clip out: its first file thumbnail or image thumbnail, else its source app's icon.
pub fn preview_png(conn: &Connection, id: i64) -> AppResult<Option<Vec<u8>>> {
    let thumb = conn.query_row("SELECT png FROM clip_thumbs WHERE clip_id = ?1 ORDER BY idx LIMIT 1", [id], |r| r.get(0)).optional()?;
    if thumb.is_some() {
        return Ok(thumb);
    }
    Ok(conn
        .query_row("SELECT COALESCE(c.thumb_png, a.icon_png) FROM clips c LEFT JOIN apps a ON a.id = c.app_id WHERE c.id = ?1", [id], |r| {
            r.get::<_, Option<Vec<u8>>>(0)
        })
        .optional()?
        .flatten())
}

pub fn content(conn: &Connection, id: i64) -> AppResult<Option<ClipContent>> {
    let row = conn
        .query_row("SELECT kind, text, image_path FROM clips WHERE id = ?1", [id], |r| {
            Ok((r.get::<_, String>(0)?, r.get::<_, Option<String>>(1)?, r.get::<_, Option<String>>(2)?))
        })
        .optional()?;
    Ok(row.map(|(kind, text, image_path)| match Kind::parse(&kind) {
        Some(Kind::Image) => ClipContent::Image(PathBuf::from(image_path.unwrap_or_default())),
        Some(Kind::Files) => ClipContent::Files(text.unwrap_or_default().lines().map(String::from).collect()),
        _ => ClipContent::Text(text.unwrap_or_default()),
    }))
}

/// The newest image or files clip still waiting for OCR (`ocr_text IS NULL`), as `(id, hash, content)`.
pub fn next_ocr_pending(conn: &Connection) -> AppResult<Option<(i64, String, ClipContent)>> {
    let row = conn
        .query_row(
            "SELECT id, hash, kind, text, image_path FROM clips
             WHERE ocr_text IS NULL AND kind IN ('image','files')
             ORDER BY last_used_at DESC, id DESC LIMIT 1",
            [],
            |r| {
                Ok((
                    r.get::<_, i64>(0)?,
                    r.get::<_, String>(1)?,
                    r.get::<_, String>(2)?,
                    r.get::<_, Option<String>>(3)?,
                    r.get::<_, Option<String>>(4)?,
                ))
            },
        )
        .optional()?;
    Ok(row.map(|(id, hash, kind, text, image_path)| {
        let content = if kind == "image" {
            ClipContent::Image(PathBuf::from(image_path.unwrap_or_default()))
        } else {
            ClipContent::Files(text.unwrap_or_default().lines().map(String::from).collect())
        };
        (id, hash, content)
    }))
}

/// Stores a clip's OCR result. A clip deleted in the meantime is a silent no-op; `hash` keeps
/// the result off a new clip that reused the id (no AUTOINCREMENT).
pub fn set_ocr_text(conn: &Connection, id: i64, hash: &str, text: &str) -> AppResult<()> {
    conn.execute("UPDATE clips SET ocr_text = ?1 WHERE id = ?2 AND hash = ?3", params![text, id, hash])?;
    Ok(())
}

pub fn touch(conn: &Connection, id: i64, now: i64) -> AppResult<()> {
    conn.execute("UPDATE clips SET last_used_at = ?1 WHERE id = ?2", params![now, id])?;
    Ok(())
}

/// Pins or unpins a clip. Returns false, changing nothing, when pinning would go past `MAX_PINNED`.
pub fn set_pinned(conn: &Connection, id: i64, pinned: bool, now: i64) -> AppResult<bool> {
    if !pinned {
        conn.execute("UPDATE clips SET pinned = 0, pinned_at = NULL WHERE id = ?1", [id])?;
        return Ok(true);
    }
    let (already, count): (bool, i64) =
        conn.query_row("SELECT COALESCE(MAX(id = ?1), 0), COUNT(*) FROM clips WHERE pinned = 1", [id], |r| {
            Ok((r.get::<_, i64>(0)? != 0, r.get(1)?))
        })?;
    if already {
        return Ok(true);
    }
    if count >= MAX_PINNED {
        return Ok(false);
    }
    conn.execute("UPDATE clips SET pinned = 1, pinned_at = ?1 WHERE id = ?2", params![now, id])?;
    Ok(true)
}

pub fn delete(conn: &Connection, id: i64) -> AppResult<()> {
    let image_path: Option<Option<String>> = conn.query_row("SELECT image_path FROM clips WHERE id = ?1", [id], |r| r.get(0)).optional()?;
    conn.execute("DELETE FROM clips WHERE id = ?1", [id])?;
    if let Some(Some(path)) = image_path {
        let _ = std::fs::remove_file(path);
    }
    Ok(())
}

/// Deletes unpinned clips matching `cond`, then their image files. Returns how many went.
fn delete_unpinned(conn: &Connection, cond: &str, args: &[Value]) -> AppResult<usize> {
    let tx = conn.unchecked_transaction()?;
    let paths: Vec<String> = {
        let mut stmt = tx.prepare(&format!("SELECT image_path FROM clips WHERE pinned = 0 AND ({cond}) AND image_path IS NOT NULL"))?;
        stmt.query_map(params_from_iter(args.iter()), |r| r.get(0))?.collect::<rusqlite::Result<_>>()?
    };
    let n = tx.execute(&format!("DELETE FROM clips WHERE pinned = 0 AND ({cond})"), params_from_iter(args.iter()))?;
    tx.commit()?;
    for path in paths {
        let _ = std::fs::remove_file(path);
    }
    Ok(n)
}

fn count_unpinned(conn: &Connection, cond: &str, args: &[Value]) -> AppResult<i64> {
    Ok(conn.query_row(&format!("SELECT COUNT(*) FROM clips WHERE pinned = 0 AND ({cond})"), params_from_iter(args.iter()), |r| r.get(0))?)
}

/// Deletes an app's unpinned clips (when it's excluded); returns how many.
pub fn delete_unpinned_from_app(conn: &Connection, app_id: i64) -> AppResult<usize> {
    delete_unpinned(conn, "app_id = ?", &[Value::Integer(app_id)])
}

/// Unpinned clips from this app, i.e. what excluding it would delete.
pub fn unpinned_count_from_app(conn: &Connection, app_id: i64) -> AppResult<i64> {
    count_unpinned(conn, "app_id = ?", &[Value::Integer(app_id)])
}

/// Unpinned clips last used before `cutoff` (ms), i.e. what `prune` would remove.
pub fn prunable_count(conn: &Connection, cutoff: i64) -> AppResult<i64> {
    count_unpinned(conn, "last_used_at < ?", &[Value::Integer(cutoff)])
}

pub fn prune(conn: &Connection, cutoff: i64) -> AppResult<usize> {
    delete_unpinned(conn, "last_used_at < ?", &[Value::Integer(cutoff)])
}

/// Deletes every clip except pinned ones.
pub fn clear(conn: &Connection) -> AppResult<usize> {
    delete_unpinned(conn, "1 = 1", &[])
}

pub fn stats(conn: &Connection, images: &Path) -> AppResult<StatsDto> {
    let (count, pinned) = conn.query_row("SELECT COUNT(*), COALESCE(SUM(pinned), 0) FROM clips", [], |r| Ok((r.get(0)?, r.get(1)?)))?;
    let mut image_bytes = 0;
    for entry in std::fs::read_dir(images)? {
        image_bytes += entry?.metadata()?.len();
    }
    Ok(StatsDto { count, pinned, image_bytes })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::source_app;
    use crate::shared::db;

    fn db() -> (Connection, tempfile::TempDir) {
        (db::open_in_memory().unwrap(), tempfile::tempdir().unwrap())
    }

    fn text(s: &str) -> NewClip {
        NewClip::Text(s.to_string())
    }

    fn files(paths: &[&str]) -> NewClip {
        NewClip::Files { paths: paths.iter().map(|p| p.to_string()).collect(), thumbs: vec![] }
    }

    fn texts(clips: &[ClipDto]) -> Vec<String> {
        clips.iter().map(|c| c.text_preview.clone().unwrap_or_default()).collect()
    }

    fn image(byte: u8) -> NewClip {
        NewClip::Image { png: vec![byte; 8], thumb_png: vec![9], width: 10, height: 20 }
    }

    fn hash_of(c: &Connection, id: i64) -> String {
        c.query_row("SELECT hash FROM clips WHERE id = ?1", [id], |r| r.get(0)).unwrap()
    }

    fn image_path(c: &Connection, id: i64) -> PathBuf {
        match content(c, id).unwrap() {
            Some(ClipContent::Image(p)) => p,
            other => panic!("expected image, got {other:?}"),
        }
    }

    #[test]
    fn same_text_bumps_instead_of_duplicating() {
        let (c, d) = db();
        let i = d.path();
        upsert(&c, i, text("hello"), None, 1).unwrap();
        upsert(&c, i, text("world"), None, 2).unwrap();
        upsert(&c, i, text("hello"), None, 3).unwrap();
        let all = list(&c, "", None, None, 0, 50).unwrap();
        assert_eq!(texts(&all), vec!["hello", "world"]);
        assert_eq!(all[0].last_used_at, 3);
    }

    #[test]
    fn history_is_not_trimmed() {
        let (c, d) = db();
        for i in 0..1100 {
            upsert(&c, d.path(), text(&format!("clip {i}")), None, i).unwrap();
        }
        assert_eq!(list(&c, "", None, None, 0, 200).unwrap().len(), 200);
        assert_eq!(list(&c, "", None, None, 1000, 200).unwrap().len(), 100);
    }

    #[test]
    fn searches_korean_with_trigram_and_short_queries_with_like() {
        let (c, d) = db();
        let i = d.path();
        upsert(&c, i, text("클립보드 히스토리 앱"), None, 1).unwrap();
        upsert(&c, i, text("cargo tauri dev"), None, 2).unwrap();
        upsert(&c, i, text("Vee"), None, 3).unwrap();
        assert_eq!(texts(&list(&c, "히스토", None, None, 0, 50).unwrap()), vec!["클립보드 히스토리 앱"]);
        assert_eq!(texts(&list(&c, "보드", None, None, 0, 50).unwrap()), vec!["클립보드 히스토리 앱"]);
        assert_eq!(texts(&list(&c, "TAURI", None, None, 0, 50).unwrap()), vec!["cargo tauri dev"]);
        assert_eq!(texts(&list(&c, "ve", None, None, 0, 50).unwrap()), vec!["Vee"]);
    }

    #[test]
    fn search_treats_special_characters_literally() {
        let (c, d) = db();
        let i = d.path();
        upsert(&c, i, text("100% done"), None, 1).unwrap();
        upsert(&c, i, text("a_b"), None, 2).unwrap();
        upsert(&c, i, text("say \"hi\" AND bye*"), None, 3).unwrap();
        upsert(&c, i, text("plain"), None, 4).unwrap();
        assert_eq!(texts(&list(&c, "%", None, None, 0, 50).unwrap()), vec!["100% done"]);
        assert_eq!(texts(&list(&c, "_", None, None, 0, 50).unwrap()), vec!["a_b"]);
        assert_eq!(texts(&list(&c, "\"hi\" AND", None, None, 0, 50).unwrap()), vec!["say \"hi\" AND bye*"]);
        assert_eq!(texts(&list(&c, "bye*", None, None, 0, 50).unwrap()), vec!["say \"hi\" AND bye*"]);
        assert!(list(&c, "NOT", None, None, 0, 50).unwrap().is_empty());
    }

    #[test]
    fn filters_by_kind() {
        let (c, d) = db();
        let i = d.path();
        upsert(&c, i, text("hello"), None, 1).unwrap();
        upsert(&c, i, text("https://example.com"), None, 2).unwrap();
        upsert(&c, i, files(&["/tmp/x"]), None, 3).unwrap();
        let links = list(&c, "", Some(Kind::Link), None, 0, 50).unwrap();
        assert_eq!(links.len(), 1);
        assert_eq!(links[0].kind, Kind::Link);
        assert_eq!(list(&c, "", Some(Kind::Files), None, 0, 50).unwrap().len(), 1);
        assert_eq!(list(&c, "", Some(Kind::Text), None, 0, 50).unwrap().len(), 1);
    }

    #[test]
    fn files_report_count_missing_and_folder() {
        let (c, d) = db();
        let i = d.path();
        let file = d.path().join("a.txt");
        std::fs::write(&file, "x").unwrap();
        let path = |p: &Path| p.to_string_lossy().into_owned();
        upsert(&c, i, files(&[&path(&file)]), None, 1).unwrap();
        upsert(&c, i, files(&[&path(d.path())]), None, 2).unwrap();
        upsert(&c, i, files(&["/definitely/not/here.txt", &path(&file)]), None, 3).unwrap();
        let all = list(&c, "", None, None, 0, 50).unwrap();
        assert_eq!((all[0].missing, all[0].is_dir, all[0].meta.as_deref()), (true, false, Some("2")));
        assert_eq!((all[1].missing, all[1].is_dir, all[1].meta.as_deref()), (false, true, Some("1")));
        assert_eq!((all[2].missing, all[2].is_dir, all[2].meta.as_deref()), (false, false, Some("1")));
    }

    #[test]
    fn large_text_preview_is_capped_but_counted() {
        let (c, d) = db();
        upsert(&c, d.path(), text(&"가".repeat(10_000)), None, 1).unwrap();
        let clip = &list(&c, "", None, None, 0, 50).unwrap()[0];
        assert_eq!(clip.text_preview.as_ref().unwrap().chars().count(), 500);
        assert_eq!(clip.char_count, 10_000);
    }

    #[test]
    fn content_of_missing_id_is_none() {
        let (c, _d) = db();
        assert_eq!(content(&c, 42).unwrap(), None);
    }

    #[test]
    fn content_round_trips_text_and_files() {
        let (c, d) = db();
        upsert(&c, d.path(), text("hi"), None, 1).unwrap();
        upsert(&c, d.path(), files(&["/a", "/b"]), None, 2).unwrap();
        let all = list(&c, "", None, None, 0, 50).unwrap();
        assert_eq!(content(&c, all[0].id).unwrap(), Some(ClipContent::Files(vec!["/a".into(), "/b".into()])));
        assert_eq!(content(&c, all[1].id).unwrap(), Some(ClipContent::Text("hi".into())));
    }

    #[test]
    fn image_has_thumb_and_dimensions() {
        let (c, d) = db();
        upsert(&c, d.path(), image(1), None, 1).unwrap();
        let clip = &list(&c, "", None, None, 0, 50).unwrap()[0];
        assert_eq!(clip.kind, Kind::Image);
        assert_eq!(clip.meta.as_deref(), Some("10×20"));
        assert!(clip.thumb.as_ref().unwrap().starts_with("data:image/png;base64,"));
    }

    #[test]
    fn delete_and_clear_remove_image_files() {
        let (c, d) = db();
        upsert(&c, d.path(), image(1), None, 1).unwrap();
        upsert(&c, d.path(), image(2), None, 2).unwrap();
        let all = list(&c, "", None, None, 0, 50).unwrap();
        let (a, b) = (image_path(&c, all[0].id), image_path(&c, all[1].id));
        delete(&c, all[0].id).unwrap();
        assert!(!a.exists() && b.exists());
        clear(&c).unwrap();
        assert!(!b.exists());
        assert!(list(&c, "", None, None, 0, 50).unwrap().is_empty());
    }

    #[test]
    fn touch_moves_clip_to_front() {
        let (c, d) = db();
        upsert(&c, d.path(), text("old"), None, 1).unwrap();
        upsert(&c, d.path(), text("new"), None, 2).unwrap();
        let old = list(&c, "", None, None, 0, 50).unwrap()[1].id;
        touch(&c, old, 3).unwrap();
        assert_eq!(texts(&list(&c, "", None, None, 0, 50).unwrap()), vec!["old", "new"]);
    }

    #[test]
    fn file_clips_carry_a_stack_of_up_to_three_layers() {
        let (c, d) = db();
        let i = d.path();
        let paths = |p: &[&str]| p.iter().map(|p| p.to_string()).collect::<Vec<_>>();
        upsert(&c, i, NewClip::Files { paths: paths(&["/a.png"]), thumbs: vec![(0, vec![1])] }, None, 1).unwrap();
        upsert(
            &c,
            i,
            NewClip::Files { paths: paths(&["/b.png", "/c.txt", "/d.png", "/e.png"]), thumbs: vec![(0, vec![2]), (2, vec![3])] },
            None,
            2,
        )
        .unwrap();
        upsert(&c, i, files(&["/f.txt", "/g.txt"]), None, 3).unwrap();
        let all = list(&c, "", None, None, 0, 50).unwrap();
        assert!(all[0].stack.is_empty());
        assert_eq!(all[1].stack.iter().map(Option::is_some).collect::<Vec<_>>(), vec![true, false, true]);
        assert_eq!(all[2].stack.len(), 1);
        assert!(all[2].stack[0].as_ref().unwrap().starts_with("data:image/png;base64,"));
        assert_eq!(preview_png(&c, all[1].id).unwrap(), Some(vec![2]));
    }

    #[test]
    fn preview_of_an_image_clip_is_its_thumbnail() {
        let (c, d) = db();
        let app = source_app::upsert_app(&c, "com.a", "A", Some(&[7, 7])).unwrap();
        upsert(&c, d.path(), image(1), Some(app), 1).unwrap();
        let id = list(&c, "", None, None, 0, 1).unwrap()[0].id;
        assert_eq!(preview_png(&c, id).unwrap(), Some(vec![9]));
    }

    #[test]
    fn preview_falls_back_to_the_app_icon() {
        let (c, d) = db();
        let app = source_app::upsert_app(&c, "com.a", "A", Some(&[7, 7])).unwrap();
        upsert(&c, d.path(), files(&["/x.txt"]), Some(app), 1).unwrap();
        upsert(&c, d.path(), text("no app"), None, 2).unwrap();
        let all = list(&c, "", None, None, 0, 50).unwrap();
        assert_eq!(preview_png(&c, all[1].id).unwrap(), Some(vec![7, 7]));
        assert_eq!(preview_png(&c, all[0].id).unwrap(), None);
    }

    #[test]
    fn delete_and_clear_remove_file_thumbnails() {
        let (c, d) = db();
        let thumbs_left = |c: &Connection| c.query_row("SELECT count(*) FROM clip_thumbs", [], |r| r.get::<_, i64>(0)).unwrap();
        let one = |p: &str| NewClip::Files { paths: vec![p.into()], thumbs: vec![(0, vec![1])] };
        upsert(&c, d.path(), one("/a.png"), None, 1).unwrap();
        delete(&c, list(&c, "", None, None, 0, 1).unwrap()[0].id).unwrap();
        assert_eq!(thumbs_left(&c), 0);
        upsert(&c, d.path(), one("/b.png"), None, 2).unwrap();
        clear(&c).unwrap();
        assert_eq!(thumbs_left(&c), 0);
    }

    #[test]
    fn formats_round_trip_in_order_are_replaced_on_bump_and_deleted_with_the_clip() {
        let (c, d) = db();
        let f = |pairs: &[(&str, &[u8])]| pairs.iter().map(|(k, v)| (k.to_string(), v.to_vec())).collect::<Vec<_>>();
        let id = upsert(&c, d.path(), text("a\tb"), None, 1).unwrap();
        set_formats(&c, id, &f(&[("public.utf8-plain-text", b"a\tb"), ("public.html", b"<table>"), ("com.adobe.pdf", b"%PDF")])).unwrap();
        assert_eq!(formats(&c, id).unwrap()[1], ("public.html".to_string(), b"<table>".to_vec()));
        assert_eq!(formats(&c, id).unwrap().len(), 3);
        let again = upsert(&c, d.path(), text("a\tb"), None, 2).unwrap();
        assert_eq!(again, id);
        set_formats(&c, id, &f(&[("public.utf8-plain-text", b"a\tb")])).unwrap();
        assert_eq!(formats(&c, id).unwrap().len(), 1);
        delete(&c, id).unwrap();
        assert!(formats(&c, id).unwrap().is_empty());
    }

    #[test]
    fn pinned_clips_leave_the_history_and_survive_a_bump() {
        let (c, d) = db();
        let a = upsert(&c, d.path(), text("keep"), None, 1).unwrap();
        upsert(&c, d.path(), text("other"), None, 2).unwrap();
        assert!(set_pinned(&c, a, true, 10).unwrap());
        assert_eq!(texts(&list_pinned(&c).unwrap()), vec!["keep"]);
        assert_eq!(texts(&list(&c, "", None, None, 0, 50).unwrap()), vec!["other"]);
        // Copying the same content again bumps it but must not unpin it.
        upsert(&c, d.path(), text("keep"), None, 3).unwrap();
        let pins = list_pinned(&c).unwrap();
        assert_eq!(texts(&pins), vec!["keep"]);
        assert!(pins[0].pinned);
        assert!(set_pinned(&c, a, false, 11).unwrap());
        assert!(list_pinned(&c).unwrap().is_empty());
        assert_eq!(texts(&list(&c, "", None, None, 0, 50).unwrap()), vec!["keep", "other"]);
    }

    #[test]
    fn prune_removes_only_unpinned_clips_older_than_the_cutoff() {
        let (c, d) = db();
        let i = d.path();
        let old_img = upsert(&c, i, image(1), None, 10).unwrap();
        let old_img_path = image_path(&c, old_img);
        upsert(&c, i, text("old"), None, 20).unwrap();
        let pinned = upsert(&c, i, text("old but pinned"), None, 30).unwrap();
        set_pinned(&c, pinned, true, 1).unwrap();
        upsert(&c, i, text("at cutoff"), None, 100).unwrap();
        upsert(&c, i, text("new"), None, 200).unwrap();
        assert_eq!(prunable_count(&c, 100).unwrap(), 2);
        assert_eq!(prune(&c, 100).unwrap(), 2);
        assert_eq!(texts(&list(&c, "", None, None, 0, 50).unwrap()), vec!["new", "at cutoff"]);
        assert_eq!(texts(&list_pinned(&c).unwrap()), vec!["old but pinned"]);
        assert!(!old_img_path.exists());
        assert_eq!(prunable_count(&c, 100).unwrap(), 0);
    }

    #[test]
    fn using_a_clip_again_keeps_it_from_being_pruned() {
        let (c, d) = db();
        let id = upsert(&c, d.path(), text("reused"), None, 10).unwrap();
        touch(&c, id, 500).unwrap();
        upsert(&c, d.path(), text("recopied"), None, 20).unwrap();
        upsert(&c, d.path(), text("recopied"), None, 600).unwrap();
        assert_eq!(prune(&c, 100).unwrap(), 0);
        assert_eq!(list(&c, "", None, None, 0, 50).unwrap().len(), 2);
    }

    #[test]
    fn clear_keeps_pinned_clips_and_their_images() {
        let (c, d) = db();
        let i = d.path();
        let kept = upsert(&c, i, image(1), None, 1).unwrap();
        let kept_path = image_path(&c, kept);
        set_pinned(&c, kept, true, 1).unwrap();
        let gone = upsert(&c, i, image(2), None, 2).unwrap();
        let gone_path = image_path(&c, gone);
        upsert(&c, i, text("gone"), None, 3).unwrap();
        assert_eq!(clear(&c).unwrap(), 2);
        assert!(list(&c, "", None, None, 0, 50).unwrap().is_empty());
        let left = list_pinned(&c).unwrap();
        assert_eq!(left.len(), 1);
        assert_eq!(left[0].id, kept);
        assert!(kept_path.exists());
        assert!(!gone_path.exists());
    }

    #[test]
    fn stats_count_clips_pins_and_image_bytes() {
        let (c, d) = db();
        let id = upsert(&c, d.path(), image(1), None, 1).unwrap(); // 8-byte png
        upsert(&c, d.path(), text("a"), None, 2).unwrap();
        set_pinned(&c, id, true, 1).unwrap();
        let st = stats(&c, d.path()).unwrap();
        assert_eq!((st.count, st.pinned, st.image_bytes), (2, 1, 8));
    }

    #[test]
    fn pin_limit_rejects_fourth() {
        let (c, d) = db();
        let ids: Vec<i64> = (1..=4).map(|n| upsert(&c, d.path(), text(&format!("c{n}")), None, n).unwrap()).collect();
        for &id in &ids[..3] {
            assert!(set_pinned(&c, id, true, 100).unwrap());
        }
        assert!(!set_pinned(&c, ids[3], true, 101).unwrap());
        assert_eq!(list_pinned(&c).unwrap().len(), 3);
        // Re-pinning a pinned clip at the limit is a no-op success, not a rejection.
        assert!(set_pinned(&c, ids[0], true, 102).unwrap());
        // Unpinning an unpinned clip is a no-op success too.
        assert!(set_pinned(&c, ids[3], false, 103).unwrap());
        // Freeing a slot lets the fourth in.
        assert!(set_pinned(&c, ids[1], false, 104).unwrap());
        assert!(set_pinned(&c, ids[3], true, 105).unwrap());
        assert_eq!(texts(&list_pinned(&c).unwrap()), vec!["c1", "c3", "c4"]);
    }

    #[test]
    fn list_excludes_pinned() {
        let (c, d) = db();
        let a = upsert(&c, d.path(), text("alpha pinned"), None, 1).unwrap();
        upsert(&c, d.path(), text("alpha loose"), None, 2).unwrap();
        set_pinned(&c, a, true, 3).unwrap();
        // Searches and kind filters only ever see the history.
        assert_eq!(texts(&list(&c, "alpha", None, None, 0, 50).unwrap()), vec!["alpha loose"]);
        assert_eq!(texts(&list(&c, "", Some(Kind::Text), None, 0, 50).unwrap()), vec!["alpha loose"]);
        assert_eq!(texts(&list_pinned(&c).unwrap()), vec!["alpha pinned"]);
    }

    #[test]
    fn list_pinned_keeps_pin_order() {
        let (c, d) = db();
        let first = upsert(&c, d.path(), text("first"), None, 1).unwrap();
        let second = upsert(&c, d.path(), text("second"), None, 2).unwrap();
        set_pinned(&c, second, true, 10).unwrap();
        set_pinned(&c, first, true, 20).unwrap();
        touch(&c, second, 30).unwrap(); // using a pin must not reorder the pinned area
        assert_eq!(texts(&list_pinned(&c).unwrap()), vec!["second", "first"]);
        // Re-pinning keeps the original slot.
        set_pinned(&c, second, true, 40).unwrap();
        assert_eq!(texts(&list_pinned(&c).unwrap()), vec!["second", "first"]);
    }

    fn ids(clips: &[ClipDto]) -> Vec<i64> {
        clips.iter().map(|c| c.id).collect()
    }

    #[test]
    fn ocr_text_is_searched_with_trigram_and_like() {
        let (c, d) = db();
        upsert(&c, d.path(), text("unrelated"), None, 1).unwrap();
        let id = upsert(&c, d.path(), image(1), None, 2).unwrap();
        set_ocr_text(&c, id, &hash_of(&c, id), "회의록 Deploy 실패").unwrap();
        assert_eq!(ids(&list(&c, "회의록", None, None, 0, 50).unwrap()), vec![id]);
        assert_eq!(ids(&list(&c, "deploy", None, None, 0, 50).unwrap()), vec![id]);
        assert_eq!(ids(&list(&c, "실패", None, None, 0, 50).unwrap()), vec![id]);
        assert!(list(&c, "없는말", None, None, 0, 50).unwrap().is_empty());
    }

    #[test]
    fn ocr_search_treats_special_characters_literally() {
        let (c, d) = db();
        upsert(&c, d.path(), text("plain"), None, 1).unwrap();
        let id = upsert(&c, d.path(), image(1), None, 2).unwrap();
        set_ocr_text(&c, id, &hash_of(&c, id), "100% done").unwrap();
        assert_eq!(ids(&list(&c, "0% d", None, None, 0, 50).unwrap()), vec![id]);
        assert_eq!(ids(&list(&c, "%", None, None, 0, 50).unwrap()), vec![id]);
    }

    #[test]
    fn text_clips_are_not_pending_and_images_are() {
        let (c, d) = db();
        upsert(&c, d.path(), text("hello"), None, 1).unwrap();
        let img = upsert(&c, d.path(), image(1), None, 2).unwrap();
        let fls = upsert(&c, d.path(), files(&["/a.png", "/b.png"]), None, 3).unwrap();
        upsert(&c, d.path(), text("https://example.com"), None, 4).unwrap();
        assert_eq!(
            next_ocr_pending(&c).unwrap(),
            Some((fls, hash_of(&c, fls), ClipContent::Files(vec!["/a.png".into(), "/b.png".into()])))
        );
        set_ocr_text(&c, fls, &hash_of(&c, fls), "").unwrap();
        assert_eq!(next_ocr_pending(&c).unwrap(), Some((img, hash_of(&c, img), ClipContent::Image(image_path(&c, img)))));
        set_ocr_text(&c, img, &hash_of(&c, img), "x").unwrap();
        assert_eq!(next_ocr_pending(&c).unwrap(), None);
    }

    #[test]
    fn pending_ignores_text_clips_with_null_ocr() {
        let (c, d) = db();
        let id = upsert(&c, d.path(), text("from an older build"), None, 1).unwrap();
        c.execute("UPDATE clips SET ocr_text = NULL WHERE id = ?1", [id]).unwrap();
        assert_eq!(next_ocr_pending(&c).unwrap(), None);
    }

    #[test]
    fn set_ocr_text_on_deleted_clip_is_ok() {
        let (c, _d) = db();
        set_ocr_text(&c, 9999, "gone", "x").unwrap();
    }

    #[test]
    fn stale_ocr_result_does_not_land_on_a_clip_that_reused_the_id() {
        let (c, d) = db();
        let id = upsert(&c, d.path(), image(1), None, 1).unwrap();
        let (pending, hash, _) = next_ocr_pending(&c).unwrap().unwrap();
        delete(&c, id).unwrap();
        let reused = upsert(&c, d.path(), image(2), None, 2).unwrap();
        assert_eq!(reused, pending);
        set_ocr_text(&c, pending, &hash, "stale").unwrap();
        let ocr: Option<String> = c.query_row("SELECT ocr_text FROM clips WHERE id = ?1", [reused], |r| r.get(0)).unwrap();
        assert_eq!(ocr, None);
    }

    #[test]
    fn recopying_an_image_keeps_its_ocr_text() {
        let (c, d) = db();
        let id = upsert(&c, d.path(), image(1), None, 1).unwrap();
        set_ocr_text(&c, id, &hash_of(&c, id), "abc").unwrap();
        assert_eq!(upsert(&c, d.path(), image(1), None, 2).unwrap(), id);
        assert_eq!(next_ocr_pending(&c).unwrap(), None);
        assert_eq!(ids(&list(&c, "abc", None, None, 0, 50).unwrap()), vec![id]);
    }
}
