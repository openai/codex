use anyhow::Result;
use rusqlite::Connection;
use rusqlite::params;
use serde::Deserialize;
use serde::Serialize;
use std::path::PathBuf;
use std::sync::Mutex;
use tracing::info;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FileChange {
    pub id: Option<i64>,
    pub timestamp: String,
    pub file_path: String,
    pub change_type: String,
    pub diff_lines_added: i32,
    pub diff_lines_removed: i32,
}

pub struct Database {
    conn: Mutex<Connection>,
}

impl Database {
    pub async fn new() -> Result<Self> {
        let db_path = Self::get_db_path()?;
        info!("Opening database at: {:?}", db_path);

        let conn = Connection::open(db_path)?;

        // Create tables
        conn.execute(
            "CREATE TABLE IF NOT EXISTS file_changes (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                timestamp DATETIME DEFAULT CURRENT_TIMESTAMP,
                file_path TEXT NOT NULL,
                change_type TEXT NOT NULL,
                diff_lines_added INTEGER DEFAULT 0,
                diff_lines_removed INTEGER DEFAULT 0
            )",
            [],
        )?;

        conn.execute(
            "CREATE INDEX IF NOT EXISTS idx_timestamp ON file_changes(timestamp DESC)",
            [],
        )?;

        info!("Database initialized successfully");
        Ok(Self {
            conn: Mutex::new(conn),
        })
    }

    fn get_db_path() -> Result<PathBuf> {
        let app_data = std::env::var("APPDATA")
            .or_else(|_| std::env::var("HOME"))
            .unwrap_or_else(|_| ".".to_string());

        let codex_dir = PathBuf::from(app_data).join("codex");
        std::fs::create_dir_all(&codex_dir)?;

        Ok(codex_dir.join("codex.db"))
    }

    pub fn insert_change(&self, change: &FileChange) -> Result<i64> {
        let conn = self.conn.lock().unwrap();
        conn.execute(
            "INSERT INTO file_changes (file_path, change_type, diff_lines_added, diff_lines_removed)
             VALUES (?1, ?2, ?3, ?4)",
            params![
                change.file_path,
                change.change_type,
                change.diff_lines_added,
                change.diff_lines_removed
            ],
        )?;

        Ok(conn.last_insert_rowid())
    }

    pub fn get_recent_changes(&self, limit: usize) -> Result<Vec<serde_json::Value>> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn.prepare(
            "SELECT id, timestamp, file_path, change_type, diff_lines_added, diff_lines_removed
             FROM file_changes
             ORDER BY timestamp DESC
             LIMIT ?1",
        )?;

        let changes = stmt
            .query_map(params![limit], |row| {
                Ok(serde_json::json!({
                    "id": row.get::<_, i64>(0)?,
                    "timestamp": row.get::<_, String>(1)?,
                    "file_path": row.get::<_, String>(2)?,
                    "change_type": row.get::<_, String>(3)?,
                    "diff_lines_added": row.get::<_, i32>(4)?,
                    "diff_lines_removed": row.get::<_, i32>(5)?,
                }))
            })?
            .collect::<Result<Vec<_>, _>>()?;

        Ok(changes)
    }

    pub fn get_statistics(&self) -> Result<serde_json::Value> {
        let conn = self.conn.lock().unwrap();
        let total_changes: i64 =
            conn.query_row("SELECT COUNT(*) FROM file_changes", [], |row| row.get(0))?;

        let total_lines_added: i64 = conn.query_row(
            "SELECT COALESCE(SUM(diff_lines_added), 0) FROM file_changes",
            [],
            |row| row.get(0),
        )?;

        let total_lines_removed: i64 = conn.query_row(
            "SELECT COALESCE(SUM(diff_lines_removed), 0) FROM file_changes",
            [],
            |row| row.get(0),
        )?;

        Ok(serde_json::json!({
            "total_changes": total_changes,
            "total_lines_added": total_lines_added,
            "total_lines_removed": total_lines_removed,
        }))
    }
}
