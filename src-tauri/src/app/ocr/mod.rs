//! Text in images via the OS OCR engine: Vision on macOS, `Windows.Media.Ocr` on Windows.

use std::path::Path;

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
