//! `Windows.Media.Ocr` text recognition.

use std::path::Path;

use ::windows::Graphics::Imaging::BitmapDecoder;
use ::windows::Media::Ocr::OcrEngine;
use ::windows::Storage::{FileAccessMode, StorageFile};
use ::windows::core::{HSTRING, Result};

pub fn recognize(path: &Path) -> std::result::Result<String, super::Unavailable> {
    let engine = engine().ok_or(super::Unavailable)?;
    // Missing, unreadable, or non-image files fail here; they just have no text.
    Ok(read(&engine, path).unwrap_or_default())
}

/// The user's profile languages first; else an installed Korean language, else any installed one.
fn engine() -> Option<OcrEngine> {
    if let Ok(engine) = OcrEngine::TryCreateFromUserProfileLanguages() {
        return Some(engine);
    }
    let languages: Vec<_> = OcrEngine::AvailableRecognizerLanguages().ok()?.into_iter().collect();
    let korean = languages.iter().find(|l| l.LanguageTag().is_ok_and(|t| t.to_string().starts_with("ko")));
    OcrEngine::TryCreateFromLanguage(korean.or(languages.first())?).ok()
}

fn read(engine: &OcrEngine, path: &Path) -> Result<String> {
    let file = StorageFile::GetFileFromPathAsync(&HSTRING::from(path))?.join()?;
    let stream = file.OpenAsync(FileAccessMode::Read)?.join()?;
    let decoder = BitmapDecoder::CreateAsync(&stream)?.join()?;
    // ponytail: oversized images are skipped, scale them down if large screenshots matter.
    let max = OcrEngine::MaxImageDimension()?;
    if decoder.PixelWidth()? > max || decoder.PixelHeight()? > max {
        return Ok(String::new());
    }
    let bitmap = decoder.GetSoftwareBitmapAsync()?.join()?;
    let result = engine.RecognizeAsync(&bitmap)?.join()?;
    let lines: Vec<String> = result.Lines()?.into_iter().filter_map(|l| l.Text().ok()).map(|t| t.to_string()).collect();
    Ok(lines.join("\n"))
}
