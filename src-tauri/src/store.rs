//! SQLite-backed clip history. Deliberately Tauri-free so it can be unit tested.

use base64::{Engine, engine::general_purpose::STANDARD};
use rusqlite::{Connection, OptionalExtension, params, params_from_iter, types::Value};
use serde::Serialize;
use sha2::{Digest, Sha256};
use std::path::{Path, PathBuf};

pub type Result<T> = std::result::Result<T, Box<dyn std::error::Error + Send + Sync>>;

pub const MAX_CLIPS: i64 = 1000;
const PREVIEW_CHARS: i64 = 500;

const SCHEMA_V1: &str = "
BEGIN;
CREATE TABLE apps (
  id        INTEGER PRIMARY KEY,
  bundle_id TEXT UNIQUE NOT NULL,
  name      TEXT NOT NULL,
  icon_png  BLOB
);
CREATE TABLE clips (
  id           INTEGER PRIMARY KEY,
  kind         TEXT NOT NULL CHECK (kind IN ('text','link','image','files')),
  hash         TEXT UNIQUE NOT NULL,
  text         TEXT,
  image_path   TEXT,
  thumb_png    BLOB,
  meta         TEXT,
  app_id       INTEGER REFERENCES apps(id),
  created_at   INTEGER NOT NULL,
  last_used_at INTEGER NOT NULL
);
CREATE INDEX clips_last_used ON clips(last_used_at DESC);
CREATE VIRTUAL TABLE clips_fts USING fts5(text, content='clips', content_rowid='id', tokenize='trigram');
CREATE TRIGGER clips_ai AFTER INSERT ON clips BEGIN
  INSERT INTO clips_fts(rowid, text) VALUES (new.id, new.text);
END;
CREATE TRIGGER clips_ad AFTER DELETE ON clips BEGIN
  INSERT INTO clips_fts(clips_fts, rowid, text) VALUES ('delete', old.id, old.text);
END;
CREATE TRIGGER clips_au AFTER UPDATE OF text ON clips BEGIN
  INSERT INTO clips_fts(clips_fts, rowid, text) VALUES ('delete', old.id, old.text);
  INSERT INTO clips_fts(rowid, text) VALUES (new.id, new.text);
END;
CREATE TABLE settings (key TEXT PRIMARY KEY, value TEXT NOT NULL);
PRAGMA user_version = 1;
COMMIT;
";

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum Kind {
    Text,
    Link,
    Image,
    Files,
}

impl Kind {
    pub fn as_str(self) -> &'static str {
        match self {
            Kind::Text => "text",
            Kind::Link => "link",
            Kind::Image => "image",
            Kind::Files => "files",
        }
    }

    pub fn parse(s: &str) -> Option<Kind> {
        match s {
            "text" => Some(Kind::Text),
            "link" => Some(Kind::Link),
            "image" => Some(Kind::Image),
            "files" => Some(Kind::Files),
            _ => None,
        }
    }
}

pub enum NewClip {
    Text(String),
    Image { png: Vec<u8>, thumb_png: Vec<u8>, width: u32, height: u32 },
    Files(Vec<String>),
}

#[derive(Debug, PartialEq)]
pub enum ClipContent {
    Text(String),
    Image(PathBuf),
    Files(Vec<String>),
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ClipDto {
    pub id: i64,
    pub kind: Kind,
    pub text_preview: Option<String>,
    pub char_count: i64,
    pub thumb: Option<String>,
    pub meta: Option<String>,
    pub app_name: Option<String>,
    pub app_icon: Option<String>,
    pub last_used_at: i64,
    pub missing: bool,
    pub is_dir: bool,
}

/// A single URL (no whitespace inside) is a link; anything else is text.
pub fn classify_text(text: &str) -> Kind {
    let t = text.trim();
    let is_url = ["http://", "https://", "ssh://", "git@"].iter().any(|p| t.starts_with(p));
    if is_url && !t.chars().any(char::is_whitespace) { Kind::Link } else { Kind::Text }
}

fn sha256_hex(parts: &[&[u8]]) -> String {
    let mut hasher = Sha256::new();
    for part in parts {
        hasher.update(part);
    }
    hasher.finalize().iter().map(|b| format!("{b:02x}")).collect()
}

fn data_url(png: &[u8]) -> String {
    format!("data:image/png;base64,{}", STANDARD.encode(png))
}

fn escape_like(q: &str) -> String {
    q.replace('\\', "\\\\").replace('%', "\\%").replace('_', "\\_")
}

/// Quote the whole query as one FTS5 phrase so operators like AND/NOT/* are literal.
fn fts_phrase(q: &str) -> String {
    format!("\"{}\"", q.replace('"', "\"\""))
}

pub struct Store {
    conn: Connection,
    images_dir: PathBuf,
}

impl Store {
    pub fn open(db_path: &Path, images_dir: &Path) -> Result<Store> {
        if let Some(parent) = db_path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        Self::init(Connection::open(db_path)?, images_dir)
    }

