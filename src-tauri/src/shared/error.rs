//! The single error type returned by commands. Serialized as `{ code, message }`.

use serde::{Serialize, Serializer, ser::SerializeStruct};

/// Display strings are bare so `message` keeps the text commands returned before `AppError`.
#[derive(Debug, thiserror::Error)]
pub enum AppError {
    #[error("{0}")]
    InvalidInput(String),
    #[error("{0}")]
    Db(#[from] rusqlite::Error),
    #[error("{0}")]
    Io(#[from] std::io::Error),
    #[error("pin_limit")]
    PinLimit,
    #[error("resize_off")]
    ResizeOff,
    /// Plugin or OS errors; `shared` must not import `tauri`, so callers pass `e.to_string()`.
    #[error("{0}")]
    Other(String),
}

impl AppError {
    pub fn code(&self) -> &'static str {
        match self {
            AppError::InvalidInput(_) => "INVALID_INPUT",
            AppError::Db(_) => "DB",
            AppError::Io(_) => "IO",
            AppError::PinLimit => "PIN_LIMIT",
            AppError::ResizeOff => "RESIZE_OFF",
            AppError::Other(_) => "OTHER",
        }
    }
}

impl Serialize for AppError {
    fn serialize<S: Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        let mut st = s.serialize_struct("AppError", 2)?;
        st.serialize_field("code", self.code())?;
        st.serialize_field("message", &self.to_string())?;
        st.end()
    }
}

pub type AppResult<T> = Result<T, AppError>;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn serializes_code_and_message() {
        let json = serde_json::to_value(AppError::PinLimit).unwrap();
        assert_eq!(json, serde_json::json!({ "code": "PIN_LIMIT", "message": "pin_limit" }));

        let json = serde_json::to_value(AppError::InvalidInput("invalid setting a=b".into())).unwrap();
        assert_eq!(json["code"], "INVALID_INPUT");
        assert_eq!(json["message"], "invalid setting a=b");
    }
}
