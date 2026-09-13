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

/// Fila de auditoría de herramientas (F4-3).
#[derive(Debug, Clone, serde::Serialize)]
pub struct ToolAuditRow {
    pub id: i64,
    pub timestamp: String,
    pub session_id: Option<String>,
    pub tool: String,
    pub args: String,
    pub success: bool,
    pub duration_ms: i64,
    pub permission: String,
    pub decided: String,
}

/// Fact del usuario (F5, memoria local).
#[derive(Debug, Clone, serde::Serialize)]
pub struct UserFact {
    pub key: String,
    pub value: String,
    pub updated_at: String,
}

/// Recordatorio persistente (F6).
#[derive(Debug, Clone)]
pub struct Reminder {
    pub id: i64,
    pub fire_at: String,
    pub text: String,
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

        // F0-7: WAL para no bloquear lectores durante escrituras + busy_timeout.
        let _ = conn.execute_batch("PRAGMA journal_mode=WAL; PRAGMA busy_timeout=5000;");
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

            -- F4-3: auditoría de herramientas ejecutadas.
            CREATE TABLE IF NOT EXISTS tool_audit (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                timestamp TEXT NOT NULL,
                session_id TEXT,
                tool TEXT NOT NULL,
                args TEXT NOT NULL DEFAULT '',
                success INTEGER NOT NULL DEFAULT 0,
                duration_ms INTEGER NOT NULL DEFAULT 0,
                permission TEXT NOT NULL DEFAULT '',
                decided TEXT NOT NULL DEFAULT 'auto'
            );
            CREATE INDEX IF NOT EXISTS idx_audit_ts ON tool_audit(timestamp DESC);

            -- F5: facts del usuario (memoria local opt-in).
            CREATE TABLE IF NOT EXISTS user_facts (
                key TEXT PRIMARY KEY,
                value TEXT NOT NULL,
                updated_at TEXT NOT NULL
            );

