//! Session Manager - SQLite CRUD para sesiones y mensajes
//!
//! Fase 1: crea DB, tablas e indices. CRUD completo en Fase 2.

use anyhow::Result;
use rusqlite::Connection;
use std::path::PathBuf;

pub struct SessionManager {
    conn: Connection,
    db_path: PathBuf,
}

impl SessionManager {
    pub async fn new() -> Result<Self> {
        // Path de la DB: ~/.local/share/kde-assistant/sessions.db
        let data_dir = dirs::data_local_dir()
            .ok_or_else(|| anyhow::anyhow!("No se pudo obtener data_local_dir"))?
            .join("kde-assistant");

        std::fs::create_dir_all(&data_dir)?;
        let db_path = data_dir.join("sessions.db");

        log::info!("SQLite DB: {}", db_path.display());

        let conn = Connection::open(&db_path)?;

        // Crear tablas
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

        log::info!("Esquema SQLite inicializado");

        Ok(Self { conn, db_path })
    }

    // TODO(Fase 2): CRUD methods
    // - create_session(title) -> Session
    // - list_sessions() -> Vec<Session>
    // - get_messages(session_id) -> Vec<Message>
    // - add_message(session_id, message) -> ()
    // - delete_session(id) -> ()
    // - update_session_title(id, title) -> ()
}
