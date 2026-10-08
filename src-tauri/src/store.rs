//! SQLite-backed clip history. Deliberately Tauri-free so it can be unit tested.

use base64::{Engine, engine::general_purpose::STANDARD};
use rusqlite::{Connection, OptionalExtension, params, params_from_iter, types::Value};
use serde::Serialize;
use sha2::{Digest, Sha256};
use std::path::{Path, PathBuf};

pub type Result<T> = std::result::Result<T, Box<dyn std::error::Error + Send + Sync>>;

const PREVIEW_CHARS: i64 = 500;

/// How many files of a files clip get a layer in the card's preview stack.
pub const STACK_LAYERS: usize = 3;

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

/// Every pasteboard representation of a clip, in the order the source app offered them,
/// so a copy back pastes exactly like the original (e.g. Excel cells stay cells).
const SCHEMA_V3: &str = "
BEGIN;
CREATE TABLE clip_formats (
  clip_id INTEGER NOT NULL,
  format  TEXT NOT NULL,
  data    BLOB NOT NULL,
  PRIMARY KEY (clip_id, format)
);
CREATE TRIGGER clip_formats_ad AFTER DELETE ON clips BEGIN
  DELETE FROM clip_formats WHERE clip_id = old.id;
END;
PRAGMA user_version = 3;
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
    /// `thumbs` holds `(file index, PNG)` for image files among the first `STACK_LAYERS`.
    Files { paths: Vec<String>, thumbs: Vec<(usize, Vec<u8>)> },
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
    /// Preview layers of a files clip; empty when none of its files has a thumbnail.
    pub stack: Vec<Option<String>>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AppDto {
    pub id: i64,
    pub name: String,
    pub icon: Option<String>,
    /// How many clips came from this app.
    pub count: i64,
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
        if version < 2 {
            conn.execute_batch(SCHEMA_V2)?;
        }
        if version < 3 {
            conn.execute_batch(SCHEMA_V3)?;
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

    /// Inserts a new clip, or moves an identical one to the front. Returns its id.
    pub fn upsert(&self, clip: NewClip, app_id: Option<i64>, now: i64) -> Result<i64> {
        let hash = match &clip {
            NewClip::Text(t) => sha256_hex(&[b"text\0", t.as_bytes()]),
            NewClip::Image { png, .. } => sha256_hex(&[b"image\0", png]),
            NewClip::Files { paths, .. } => sha256_hex(&[b"files\0", paths.join("\n").as_bytes()]),
        };
        let bumped: Option<i64> = self
            .conn
            .query_row(
                "UPDATE clips SET last_used_at = ?1, app_id = ?2 WHERE hash = ?3 RETURNING id",
                params![now, app_id, hash],
                |r| r.get(0),
            )
            .optional()?;
        if let Some(id) = bumped {
            return Ok(id);
        }
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
        Ok(id)
    }

    /// Replaces a clip's raw pasteboard formats; the latest copy of identical content wins.
    pub fn set_formats(&self, id: i64, formats: &[(String, Vec<u8>)]) -> Result<()> {
        let tx = self.conn.unchecked_transaction()?;
        tx.execute("DELETE FROM clip_formats WHERE clip_id = ?1", [id])?;
        for (format, data) in formats {
            tx.execute(
                "INSERT OR REPLACE INTO clip_formats (clip_id, format, data) VALUES (?1, ?2, ?3)",
                params![id, format, data],
            )?;
        }
        tx.commit()?;
        Ok(())
    }

    pub fn formats(&self, id: i64) -> Result<Vec<(String, Vec<u8>)>> {
        let mut stmt = self.conn.prepare("SELECT format, data FROM clip_formats WHERE clip_id = ?1 ORDER BY rowid")?;
        let rows = stmt.query_map([id], |r| Ok((r.get(0)?, r.get(1)?)))?;
        Ok(rows.collect::<rusqlite::Result<_>>()?)
    }

    pub fn list(&self, query: &str, kind: Option<Kind>, app_id: Option<i64>, offset: i64, limit: i64) -> Result<Vec<ClipDto>> {
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

    fn stack(&self, id: i64, files: usize) -> Result<Vec<Option<String>>> {
        let mut stmt = self.conn.prepare("SELECT idx, png FROM clip_thumbs WHERE clip_id = ?1")?;
        let thumbs: std::collections::HashMap<i64, Vec<u8>> =
            stmt.query_map([id], |r| Ok((r.get(0)?, r.get(1)?)))?.collect::<rusqlite::Result<_>>()?;
        if thumbs.is_empty() {
            return Ok(Vec::new());
        }
        Ok((0..files.min(STACK_LAYERS) as i64).map(|i| thumbs.get(&i).map(|png| data_url(png))).collect())
    }

    /// Image for dragging a clip out: its first file thumbnail or image thumbnail, else its source app's icon.
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
                "SELECT COALESCE(c.thumb_png, a.icon_png) FROM clips c LEFT JOIN apps a ON a.id = c.app_id WHERE c.id = ?1",
                [id],
                |r| r.get::<_, Option<Vec<u8>>>(0),
            )
            .optional()?
            .flatten())
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

    fn files(paths: &[&str]) -> NewClip {
        NewClip::Files { paths: paths.iter().map(|p| p.to_string()).collect(), thumbs: vec![] }
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
        let all = s.list("", None, None, 0, 50).unwrap();
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
    fn history_is_not_trimmed() {
        let (s, _d) = store();
        for i in 0..1100 {
            s.upsert(text(&format!("clip {i}")), None, i).unwrap();
        }
        assert_eq!(s.list("", None, None, 0, 200).unwrap().len(), 200);
        assert_eq!(s.list("", None, None, 1000, 200).unwrap().len(), 100);
    }

    #[test]
    fn searches_korean_with_trigram_and_short_queries_with_like() {
        let (s, _d) = store();
        s.upsert(text("클립보드 히스토리 앱"), None, 1).unwrap();
        s.upsert(text("cargo tauri dev"), None, 2).unwrap();
        s.upsert(text("Vee"), None, 3).unwrap();
        assert_eq!(texts(&s.list("히스토", None, None, 0, 50).unwrap()), vec!["클립보드 히스토리 앱"]);
        assert_eq!(texts(&s.list("보드", None, None, 0, 50).unwrap()), vec!["클립보드 히스토리 앱"]);
        assert_eq!(texts(&s.list("TAURI", None, None, 0, 50).unwrap()), vec!["cargo tauri dev"]);
        assert_eq!(texts(&s.list("ve", None, None, 0, 50).unwrap()), vec!["Vee"]);
    }

    #[test]
    fn search_treats_special_characters_literally() {
        let (s, _d) = store();
        s.upsert(text("100% done"), None, 1).unwrap();
        s.upsert(text("a_b"), None, 2).unwrap();
        s.upsert(text("say \"hi\" AND bye*"), None, 3).unwrap();
        s.upsert(text("plain"), None, 4).unwrap();
        assert_eq!(texts(&s.list("%", None, None, 0, 50).unwrap()), vec!["100% done"]);
        assert_eq!(texts(&s.list("_", None, None, 0, 50).unwrap()), vec!["a_b"]);
        assert_eq!(texts(&s.list("\"hi\" AND", None, None, 0, 50).unwrap()), vec!["say \"hi\" AND bye*"]);
        assert_eq!(texts(&s.list("bye*", None, None, 0, 50).unwrap()), vec!["say \"hi\" AND bye*"]);
        assert!(s.list("NOT", None, None, 0, 50).unwrap().is_empty());
    }

    #[test]
    fn filters_by_kind() {
        let (s, _d) = store();
        s.upsert(text("hello"), None, 1).unwrap();
        s.upsert(text("https://example.com"), None, 2).unwrap();
        s.upsert(files(&["/tmp/x"]), None, 3).unwrap();
        let links = s.list("", Some(Kind::Link), None, 0, 50).unwrap();
        assert_eq!(links.len(), 1);
        assert_eq!(links[0].kind, Kind::Link);
        assert_eq!(s.list("", Some(Kind::Files), None, 0, 50).unwrap().len(), 1);
        assert_eq!(s.list("", Some(Kind::Text), None, 0, 50).unwrap().len(), 1);
    }

    #[test]
    fn files_report_count_missing_and_folder() {
        let (s, d) = store();
        let file = d.path().join("a.txt");
        std::fs::write(&file, "x").unwrap();
        let path = |p: &Path| p.to_string_lossy().into_owned();
        s.upsert(files(&[&path(&file)]), None, 1).unwrap();
        s.upsert(files(&[&path(d.path())]), None, 2).unwrap();
        s.upsert(files(&["/definitely/not/here.txt", &path(&file)]), None, 3).unwrap();
        let all = s.list("", None, None, 0, 50).unwrap();
        assert_eq!((all[0].missing, all[0].is_dir, all[0].meta.as_deref()), (true, false, Some("2")));
        assert_eq!((all[1].missing, all[1].is_dir, all[1].meta.as_deref()), (false, true, Some("1")));
        assert_eq!((all[2].missing, all[2].is_dir, all[2].meta.as_deref()), (false, false, Some("1")));
    }

    #[test]
    fn large_text_preview_is_capped_but_counted() {
        let (s, _d) = store();
        s.upsert(text(&"가".repeat(10_000)), None, 1).unwrap();
        let clip = &s.list("", None, None, 0, 50).unwrap()[0];
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
        s.upsert(files(&["/a", "/b"]), None, 2).unwrap();
        let all = s.list("", None, None, 0, 50).unwrap();
        assert_eq!(s.content(all[0].id).unwrap(), Some(ClipContent::Files(vec!["/a".into(), "/b".into()])));
        assert_eq!(s.content(all[1].id).unwrap(), Some(ClipContent::Text("hi".into())));
    }

    #[test]
    fn image_has_thumb_and_dimensions() {
        let (s, _d) = store();
        s.upsert(image(1), None, 1).unwrap();
        let clip = &s.list("", None, None, 0, 50).unwrap()[0];
        assert_eq!(clip.kind, Kind::Image);
        assert_eq!(clip.meta.as_deref(), Some("10×20"));
        assert!(clip.thumb.as_ref().unwrap().starts_with("data:image/png;base64,"));
    }

    #[test]
    fn delete_and_clear_remove_image_files() {
        let (s, _d) = store();
        s.upsert(image(1), None, 1).unwrap();
        s.upsert(image(2), None, 2).unwrap();
        let all = s.list("", None, None, 0, 50).unwrap();
        let (a, b) = (image_path(&s, all[0].id), image_path(&s, all[1].id));
        s.delete(all[0].id).unwrap();
        assert!(!a.exists() && b.exists());
        s.clear().unwrap();
        assert!(!b.exists());
        assert!(s.list("", None, None, 0, 50).unwrap().is_empty());
    }

    #[test]
    fn touch_moves_clip_to_front() {
        let (s, _d) = store();
        s.upsert(text("old"), None, 1).unwrap();
        s.upsert(text("new"), None, 2).unwrap();
        let old = s.list("", None, None, 0, 50).unwrap()[1].id;
        s.touch(old, 3).unwrap();
        assert_eq!(texts(&s.list("", None, None, 0, 50).unwrap()), vec!["old", "new"]);
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
        let clip = &s.list("", None, None, 0, 50).unwrap()[0];
        assert_eq!(clip.app_name.as_deref(), Some("A2"));
        assert!(clip.app_icon.is_some());
    }

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
        assert_eq!(texts(&reopened.list("", None, None, 0, 50).unwrap()), vec!["persist me"]);
    }

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
        let all = s.list("", None, None, 0, 50).unwrap();
        assert!(all[0].stack.is_empty());
        assert_eq!(all[1].stack.iter().map(Option::is_some).collect::<Vec<_>>(), vec![true, false, true]);
        assert_eq!(all[2].stack.len(), 1);
        assert!(all[2].stack[0].as_ref().unwrap().starts_with("data:image/png;base64,"));
        assert_eq!(s.preview_png(all[1].id).unwrap(), Some(vec![2]));
    }

    #[test]
    fn preview_of_an_image_clip_is_its_thumbnail() {
        let (s, _d) = store();
        let app = s.upsert_app("com.a", "A", Some(&[7, 7])).unwrap();
        s.upsert(image(1), Some(app), 1).unwrap();
        let id = s.list("", None, None, 0, 1).unwrap()[0].id;
        assert_eq!(s.preview_png(id).unwrap(), Some(vec![9]));
    }

    #[test]
    fn preview_falls_back_to_the_app_icon() {
        let (s, _d) = store();
        let app = s.upsert_app("com.a", "A", Some(&[7, 7])).unwrap();
        s.upsert(files(&["/x.txt"]), Some(app), 1).unwrap();
        s.upsert(text("no app"), None, 2).unwrap();
        let all = s.list("", None, None, 0, 50).unwrap();
        assert_eq!(s.preview_png(all[1].id).unwrap(), Some(vec![7, 7]));
        assert_eq!(s.preview_png(all[0].id).unwrap(), None);
    }

    #[test]
    fn delete_and_clear_remove_file_thumbnails() {
        let (s, _d) = store();
        let thumbs_left = |s: &Store| s.conn.query_row("SELECT count(*) FROM clip_thumbs", [], |r| r.get::<_, i64>(0)).unwrap();
        let one = |p: &str| NewClip::Files { paths: vec![p.into()], thumbs: vec![(0, vec![1])] };
        s.upsert(one("/a.png"), None, 1).unwrap();
        s.delete(s.list("", None, None, 0, 1).unwrap()[0].id).unwrap();
        assert_eq!(thumbs_left(&s), 0);
        s.upsert(one("/b.png"), None, 2).unwrap();
        s.clear().unwrap();
        assert_eq!(thumbs_left(&s), 0);
    }

    #[test]
    fn v1_database_migrates_to_latest() {
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
        assert!(s.list("", None, None, 0, 50).unwrap()[0].stack.is_empty());
        let version: i64 = s.conn.query_row("PRAGMA user_version", [], |r| r.get(0)).unwrap();
        assert_eq!(version, 3);
    }

    #[test]
    fn formats_round_trip_in_order_are_replaced_on_bump_and_deleted_with_the_clip() {
        let (s, _d) = store();
        let f = |pairs: &[(&str, &[u8])]| pairs.iter().map(|(k, v)| (k.to_string(), v.to_vec())).collect::<Vec<_>>();
        let id = s.upsert(text("a\tb"), None, 1).unwrap();
        s.set_formats(id, &f(&[("public.utf8-plain-text", b"a\tb"), ("public.html", b"<table>"), ("com.adobe.pdf", b"%PDF")]))
            .unwrap();
        assert_eq!(s.formats(id).unwrap()[1], ("public.html".to_string(), b"<table>".to_vec()));
        assert_eq!(s.formats(id).unwrap().len(), 3);
        let again = s.upsert(text("a\tb"), None, 2).unwrap();
        assert_eq!(again, id);
        s.set_formats(id, &f(&[("public.utf8-plain-text", b"a\tb")])).unwrap();
        assert_eq!(s.formats(id).unwrap().len(), 1);
        s.delete(id).unwrap();
        assert!(s.formats(id).unwrap().is_empty());
    }
}
