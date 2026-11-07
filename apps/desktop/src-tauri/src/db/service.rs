use std::path::{Path, PathBuf};
use std::sync::Mutex;
use rusqlite::{params, Connection, Result as SqlResult};
use chrono::Utc;
use crate::db::models::{ChatDetailRecord, ChatSummaryRecord, SaveChatRequest, WorkspaceRecord};

pub struct DatabaseService {
    conn: Mutex<Connection>,
    _db_path: PathBuf,
}

impl DatabaseService {
    pub fn new(db_path: PathBuf) -> SqlResult<Self> {
        if let Some(parent) = db_path.parent() {
            let _ = std::fs::create_dir_all(parent);
        }

        let conn = Connection::open(&db_path)?;

        // Enable WAL mode for concurrent read performance & durability
        conn.execute_batch(
            "PRAGMA journal_mode = WAL;
             PRAGMA synchronous = NORMAL;
             PRAGMA foreign_keys = ON;",
        )?;

        let service = Self {
            conn: Mutex::new(conn),
            _db_path: db_path,
        };

        service.init_schema()?;
        Ok(service)
    }

    pub fn in_memory() -> SqlResult<Self> {
        let conn = Connection::open_in_memory()?;
        let service = Self {
            conn: Mutex::new(conn),
            _db_path: PathBuf::from(":memory:"),
        };
        service.init_schema()?;
        Ok(service)
    }

