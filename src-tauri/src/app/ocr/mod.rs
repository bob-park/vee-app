//! Text in images via the OS OCR engine: Vision on macOS, `Windows.Media.Ocr` on Windows,
//! and the background worker that fills `ocr_text` for image and files clips.

use crate::{
    app::{
        state::AppState,
        watcher::{MAX_IMAGE_BYTES, is_image_path},
    },
    domain::{
        clip::{self, ClipContent},
        settings,
    },
    shared::error::AppResult,
};
use rusqlite::Connection;
use std::path::{Path, PathBuf};
use std::sync::mpsc::{Sender, channel};
use std::sync::{Mutex, OnceLock};
use std::time::Duration;
use tauri::{AppHandle, Manager};

#[cfg(target_os = "macos")]
mod macos;
#[cfg(windows)]
mod windows;

/// The OS has no usable OCR engine (Windows: no OCR language installed; other OSes: always).
#[derive(Debug, PartialEq, Eq)]
pub struct Unavailable;

/// Recognized lines joined with `\n`. `Ok("")` when the image has no text or can't be read.
pub fn recognize(path: &Path) -> Result<String, Unavailable> {
    #[cfg(target_os = "macos")]
    return macos::recognize(path);
    #[cfg(windows)]
    return windows::recognize(path);
    #[cfg(not(any(target_os = "macos", windows)))]
    {
        let _ = path;
        Err(Unavailable)
    }
}

/// Rest after each recognized image so a long backfill doesn't hog a core.
const PAUSE: Duration = Duration::from_millis(200);

static WAKE: OnceLock<Sender<()>> = OnceLock::new();

#[derive(Debug, PartialEq, Eq)]
enum Step {
    /// Nothing to do (no pending clip or OCR off): wait for a wake.
    Idle,
    /// One clip handled; look for the next.
    Done,
    /// No OCR engine: give up until the next launch.
    Stop,
}

/// Image files worth OCR in a clip: the stored PNG, or the image files among copied paths.
fn image_paths(content: ClipContent) -> Vec<PathBuf> {
    match content {
        ClipContent::Image(path) => vec![path],
        ClipContent::Files(lines) => lines
            .into_iter()
            .filter(|p| is_image_path(p))
            .filter(|p| std::fs::metadata(p).is_ok_and(|m| m.is_file() && m.len() <= MAX_IMAGE_BYTES as u64))
            .map(PathBuf::from)
            .collect(),
        ClipContent::Text(_) => Vec::new(),
    }
}

/// OCRs the newest pending clip. The DB lock is released while `recognize` runs.
fn worker_step(db: &Mutex<Connection>, recognize: impl Fn(&Path) -> Result<String, Unavailable>) -> AppResult<Step> {
    let (id, content) = {
        let conn = db.lock().unwrap();
        if settings::get(&conn, "ocr") != "on" {
            return Ok(Step::Idle);
        }
        match clip::next_ocr_pending(&conn)? {
            Some(pending) => pending,
            None => return Ok(Step::Idle),
        }
    };
    let mut texts = Vec::new();
    for path in image_paths(content) {
        let Ok(text) = recognize(&path) else { return Ok(Step::Stop) };
        if !text.is_empty() {
            texts.push(text);
        }
    }
    clip::set_ocr_text(&db.lock().unwrap(), id, &texts.join("\n"))?;
    Ok(Step::Done)
}

/// Tells the worker to look for pending clips. A no-op before `spawn`.
pub fn wake() {
    if let Some(tx) = WAKE.get() {
        let _ = tx.send(());
    }
}

