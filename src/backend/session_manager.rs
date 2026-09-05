//! Session Manager - SQLite CRUD para sesiones y mensajes
//!
//! Tablas:
//! - sessions(id, title, created_at, updated_at)
//! - messages(id, session_id, role, content, tool_name, tool_call_id, tool_result, image_url, timestamp)

use anyhow::{Context, Result};
use chrono::Utc;
use rusqlite::{params, Connection, OptionalExtension, Row};
use std::path::PathBuf;
use uuid::Uuid;

use crate::models::Message;

#[derive(Debug, Clone)]
pub struct Session {
    pub id: String,
    pub title: String,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Clone)]
pub struct MessageRow {
    pub id: i64,
    pub session_id: String,
    pub role: String,
    pub content: String,
    pub tool_name: Option<String>,
    pub tool_call_id: Option<String>,
    pub tool_result: Option<String>,
    pub image_url: Option<String>,
    pub timestamp: String,
}

pub struct SessionManager {
    conn: Connection,
    #[allow(dead_code)]
    db_path: PathBuf,
}

impl SessionManager {
    pub async fn new() -> Result<Self> {
        let data_dir = dirs::data_local_dir()
            .ok_or_else(|| anyhow::anyhow!("No se pudo obtener data_local_dir"))?
            .join("kde-assistant");

        std::fs::create_dir_all(&data_dir).context("creando directorio de datos")?;
        let db_path = data_dir.join("sessions.db");

        log::info!("SQLite DB: {}", db_path.display());

        let conn = Connection::open(&db_path).context("abriendo SQLite")?;

        conn.execute_batch(
            r#"
            CREATE TABLE IF NOT EXISTS sessions (
                id TEXT PRIMARY KEY,
                title TEXT NOT NULL,
                created_at TEXT NOT NULL,
                updated_at TEXT NOT NULL
            );

            CREATE TABLE IF NOT EXISTS messages (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                session_id TEXT NOT NULL,
                role TEXT NOT NULL,
                content TEXT NOT NULL,
                tool_name TEXT,
                tool_call_id TEXT,
                tool_result TEXT,
                image_url TEXT,
                timestamp TEXT NOT NULL,
                FOREIGN KEY (session_id) REFERENCES sessions(id)
            );

            CREATE INDEX IF NOT EXISTS idx_messages_session ON messages(session_id);
            CREATE INDEX IF NOT EXISTS idx_sessions_updated ON sessions(updated_at DESC);
            "#,
        )?;

        Ok(Self { conn, db_path })
    }

    pub fn create_session(&self, title: impl Into<String>) -> Result<Session> {
        let id = Uuid::new_v4().to_string();
        let now = Utc::now().to_rfc3339();
        let title = title.into();

        self.conn.execute(
            "INSERT INTO sessions (id, title, created_at, updated_at) VALUES (?1, ?2, ?3, ?4)",
            params![id, title, now, now],
        )?;

        Ok(Session {
            id,
            title,
            created_at: now.clone(),
            updated_at: now,
        })
    }

    pub fn list_sessions(&self) -> Result<Vec<Session>> {
        let mut stmt = self.conn.prepare(
            "SELECT id, title, created_at, updated_at FROM sessions ORDER BY updated_at DESC",
        )?;
        let iter = stmt.query_map([], row_to_session)?;
        let mut out = Vec::new();
        for s in iter {
            out.push(s?);
        }
        Ok(out)
    }

    pub fn get_session(&self, id: &str) -> Result<Option<Session>> {
        let mut stmt = self
            .conn
            .prepare("SELECT id, title, created_at, updated_at FROM sessions WHERE id = ?1")?;
        let s = stmt.query_row(params![id], row_to_session).optional()?;
        Ok(s)
    }

    pub fn delete_session(&self, id: &str) -> Result<()> {
        self.conn
            .execute("DELETE FROM messages WHERE session_id = ?1", params![id])?;
        self.conn
            .execute("DELETE FROM sessions WHERE id = ?1", params![id])?;
        Ok(())
    }

    pub fn touch_session(&self, id: &str) -> Result<()> {
        let now = Utc::now().to_rfc3339();
        self.conn.execute(
            "UPDATE sessions SET updated_at = ?1 WHERE id = ?2",
            params![now, id],
        )?;
        Ok(())
    }

    pub fn update_session_title(&self, id: &str, title: &str) -> Result<()> {
        let now = Utc::now().to_rfc3339();
        self.conn.execute(
            "UPDATE sessions SET title = ?1, updated_at = ?2 WHERE id = ?3",
            params![title, now, id],
        )?;
        Ok(())
    }

