//! Opens the SQLite database and applies migrations tracked by `PRAGMA user_version`.

use crate::shared::error::AppResult;
use rusqlite::Connection;
use std::path::Path;

/// One entry per schema version, applied in order. Never edit a shipped entry; append a new file.
const MIGRATIONS: &[&str] = &[
    include_str!("migrations/0001_init.sql"),
    include_str!("migrations/0002_clip_thumbs.sql"),
    include_str!("migrations/0003_clip_formats.sql"),
    include_str!("migrations/0004_pins_and_exclusions.sql"),
    include_str!("migrations/0005_pin_order.sql"),
];

pub fn open(path: &Path) -> AppResult<Connection> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    init(Connection::open(path)?)
}

#[cfg(test)]
pub fn open_in_memory() -> AppResult<Connection> {
    init(Connection::open_in_memory()?)
}

fn init(mut conn: Connection) -> AppResult<Connection> {
    conn.query_row("PRAGMA journal_mode=WAL", [], |_| Ok(()))?;
    migrate(&mut conn)?;
    Ok(conn)
}

/// Runs each pending migration in its own transaction. A DB from a newer app (version above
/// `MIGRATIONS.len()`) is left untouched, as the app has always done.
fn migrate(conn: &mut Connection) -> AppResult<()> {
    let current: i64 = conn.query_row("PRAGMA user_version", [], |r| r.get(0))?;
    for (i, sql) in MIGRATIONS.iter().enumerate().skip(current.max(0) as usize) {
        let tx = conn.transaction()?;
        tx.execute_batch(sql)?;
        tx.pragma_update(None, "user_version", (i + 1) as i64)?;
        tx.commit()?;
    }
    Ok(())
}

/// Escapes `%`, `_` and `\` for a `LIKE … ESCAPE '\'` pattern.
pub fn escape_like(q: &str) -> String {
    q.replace('\\', "\\\\").replace('%', "\\%").replace('_', "\\_")
}

/// The schema constants `store.rs` shipped (v1–v5), kept verbatim to check the migrations against.
#[cfg(test)]
pub(crate) mod legacy_schema {
    pub const V1: &str = "
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

    pub const V2: &str = "
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

    pub const V3: &str = "
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

    pub const V4: &str = "
BEGIN;
ALTER TABLE clips ADD COLUMN pinned INTEGER NOT NULL DEFAULT 0;
ALTER TABLE apps ADD COLUMN excluded INTEGER NOT NULL DEFAULT 0;
PRAGMA user_version = 4;
COMMIT;
";

    pub const V5: &str = "
BEGIN;
ALTER TABLE clips ADD COLUMN pinned_at INTEGER;
UPDATE clips SET pinned = 0
 WHERE pinned = 1
   AND id NOT IN (SELECT id FROM clips WHERE pinned = 1 ORDER BY last_used_at DESC, id DESC LIMIT 3);
UPDATE clips SET pinned_at = last_used_at WHERE pinned = 1;
PRAGMA user_version = 5;
COMMIT;
";
}

#[cfg(test)]
mod tests {
    use super::*;

    fn user_version(conn: &Connection) -> i64 {
        conn.query_row("PRAGMA user_version", [], |r| r.get(0)).unwrap()
    }

    fn columns(conn: &Connection, table: &str) -> Vec<String> {
        let mut stmt = conn.prepare(&format!("PRAGMA table_info({table})")).unwrap();
        stmt.query_map([], |r| r.get(1)).unwrap().map(Result::unwrap).collect()
    }