/// Starts the worker thread: it OCRs every pending clip, newest first, then sleeps until woken.
pub fn spawn(app: AppHandle) {
    let (tx, rx) = channel();
    if WAKE.set(tx).is_err() {
        return;
    }
    std::thread::spawn(move || {
        let state = app.state::<AppState>();
        let paced = |path: &Path| {
            let result = recognize(path);
            std::thread::sleep(PAUSE);
            result
        };
        loop {
            // No run loop on this thread, so drain autoreleased Vision objects per clip.
            #[cfg(target_os = "macos")]
            let step = objc2::rc::autoreleasepool(|_| worker_step(&state.db, paced));
            #[cfg(not(target_os = "macos"))]
            let step = worker_step(&state.db, paced);
            match step {
                Ok(Step::Done) => continue,
                Ok(Step::Stop) => return log::warn!("OCR unavailable; image text won't be searchable this run"),
                Ok(Step::Idle) => {}
                Err(e) => log::warn!("OCR step failed: {e}"),
            }
            if rx.recv().is_err() {
                return;
            }
        }
    });
}

#[cfg(all(test, any(target_os = "macos", windows)))]
mod tests {
    use super::*;
    use std::path::PathBuf;
    use std::time::Instant;

    fn fixture() -> PathBuf {
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("src/app/ocr/fixtures/ko_en.png")
    }

    #[test]
    fn reads_korean_and_english_from_a_rendered_image() {
        let text = recognize(&fixture()).unwrap();
        assert!(text.contains("Clipboard"), "{text:?}");
        // Windows needs a Korean OCR language installed; only assert it where it is built in.
        #[cfg(target_os = "macos")]
        assert!(text.contains("클립보드"), "{text:?}");
    }

    #[test]
    fn corrupt_image_is_empty_not_error() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("x.png");
        std::fs::write(&path, "not an image").unwrap();
        assert_eq!(recognize(&path), Ok(String::new()));
    }

    #[test]
    fn missing_file_is_empty_not_error() {
        let dir = tempfile::tempdir().unwrap();
        assert_eq!(recognize(&dir.path().join("gone.png")), Ok(String::new()));
    }

    fn is_korean(word: &str) -> bool {
        word.chars().any(|c| ('\u{AC00}'..='\u{D7A3}').contains(&c))
    }

    /// Accuracy and speed on a folder of real screenshots. `VEE_OCR_BENCH_DIR` holds the images
    /// and `expected.tsv` (`<file name>\t<word>,<word>,…`).
    #[test]
    #[ignore]
    fn bench() {
        let dir = PathBuf::from(std::env::var("VEE_OCR_BENCH_DIR").expect("set VEE_OCR_BENCH_DIR"));
        let tsv = std::fs::read_to_string(dir.join("expected.tsv")).expect("expected.tsv");
        // (hits, total) for Korean words and for the rest.
        let (mut ko, mut other) = ((0, 0), (0, 0));
        let mut times = Vec::new();
        for line in tsv.lines().filter(|l| !l.trim().is_empty()) {
            let (file, words) = line.split_once('\t').unwrap_or((line, ""));
            let start = Instant::now();
            let text = recognize(&dir.join(file)).expect("OCR unavailable").to_lowercase();
            let ms = start.elapsed().as_millis();
            times.push(ms);
            let mut missed = Vec::new();
            for word in words.split(',').map(str::trim).filter(|w| !w.is_empty()) {
                let hit = text.contains(&word.to_lowercase());
                let bucket = if is_korean(word) { &mut ko } else { &mut other };
                bucket.1 += 1;
                if hit {
                    bucket.0 += 1;
                } else {
                    missed.push(word);
                }
            }
            println!("{file}\t{ms} ms\tmissed: {missed:?}");
        }
        let rate = |(hit, total): (u32, u32)| if total == 0 { 0.0 } else { 100.0 * hit as f64 / total as f64 };
        let mean = times.iter().sum::<u128>() as f64 / times.len().max(1) as f64;
        println!("korean words: {}/{} ({:.1}%)", ko.0, ko.1, rate(ko));
        println!("other words: {}/{} ({:.1}%)", other.0, other.1, rate(other));
        println!("mean {mean:.0} ms, max {} ms", times.iter().max().unwrap_or(&0));
    }
}

#[cfg(test)]
mod worker_tests {
    use super::*;
    use crate::domain::{clip::NewClip, settings};
    use crate::shared::db;
    use rusqlite::Connection;
    use std::cell::Cell;
    use std::sync::Mutex;