    pub fn add_message(&self, session_id: &str, message: &Message) -> Result<i64> {
        let (role, content, tool_call_id) = match message {
            Message::System { content } => ("system", content.clone(), None),
            Message::User { content } => ("user", content.clone(), None),
            Message::Assistant { content } => ("assistant", content.clone(), None),
            Message::Tool {
                tool_call_id,
                content,
            } => ("tool", content.clone(), Some(tool_call_id.clone())),
        };

        let now = Utc::now().to_rfc3339();

        self.conn.execute(
            "INSERT INTO messages (session_id, role, content, tool_call_id, timestamp)
             VALUES (?1, ?2, ?3, ?4, ?5)",
            params![session_id, role, content, tool_call_id, now],
        )?;

        let id = self.conn.last_insert_rowid();
        self.touch_session(session_id)?;
        Ok(id)
    }

    pub fn get_messages(&self, session_id: &str) -> Result<Vec<Message>> {
        let mut stmt = self.conn.prepare(
            "SELECT role, content, tool_call_id FROM messages
             WHERE session_id = ?1 ORDER BY id ASC",
        )?;

        let rows = stmt.query_map(params![session_id], |row| {
            let role: String = row.get(0)?;
            let content: String = row.get(1)?;
            let tool_call_id: Option<String> = row.get(2)?;
            Ok((role, content, tool_call_id))
        })?;

        let mut out = Vec::new();
        for r in rows {
            let (role, content, tool_call_id) = r?;
            let msg = match role.as_str() {
                "system" => Message::system(content),
                "user" => Message::user(content),
                "assistant" => Message::assistant(content),
                "tool" => Message::tool(
                    tool_call_id.unwrap_or_else(|| "unknown".to_string()),
                    content,
                ),
                _ => continue,
            };
            out.push(msg);
        }
        Ok(out)
    }

    pub fn count_messages(&self, session_id: &str) -> Result<i64> {
        let n: i64 = self.conn.query_row(
            "SELECT COUNT(*) FROM messages WHERE session_id = ?1",
            params![session_id],
            |r| r.get(0),
        )?;
        Ok(n)
    }
}

fn row_to_session(row: &Row) -> rusqlite::Result<Session> {
    Ok(Session {
        id: row.get(0)?,
        title: row.get(1)?,
        created_at: row.get(2)?,
        updated_at: row.get(3)?,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use rusqlite::Connection;

    fn test_db() -> Result<SessionManager> {
        let conn = Connection::open_in_memory()?;
        conn.execute_batch(
            r#"
            CREATE TABLE sessions (
                id TEXT PRIMARY KEY,
                title TEXT NOT NULL,
                created_at TEXT NOT NULL,
                updated_at TEXT NOT NULL
            );
            CREATE TABLE messages (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                session_id TEXT NOT NULL,
                role TEXT NOT NULL,
                content TEXT NOT NULL,
                tool_name TEXT,
                tool_call_id TEXT,
                tool_result TEXT,
                image_url TEXT,
                timestamp TEXT NOT NULL
            );
            "#,
        )?;
        Ok(SessionManager {
            conn,
            db_path: PathBuf::from(":memory:"),
        })
    }

    #[test]
    fn create_and_list_sessions() {
        let m = test_db().unwrap();
        let s1 = m.create_session("Test 1").unwrap();
        let s2 = m.create_session("Test 2").unwrap();
        let list = m.list_sessions().unwrap();
        assert_eq!(list.len(), 2);
        assert!(list.iter().any(|s| s.id == s1.id));
        assert!(list.iter().any(|s| s.id == s2.id));
    }

    #[test]
    fn add_and_get_messages() {
        let m = test_db().unwrap();
        let s = m.create_session("S").unwrap();
        m.add_message(&s.id, &Message::user("hola")).unwrap();
        m.add_message(&s.id, &Message::assistant("hola!")).unwrap();
        let msgs = m.get_messages(&s.id).unwrap();
        assert_eq!(msgs.len(), 2);
        assert_eq!(m.count_messages(&s.id).unwrap(), 2);
    }

    #[test]
    fn delete_session() {
        let m = test_db().unwrap();
        let s = m.create_session("X").unwrap();
        m.add_message(&s.id, &Message::user("a")).unwrap();
        m.delete_session(&s.id).unwrap();
        assert!(m.get_session(&s.id).unwrap().is_none());
        assert_eq!(m.count_messages(&s.id).unwrap(), 0);
    }
}
