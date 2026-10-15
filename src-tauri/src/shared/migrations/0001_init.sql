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
