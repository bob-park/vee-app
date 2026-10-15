-- Thumbnails of image files inside a files clip, keyed by the file's position.
CREATE TABLE clip_thumbs (
  clip_id INTEGER NOT NULL,
  idx     INTEGER NOT NULL,
  png     BLOB NOT NULL,
  PRIMARY KEY (clip_id, idx)
);
CREATE TRIGGER clip_thumbs_ad AFTER DELETE ON clips BEGIN
  DELETE FROM clip_thumbs WHERE clip_id = old.id;
END;
