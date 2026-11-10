use rusqlite::{params, Connection};
use serde::Serialize;
use sha2::{Digest, Sha256};
use std::path::Path;

/// Default number of unpinned items kept in history.
pub const DEFAULT_LIMIT: i64 = 200;

#[derive(Debug, Serialize, Clone)]
pub struct ClipItem {
    pub id: i64,
    pub kind: String,
    pub content: Option<String>,
    pub image_path: Option<String>,
    pub pinned: bool,
    pub category: Option<String>,
    pub created_at: i64,
}

pub fn hash_text(text: &str) -> String {
    let mut h = Sha256::new();
    h.update(text.as_bytes());
    format!("{:x}", h.finalize())
}

pub fn now_ms() -> i64 {
    use std::time::{SystemTime, UNIX_EPOCH};
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis() as i64)
        .unwrap_or(0)
}

pub fn open(path: &Path) -> rusqlite::Result<Connection> {
    let conn = Connection::open(path)?;
    conn.pragma_update(None, "journal_mode", "WAL")?;
    init(&conn)?;
    Ok(conn)
}

pub fn init(conn: &Connection) -> rusqlite::Result<()> {
    conn.execute_batch(
        r#"
        CREATE TABLE IF NOT EXISTS clip_item (
          id          INTEGER PRIMARY KEY AUTOINCREMENT,
          kind        TEXT NOT NULL,
          content     TEXT,
          image_path  TEXT,
          hash        TEXT NOT NULL,
          pinned      INTEGER NOT NULL DEFAULT 0,
          category    TEXT,
          created_at  INTEGER NOT NULL
        );
        CREATE INDEX IF NOT EXISTS idx_clip_created ON clip_item(created_at DESC);
        CREATE UNIQUE INDEX IF NOT EXISTS idx_clip_hash ON clip_item(hash);

        CREATE TABLE IF NOT EXISTS snippet (
          id    INTEGER PRIMARY KEY AUTOINCREMENT,
          title TEXT NOT NULL,
          body  TEXT NOT NULL
        );

        CREATE TABLE IF NOT EXISTS setting (
          key   TEXT PRIMARY KEY,
          value TEXT
        );
        "#,
    )
}

fn row_to_item(row: &rusqlite::Row) -> rusqlite::Result<ClipItem> {
    Ok(ClipItem {
        id: row.get(0)?,
        kind: row.get(1)?,
        content: row.get(2)?,
        image_path: row.get(3)?,
        pinned: row.get::<_, i64>(4)? != 0,
        category: row.get(5)?,
        created_at: row.get(6)?,
    })
}

const SELECT: &str =
    "SELECT id, kind, content, image_path, pinned, category, created_at FROM clip_item";

/// Insert a new clip. If the same content was copied before, the existing row is
/// moved back to the top instead of failing on the unique hash index.
pub fn insert(
    conn: &Connection,
    kind: &str,
    content: Option<&str>,
    image_path: Option<&str>,
    hash: &str,
) -> rusqlite::Result<i64> {
    conn.execute(
        "INSERT INTO clip_item (kind, content, image_path, hash, created_at)
         VALUES (?1, ?2, ?3, ?4, ?5)
         ON CONFLICT(hash) DO UPDATE SET created_at = excluded.created_at",
        params![kind, content, image_path, hash, now_ms()],
    )?;
    conn.query_row(
        "SELECT id FROM clip_item WHERE hash = ?1",
        params![hash],
        |r| r.get(0),
    )
}

pub fn list(conn: &Connection, limit: i64) -> rusqlite::Result<Vec<ClipItem>> {
    let sql = format!("{SELECT} ORDER BY pinned DESC, created_at DESC LIMIT ?1");
    let mut stmt = conn.prepare(&sql)?;
    let rows = stmt.query_map(params![limit], row_to_item)?;
    rows.collect()
}

