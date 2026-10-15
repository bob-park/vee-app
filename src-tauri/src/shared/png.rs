//! PNG bytes as data URLs for the webview.

use base64::{Engine, engine::general_purpose::STANDARD};

pub fn data_url(png: &[u8]) -> String {
    format!("data:image/png;base64,{}", STANDARD.encode(png))
}
