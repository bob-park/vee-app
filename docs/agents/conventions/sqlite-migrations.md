---
title: SQLite Migrations
scope: src-tauri/src/shared/**, src-tauri/src/domain/*/repository.rs
applies_to: adding or changing a table, column, index, or seed row
related:
  - ./rust-modules.md
  - ../testing.md
---

# SQLite Migrations

> One SQL file per schema version in `src-tauri/src/shared/migrations/` (`0001`–`0005` today), appended to `MIGRATIONS` in `shared/db.rs`. `PRAGMA user_version` records how many have run. Never edit a migration that has shipped. A DB from a **newer** app still opens.

## Adding a migration

1. Create `src-tauri/src/shared/migrations/000N_<what>.sql`, where N is the next number. Example: `0006_clip_notes.sql`.
2. Append it to `MIGRATIONS`:

   ```rust
   const MIGRATIONS: &[&str] = &[
       include_str!("migrations/0001_init.sql"),
       // …
       include_str!("migrations/0005_pin_order.sql"),
       include_str!("migrations/0006_clip_notes.sql"),
   ];
   ```

3. `db::migrate` runs each pending file in its own transaction and then sets `user_version = index + 1`.

## Newer databases (downgrade-safe)

If the DB's `user_version` is **higher** than `MIGRATIONS.len()` (an older build reinstalled over newer data), `migrate` leaves it untouched and the app **opens it anyway**. There is no `SchemaTooNew` error. So a migration must keep the schema readable by older builds:

- Add tables and columns (with defaults); don't drop, rename or retype what older builds read.
- `opens_a_database_from_a_newer_app` in `shared/db.rs` guards this.

## Rules

- **Never edit, reorder, or delete a shipped file.** Installed apps have already recorded that version. To fix a mistake, add a new migration.
- Don't put `BEGIN`/`COMMIT` or `PRAGMA user_version` inside the SQL file; `migrate` handles both.
- Seed defaults in the migration only when every install needs the row. Otherwise, fall back in code. For example, `settings::get` returns a per-key default when the row is missing.
- Readers must tolerate unknown or old values instead of failing: `resolveLocale` and `i18n::resolve_locale` treat anything but `ko`/`en` as `system`; `cutoff_for` treats a non-number retention as "keep everything".
- A destructive change (dropping or renaming a column, deleting rows) needs the user's confirmation first, because it touches real user history. If the DB can't be opened, the app shows an error and quits — it never wipes the history.

## Tests

- Use `db::open_in_memory()` (migrated, `#[cfg(test)]` only) for domain tests.
- Use `tempfile::tempdir()` with `db::open(path)` when the test is about files: first launch, reopen, parent dir creation.
- Every new migration needs a test that opens a DB at the previous version and checks the new shape and that user data survived. Update the expected version in `fresh_database_reaches_version_5` / `upgrades_from_each_shipped_version`.
- `legacy_schema` in `shared/db.rs` is the v1–v5 SQL exactly as it shipped inline before migrations moved to files. Tests use it to prove the files reproduce the shipped schema and to build old databases. Don't change it.
