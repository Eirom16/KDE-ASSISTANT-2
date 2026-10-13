//! Bounded context reconstruction. Original history is never rewritten or summarized.
//! Size guards use bundled SQLite octet_length(column), which skips overflow payload reads.
use super::session_manager::SessionManager;
use crate::models::{AiConfig, Message, Tool};
use anyhow::{ensure, Context, Result};
use rusqlite::{params, OptionalExtension};
use serde::Serialize;
use std::{collections::HashSet, io::Write};

struct Counter(usize);
impl Write for Counter {
    fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
        self.0 = self.0.saturating_add(buf.len());
        Ok(buf.len())
    }
    fn flush(&mut self) -> std::io::Result<()> {
        Ok(())
    }
}
// Includes JSON escaping of arguments a second time in OpenAI's string field,
// message framing, and a conservative one-token-per-byte envelope.
fn charge(value: &impl Serialize) -> Result<usize> {
    let mut count = Counter(0);
    serde_json::to_writer(&mut count, value)?;
    Ok(count.0.saturating_mul(2).saturating_add(64))
}
pub fn input_budget(cfg: &AiConfig, tools: &[Tool], response: u32) -> Result<usize> {
    ensure!(
        response > 0 && response < cfg.context_window_tokens,
        "context_budget_invalid: response reservation exceeds window"
    );
    let total = cfg
        .max_context_bytes
        .min((cfg.context_window_tokens - response) as usize);
    total
        .checked_sub(charge(&tools)?)
        .filter(|v| *v > 0)
        .context("context_budget_exceeded: essential tool schemas exceed budget")
}
pub fn validate_groups(messages: &[Message]) -> Result<()> {
    let mut pending = HashSet::new();
    for message in messages {
        match message {
            Message::Tool { tool_call_id, .. } => {
                ensure!(
                    pending.remove(tool_call_id.as_str()),
                    "context_invalid: orphan or duplicate tool result"
                );
            }
            Message::Assistant { tool_calls, .. } => {
                ensure!(pending.is_empty(), "context_invalid: incomplete tool group");
                for call in tool_calls {
                    ensure!(
                        !call.id.is_empty() && pending.insert(call.id.as_str()),
                        "context_invalid: duplicate or missing tool identity"
                    );
                }
            }
            _ => ensure!(pending.is_empty(), "context_invalid: incomplete tool group"),
        }
    }
    ensure!(pending.is_empty(), "context_invalid: incomplete tool group");
    Ok(())
}
pub fn validate_request(
    cfg: &AiConfig,
    messages: &[Message],
    tools: &[Tool],
    response: u32,
) -> Result<()> {
    validate_groups(messages)?;
    ensure!(
        charge(&messages)? <= input_budget(cfg, tools, response)?,
        "context_budget_exceeded: request including response reservation exceeds budget"
    );
    Ok(())
}
impl SessionManager {
    /// Bounded legacy summary input; never used as conversation context replacement.
    pub(crate) fn summary_excerpt(&self, session: &str) -> Result<(String, i64, String)> {
        let (previous,count):(String,i64)=self.conn.query_row("SELECT substr(COALESCE(summary,''),1,1500),COALESCE(summary_count,0) FROM sessions WHERE id=?1",[session],|r|Ok((r.get(0)?,r.get(1)?)))?;
        let total = self.count_messages(session)?;
        let end = total.saturating_sub(20).max(0);
        let start = count.clamp(0, end);
        let mut text = String::new();
        let mut stmt=self.conn.prepare("SELECT role,substr(content,1,600) FROM messages WHERE session_id=?1 ORDER BY id LIMIT ?2 OFFSET ?3")?;
        let mut rows = stmt.query(params![session, (end - start).min(32), start])?;
        while let Some(row) = rows.next()? {
            let role: String = row.get(0)?;
            let content: String = row.get(1)?;
            if text.len() + role.len() + content.len() + 4 > 12000 {
                break;
            }
            text.push_str(&format!("[{role}] {content}\n"));
        }
        Ok((previous, count, text))
    }
    pub fn context_facts(&self, max_bytes: usize) -> Result<Vec<super::session_manager::UserFact>> {
        let size: i64 = self.conn.query_row("SELECT COALESCE(SUM(octet_length(key)+octet_length(value)+octet_length(updated_at)+128),0) FROM user_facts",[],|r|r.get(0))?;
        ensure!(
            size as usize <= max_bytes,
            "context_budget_exceeded: essential remembered facts"
        );
        self.list_facts()
    }
    pub fn retry_input(&self, session: &str, max_bytes: usize) -> Result<String> {
        let (id,role,size):(i64,String,i64)=self.conn.query_row("SELECT id,role,octet_length(content) FROM messages WHERE session_id=?1 ORDER BY id DESC LIMIT 1",[session],|r|Ok((r.get(0)?,r.get(1)?,r.get(2)?)))?;
        ensure!(role == "user", "regeneration_requires_unanswered_user");
        ensure!(
            size as usize <= max_bytes,
            "context_budget_exceeded: retry input"
        );
        Ok(self
            .conn
            .query_row("SELECT content FROM messages WHERE id=?1", [id], |r| {
                r.get(0)
            })?)
    }

