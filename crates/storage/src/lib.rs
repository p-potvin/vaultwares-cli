use std::fs;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use chrono::Utc;
use rusqlite::{params, Connection};
use serde::{Deserialize, Serialize};

pub trait SessionStore {
    fn save_session(&self, record: &SessionRecord) -> Result<()>;
    fn get_session(&self, session_id: &str) -> Result<Option<SessionRecord>>;
}

pub trait TaskStore {
    fn save_task(&self, record: &TaskRecord) -> Result<()>;
    fn list_tasks(&self) -> Result<Vec<TaskRecord>>;
}

pub trait LaneStore {
    fn save_lane(&self, record: &LaneRecord) -> Result<()>;
    fn list_lanes(&self) -> Result<Vec<LaneRecord>>;
}

pub trait AuditStore {
    fn append_event(&self, record: &AuditRecord) -> Result<()>;
}

pub trait BlobStore {
    fn put_blob(&self, key: &str, bytes: &[u8]) -> Result<PathBuf>;
    fn get_blob(&self, key: &str) -> Result<Option<Vec<u8>>>;
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct SessionRecord {
    pub session_id: String,
    pub provider: String,
    pub model: String,
    pub transcript_path: String,
    pub updated_at: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct TaskRecord {
    pub task_id: String,
    pub title: String,
    pub status: String,
    pub lane_id: Option<String>,
    pub updated_at: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct LaneRecord {
    pub lane_id: String,
    pub worker_name: String,
    pub status: String,
    pub last_heartbeat_at: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct AuditRecord {
    pub correlation_id: String,
    pub event_type: String,
    pub details_json: String,
    pub created_at: i64,
}

#[derive(Debug, Clone)]
pub struct SqliteStorage {
    db_path: PathBuf,
    blob_root: PathBuf,
}

impl SqliteStorage {
    pub fn open(base_dir: impl AsRef<Path>) -> Result<Self> {
        let base_dir = base_dir.as_ref();
        fs::create_dir_all(base_dir).with_context(|| {
            format!(
                "failed to create storage base directory at {}",
                base_dir.display()
            )
        })?;
        let blob_root = base_dir.join("blobs");
        fs::create_dir_all(&blob_root)
            .with_context(|| format!("failed to create blob root at {}", blob_root.display()))?;
        let storage = Self {
            db_path: base_dir.join("state.db"),
            blob_root,
        };
        storage.migrate()?;
        Ok(storage)
    }

    fn connection(&self) -> Result<Connection> {
        Connection::open(&self.db_path).with_context(|| {
            format!(
                "failed to open sqlite database at {}",
                self.db_path.display()
            )
        })
    }

    fn migrate(&self) -> Result<()> {
        let connection = self.connection()?;
        connection.execute_batch(
            "
            CREATE TABLE IF NOT EXISTS sessions (
                session_id TEXT PRIMARY KEY,
                provider TEXT NOT NULL,
                model TEXT NOT NULL,
                transcript_path TEXT NOT NULL,
                updated_at INTEGER NOT NULL
            );
            CREATE TABLE IF NOT EXISTS tasks (
                task_id TEXT PRIMARY KEY,
                title TEXT NOT NULL,
                status TEXT NOT NULL,
                lane_id TEXT,
                updated_at INTEGER NOT NULL
            );
            CREATE TABLE IF NOT EXISTS lanes (
                lane_id TEXT PRIMARY KEY,
                worker_name TEXT NOT NULL,
                status TEXT NOT NULL,
                last_heartbeat_at INTEGER NOT NULL
            );
            CREATE TABLE IF NOT EXISTS audit_log (
                correlation_id TEXT NOT NULL,
                event_type TEXT NOT NULL,
                details_json TEXT NOT NULL,
                created_at INTEGER NOT NULL
            );
            ",
        )?;
        Ok(())
    }
}

impl SessionStore for SqliteStorage {
    fn save_session(&self, record: &SessionRecord) -> Result<()> {
        let connection = self.connection()?;
        connection.execute(
            "
            INSERT INTO sessions (session_id, provider, model, transcript_path, updated_at)
            VALUES (?1, ?2, ?3, ?4, ?5)
            ON CONFLICT(session_id) DO UPDATE SET
                provider = excluded.provider,
                model = excluded.model,
                transcript_path = excluded.transcript_path,
                updated_at = excluded.updated_at
            ",
            params![
                record.session_id,
                record.provider,
                record.model,
                record.transcript_path,
                record.updated_at
            ],
        )?;
        Ok(())
    }

    fn get_session(&self, session_id: &str) -> Result<Option<SessionRecord>> {
        let connection = self.connection()?;
        let mut statement = connection.prepare(
            "SELECT session_id, provider, model, transcript_path, updated_at FROM sessions WHERE session_id = ?1",
        )?;
        let mut rows = statement.query(params![session_id])?;
        let Some(row) = rows.next()? else {
            return Ok(None);
        };
        Ok(Some(SessionRecord {
            session_id: row.get(0)?,
            provider: row.get(1)?,
            model: row.get(2)?,
            transcript_path: row.get(3)?,
            updated_at: row.get(4)?,
        }))
    }
}

impl TaskStore for SqliteStorage {
    fn save_task(&self, record: &TaskRecord) -> Result<()> {
        let connection = self.connection()?;
        connection.execute(
            "
            INSERT INTO tasks (task_id, title, status, lane_id, updated_at)
            VALUES (?1, ?2, ?3, ?4, ?5)
            ON CONFLICT(task_id) DO UPDATE SET
                title = excluded.title,
                status = excluded.status,
                lane_id = excluded.lane_id,
                updated_at = excluded.updated_at
            ",
            params![
                record.task_id,
                record.title,
                record.status,
                record.lane_id,
                record.updated_at
            ],
        )?;
        Ok(())
    }

    fn list_tasks(&self) -> Result<Vec<TaskRecord>> {
        let connection = self.connection()?;
        let mut statement = connection.prepare(
            "SELECT task_id, title, status, lane_id, updated_at FROM tasks ORDER BY updated_at DESC",
        )?;
        let rows = statement.query_map([], |row| {
            Ok(TaskRecord {
                task_id: row.get(0)?,
                title: row.get(1)?,
                status: row.get(2)?,
                lane_id: row.get(3)?,
                updated_at: row.get(4)?,
            })
        })?;
        let mut tasks = Vec::new();
        for row in rows {
            tasks.push(row?);
        }
        Ok(tasks)
    }
}

impl LaneStore for SqliteStorage {
    fn save_lane(&self, record: &LaneRecord) -> Result<()> {
        let connection = self.connection()?;
        connection.execute(
            "
            INSERT INTO lanes (lane_id, worker_name, status, last_heartbeat_at)
            VALUES (?1, ?2, ?3, ?4)
            ON CONFLICT(lane_id) DO UPDATE SET
                worker_name = excluded.worker_name,
                status = excluded.status,
                last_heartbeat_at = excluded.last_heartbeat_at
            ",
            params![
                record.lane_id,
                record.worker_name,
                record.status,
                record.last_heartbeat_at
            ],
        )?;
        Ok(())
    }

    fn list_lanes(&self) -> Result<Vec<LaneRecord>> {
        let connection = self.connection()?;
        let mut statement = connection.prepare(
            "SELECT lane_id, worker_name, status, last_heartbeat_at FROM lanes ORDER BY worker_name ASC",
        )?;
        let rows = statement.query_map([], |row| {
            Ok(LaneRecord {
                lane_id: row.get(0)?,
                worker_name: row.get(1)?,
                status: row.get(2)?,
                last_heartbeat_at: row.get(3)?,
            })
        })?;
        let mut lanes = Vec::new();
        for row in rows {
            lanes.push(row?);
        }
        Ok(lanes)
    }
}

impl AuditStore for SqliteStorage {
    fn append_event(&self, record: &AuditRecord) -> Result<()> {
        let connection = self.connection()?;
        connection.execute(
            "
            INSERT INTO audit_log (correlation_id, event_type, details_json, created_at)
            VALUES (?1, ?2, ?3, ?4)
            ",
            params![
                record.correlation_id,
                record.event_type,
                record.details_json,
                record.created_at
            ],
        )?;
        Ok(())
    }
}

impl BlobStore for SqliteStorage {
    fn put_blob(&self, key: &str, bytes: &[u8]) -> Result<PathBuf> {
        let path = self.blob_root.join(key);
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)?;
        }
        fs::write(&path, bytes)?;
        Ok(path)
    }

    fn get_blob(&self, key: &str) -> Result<Option<Vec<u8>>> {
        let path = self.blob_root.join(key);
        if !path.exists() {
            return Ok(None);
        }
        Ok(Some(fs::read(path)?))
    }
}

#[must_use] 
pub fn now_timestamp() -> i64 {
    Utc::now().timestamp()
}

#[cfg(test)]
mod tests {
    use super::{
        now_timestamp, AuditRecord, AuditStore, BlobStore, LaneRecord, LaneStore, SessionRecord,
        SessionStore, SqliteStorage, TaskRecord, TaskStore,
    };

    #[test]
    fn sqlite_storage_round_trips_core_records() {
        let root =
            std::env::temp_dir().join(format!("vaultwares-cli-storage-{}", std::process::id()));
        if root.exists() {
            std::fs::remove_dir_all(&root).expect("cleanup old temp dir");
        }
        let storage = SqliteStorage::open(&root).expect("open storage");

        let session = SessionRecord {
            session_id: "session-1".to_string(),
            provider: "anthropic".to_string(),
            model: "claude-opus-4-6".to_string(),
            transcript_path: "sessions/session-1.jsonl".to_string(),
            updated_at: now_timestamp(),
        };
        storage.save_session(&session).expect("save session");
        assert_eq!(
            storage.get_session("session-1").expect("load session"),
            Some(session)
        );

        let task = TaskRecord {
            task_id: "task-1".to_string(),
            title: "Bootstrap repo".to_string(),
            status: "running".to_string(),
            lane_id: Some("lane-1".to_string()),
            updated_at: now_timestamp(),
        };
        storage.save_task(&task).expect("save task");
        assert_eq!(storage.list_tasks().expect("list tasks").len(), 1);

        let lane = LaneRecord {
            lane_id: "lane-1".to_string(),
            worker_name: "bootstrap".to_string(),
            status: "healthy".to_string(),
            last_heartbeat_at: now_timestamp(),
        };
        storage.save_lane(&lane).expect("save lane");
        assert_eq!(storage.list_lanes().expect("list lanes").len(), 1);

        storage
            .append_event(&AuditRecord {
                correlation_id: "c123abc".to_string(),
                event_type: "worker.spawn".to_string(),
                details_json: "{\"ok\":true}".to_string(),
                created_at: now_timestamp(),
            })
            .expect("append event");

        storage
            .put_blob("tool-output/stdout.txt", b"hello")
            .expect("write blob");
        assert_eq!(
            storage
                .get_blob("tool-output/stdout.txt")
                .expect("read blob")
                .expect("blob exists"),
            b"hello"
        );

        std::fs::remove_dir_all(root).expect("cleanup temp dir");
    }
}
