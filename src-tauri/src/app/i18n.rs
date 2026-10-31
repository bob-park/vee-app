//! Backend-only labels (tray menu, native dialogs) and locale resolution.

pub struct Labels {
    pub open: &'static str,
    pub settings: &'static str,
    pub check_update: &'static str,
    pub restart_update: &'static str,
    pub quit: &'static str,
}

pub fn labels(locale: &str) -> Labels {
    if locale == "ko" {
        Labels {
            open: "열기", settings: "설정", check_update: "업데이트 확인", restart_update: "업데이트 후 재시작", quit: "종료"
        }
    } else {
        Labels { open: "Open", settings: "Settings", check_update: "Check for Updates", restart_update: "Restart to Update", quit: "Quit" }
    }
}

/// Same rule as the frontend: "system" follows the OS language.
pub fn resolve_locale(setting: &str) -> &'static str {
    match setting {
        "ko" => "ko",
        "en" => "en",
        _ if sys_locale::get_locale().is_some_and(|l| l.to_lowercase().starts_with("ko")) => "ko",
        _ => "en",
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn explicit_locale_wins_over_system() {
        assert_eq!(resolve_locale("ko"), "ko");
        assert_eq!(resolve_locale("en"), "en");
        assert!(["ko", "en"].contains(&resolve_locale("system")));
        assert_eq!(labels("ko").quit, "종료");
        assert_eq!(labels("en").quit, "Quit");
    }
}