    fn schema(conn: &Connection) -> Vec<(String, String, Option<String>)> {
        let mut stmt = conn.prepare("SELECT type, name, sql FROM sqlite_master ORDER BY name").unwrap();
        stmt.query_map([], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?))).unwrap().map(Result::unwrap).collect()
    }

    #[test]
    fn opens_a_database_from_a_newer_app() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("app.db");
        open(&path).unwrap().pragma_update(None, "user_version", 99).unwrap();
        let conn = open(&path).unwrap();
        assert_eq!(user_version(&conn), 99);
    }

    #[test]
    fn fresh_database_reaches_version_5() {
        let conn = open_in_memory().unwrap();
        assert_eq!(user_version(&conn), 5);
        for table in ["apps", "clips", "settings", "clip_thumbs", "clip_formats"] {
            assert!(!columns(&conn, table).is_empty(), "missing table {table}");
        }
        let clips = columns(&conn, "clips");
        assert!(clips.contains(&"pinned".into()) && clips.contains(&"pinned_at".into()), "{clips:?}");
        assert!(columns(&conn, "apps").contains(&"excluded".into()));
    }

    #[test]
    fn migrations_match_the_shipped_schema() {
        let legacy = Connection::open_in_memory().unwrap();
        for sql in [legacy_schema::V1, legacy_schema::V2, legacy_schema::V3, legacy_schema::V4, legacy_schema::V5] {
            legacy.execute_batch(sql).unwrap();
        }
        let conn = open_in_memory().unwrap();
        assert_eq!(schema(&conn), schema(&legacy));
        assert_eq!(user_version(&conn), user_version(&legacy));
    }

    #[test]
    fn upgrades_from_each_shipped_version() {
        let legacy = [legacy_schema::V1, legacy_schema::V2, legacy_schema::V3, legacy_schema::V4, legacy_schema::V5];
        for k in 1..=legacy.len() {
            let dir = tempfile::tempdir().unwrap();
            let path = dir.path().join("app.db");
            let conn = Connection::open(&path).unwrap();
            for sql in &legacy[..k] {
                conn.execute_batch(sql).unwrap();
            }
            drop(conn);
            let conn = open(&path).unwrap();
            assert_eq!(user_version(&conn), 5, "from v{k}");
            assert!(columns(&conn, "clips").contains(&"pinned_at".into()), "from v{k}");
        }
    }

    #[test]
    fn open_creates_missing_parent_dirs() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("a/b/app.db");
        open(&path).unwrap();
        assert!(path.exists());
    }

    fn texts(clips: Vec<crate::domain::clip::ClipDto>) -> Vec<String> {
        clips.into_iter().map(|c| c.text_preview.unwrap_or_default()).collect()
    }

    #[test]
    fn history_survives_reopen() {
        use crate::domain::clip::{self, NewClip};
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("vee.db");
        clip::upsert(&open(&path).unwrap(), dir.path(), NewClip::Text("persist me".into()), None, 1).unwrap();
        let reopened = open(&path).unwrap();
        assert_eq!(texts(clip::list(&reopened, "", "all", None, 0, 50).unwrap()), vec!["persist me"]);
    }

    #[test]
    fn v1_database_migrates_to_latest() {
        use crate::domain::clip;
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("vee.db");
        {
            let conn = Connection::open(&path).unwrap();
            conn.execute_batch(legacy_schema::V1).unwrap();
            conn.execute(
                "INSERT INTO clips (kind, hash, text, meta, created_at, last_used_at) VALUES ('files', 'h', '/x.png', '1', 1, 1)",
                [],
            )
            .unwrap();
        }
        let conn = open(&path).unwrap();
        let clip = &clip::list(&conn, "", "all", None, 0, 50).unwrap()[0];
        assert!(clip.stack.is_empty());
        assert!(!clip.pinned);
        assert_eq!(user_version(&conn), 5);
    }

    #[test]
    fn v5_migration_keeps_three_most_recent() {
        use crate::domain::clip;
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("vee.db");
        {
            let conn = Connection::open(&path).unwrap();
            for schema in [legacy_schema::V1, legacy_schema::V2, legacy_schema::V3, legacy_schema::V4] {
                conn.execute_batch(schema).unwrap();
            }
            for n in 1..=5i64 {
                conn.execute(
                    "INSERT INTO clips (kind, hash, text, created_at, last_used_at, pinned)
                     VALUES ('text', ?1, ?2, ?3, ?3, 1)",
                    rusqlite::params![format!("h{n}"), format!("pin {n}"), n],
                )
                .unwrap();
            }
        }
        let conn = open(&path).unwrap();
        assert_eq!(texts(clip::list_pinned(&conn).unwrap()), vec!["pin 3", "pin 4", "pin 5"]);
        // The rest are unpinned, not deleted.
        assert_eq!(texts(clip::list(&conn, "", "all", None, 0, 50).unwrap()), vec!["pin 2", "pin 1"]);
    }

    #[test]
    fn v5_database_keeps_user_data() {
        use crate::domain::{clip, settings, source_app};
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("vee.db");
        {
            let conn = Connection::open(&path).unwrap();
            for sql in [legacy_schema::V1, legacy_schema::V2, legacy_schema::V3, legacy_schema::V4, legacy_schema::V5] {
                conn.execute_batch(sql).unwrap();
            }
            conn.execute_batch(
                "INSERT INTO apps (id, bundle_id, name, excluded) VALUES (1, 'com.agilebits.onepassword', '1Password', 1);
                 INSERT INTO clips (kind, hash, text, created_at, last_used_at) VALUES ('text', 'h1', 'loose', 1, 1);
                 INSERT INTO clips (kind, hash, text, created_at, last_used_at, pinned, pinned_at) VALUES ('text', 'h2', 'kept', 2, 2, 1, 2);
                 INSERT INTO settings (key, value) VALUES ('theme', 'dark');",
            )
            .unwrap();
        }
        let conn = open(&path).unwrap();
        assert_eq!(texts(clip::list(&conn, "", "all", None, 0, 50).unwrap()), vec!["loose"]);
        assert_eq!(texts(clip::list_pinned(&conn).unwrap()), vec!["kept"]);
        let excluded = source_app::excluded_apps(&conn).unwrap();
        assert_eq!(excluded.iter().map(|a| a.name.as_str()).collect::<Vec<_>>(), vec!["1Password"]);
        assert!(source_app::is_excluded(&conn, "com.agilebits.onepassword").unwrap());
        assert_eq!(settings::get(&conn, "theme"), "dark");
    }
}
