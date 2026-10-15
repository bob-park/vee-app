//! SQL for the `apps` table: the apps clips were copied from.

use super::model::AppDto;
use crate::shared::{db::escape_like, error::AppResult, png::data_url};
use rusqlite::{Connection, OptionalExtension, params};

pub fn app_known(conn: &Connection, bundle_id: &str) -> AppResult<bool> {
    let found = conn.query_row("SELECT 1 FROM apps WHERE bundle_id = ?1", [bundle_id], |_| Ok(())).optional()?;
    Ok(found.is_some())
}

pub fn upsert_app(conn: &Connection, bundle_id: &str, name: &str, icon_png: Option<&[u8]>) -> AppResult<i64> {
    Ok(conn.query_row(
        "INSERT INTO apps (bundle_id, name, icon_png) VALUES (?1, ?2, ?3)
         ON CONFLICT(bundle_id) DO UPDATE SET
           name = excluded.name,
           icon_png = COALESCE(excluded.icon_png, apps.icon_png)
         RETURNING id",
        params![bundle_id, name, icon_png],
        |r| r.get(0),
    )?)
}

/// Unknown apps are not excluded.
pub fn is_excluded(conn: &Connection, bundle_id: &str) -> AppResult<bool> {
    let excluded: Option<bool> =
        conn.query_row("SELECT excluded FROM apps WHERE bundle_id = ?1", [bundle_id], |r| r.get(0)).optional()?;
    Ok(excluded.unwrap_or(false))
}

/// Only flips the flag; deleting the app's clips is the domain API's job.
pub fn set_excluded(conn: &Connection, app_id: i64, excluded: bool) -> AppResult<()> {
    conn.execute("UPDATE apps SET excluded = ?1 WHERE id = ?2", params![excluded, app_id])?;
    Ok(())
}

pub fn excluded_apps(conn: &Connection) -> AppResult<Vec<AppDto>> {
    apps_with_counts(conn, "a.excluded = 1", "a.name")
}

/// Every app not excluded, including ones whose clips are all gone, most clips first.
pub fn known_apps(conn: &Connection) -> AppResult<Vec<AppDto>> {
    apps_with_counts(conn, "a.excluded = 0", "COUNT(c.id) DESC, a.name")
}

fn apps_with_counts(conn: &Connection, cond: &str, order: &str) -> AppResult<Vec<AppDto>> {
    let mut stmt = conn.prepare(&format!(
        "SELECT a.id, a.name, a.icon_png, COUNT(c.id)
         FROM apps a LEFT JOIN clips c ON c.app_id = a.id
         WHERE {cond}
         GROUP BY a.id
         ORDER BY {order}"
    ))?;
    let rows = stmt.query_map([], |r| {
        Ok(AppDto {
            id: r.get(0)?,
            name: r.get(1)?,
            icon: r.get::<_, Option<Vec<u8>>>(2)?.map(|b| data_url(&b)),
            count: r.get(3)?,
        })
    })?;
    Ok(rows.collect::<rusqlite::Result<_>>()?)
}

/// Apps that have clips, whose name contains `query` (ASCII case-insensitive), most clips first.
pub fn apps(conn: &Connection, query: &str, limit: i64) -> AppResult<Vec<AppDto>> {
    let mut stmt = conn.prepare(
        "SELECT a.id, a.name, a.icon_png, COUNT(c.id) AS n
         FROM apps a JOIN clips c ON c.app_id = a.id
         WHERE a.name LIKE ?1 ESCAPE '\\'
         GROUP BY a.id
         ORDER BY n DESC, a.name ASC
         LIMIT ?2",
    )?;
    let pattern = format!("%{}%", escape_like(query.trim()));
    let rows = stmt.query_map(params![pattern, limit], |r| {
        Ok(AppDto {
            id: r.get(0)?,
            name: r.get(1)?,
            icon: r.get::<_, Option<Vec<u8>>>(2)?.map(|b| data_url(&b)),
            count: r.get(3)?,
        })
    })?;
    Ok(rows.collect::<rusqlite::Result<_>>()?)
}
