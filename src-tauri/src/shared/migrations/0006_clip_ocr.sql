-- OCR text for image and files clips, searched alongside `text`.
-- NULL = OCR pending (image/files only); text and link clips have nothing to recognize.
ALTER TABLE clips ADD COLUMN ocr_text TEXT;
UPDATE clips SET ocr_text = '' WHERE kind IN ('text','link');
DROP TRIGGER clips_ai;
DROP TRIGGER clips_ad;
DROP TRIGGER clips_au;
DROP TABLE clips_fts;
CREATE VIRTUAL TABLE clips_fts USING fts5(text, ocr_text, content='clips', content_rowid='id', tokenize='trigram');
CREATE TRIGGER clips_ai AFTER INSERT ON clips BEGIN
  INSERT INTO clips_fts(rowid, text, ocr_text) VALUES (new.id, new.text, new.ocr_text);
END;
CREATE TRIGGER clips_ad AFTER DELETE ON clips BEGIN
  INSERT INTO clips_fts(clips_fts, rowid, text, ocr_text) VALUES ('delete', old.id, old.text, old.ocr_text);
END;
CREATE TRIGGER clips_au AFTER UPDATE OF text, ocr_text ON clips BEGIN
  INSERT INTO clips_fts(clips_fts, rowid, text, ocr_text) VALUES ('delete', old.id, old.text, old.ocr_text);
  INSERT INTO clips_fts(rowid, text, ocr_text) VALUES (new.id, new.text, new.ocr_text);
END;
INSERT INTO clips_fts(clips_fts) VALUES ('rebuild');