    pub fn open_in_memory(images_dir: &Path) -> Result<Store> {
        Self::init(Connection::open_in_memory()?, images_dir)
    }

    fn init(conn: Connection, images_dir: &Path) -> Result<Store> {
        std::fs::create_dir_all(images_dir)?;
        conn.query_row("PRAGMA journal_mode=WAL", [], |_| Ok(()))?;
        let version: i64 = conn.query_row("PRAGMA user_version", [], |r| r.get(0))?;
        if version < 1 {
            conn.execute_batch(SCHEMA_V1)?;
        }
        Ok(Store { conn, images_dir: images_dir.to_path_buf() })
    }

    pub fn app_known(&self, bundle_id: &str) -> Result<bool> {
        let found = self
            .conn
            .query_row("SELECT 1 FROM apps WHERE bundle_id = ?1", [bundle_id], |_| Ok(()))
            .optional()?;
        Ok(found.is_some())
    }

    pub fn upsert_app(&self, bundle_id: &str, name: &str, icon_png: Option<&[u8]>) -> Result<i64> {
        Ok(self.conn.query_row(
            "INSERT INTO apps (bundle_id, name, icon_png) VALUES (?1, ?2, ?3)
             ON CONFLICT(bundle_id) DO UPDATE SET
               name = excluded.name,
               icon_png = COALESCE(excluded.icon_png, apps.icon_png)
             RETURNING id",
            params![bundle_id, name, icon_png],
            |r| r.get(0),
        )?)
    }

    /// Inserts a new clip, or moves an identical one to the front.
    pub fn upsert(&self, clip: NewClip, app_id: Option<i64>, now: i64) -> Result<()> {
        let hash = match &clip {
            NewClip::Text(t) => sha256_hex(&[b"text\0", t.as_bytes()]),
            NewClip::Image { png, .. } => sha256_hex(&[b"image\0", png]),
            NewClip::Files(f) => sha256_hex(&[b"files\0", f.join("\n").as_bytes()]),
        };
        let bumped = self.conn.execute(
            "UPDATE clips SET last_used_at = ?1, app_id = ?2 WHERE hash = ?3",
            params![now, app_id, hash],
        )?;
        if bumped > 0 {
            return Ok(());
        }
        let (kind, text, image_path, thumb, meta) = match clip {
            NewClip::Text(t) => (classify_text(&t), Some(t), None, None, None),
            NewClip::Image { png, thumb_png, width, height } => {
                let path = self.images_dir.join(format!("{hash}.png"));
                std::fs::write(&path, png)?;
                let path = path.to_string_lossy().into_owned();
                (Kind::Image, None, Some(path), Some(thumb_png), Some(format!("{width}×{height}")))
            }
            NewClip::Files(f) => {
                let count = f.len().to_string();
                (Kind::Files, Some(f.join("\n")), None, None, Some(count))
            }
        };
        self.conn.execute(
            "INSERT INTO clips (kind, hash, text, image_path, thumb_png, meta, app_id, created_at, last_used_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?8)",
            params![kind.as_str(), hash, text, image_path, thumb, meta, app_id, now],
        )?;
        self.trim()
    }

    fn trim(&self) -> Result<()> {
        let mut stmt = self
            .conn
            .prepare("SELECT id FROM clips ORDER BY last_used_at DESC, id DESC LIMIT -1 OFFSET ?1")?;
        let ids: Vec<i64> = stmt.query_map([MAX_CLIPS], |r| r.get(0))?.collect::<rusqlite::Result<_>>()?;
        for id in ids {
            self.delete(id)?;
        }
        Ok(())
    }