            -- F6: recordatorios persistentes (sobreviven reinicios).
            CREATE TABLE IF NOT EXISTS reminders (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                fire_at TEXT NOT NULL,
                text TEXT NOT NULL,
                created_at TEXT NOT NULL,
                done INTEGER NOT NULL DEFAULT 0
            );
            CREATE INDEX IF NOT EXISTS idx_reminders_fire ON reminders(done, fire_at);
            "#,
        )?;

        // F5: migración de sesiones viejas (summary para hilos largos).
        ensure_session_summary_columns(&conn)?;

        Ok(Self { conn, db_path })
    }

    /// Registra una ejecución en la auditoría (F4-3).
    #[allow(clippy::too_many_arguments)]
    pub fn record_tool_audit(
        &self,
        session_id: Option<&str>,
        tool: &str,
        args_json: &str,
        success: bool,
        duration_ms: u64,
        permission: &str,
        decided: &str,
    ) -> Result<()> {
        // Args recortados (pueden llevar contenidos grandes).
        let args: String = args_json.chars().take(2000).collect();
        self.conn.execute(
            "INSERT INTO tool_audit (timestamp, session_id, tool, args, success, duration_ms, permission, decided)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
            params![
                Utc::now().to_rfc3339(),
                session_id,
                tool,
                args,
                success as i32,
                duration_ms as i64,
                permission,
                decided
            ],
        )?;
        Ok(())
    }
    /// Últimas `limit` entradas de auditoría (más recientes primero).
    pub fn list_tool_audit(&self, limit: i64) -> Result<Vec<ToolAuditRow>> {
        let limit = limit.clamp(1, 200);
        let mut stmt = self.conn.prepare(
            "SELECT id, timestamp, session_id, tool, args, success, duration_ms, permission, decided
             FROM tool_audit ORDER BY id DESC LIMIT ?1",
        )?;
        let rows = stmt.query_map(params![limit], |row| {
            Ok(ToolAuditRow {
                id: row.get(0)?,
                timestamp: row.get(1)?,
                session_id: row.get(2)?,
                tool: row.get(3)?,
                args: row.get(4)?,
                success: row.get::<_, i32>(5)? != 0,
                duration_ms: row.get(6)?,
                permission: row.get(7)?,
                decided: row.get(8)?,
            })
        })?;
        let mut out = Vec::new();
        for r in rows {
            out.push(r?);
        }
        Ok(out)
    }

    /// Titulo canonico de la sesion unica de voz.
    pub const VOICE_SESSION_TITLE: &'static str = "Conversacion por voz";

    /// Reutiliza la sesion de voz existente (la mas reciente con ese
    /// titulo) o la crea. Evita fragmentar el historial del habla en
    /// N sesiones homonimas y da contexto al pipeline de voz.
    pub fn get_or_create_voice_session(&self) -> Result<Session> {
        if let Ok(list) = self.list_sessions() {
            if let Some(s) = list
                .into_iter()
                .find(|s| s.title == Self::VOICE_SESSION_TITLE)
            {
                return Ok(s);
            }
        }
        self.create_session(Self::VOICE_SESSION_TITLE)
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

    // === Facts del usuario (F5, memoria local) ===

    /// Guarda o actualiza un fact (clave 1-40 chars, valor 1-500).
    pub fn upsert_fact(&self, key: &str, value: &str) -> Result<()> {
        let key = key.trim();
        let value = value.trim();
        if key.is_empty() || value.is_empty() {
            anyhow::bail!("clave o valor vacío");
        }
        if key.chars().count() > 40 {
            anyhow::bail!("clave demasiado larga (máx 40)");
        }
        if value.chars().count() > 500 {
            anyhow::bail!("valor demasiado largo (máx 500)");
        }
        // Sin shell ni paths aquí; solo validación de texto.
        let now = Utc::now().to_rfc3339();
        self.conn.execute(
            "INSERT INTO user_facts (key, value, updated_at) VALUES (?1, ?2, ?3)
             ON CONFLICT(key) DO UPDATE SET value = excluded.value, updated_at = excluded.updated_at",
            params![key, value, now],
        )?;
        Ok(())
    }

    pub fn delete_fact(&self, key: &str) -> Result<()> {
        self.conn
            .execute("DELETE FROM user_facts WHERE key = ?1", params![key.trim()])?;
        Ok(())
    }

    pub fn list_facts(&self) -> Result<Vec<UserFact>> {
        let mut stmt = self
            .conn
            .prepare("SELECT key, value, updated_at FROM user_facts ORDER BY key ASC")?;
        let rows = stmt.query_map([], |row| {
            Ok(UserFact {
                key: row.get(0)?,
                value: row.get(1)?,
                updated_at: row.get(2)?,
            })
        })?;
        let mut out = Vec::new();
        for r in rows {
            out.push(r?);
        }
        Ok(out)
    }

    // === Resumen de hilos largos (F5) ===

    pub fn get_summary(&self, session_id: &str) -> Result<(String, i64)> {
        let row: (String, i64) = self.conn.query_row(
            "SELECT COALESCE(summary, ''), COALESCE(summary_count, 0) FROM sessions WHERE id = ?1",
            params![session_id],
            |r| Ok((r.get(0)?, r.get(1)?)),
        )?;
        Ok(row)
    }

    pub fn set_summary(&self, session_id: &str, summary: &str, count: i64) -> Result<()> {
        self.conn.execute(
            "UPDATE sessions SET summary = ?1, summary_count = ?2 WHERE id = ?3",
            params![summary, count, session_id],
        )?;
        Ok(())
    }

    // === Recordatorios persistentes (F6) ===

    /// Crea un recordatorio para `fire_at` (RFC3339). Retorna el id.
    pub fn add_reminder(&self, fire_at: &str, text: &str) -> Result<i64> {
        let now = Utc::now().to_rfc3339();
        self.conn.execute(
            "INSERT INTO reminders (fire_at, text, created_at, done) VALUES (?1, ?2, ?3, 0)",
            params![fire_at, text, now],
        )?;
        Ok(self.conn.last_insert_rowid())
    }

    /// Recordatorios pendientes (done=0), ordenados por hora.
    pub fn pending_reminders(&self) -> Result<Vec<Reminder>> {
        let mut stmt = self.conn.prepare(
            "SELECT id, fire_at, text FROM reminders WHERE done = 0 ORDER BY fire_at ASC",
        )?;
        let rows = stmt.query_map([], |row| {
            Ok(Reminder {
                id: row.get(0)?,
                fire_at: row.get(1)?,
                text: row.get(2)?,
            })
        })?;
        let mut out = Vec::new();
        for r in rows {
            out.push(r?);
        }
        Ok(out)
    }

    pub fn mark_reminder_done(&self, id: i64) -> Result<()> {
        self.conn
            .execute("UPDATE reminders SET done = 1 WHERE id = ?1", params![id])?;
        Ok(())
    }

    pub fn add_message(&self, session_id: &str, message: &Message) -> Result<i64> {
        let (role, content, tool_call_id, tool_result, image_url) = match message {
            Message::System { content } => ("system", content.clone(), None, None, None),
            Message::User { content } => ("user", content.clone(), None, None, None),
            Message::Assistant {
                content,
                tool_calls,
            } => {
                // Persistir los tool calls como JSON para reconstruir el
                // historial en formato OpenAI (Groq lo exige).
                let tools_json = if tool_calls.is_empty() {
                    None
                } else {
                    serde_json::to_string(tool_calls).ok()
                };
                ("assistant", content.clone(), None, tools_json, None)
            }
            Message::Tool {
                tool_call_id,
                content,
                image_url,
            } => (
                "tool",
                content.clone(),
                Some(tool_call_id.clone()),
                None,
                image_url.clone(),
            ),
        };

        let now = Utc::now().to_rfc3339();

        self.conn.execute(
            "INSERT INTO messages (session_id, role, content, tool_call_id, tool_result, image_url, timestamp)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
            params![
                session_id,
                role,
                content,
                tool_call_id,
                tool_result,
                image_url,
                now
            ],
        )?;

        let id = self.conn.last_insert_rowid();
        self.touch_session(session_id)?;
        Ok(id)
    }

    pub fn get_messages(&self, session_id: &str) -> Result<Vec<Message>> {
        Ok(self
            .get_messages_with_timestamps(session_id)?
            .into_iter()
            .map(|(m, _)| m)
            .collect())
    }

    /// Mensajes con su `timestamp` (RFC3339) para mostrar hora real en la UI.
    pub fn get_messages_with_timestamps(&self, session_id: &str) -> Result<Vec<(Message, String)>> {
        let mut stmt = self.conn.prepare(
            "SELECT role, content, tool_call_id, tool_result, image_url, timestamp FROM messages
             WHERE session_id = ?1 ORDER BY id ASC",
        )?;

        let rows = stmt.query_map(params![session_id], |row| {
            let role: String = row.get(0)?;
            let content: String = row.get(1)?;
            let tool_call_id: Option<String> = row.get(2)?;
            let tool_result: Option<String> = row.get(3)?;
            let image_url: Option<String> = row.get(4)?;
            let timestamp: String = row.get(5)?;
            Ok((
                role,
                content,
                tool_call_id,
                tool_result,
                image_url,
                timestamp,
            ))
        })?;

        let mut out = Vec::new();
        for r in rows {
            let (role, content, tool_call_id, tool_result, image_url, timestamp) = r?;
            let msg = match role.as_str() {
                "system" => Message::system(content),
                "user" => Message::user(content),
                "assistant" => {
                    let tool_calls: Vec<crate::models::ToolCall> = tool_result
                        .as_deref()
                        .and_then(|j| serde_json::from_str(j).ok())
                        .unwrap_or_default();
                    Message::assistant_with_tools(content, tool_calls)
                }
                "tool" => Message::tool_with_image(
                    tool_call_id.unwrap_or_else(|| "unknown".to_string()),
                    content,
                    image_url,
                ),
                _ => continue,
            };
            out.push((msg, timestamp));
        }
        Ok(out)
    }

    /// Borra los mensajes posteriores al último `user` (para Regenerar).
    /// Retorna cuántos borró. Si no hay `user`, no borra nada.
    pub fn delete_trailing_after_last_user(&self, session_id: &str) -> Result<usize> {
        let last_user_id: Option<i64> = self
            .conn
            .query_row(
                "SELECT MAX(id) FROM messages WHERE session_id = ?1 AND role = 'user'",
                params![session_id],
                |r| r.get(0),
            )
            .unwrap_or(None);
        let Some(uid) = last_user_id else {
            return Ok(0);
        };
        let n = self.conn.execute(
            "DELETE FROM messages WHERE session_id = ?1 AND id > ?2",
            params![session_id, uid],
        )?;
        if n > 0 {
            self.touch_session(session_id)?;
        }
        Ok(n)
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

/// Añade `summary`/`summary_count` a DBs creadas antes de F5.
fn ensure_session_summary_columns(conn: &Connection) -> Result<()> {
    let mut has_summary = false;
    let mut has_count = false;
    let mut stmt = conn.prepare("PRAGMA table_info(sessions)")?;
    let cols = stmt.query_map([], |row| row.get::<_, String>(1))?;
    for c in cols.flatten() {
        if c == "summary" {
            has_summary = true;
        } else if c == "summary_count" {
            has_count = true;
        }
    }
    if !has_summary {
        conn.execute(
            "ALTER TABLE sessions ADD COLUMN summary TEXT NOT NULL DEFAULT ''",
            [],
        )?;
    }
    if !has_count {
        conn.execute(
            "ALTER TABLE sessions ADD COLUMN summary_count INTEGER NOT NULL DEFAULT 0",
            [],
        )?;
    }
    Ok(())
}

/// ¿Toca resumir? A partir de 60 mensajes y cada 40 nuevos desde el último resumen.
pub fn should_summarize(count: i64, summary_count: i64) -> bool {
    count >= 60 && count - summary_count >= 40
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
                updated_at TEXT NOT NULL,
                summary TEXT NOT NULL DEFAULT '',
                summary_count INTEGER NOT NULL DEFAULT 0
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
            CREATE TABLE tool_audit (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                timestamp TEXT NOT NULL,
                session_id TEXT,
                tool TEXT NOT NULL,
                args TEXT NOT NULL DEFAULT '',
                success INTEGER NOT NULL DEFAULT 0,
                duration_ms INTEGER NOT NULL DEFAULT 0,
                permission TEXT NOT NULL DEFAULT '',
                decided TEXT NOT NULL DEFAULT 'auto'
            );
            CREATE TABLE user_facts (
                key TEXT PRIMARY KEY,
                value TEXT NOT NULL,
                updated_at TEXT NOT NULL
            );
            CREATE TABLE reminders (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                fire_at TEXT NOT NULL,
                text TEXT NOT NULL,
                created_at TEXT NOT NULL,
                done INTEGER NOT NULL DEFAULT 0
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

    #[test]
    fn messages_with_timestamps() {
        let m = test_db().unwrap();
        let s = m.create_session("S").unwrap();
        m.add_message(&s.id, &Message::user("hola")).unwrap();
        let with_ts = m.get_messages_with_timestamps(&s.id).unwrap();
        assert_eq!(with_ts.len(), 1);
        assert!(!with_ts[0].1.is_empty());
    }

    #[test]
    fn delete_trailing_keeps_last_user() {
        let m = test_db().unwrap();
        let s = m.create_session("S").unwrap();
        m.add_message(&s.id, &Message::user("q")).unwrap();
        m.add_message(&s.id, &Message::assistant("a1")).unwrap();
        m.add_message(&s.id, &Message::user("q2")).unwrap();
        m.add_message(&s.id, &Message::assistant("a2")).unwrap();
        let n = m.delete_trailing_after_last_user(&s.id).unwrap();
        assert_eq!(n, 1);
        let msgs = m.get_messages(&s.id).unwrap();
        assert_eq!(msgs.len(), 3);
    }

    #[test]
    fn delete_trailing_without_user_deletes_nothing() {
        let m = test_db().unwrap();
        let s = m.create_session("S").unwrap();
        assert_eq!(m.delete_trailing_after_last_user(&s.id).unwrap(), 0);
    }

    /// La sesion de voz es unica: get_or_create no debe crear duplicadas
    /// y siempre devuelve la mas reciente con el titulo canonico.
    #[test]
    fn voice_session_is_reused_not_duplicated() {
        let m = test_db().unwrap();
        let a = m.get_or_create_voice_session().unwrap();
        let b = m.get_or_create_voice_session().unwrap();
        assert_eq!(a.id, b.id);
        let list = m.list_sessions().unwrap();
        assert_eq!(
            list.iter()
                .filter(|s| s.title == SessionManager::VOICE_SESSION_TITLE)
                .count(),
            1
        );
    }

    /// F0-1: tras un turno con tool calls, recargar la sesión conserva
    /// el assistant_with_tools, el mensaje tool y su image_url.
    #[test]
    fn tool_turns_persist_roundtrip_with_image() {
        let m = test_db().unwrap();
        let s = m.create_session("S").unwrap();
        m.add_message(&s.id, &Message::user("muéstrame una imagen"))
            .unwrap();
        let tc = crate::models::ToolCall {
            id: "call_1".into(),
            name: "show_image".into(),
            arguments: std::collections::HashMap::new(),
        };
        m.add_message(&s.id, &Message::assistant_with_tools("", vec![tc]))
            .unwrap();
        m.add_message(
            &s.id,
            &Message::tool_with_image("call_1", "Imagen lista", Some("/tmp/x.png".into())),
        )
        .unwrap();

        let msgs = m.get_messages(&s.id).unwrap();
        assert_eq!(msgs.len(), 3);
        match &msgs[1] {
            Message::Assistant { tool_calls, .. } => {
                assert_eq!(tool_calls.len(), 1);
                assert_eq!(tool_calls[0].name, "show_image");
            }
            other => panic!("esperaba assistant, llegó: {other:?}"),
        }
        match &msgs[2] {
            Message::Tool {
                tool_call_id,
                image_url,
                ..
            } => {
                assert_eq!(tool_call_id, "call_1");
                assert_eq!(image_url.as_deref(), Some("/tmp/x.png"));
            }
            other => panic!("esperaba tool, llegó: {other:?}"),
        }
    }

    #[test]
    fn tool_audit_roundtrip() {
        let m = test_db().unwrap();
        let s = m.create_session("S").unwrap();
        m.record_tool_audit(
            Some(&s.id),
            "read_file",
            r#"{"path":"/x"}"#,
            true,
            12,
            "green",
            "auto",
        )
        .unwrap();
        m.record_tool_audit(None, "edit_file", "{}", false, 3, "red", "denied")
            .unwrap();
        let rows = m.list_tool_audit(10).unwrap();
        assert_eq!(rows.len(), 2);
        // Más recientes primero.
        assert_eq!(rows[0].tool, "edit_file");
        assert!(!rows[0].success);
        assert_eq!(rows[0].decided, "denied");
        assert_eq!(rows[1].tool, "read_file");
        assert!(rows[1].success);
        assert_eq!(m.list_tool_audit(1).unwrap().len(), 1);
    }

    #[test]
    fn facts_crud() {
        let m = test_db().unwrap();
        assert!(m.list_facts().unwrap().is_empty());
        m.upsert_fact("nombre", "Ada").unwrap();
        m.upsert_fact("  idioma  ", "  es ").unwrap();
        let facts = m.list_facts().unwrap();
        assert_eq!(facts.len(), 2);
        // Ordenados por clave.
        assert_eq!(facts[0].key, "idioma");
        assert_eq!(facts[0].value, "es");
        m.upsert_fact("nombre", "Ada Lovelace").unwrap();
        assert_eq!(m.list_facts().unwrap().len(), 2);
        m.delete_fact("nombre").unwrap();
        assert_eq!(m.list_facts().unwrap().len(), 1);
        // Validación.
        assert!(m.upsert_fact("", "x").is_err());
        assert!(m.upsert_fact("k", "").is_err());
        assert!(m.upsert_fact(&"k".repeat(41), "x").is_err());
    }

    #[test]
    fn summary_get_set() {
        let m = test_db().unwrap();
        let s = m.create_session("S").unwrap();
        let (sum, cnt) = m.get_summary(&s.id).unwrap();
        assert!(sum.is_empty() && cnt == 0);
        m.set_summary(&s.id, "Resumen...", 40).unwrap();
        let (sum, cnt) = m.get_summary(&s.id).unwrap();
        assert_eq!(sum, "Resumen...");
        assert_eq!(cnt, 40);
    }

    #[test]
    fn should_summarize_thresholds() {
        assert!(!should_summarize(59, 0));
        assert!(should_summarize(60, 0));
        assert!(should_summarize(100, 60));
        // Menos de 40 nuevos desde el último resumen: no.
        assert!(!should_summarize(99, 60));
        assert!(should_summarize(100, 60));
    }

    #[test]
    fn reminders_roundtrip() {
        let m = test_db().unwrap();
        assert!(m.pending_reminders().unwrap().is_empty());
        let id = m
            .add_reminder("2030-01-01T00:00:00+00:00", "Año nuevo")
            .unwrap();
        assert!(id > 0);
        let pending = m.pending_reminders().unwrap();
        assert_eq!(pending.len(), 1);
        assert_eq!(pending[0].text, "Año nuevo");
        m.mark_reminder_done(id).unwrap();
        assert!(m.pending_reminders().unwrap().is_empty());
    }
}