    fn init_schema(&self) -> SqlResult<()> {
        let conn = self.conn.lock().unwrap();
        conn.execute_batch(
            "CREATE TABLE IF NOT EXISTS workspaces (
                path TEXT PRIMARY KEY,
                name TEXT NOT NULL,
                last_opened_at TEXT NOT NULL
            );

            CREATE TABLE IF NOT EXISTS chats (
                id TEXT PRIMARY KEY,
                workspace_path TEXT NOT NULL,
                title TEXT NOT NULL,
                created_at TEXT NOT NULL,
                updated_at TEXT NOT NULL,
                status TEXT NOT NULL,
                branch_name TEXT,
                worktree_path TEXT,
                plan_json TEXT,
                worker_summary TEXT,
                events_json TEXT,
                verification_json TEXT,
                FOREIGN KEY (workspace_path) REFERENCES workspaces(path) ON DELETE CASCADE
            );

            CREATE INDEX IF NOT EXISTS idx_chats_workspace ON chats(workspace_path);
            CREATE INDEX IF NOT EXISTS idx_chats_updated ON chats(updated_at DESC);",
        )?;
        Ok(())
    }

    pub fn record_workspace(&self, path: &str, name: &str) -> SqlResult<()> {
        let conn = self.conn.lock().unwrap();
        let now = Utc::now().to_rfc3339();
        conn.execute(
            "INSERT INTO workspaces (path, name, last_opened_at)
             VALUES (?1, ?2, ?3)
             ON CONFLICT(path) DO UPDATE SET
                name = excluded.name,
                last_opened_at = excluded.last_opened_at;",
            params![path, name, now],
        )?;
        Ok(())
    }

    pub fn get_recent_workspaces(&self, limit: usize) -> SqlResult<Vec<WorkspaceRecord>> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn.prepare(
            "SELECT path, name, last_opened_at
             FROM workspaces
             ORDER BY last_opened_at DESC
             LIMIT ?1",
        )?;

        let rows = stmt.query_map(params![limit as i64], |row| {
            Ok(WorkspaceRecord {
                path: row.get(0)?,
                name: row.get(1)?,
                last_opened_at: row.get(2)?,
            })
        })?;

        let mut results = Vec::new();
        for r in rows {
            results.push(r?);
        }
        Ok(results)
    }

    pub fn save_chat(&self, req: &SaveChatRequest) -> SqlResult<()> {
        let conn = self.conn.lock().unwrap();
        let now = Utc::now().to_rfc3339();

        // Ensure workspace is recorded first
        let _ = conn.execute(
            "INSERT OR IGNORE INTO workspaces (path, name, last_opened_at)
             VALUES (?1, ?2, ?3);",
            params![
                &req.workspace_path,
                Path::new(&req.workspace_path)
                    .file_name()
                    .and_then(|n| n.to_str())
                    .unwrap_or("workspace"),
                &now
            ],
        );

        conn.execute(
            "INSERT INTO chats (
                id, workspace_path, title, created_at, updated_at, status,
                branch_name, worktree_path, plan_json, worker_summary,
                events_json, verification_json
             ) VALUES (?1, ?2, ?3, ?4, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11)
             ON CONFLICT(id) DO UPDATE SET
                title = excluded.title,
                updated_at = excluded.updated_at,
                status = excluded.status,
                branch_name = COALESCE(excluded.branch_name, chats.branch_name),
                worktree_path = COALESCE(excluded.worktree_path, chats.worktree_path),
                plan_json = COALESCE(excluded.plan_json, chats.plan_json),
                worker_summary = COALESCE(excluded.worker_summary, chats.worker_summary),
                events_json = COALESCE(excluded.events_json, chats.events_json),
                verification_json = COALESCE(excluded.verification_json, chats.verification_json);",
            params![
                req.id,
                req.workspace_path,
                req.title,
                now,
                req.status,
                req.branch_name,
                req.worktree_path,
                req.plan_json,
                req.worker_summary,
                req.events_json,
                req.verification_json,
            ],
        )?;

        Ok(())
    }

    pub fn list_chats_for_workspace(&self, workspace_path: &str) -> SqlResult<Vec<ChatSummaryRecord>> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn.prepare(
            "SELECT id, workspace_path, title, created_at, updated_at, status, branch_name, worktree_path
             FROM chats
             WHERE workspace_path = ?1
             ORDER BY updated_at DESC",
        )?;

        let rows = stmt.query_map(params![workspace_path], |row| {
            Ok(ChatSummaryRecord {
                id: row.get(0)?,
                workspace_path: row.get(1)?,
                title: row.get(2)?,
                created_at: row.get(3)?,
                updated_at: row.get(4)?,
                status: row.get(5)?,
                branch_name: row.get(6)?,
                worktree_path: row.get(7)?,
                files_count: 0,
            })
        })?;

        let mut results = Vec::new();
        for r in rows {
            results.push(r?);
        }
        Ok(results)
    }

    pub fn get_chat(&self, task_id: &str) -> SqlResult<Option<ChatDetailRecord>> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn.prepare(
            "SELECT id, workspace_path, title, created_at, updated_at, status,
                    branch_name, worktree_path, plan_json, worker_summary,
                    events_json, verification_json
             FROM chats
             WHERE id = ?1",
        )?;

        let mut rows = stmt.query(params![task_id])?;
        if let Some(row) = rows.next()? {
            Ok(Some(ChatDetailRecord {
                id: row.get(0)?,
                workspace_path: row.get(1)?,
                title: row.get(2)?,
                created_at: row.get(3)?,
                updated_at: row.get(4)?,
                status: row.get(5)?,
                branch_name: row.get(6)?,
                worktree_path: row.get(7)?,
                plan_json: row.get(8)?,
                worker_summary: row.get(9)?,
                events_json: row.get(10)?,
                verification_json: row.get(11)?,
            }))
        } else {
            Ok(None)
        }
    }

    pub fn delete_chat(&self, task_id: &str) -> SqlResult<()> {
        let conn = self.conn.lock().unwrap();
        conn.execute("DELETE FROM chats WHERE id = ?1", params![task_id])?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_workspace_and_chat_persistence() {
        let db = DatabaseService::in_memory().expect("in memory db");

        // Record workspace
        db.record_workspace("A:\\ZenCode", "ZenCode").unwrap();
        let recent = db.get_recent_workspaces(10).unwrap();
        assert_eq!(recent.len(), 1);
        assert_eq!(recent[0].name, "ZenCode");

        // Save chat
        let req = SaveChatRequest {
            id: "task-101".to_string(),
            workspace_path: "A:\\ZenCode".to_string(),
            title: "Add format_task_id helper".to_string(),
            status: "active".to_string(),
            branch_name: Some("zencode/task-101".to_string()),
            worktree_path: Some("A:\\ZenCode\\.zencode-worktrees\\task-101".to_string()),
            plan_json: Some("{\"steps\": []}".to_string()),
            worker_summary: Some("Implemented helper".to_string()),
            events_json: Some("[]".to_string()),
            verification_json: Some("[]".to_string()),
        };
        db.save_chat(&req).unwrap();

        // List chats for workspace
        let chats = db.list_chats_for_workspace("A:\\ZenCode").unwrap();
        assert_eq!(chats.len(), 1);
        assert_eq!(chats[0].id, "task-101");
        assert_eq!(chats[0].title, "Add format_task_id helper");

        // Retrieve detail
        let detail = db.get_chat("task-101").unwrap().expect("chat exists");
        assert_eq!(detail.branch_name.as_deref(), Some("zencode/task-101"));
        assert_eq!(detail.worker_summary.as_deref(), Some("Implemented helper"));

        // Delete chat
        db.delete_chat("task-101").unwrap();
        let chats_after = db.list_chats_for_workspace("A:\\ZenCode").unwrap();
        assert_eq!(chats_after.len(), 0);
    }
}