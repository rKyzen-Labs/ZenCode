use std::path::{Path, PathBuf};
use std::sync::Mutex;
use rusqlite::{params, Connection, Result as SqlResult};
use chrono::Utc;
use crate::db::models::{ChatDetailRecord, ChatMessageRecord, ChatSummaryRecord, SaveChatRequest, WorkspaceRecord};

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
                last_opened_at TEXT NOT NULL,
                is_context_preview_trusted INTEGER NOT NULL DEFAULT 0,
                default_worker_type TEXT NOT NULL DEFAULT 'local',
                groq_model TEXT NOT NULL DEFAULT 'openai/gpt-oss-120b'
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

            CREATE TABLE IF NOT EXISTS chat_messages (
                id TEXT PRIMARY KEY,
                chat_id TEXT NOT NULL,
                role TEXT NOT NULL,
                content TEXT NOT NULL,
                plan_json TEXT,
                worker_summary TEXT,
                files_changed_json TEXT,
                created_at TEXT NOT NULL,
                commit_hash TEXT,
                FOREIGN KEY (chat_id) REFERENCES chats(id) ON DELETE CASCADE
            );

            CREATE INDEX IF NOT EXISTS idx_chats_workspace ON chats(workspace_path);
            CREATE INDEX IF NOT EXISTS idx_chats_updated ON chats(updated_at DESC);
            CREATE INDEX IF NOT EXISTS idx_chat_messages_chat_created ON chat_messages(chat_id, created_at ASC);",
        )?;

        // Migration: add columns if upgrading existing database
        let _ = conn.execute(
            "ALTER TABLE workspaces ADD COLUMN is_context_preview_trusted INTEGER NOT NULL DEFAULT 0;",
            [],
        );
        let _ = conn.execute(
            "ALTER TABLE workspaces ADD COLUMN default_worker_type TEXT NOT NULL DEFAULT 'local';",
            [],
        );
        let _ = conn.execute(
            "ALTER TABLE workspaces ADD COLUMN groq_model TEXT NOT NULL DEFAULT 'openai/gpt-oss-120b';",
            [],
        );
        let _ = conn.execute(
            "ALTER TABLE chat_messages ADD COLUMN commit_hash TEXT;",
            [],
        );
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

    pub fn set_workspace_context_preview_trusted(&self, path: &str, trusted: bool) -> SqlResult<()> {
        let conn = self.conn.lock().unwrap();
        conn.execute(
            "UPDATE workspaces SET is_context_preview_trusted = ?1 WHERE path = ?2;",
            params![trusted as i32, path],
        )?;
        Ok(())
    }

    pub fn is_workspace_context_preview_trusted(&self, path: &str) -> SqlResult<bool> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn.prepare("SELECT is_context_preview_trusted FROM workspaces WHERE path = ?1")?;
        let mut rows = stmt.query(params![path])?;
        if let Some(row) = rows.next()? {
            let val: i32 = row.get(0)?;
            Ok(val != 0)
        } else {
            Ok(false)
        }
    }

    pub fn get_workspace_worker_config(&self, path: &str) -> SqlResult<(String, Option<String>)> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn.prepare("SELECT default_worker_type, groq_model FROM workspaces WHERE path = ?1")?;
        let mut rows = stmt.query(params![path])?;
        if let Some(row) = rows.next()? {
            let worker_type: String = row.get(0).unwrap_or_else(|_| "local".to_string());
            let model: Option<String> = row.get(1).ok();
            Ok((worker_type, model))
        } else {
            Ok(("local".to_string(), Some("openai/gpt-oss-120b".to_string())))
        }
    }

    pub fn set_workspace_worker_config(&self, path: &str, worker_type: &str, groq_model: Option<&str>) -> SqlResult<()> {
        let conn = self.conn.lock().unwrap();
        let model = groq_model.unwrap_or("openai/gpt-oss-120b");
        conn.execute(
            "UPDATE workspaces SET default_worker_type = ?1, groq_model = ?2 WHERE path = ?3;",
            params![worker_type, model, path],
        )?;
        Ok(())
    }

    pub fn get_recent_workspaces(&self, limit: usize) -> SqlResult<Vec<WorkspaceRecord>> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn.prepare(
            "SELECT path, name, last_opened_at, is_context_preview_trusted, default_worker_type, groq_model
             FROM workspaces
             ORDER BY last_opened_at DESC
             LIMIT ?1",
        )?;

        let rows = stmt.query_map(params![limit as i64], |row| {
            let trusted: i32 = row.get(3).unwrap_or(0);
            let worker_type: String = row.get(4).unwrap_or_else(|_| "local".to_string());
            let groq_model: Option<String> = row.get(5).ok();
            Ok(WorkspaceRecord {
                path: row.get(0)?,
                name: row.get(1)?,
                last_opened_at: row.get(2)?,
                is_context_preview_trusted: trusted != 0,
                default_worker_type: worker_type,
                groq_model,
            })
        })?;

        let mut results = Vec::new();
        for r in rows {
            results.push(r?);
        }
        Ok(results)
    }

    pub fn remove_workspace(&self, workspace_path: &str) -> SqlResult<()> {
        let conn = self.conn.lock().unwrap();
        conn.execute("DELETE FROM workspaces WHERE path = ?1", params![workspace_path])?;
        Ok(())
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

    pub fn list_all_recent_chats(&self, limit: usize) -> SqlResult<Vec<ChatSummaryRecord>> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn.prepare(
            "SELECT id, workspace_path, title, created_at, updated_at, status, branch_name, worktree_path
             FROM chats
             ORDER BY updated_at DESC
             LIMIT ?1",
        )?;

        let rows = stmt.query_map(params![limit as i64], |row| {
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

    pub fn search_chats(&self, query: &str) -> SqlResult<Vec<ChatSummaryRecord>> {
        let conn = self.conn.lock().unwrap();
        let pattern = format!("%{}%", query);
        let mut stmt = conn.prepare(
            "SELECT id, workspace_path, title, created_at, updated_at, status, branch_name, worktree_path
             FROM chats
             WHERE title LIKE ?1 OR workspace_path LIKE ?1 OR plan_json LIKE ?1
             ORDER BY updated_at DESC
             LIMIT 50",
        )?;

        let rows = stmt.query_map(params![pattern], |row| {
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

    pub fn add_chat_message(&self, msg: &ChatMessageRecord) -> SqlResult<()> {
        let conn = self.conn.lock().unwrap();

        // 1. Ensure parent chat exists in `chats` table so foreign key constraint never fails
        let chat_exists: bool = conn
            .query_row(
                "SELECT 1 FROM chats WHERE id = ?1;",
                params![&msg.chat_id],
                |_| Ok(true),
            )
            .unwrap_or(false);

        if !chat_exists {
            let workspace_path: String = conn
                .query_row(
                    "SELECT path FROM workspaces ORDER BY last_opened_at DESC LIMIT 1;",
                    [],
                    |row| row.get(0),
                )
                .unwrap_or_else(|_| "default".to_string());

            let now = Utc::now().to_rfc3339();
            let title = if msg.content.len() > 35 {
                format!("{}...", &msg.content[..32])
            } else {
                msg.content.clone()
            };

            let _ = conn.execute(
                "INSERT OR IGNORE INTO chats (id, workspace_path, title, created_at, updated_at, status)
                 VALUES (?1, ?2, ?3, ?4, ?5, 'active');",
                params![&msg.chat_id, &workspace_path, &title, &now, &now],
            );
        }

        conn.execute(
            "INSERT INTO chat_messages (
                id, chat_id, role, content, plan_json, worker_summary, files_changed_json, created_at, commit_hash
             ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9);",
            params![
                &msg.id,
                &msg.chat_id,
                &msg.role,
                &msg.content,
                &msg.plan_json,
                &msg.worker_summary,
                &msg.files_changed_json,
                &msg.created_at,
                &msg.commit_hash,
            ],
        )?;

        // Update parent chat updated_at
        let _ = conn.execute(
            "UPDATE chats SET updated_at = ?1 WHERE id = ?2;",
            params![&msg.created_at, &msg.chat_id],
        );
        Ok(())
    }

    pub fn update_message_files_changed(&self, message_id: &str, files: &[String]) -> SqlResult<()> {
        let conn = self.conn.lock().unwrap();
        let json_str = serde_json::to_string(files).unwrap_or_else(|_| "[]".to_string());
        conn.execute(
            "UPDATE chat_messages SET files_changed_json = ?1 WHERE id = ?2;",
            params![&json_str, message_id],
        )?;
        Ok(())
    }

    pub fn update_message_commit_hash(&self, message_id: &str, commit_hash: &str) -> SqlResult<()> {
        let conn = self.conn.lock().unwrap();
        conn.execute(
            "UPDATE chat_messages SET commit_hash = ?1 WHERE id = ?2;",
            params![commit_hash, message_id],
        )?;
        Ok(())
    }

    pub fn get_message_commit_hash(&self, message_id: &str) -> SqlResult<Option<String>> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn.prepare("SELECT commit_hash FROM chat_messages WHERE id = ?1;")?;
        let mut rows = stmt.query(params![message_id])?;
        if let Some(row) = rows.next()? {
            Ok(row.get(0)?)
        } else {
            Ok(None)
        }
    }

    pub fn finalize_latest_assistant_message(
        &self,
        chat_id: &str,
        summary: &str,
        files_changed_json: Option<&str>,
        commit_hash: Option<&str>,
    ) -> SqlResult<bool> {
        let conn = self.conn.lock().unwrap();
        let existing_id: Option<String> = conn
            .query_row(
                "SELECT id FROM chat_messages WHERE chat_id = ?1 AND role = 'assistant' ORDER BY created_at DESC LIMIT 1;",
                params![chat_id],
                |row| row.get(0),
            )
            .ok();

        if let Some(id) = existing_id {
            conn.execute(
                "UPDATE chat_messages 
                 SET content = ?1, worker_summary = ?1, files_changed_json = ?2, commit_hash = ?3
                 WHERE id = ?4;",
                params![summary, files_changed_json, commit_hash, &id],
            )?;
            Ok(true)
        } else {
            Ok(false)
        }
    }

    pub fn get_chat_messages(&self, chat_id: &str) -> SqlResult<Vec<ChatMessageRecord>> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn.prepare(
            "SELECT id, chat_id, role, content, plan_json, worker_summary, files_changed_json, created_at, commit_hash
             FROM chat_messages
             WHERE chat_id = ?1
             ORDER BY created_at ASC;"
        )?;
        let rows = stmt.query_map(params![chat_id], |row| {
            Ok(ChatMessageRecord {
                id: row.get(0)?,
                chat_id: row.get(1)?,
                role: row.get(2)?,
                content: row.get(3)?,
                plan_json: row.get(4)?,
                worker_summary: row.get(5)?,
                files_changed_json: row.get(6)?,
                created_at: row.get(7)?,
                commit_hash: row.get(8)?,
            })
        })?;
        let mut results = Vec::new();
        for r in rows {
            results.push(r?);
        }

        // Self-heal: If no user message is present in chat_messages, check parent chat title in `chats`
        let has_user_msg = results.iter().any(|m| m.role == "user");
        if !has_user_msg {
            let chat_info: Option<(String, String)> = conn
                .query_row(
                    "SELECT title, created_at FROM chats WHERE id = ?1;",
                    params![chat_id],
                    |row| Ok((row.get(0)?, row.get(1)?)),
                )
                .ok();

            if let Some((title, created_at)) = chat_info {
                if !title.trim().is_empty() {
                    let init_msg = ChatMessageRecord {
                        id: format!("msg_init_{}", chat_id),
                        chat_id: chat_id.to_string(),
                        role: "user".to_string(),
                        content: title.clone(),
                        plan_json: None,
                        worker_summary: None,
                        files_changed_json: None,
                        created_at: created_at.clone(),
                        commit_hash: None,
                    };
                    let _ = conn.execute(
                        "INSERT OR IGNORE INTO chat_messages (
                            id, chat_id, role, content, plan_json, worker_summary, files_changed_json, created_at, commit_hash
                        ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9);",
                        params![
                            &init_msg.id,
                            &init_msg.chat_id,
                            &init_msg.role,
                            &init_msg.content,
                            &init_msg.plan_json,
                            &init_msg.worker_summary,
                            &init_msg.files_changed_json,
                            &init_msg.created_at,
                            &init_msg.commit_hash,
                        ],
                    );
                    results.insert(0, init_msg);
                }
            }
        }

        Ok(results)
    }

    pub fn get_recent_exchanges(&self, chat_id: &str, limit: usize) -> SqlResult<Vec<zencode_core::planner::PriorExchange>> {
        let messages = self.get_chat_messages(chat_id)?;
        let start = if messages.len() > limit * 2 {
            messages.len() - limit * 2
        } else {
            0
        };
        let recent = &messages[start..];
        let mut exchanges = Vec::new();
        for m in recent {
            let files_changed: Vec<String> = m.files_changed_json
                .as_ref()
                .and_then(|s| {
                    if let Ok(list) = serde_json::from_str::<Vec<String>>(s) {
                        Some(list)
                    } else if let Ok(inv) = serde_json::from_str::<crate::git::models::ChangeInventory>(s) {
                        Some(inv.files.into_iter().map(|f| f.path).collect())
                    } else if let Ok(val) = serde_json::from_str::<serde_json::Value>(s) {
                        val.get("files")
                            .and_then(|f| f.as_array())
                            .map(|arr| {
                                arr.iter()
                                    .filter_map(|item| {
                                        item.get("path")
                                            .and_then(|p| p.as_str())
                                            .map(|p| p.to_string())
                                    })
                                    .collect()
                            })
                    } else {
                        None
                    }
                })
                .unwrap_or_default();
            exchanges.push(zencode_core::planner::PriorExchange {
                role: m.role.clone(),
                content: m.content.clone(),
                files_changed,
                summary: m.worker_summary.clone(),
            });
        }
        Ok(exchanges)
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

        // Test workspace worker config
        let (worker_type, groq_model) = db.get_workspace_worker_config("A:\\ZenCode").unwrap();
        assert_eq!(worker_type, "local");
        assert_eq!(groq_model.as_deref(), Some("openai/gpt-oss-120b"));

        db.set_workspace_worker_config("A:\\ZenCode", "groq", Some("openai/gpt-oss-20b")).unwrap();
        let (worker_type2, groq_model2) = db.get_workspace_worker_config("A:\\ZenCode").unwrap();
        assert_eq!(worker_type2, "groq");
        assert_eq!(groq_model2.as_deref(), Some("openai/gpt-oss-20b"));
    }

    #[test]
    fn test_chat_messages_history_persistence() {
        let db = DatabaseService::in_memory().expect("in memory db");
        db.record_workspace("A:\\ZenCode", "ZenCode").unwrap();

        let req = SaveChatRequest {
            id: "chat-multi".to_string(),
            workspace_path: "A:\\ZenCode".to_string(),
            title: "Multi-turn chat".to_string(),
            status: "active".to_string(),
            branch_name: None,
            worktree_path: None,
            plan_json: None,
            worker_summary: None,
            events_json: None,
            verification_json: None,
        };
        db.save_chat(&req).unwrap();

        // Message 1: User
        db.add_chat_message(&ChatMessageRecord {
            id: "msg-1".into(),
            chat_id: "chat-multi".into(),
            role: "user".into(),
            content: "Create a math.rs file with an add function".into(),
            plan_json: None,
            worker_summary: None,
            files_changed_json: None,
            created_at: "2026-11-08T12:00:00Z".into(),
            commit_hash: None,
        }).unwrap();

        // Message 2: Assistant
        db.add_chat_message(&ChatMessageRecord {
            id: "msg-2".into(),
            chat_id: "chat-multi".into(),
            role: "assistant".into(),
            content: "Created math.rs with add function".into(),
            plan_json: Some("{\"steps\":[]}".into()),
            worker_summary: Some("Created math.rs with add function".into()),
            files_changed_json: Some("[\"src/math.rs\"]".into()),
            created_at: "2026-11-08T12:01:00Z".into(),
            commit_hash: Some("sha_abc123".into()),
        }).unwrap();

        // Message 3: User follow-up
        db.add_chat_message(&ChatMessageRecord {
            id: "msg-3".into(),
            chat_id: "chat-multi".into(),
            role: "user".into(),
            content: "Also add input validation to the add function".into(),
            plan_json: None,
            worker_summary: None,
            files_changed_json: None,
            created_at: "2026-11-08T12:02:00Z".into(),
            commit_hash: None,
        }).unwrap();

        let msgs = db.get_chat_messages("chat-multi").unwrap();
        assert_eq!(msgs.len(), 3);
        assert_eq!(msgs[0].role, "user");
        assert_eq!(msgs[1].role, "assistant");
        assert_eq!(msgs[1].files_changed_json.as_deref(), Some("[\"src/math.rs\"]"));
        assert_eq!(msgs[1].commit_hash.as_deref(), Some("sha_abc123"));
        assert_eq!(msgs[2].content, "Also add input validation to the add function");

        db.update_message_commit_hash("msg-1", "sha_user1").unwrap();
        assert_eq!(db.get_message_commit_hash("msg-1").unwrap().as_deref(), Some("sha_user1"));

        let exchanges = db.get_recent_exchanges("chat-multi", 5).unwrap();
        assert_eq!(exchanges.len(), 3);
        assert_eq!(exchanges[1].files_changed, vec!["src/math.rs".to_string()]);
        assert_eq!(exchanges[1].summary.as_deref(), Some("Created math.rs with add function"));
    }
}