    fn setup() -> (Mutex<Connection>, tempfile::TempDir) {
        (Mutex::new(db::open_in_memory().unwrap()), tempfile::tempdir().unwrap())
    }

    fn image(byte: u8) -> NewClip {
        NewClip::Image { png: vec![byte; 8], thumb_png: vec![9], width: 10, height: 20 }
    }

    fn ocr_text(db: &Mutex<Connection>, id: i64) -> Option<String> {
        db.lock().unwrap().query_row("SELECT ocr_text FROM clips WHERE id = ?1", [id], |r| r.get(0)).unwrap()
    }

    #[test]
    fn step_writes_text_for_newest_image() {
        let (db, dir) = setup();
        let (old, new) = {
            let c = db.lock().unwrap();
            (clip::upsert(&c, dir.path(), image(1), None, 1).unwrap(), clip::upsert(&c, dir.path(), image(2), None, 2).unwrap())
        };
        assert_eq!(worker_step(&db, |_| Ok("hello".into())).unwrap(), Step::Done);
        assert_eq!(ocr_text(&db, new).as_deref(), Some("hello"));
        assert_eq!(ocr_text(&db, old), None);
    }

    #[test]
    fn step_ocrs_every_image_file_in_a_files_clip() {
        let (db, dir) = setup();
        let mut paths: Vec<String> = (0..5)
            .map(|i| {
                let p = dir.path().join(format!("{i}.png"));
                std::fs::write(&p, "x").unwrap();
                p.to_string_lossy().into_owned()
            })
            .collect();
        let pdf = dir.path().join("doc.pdf");
        std::fs::write(&pdf, "x").unwrap();
        paths.insert(2, pdf.to_string_lossy().into_owned());
        let id = clip::upsert(&db.lock().unwrap(), dir.path(), NewClip::Files { paths, thumbs: vec![] }, None, 1).unwrap();
        let calls = Cell::new(0);
        let step = worker_step(&db, |p| {
            calls.set(calls.get() + 1);
            Ok(p.file_name().unwrap().to_string_lossy().into_owned())
        });
        assert_eq!(step.unwrap(), Step::Done);
        assert_eq!(calls.get(), 5);
        assert_eq!(ocr_text(&db, id).as_deref(), Some("0.png\n1.png\n2.png\n3.png\n4.png"));
    }

    #[test]
    fn files_clip_without_images_is_marked_done_without_ocr() {
        let (db, dir) = setup();
        let files = NewClip::Files { paths: vec!["/a/report.pdf".into(), "/a/missing.png".into()], thumbs: vec![] };
        let id = clip::upsert(&db.lock().unwrap(), dir.path(), files, None, 1).unwrap();
        assert_eq!(worker_step(&db, |_| panic!("no image to OCR")).unwrap(), Step::Done);
        assert_eq!(ocr_text(&db, id).as_deref(), Some(""));
    }

    #[test]
    fn step_is_idle_when_setting_off() {
        let (db, dir) = setup();
        let id = {
            let c = db.lock().unwrap();
            settings::set(&c, "ocr", "off").unwrap();
            clip::upsert(&c, dir.path(), image(1), None, 1).unwrap()
        };
        assert_eq!(worker_step(&db, |_| panic!("OCR is off")).unwrap(), Step::Idle);
        assert_eq!(ocr_text(&db, id), None);
    }

    #[test]
    fn step_is_idle_when_nothing_is_pending() {
        let (db, _dir) = setup();
        assert_eq!(worker_step(&db, |_| panic!("nothing pending")).unwrap(), Step::Idle);
    }

    #[test]
    fn unavailable_engine_stops_and_leaves_pending() {
        let (db, dir) = setup();
        let id = clip::upsert(&db.lock().unwrap(), dir.path(), image(1), None, 1).unwrap();
        assert_eq!(worker_step(&db, |_| Err(Unavailable)).unwrap(), Step::Stop);
        assert_eq!(ocr_text(&db, id), None);
    }
}
