use serde::Serialize;
use std::path::PathBuf;

/// How many files of a files clip get a layer in the card's preview stack.
pub const STACK_LAYERS: usize = 3;

/// How many clips can be pinned at once.
pub const MAX_PINNED: i64 = 3;

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

/// A single URL (no whitespace inside) is a link; anything else is text.
pub fn classify_text(text: &str) -> Kind {
    let t = text.trim();
    let is_url = ["http://", "https://", "ssh://", "git@"].iter().any(|p| t.starts_with(p));
    if is_url && !t.chars().any(char::is_whitespace) { Kind::Link } else { Kind::Text }
}

pub enum NewClip {
    Text(String),
    Image {
        png: Vec<u8>,
        thumb_png: Vec<u8>,
        width: u32,
        height: u32,
    },
    /// `thumbs` holds `(file index, PNG)` for image files among the first `STACK_LAYERS`.
    Files {
        paths: Vec<String>,
        thumbs: Vec<(usize, Vec<u8>)>,
    },
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
    pub pinned: bool,
    /// Preview layers of a files clip; empty when none of its files has a thumbnail.
    pub stack: Vec<Option<String>>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct StatsDto {
    pub count: i64,
    pub pinned: i64,
    pub image_bytes: u64,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn classifies_links() {
        assert_eq!(classify_text("https://example.com/a?b=1"), Kind::Link);
        assert_eq!(classify_text("  git@github.com:bob-park/vee-app.git\n"), Kind::Link);
        assert_eq!(classify_text("ssh://host/repo"), Kind::Link);
        assert_eq!(classify_text("see https://example.com"), Kind::Text);
        assert_eq!(classify_text("https://a.com\nhttps://b.com"), Kind::Text);
        assert_eq!(classify_text("hello"), Kind::Text);
    }
}
