-- Every pasteboard representation of a clip, in the order the source app offered them,
-- so a copy back pastes exactly like the original (e.g. Excel cells stay cells).
CREATE TABLE clip_formats (
  clip_id INTEGER NOT NULL,
  format  TEXT NOT NULL,
  data    BLOB NOT NULL,
  PRIMARY KEY (clip_id, format)
);
CREATE TRIGGER clip_formats_ad AFTER DELETE ON clips BEGIN
  DELETE FROM clip_formats WHERE clip_id = old.id;
END;
