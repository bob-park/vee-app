//! SQL for the `settings` table only.

use crate::shared::error::AppResult;
use rusqlite::{Connection, OptionalExtension, params};

pub fn get(conn: &Connection, key: &str) -> AppResult<Option<String>> {
    Ok(conn.query_row("SELECT value FROM settings WHERE key = ?1", [key], |r| r.get(0)).optional()?)
}

pub fn set(conn: &Connection, key: &str, value: &str) -> AppResult<()> {
    conn.execute(
        "INSERT INTO settings (key, value) VALUES (?1, ?2)
         ON CONFLICT(key) DO UPDATE SET value = excluded.value",
        params![key, value],
    )?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::shared::db;

    #[test]
    fn settings_round_trip() {
        let conn = db::open_in_memory().unwrap();
        assert_eq!(get(&conn, "theme").unwrap(), None);
        set(&conn, "theme", "dark").unwrap();
        set(&conn, "theme", "light").unwrap();
        assert_eq!(get(&conn, "theme").unwrap().as_deref(), Some("light"));
    }
}