pub fn search(conn: &Connection, query: &str, limit: i64) -> rusqlite::Result<Vec<ClipItem>> {
    let sql = format!(
        "{SELECT} WHERE content LIKE ?1 ORDER BY pinned DESC, created_at DESC LIMIT ?2"
    );
    let mut stmt = conn.prepare(&sql)?;
    let like = format!("%{query}%");
    let rows = stmt.query_map(params![like, limit], row_to_item)?;
    rows.collect()
}

pub fn get(conn: &Connection, id: i64) -> rusqlite::Result<ClipItem> {
    let sql = format!("{SELECT} WHERE id = ?1");
    conn.query_row(&sql, params![id], row_to_item)
}

pub fn toggle_pin(conn: &Connection, id: i64) -> rusqlite::Result<bool> {
    conn.execute(
        "UPDATE clip_item SET pinned = CASE pinned WHEN 0 THEN 1 ELSE 0 END WHERE id = ?1",
        params![id],
    )?;
    conn.query_row(
        "SELECT pinned FROM clip_item WHERE id = ?1",
        params![id],
        |r| Ok(r.get::<_, i64>(0)? != 0),
    )
}

/// Deletes one item. Returns the image path it owned, if any, so the caller can
/// remove the file from disk.
pub fn delete(conn: &Connection, id: i64) -> rusqlite::Result<Option<String>> {
    let path: Option<String> = conn
        .query_row(
            "SELECT image_path FROM clip_item WHERE id = ?1",
            params![id],
            |r| r.get(0),
        )
        .unwrap_or(None);
    conn.execute("DELETE FROM clip_item WHERE id = ?1", params![id])?;
    Ok(path)
}

/// Clears every unpinned item. Returns the image paths that were removed.
pub fn clear_unpinned(conn: &Connection) -> rusqlite::Result<Vec<String>> {
    let paths = {
        let mut stmt =
            conn.prepare("SELECT image_path FROM clip_item WHERE pinned = 0 AND image_path IS NOT NULL")?;
        let rows = stmt.query_map([], |r| r.get::<_, String>(0))?;
        rows.collect::<rusqlite::Result<Vec<String>>>()?
    };
    conn.execute("DELETE FROM clip_item WHERE pinned = 0", [])?;
    Ok(paths)
}

/// Drops the oldest unpinned items past `limit`. Returns their image paths.
pub fn trim(conn: &Connection, limit: i64) -> rusqlite::Result<Vec<String>> {
    let paths = {
        let mut stmt = conn.prepare(
            "SELECT image_path FROM clip_item
             WHERE pinned = 0 AND image_path IS NOT NULL
               AND id NOT IN (
                 SELECT id FROM clip_item WHERE pinned = 0 ORDER BY created_at DESC LIMIT ?1
               )",
        )?;
        let rows = stmt.query_map(params![limit], |r| r.get::<_, String>(0))?;
        rows.collect::<rusqlite::Result<Vec<String>>>()?
    };
    conn.execute(
        "DELETE FROM clip_item
         WHERE pinned = 0 AND id NOT IN (
           SELECT id FROM clip_item WHERE pinned = 0 ORDER BY created_at DESC LIMIT ?1
         )",
        params![limit],
    )?;
    Ok(paths)
}

pub fn get_setting(conn: &Connection, key: &str) -> Option<String> {
    conn.query_row(
        "SELECT value FROM setting WHERE key = ?1",
        params![key],
        |r| r.get(0),
    )
    .ok()
}

pub fn set_setting(conn: &Connection, key: &str, value: &str) -> rusqlite::Result<()> {
    conn.execute(
        "INSERT INTO setting (key, value) VALUES (?1, ?2)
         ON CONFLICT(key) DO UPDATE SET value = excluded.value",
        params![key, value],
    )?;
    Ok(())
}

pub fn history_limit(conn: &Connection) -> i64 {
    get_setting(conn, "history_limit")
        .and_then(|v| v.parse().ok())
        .unwrap_or(DEFAULT_LIMIT)
}