    /// A causal suffix of user input units. Retries share an input through turn_inputs.
    /// Metadata is read before payloads; oversized units never enter memory.
    pub fn select_context(
        &self,
        session: Option<&str>,
        essential: Vec<Message>,
        current: Option<Message>,
        cfg: &AiConfig,
        tools: &[Tool],
    ) -> Result<Vec<Message>> {
        let mut budget = input_budget(cfg, tools, cfg.max_tokens)?;
        let mut prefix = essential;
        let mut historical_systems = Vec::new();
        let mut selected_start = i64::MAX;
        if let Some(sid) = session {
            let size: i64 = self.conn.query_row("SELECT COALESCE(SUM(octet_length(content)+128),0) FROM messages WHERE session_id=?1 AND role='system'", [sid], |r|r.get(0))?;
            ensure!(
                size as usize <= budget,
                "context_budget_exceeded: essential historical instructions"
            );
            let mut stmt = self.conn.prepare(
                "SELECT id,content FROM messages WHERE session_id=?1 AND role='system' ORDER BY id",
            )?;
            for row in
                stmt.query_map([sid], |r| Ok((r.get::<_, i64>(0)?, r.get::<_, String>(1)?)))?
            {
                let (id, content) = row?;
                historical_systems.push((id, Message::system(content)));
            }
            let uncertain: bool=self.conn.query_row("SELECT EXISTS(SELECT 1 FROM operation_journal WHERE session_id=?1 AND state IN ('unknown_outcome','dispatched'))",[sid],|r|r.get(0))?;
            if uncertain {
                prefix.push(Message::system("Esta sesión conserva operaciones inciertas. La operación pudo haberse realizado, pero no se pudo confirmar su resultado. No supongas éxito ni rollback ni repitas acciones previas sin una solicitud nueva y explícita después de inspeccionar el posible efecto."));
            }
        }
        let fixed = charge(&prefix)?
            + charge(&historical_systems)?
            + current.as_ref().map(charge).transpose()?.unwrap_or(0)
            + 128;
        ensure!(
            fixed <= budget,
            "context_budget_exceeded: essential instructions or current input"
        );
        budget -= fixed;
        let mut units = Vec::new();
        if let Some(sid) = session {
            let mut end = i64::MAX;
            let mut newest = true;
            loop {
                let start: Option<i64> = self.conn.query_row("SELECT id FROM messages WHERE session_id=?1 AND role='user' AND id<?2 ORDER BY id DESC LIMIT 1", params![sid,end], |r|r.get(0)).optional()?;
                let start = start.unwrap_or(0);
                let (size, count): (i64,i64) = self.conn.query_row("SELECT COALESCE(SUM(octet_length(content)+COALESCE(octet_length(tool_result),0)+COALESCE(octet_length(tool_call_id),0)+COALESCE(octet_length(image_url),0)+octet_length(role)+256),0),COUNT(*) FROM messages WHERE session_id=?1 AND id>=?2 AND id<?3 AND role!='system'",params![sid,start,end],|r|Ok((r.get(0)?,r.get(1)?)))?;
                if count == 0 {
                    if start == 0 {
                        break;
                    }
                    end = start;
                    continue;
                }
                if size as usize > budget {
                    ensure!(!newest, "context_budget_exceeded: most recent logical turn");
                    break;
                }
                let mut group = Vec::new();
                let mut stmt = self.conn.prepare("SELECT role,content,tool_call_id,tool_result,image_url FROM messages WHERE session_id=?1 AND id>=?2 AND id<?3 ORDER BY id")?;
                let mut rows = stmt.query(params![sid, start, end])?;
                while let Some(row) = rows.next()? {
                    let content: String = row.get(1)?;
                    group.push(match row.get::<_, String>(0)?.as_str() {
                        "system" => Message::system(content),
                        "user" => Message::user(content),
                        "assistant" => Message::assistant_with_tools(
                            content,
                            row.get::<_, Option<String>>(3)?
                                .map(|s| serde_json::from_str(&s))
                                .transpose()
                                .context("context_invalid: malformed stored tool calls")?
                                .unwrap_or_default(),
                        ),
                        "tool" => Message::tool_with_image(
                            row.get::<_, Option<String>>(2)?
                                .context("context_invalid: missing tool result identity")?,
                            content,
                            row.get(4)?,
                        ),
                        _ => anyhow::bail!("context_invalid: unknown historical role"),
                    });
                }
                let inconsistent: bool=self.conn.query_row("SELECT EXISTS(SELECT 1 FROM messages m LEFT JOIN conversation_turns t ON t.id=m.turn_id WHERE m.session_id=?1 AND m.id>=?2 AND m.id<?3 AND m.role IN ('assistant','tool') AND m.turn_id IS NOT NULL AND (t.id IS NULL OR t.session_id IS NOT m.session_id OR t.state!='completed'))",params![sid,start,end],|r|r.get(0))?;
                ensure!(
                    !inconsistent,
                    "context_invalid: uncompleted or unrelated historical output"
                );
                // Strict policy: don't silently hide an incomplete legacy group.
                validate_groups(&group)?;
                let unresolved: bool = self.conn.query_row("SELECT EXISTS(SELECT 1 FROM operation_journal j WHERE j.session_id=?1 AND j.state IN ('unknown_outcome','failed','abandoned','dispatched') AND j.turn_id IN (SELECT turn_id FROM turn_inputs WHERE user_message_id=?2 UNION SELECT turn_id FROM messages WHERE session_id=?1 AND id>=?2 AND id<?3 AND turn_id IS NOT NULL))", params![sid,start,end], |r|r.get(0))?;
                let interrupted: bool = self.conn.query_row("SELECT EXISTS(SELECT 1 FROM conversation_turns t WHERE t.session_id=?1 AND t.state IN ('failed','cancelled') AND t.id IN (SELECT turn_id FROM turn_inputs WHERE user_message_id=?2 UNION SELECT turn_id FROM messages WHERE session_id=?1 AND id>=?2 AND id<?3 AND turn_id IS NOT NULL))", params![sid,start,end], |r|r.get(0))?;
                if unresolved || interrupted {
                    group.push(Message::system("Este input tiene intentos cancelados, fallidos o con operaciones abandonadas/inciertas. No supongas éxito ni rollback. No repitas acciones anteriores sin una solicitud nueva y explícita después de inspeccionar el posible efecto."));
                }
                let cost = charge(&group)?;
                if cost > budget {
                    ensure!(!newest, "context_budget_exceeded: most recent logical turn");
                    break;
                }
                budget -= cost;
                selected_start = start;
                units.push(group);
                newest = false;
                if start == 0 {
                    break;
                }
                end = start;
            }
        }
        prefix.extend(
            historical_systems
                .into_iter()
                .filter(|(id, _)| *id < selected_start)
                .map(|(_, message)| message),
        );
        for group in units.into_iter().rev() {
            prefix.extend(group);
        }
        if let Some(current) = current {
            prefix.push(current);
        }
        validate_request(cfg, &prefix, tools, cfg.max_tokens)?;
        Ok(prefix)
    }
}