    pub fn list(&self, query: &str, kind: Option<Kind>, offset: i64, limit: i64) -> Result<Vec<ClipDto>> {
        let mut sql = String::from(
            "SELECT c.id, c.kind, substr(c.text, 1, ?), COALESCE(length(c.text), 0), c.thumb_png, c.meta,
                    a.name, a.icon_png, c.last_used_at,
                    CASE c.kind WHEN 'files' THEN c.text WHEN 'image' THEN c.image_path END
             FROM clips c LEFT JOIN apps a ON a.id = c.app_id
             WHERE 1 = 1",
        );
        let mut args = vec![Value::Integer(PREVIEW_CHARS)];
        if let Some(k) = kind {
            sql.push_str(" AND c.kind = ?");
            args.push(Value::Text(k.as_str().into()));
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

        let mut stmt = self.conn.prepare(&sql)?;
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
            })
        })?;
        Ok(rows.collect::<rusqlite::Result<_>>()?)
    }

    pub fn content(&self, id: i64) -> Result<Option<ClipContent>> {
        let row = self
            .conn
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

    pub fn touch(&self, id: i64, now: i64) -> Result<()> {
        self.conn.execute("UPDATE clips SET last_used_at = ?1 WHERE id = ?2", params![now, id])?;
        Ok(())
    }

    pub fn delete(&self, id: i64) -> Result<()> {
        let image_path: Option<Option<String>> = self
            .conn
            .query_row("SELECT image_path FROM clips WHERE id = ?1", [id], |r| r.get(0))
            .optional()?;
        self.conn.execute("DELETE FROM clips WHERE id = ?1", [id])?;
        if let Some(Some(path)) = image_path {
            let _ = std::fs::remove_file(path);
        }
        Ok(())
    }

    pub fn clear(&self) -> Result<()> {
        self.conn.execute("DELETE FROM clips", [])?;
        for entry in std::fs::read_dir(&self.images_dir)? {
            let _ = std::fs::remove_file(entry?.path());
        }
        Ok(())
    }

    pub fn get_setting(&self, key: &str) -> Result<Option<String>> {
        Ok(self
            .conn
            .query_row("SELECT value FROM settings WHERE key = ?1", [key], |r| r.get(0))
            .optional()?)
    }

    pub fn set_setting(&self, key: &str, value: &str) -> Result<()> {
        self.conn.execute(
            "INSERT INTO settings (key, value) VALUES (?1, ?2)
             ON CONFLICT(key) DO UPDATE SET value = excluded.value",
            params![key, value],
        )?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::Path;

    fn store() -> (Store, tempfile::TempDir) {
        let dir = tempfile::tempdir().unwrap();
        let store = Store::open_in_memory(&dir.path().join("images")).unwrap();
        (store, dir)
    }

    fn text(s: &str) -> NewClip {
        NewClip::Text(s.to_string())
    }

    fn texts(clips: &[ClipDto]) -> Vec<String> {
        clips.iter().map(|c| c.text_preview.clone().unwrap_or_default()).collect()
    }

    fn image(byte: u8) -> NewClip {
        NewClip::Image { png: vec![byte; 8], thumb_png: vec![9], width: 10, height: 20 }
    }

    fn image_path(store: &Store, id: i64) -> std::path::PathBuf {
        match store.content(id).unwrap() {
            Some(ClipContent::Image(p)) => p,
            other => panic!("expected image, got {other:?}"),
        }
    }

    #[test]
    fn same_text_bumps_instead_of_duplicating() {
        let (s, _d) = store();
        s.upsert(text("hello"), None, 1).unwrap();
        s.upsert(text("world"), None, 2).unwrap();
        s.upsert(text("hello"), None, 3).unwrap();
        let all = s.list("", None, 0, 50).unwrap();
        assert_eq!(texts(&all), vec!["hello", "world"]);
        assert_eq!(all[0].last_used_at, 3);
    }

    #[test]
    fn classifies_links() {
        assert_eq!(classify_text("https://example.com/a?b=1"), Kind::Link);
        assert_eq!(classify_text("  git@github.com:bob-park/vee-app.git\n"), Kind::Link);
        assert_eq!(classify_text("ssh://host/repo"), Kind::Link);
        assert_eq!(classify_text("see https://example.com"), Kind::Text);
        assert_eq!(classify_text("https://a.com\nhttps://b.com"), Kind::Text);
        assert_eq!(classify_text("hello"), Kind::Text);
    }

    #[test]
    fn trims_oldest_beyond_max_and_removes_image_file() {
        let (s, _d) = store();
        s.upsert(image(1), None, 0).unwrap();
        let first = s.list("", None, 0, 1).unwrap()[0].id;
        let path = image_path(&s, first);
        assert!(path.exists());
        for i in 1..=MAX_CLIPS {
            s.upsert(text(&format!("clip {i}")), None, i).unwrap();
        }
        let all = s.list("", None, 0, 2000).unwrap();
        assert_eq!(all.len() as i64, MAX_CLIPS);
        assert!(all.iter().all(|c| c.kind != Kind::Image));
        assert!(!path.exists());
    }

    #[test]
    fn searches_korean_with_trigram_and_short_queries_with_like() {
        let (s, _d) = store();
        s.upsert(text("클립보드 히스토리 앱"), None, 1).unwrap();
        s.upsert(text("cargo tauri dev"), None, 2).unwrap();
        s.upsert(text("Vee"), None, 3).unwrap();
        assert_eq!(texts(&s.list("히스토", None, 0, 50).unwrap()), vec!["클립보드 히스토리 앱"]);
        assert_eq!(texts(&s.list("보드", None, 0, 50).unwrap()), vec!["클립보드 히스토리 앱"]);
        assert_eq!(texts(&s.list("TAURI", None, 0, 50).unwrap()), vec!["cargo tauri dev"]);
        assert_eq!(texts(&s.list("ve", None, 0, 50).unwrap()), vec!["Vee"]);
    }

    #[test]
    fn search_treats_special_characters_literally() {
        let (s, _d) = store();
        s.upsert(text("100% done"), None, 1).unwrap();
        s.upsert(text("a_b"), None, 2).unwrap();
        s.upsert(text("say \"hi\" AND bye*"), None, 3).unwrap();
        s.upsert(text("plain"), None, 4).unwrap();
        assert_eq!(texts(&s.list("%", None, 0, 50).unwrap()), vec!["100% done"]);
        assert_eq!(texts(&s.list("_", None, 0, 50).unwrap()), vec!["a_b"]);
        assert_eq!(texts(&s.list("\"hi\" AND", None, 0, 50).unwrap()), vec!["say \"hi\" AND bye*"]);
        assert_eq!(texts(&s.list("bye*", None, 0, 50).unwrap()), vec!["say \"hi\" AND bye*"]);
        assert!(s.list("NOT", None, 0, 50).unwrap().is_empty());
    }

    #[test]
    fn filters_by_kind() {
        let (s, _d) = store();
        s.upsert(text("hello"), None, 1).unwrap();
        s.upsert(text("https://example.com"), None, 2).unwrap();
        s.upsert(NewClip::Files(vec!["/tmp/x".into()]), None, 3).unwrap();
        let links = s.list("", Some(Kind::Link), 0, 50).unwrap();
        assert_eq!(links.len(), 1);
        assert_eq!(links[0].kind, Kind::Link);
        assert_eq!(s.list("", Some(Kind::Files), 0, 50).unwrap().len(), 1);
        assert_eq!(s.list("", Some(Kind::Text), 0, 50).unwrap().len(), 1);
    }

    #[test]
    fn files_report_count_missing_and_folder() {
        let (s, d) = store();
        let file = d.path().join("a.txt");
        std::fs::write(&file, "x").unwrap();
        let path = |p: &Path| p.to_string_lossy().into_owned();
        s.upsert(NewClip::Files(vec![path(&file)]), None, 1).unwrap();
        s.upsert(NewClip::Files(vec![path(d.path())]), None, 2).unwrap();
        s.upsert(NewClip::Files(vec!["/definitely/not/here.txt".into(), path(&file)]), None, 3).unwrap();
        let all = s.list("", None, 0, 50).unwrap();
        assert_eq!((all[0].missing, all[0].is_dir, all[0].meta.as_deref()), (true, false, Some("2")));
        assert_eq!((all[1].missing, all[1].is_dir, all[1].meta.as_deref()), (false, true, Some("1")));
        assert_eq!((all[2].missing, all[2].is_dir, all[2].meta.as_deref()), (false, false, Some("1")));
    }

    #[test]
    fn large_text_preview_is_capped_but_counted() {
        let (s, _d) = store();
        s.upsert(text(&"가".repeat(10_000)), None, 1).unwrap();
        let clip = &s.list("", None, 0, 50).unwrap()[0];
        assert_eq!(clip.text_preview.as_ref().unwrap().chars().count(), 500);
        assert_eq!(clip.char_count, 10_000);
    }

    #[test]
    fn content_of_missing_id_is_none() {
        let (s, _d) = store();
        assert_eq!(s.content(42).unwrap(), None);
    }

    #[test]
    fn content_round_trips_text_and_files() {
        let (s, _d) = store();
        s.upsert(text("hi"), None, 1).unwrap();
        s.upsert(NewClip::Files(vec!["/a".into(), "/b".into()]), None, 2).unwrap();
        let all = s.list("", None, 0, 50).unwrap();
        assert_eq!(s.content(all[0].id).unwrap(), Some(ClipContent::Files(vec!["/a".into(), "/b".into()])));
        assert_eq!(s.content(all[1].id).unwrap(), Some(ClipContent::Text("hi".into())));
    }

    #[test]
    fn image_has_thumb_and_dimensions() {
        let (s, _d) = store();
        s.upsert(image(1), None, 1).unwrap();
        let clip = &s.list("", None, 0, 50).unwrap()[0];
        assert_eq!(clip.kind, Kind::Image);
        assert_eq!(clip.meta.as_deref(), Some("10×20"));
        assert!(clip.thumb.as_ref().unwrap().starts_with("data:image/png;base64,"));
    }

    #[test]
    fn delete_and_clear_remove_image_files() {
        let (s, _d) = store();
        s.upsert(image(1), None, 1).unwrap();
        s.upsert(image(2), None, 2).unwrap();
        let all = s.list("", None, 0, 50).unwrap();
        let (a, b) = (image_path(&s, all[0].id), image_path(&s, all[1].id));
        s.delete(all[0].id).unwrap();
        assert!(!a.exists() && b.exists());
        s.clear().unwrap();
        assert!(!b.exists());
        assert!(s.list("", None, 0, 50).unwrap().is_empty());
    }

    #[test]
    fn touch_moves_clip_to_front() {
        let (s, _d) = store();
        s.upsert(text("old"), None, 1).unwrap();
        s.upsert(text("new"), None, 2).unwrap();
        let old = s.list("", None, 0, 50).unwrap()[1].id;
        s.touch(old, 3).unwrap();
        assert_eq!(texts(&s.list("", None, 0, 50).unwrap()), vec!["old", "new"]);
    }

    #[test]
    fn upsert_app_keeps_existing_icon() {
        let (s, _d) = store();
        assert!(!s.app_known("com.a").unwrap());
        let first = s.upsert_app("com.a", "A", Some(&[1, 2])).unwrap();
        let second = s.upsert_app("com.a", "A2", None).unwrap();
        assert_eq!(first, second);
        assert!(s.app_known("com.a").unwrap());
        s.upsert(text("x"), Some(first), 1).unwrap();
        let clip = &s.list("", None, 0, 50).unwrap()[0];
        assert_eq!(clip.app_name.as_deref(), Some("A2"));
        assert!(clip.app_icon.is_some());
    }

    #[test]
    fn settings_round_trip() {
        let (s, _d) = store();
        assert_eq!(s.get_setting("theme").unwrap(), None);
        s.set_setting("theme", "dark").unwrap();
        s.set_setting("theme", "light").unwrap();
        assert_eq!(s.get_setting("theme").unwrap().as_deref(), Some("light"));
    }

    #[test]
    fn history_survives_reopen() {
        let dir = tempfile::tempdir().unwrap();
        let db = dir.path().join("vee.db");
        let images = dir.path().join("images");
        Store::open(&db, &images).unwrap().upsert(text("persist me"), None, 1).unwrap();
        let reopened = Store::open(&db, &images).unwrap();
        assert_eq!(texts(&reopened.list("", None, 0, 50).unwrap()), vec!["persist me"]);
    }
}
