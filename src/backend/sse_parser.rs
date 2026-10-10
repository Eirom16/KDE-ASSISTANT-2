//! Bounded, byte incremental SSE framing and OpenAI-compatible chat deltas.
use crate::models::ToolCall;
use serde::Deserialize;
use std::collections::{BTreeMap, HashSet};

pub const MAX_EVENT_BYTES: usize = 256 * 1024;
const MAX_RESPONSE_BYTES: usize = 4 * 1024 * 1024;
const MAX_TOOLS: usize = 32;

#[derive(Debug, thiserror::Error)]
pub enum StreamError {
    #[error("sse_buffer_limit: buffer excede el límite")]
    BufferLimit,
    #[error("sse_utf8: UTF-8 inválido")]
    Utf8,
    #[error("sse_malformed: {0}")]
    Malformed(String),
    #[error("provider_error: {0}")]
    Provider(String),
    #[error("sse_premature_eof: falta terminación [DONE] o evento incompleto")]
    PrematureEof,
    #[error("sse_after_done: terminación duplicada o datos posteriores")]
    AfterDone,
}

#[derive(Debug, Default, PartialEq, Eq)]
pub struct SseEvent {
    pub event: String,
    pub data: String,
    pub id: Option<String>,
    pub retry: Option<u64>,
}

#[derive(Default)]
pub struct SseParser {
    line: Vec<u8>,
    event: SseEvent,
    has_data: bool,
    skip_lf: bool,
    event_bytes: usize,
    started: bool,
    last_id: Option<String>,
}
impl SseParser {
    /// Never decode partial network chunks. A complete line is a UTF-8 boundary.
    pub fn push(&mut self, bytes: &[u8]) -> Result<Vec<SseEvent>, StreamError> {
        let mut events = Vec::new();
        for &byte in bytes {
            if self.skip_lf {
                self.skip_lf = false;
                if byte == b'\n' {
                    continue;
                }
            }
            if byte == b'\n' || byte == b'\r' {
                self.skip_lf = byte == b'\r';
                if let Some(event) = self.end_line()? {
                    events.push(event);
                }
            } else {
                if self.line.len() + self.event_bytes >= MAX_EVENT_BYTES {
                    return Err(StreamError::BufferLimit);
                }
                self.line.push(byte);
            }
        }
        Ok(events)
    }
    fn end_line(&mut self) -> Result<Option<SseEvent>, StreamError> {
        let bytes = std::mem::take(&mut self.line);
        let line = std::str::from_utf8(&bytes).map_err(|_| StreamError::Utf8)?;
        let line = if !self.started {
            self.started = true;
            line.strip_prefix('\u{feff}').unwrap_or(line)
        } else {
            line
        };
        if line.is_empty() {
            self.event_bytes = 0;
            let mut event = std::mem::take(&mut self.event);
            if std::mem::take(&mut self.has_data) {
                event.data.pop(); // Last data line's newline.
                event.id = self.last_id.clone();
                return Ok(Some(event));
            }
            return Ok(None);
        }
        self.event_bytes += bytes.len() + 1;
        if self.event_bytes > MAX_EVENT_BYTES {
            return Err(StreamError::BufferLimit);
        }
        if line.starts_with(':') {
            return Ok(None);
        }
        let (field, value) = line.split_once(':').unwrap_or((line, ""));
        let value = value.strip_prefix(' ').unwrap_or(value);
        match field {
            "data" => {
                self.has_data = true;
                self.event.data.push_str(value);
                self.event.data.push('\n');
            }
            "event" => self.event.event = value.to_string(),
            "id" if !value.contains('\0') => self.last_id = Some(value.to_string()),
            "retry" if !value.is_empty() && value.bytes().all(|b| b.is_ascii_digit()) => {
                self.event.retry = value.parse().ok();
            }
            _ => {}
        }
        Ok(None)
    }
    pub fn finish(&self) -> Result<(), StreamError> {
        if !self.line.is_empty() || self.event_bytes != 0 || self.has_data {
            Err(StreamError::PrematureEof)
        } else {
            Ok(())
        }
    }
}

#[derive(Deserialize)]
struct Chunk {
    choices: Vec<Choice>,
}
#[derive(Deserialize)]
struct Choice {
    #[serde(default)]
    index: usize,
    #[serde(default)]
    delta: Delta,
    #[serde(default)]
    finish_reason: Option<String>,
}
#[derive(Default, Deserialize)]
struct Delta {
    content: Option<String>,
    tool_calls: Option<Vec<ToolDelta>>,
}
#[derive(Deserialize)]
struct ToolDelta {
    index: usize,
    id: Option<String>,
    #[serde(rename = "type")]
    kind: Option<String>,
    function: Option<FunctionDelta>,
}
#[derive(Deserialize)]
struct FunctionDelta {
    name: Option<String>,
    arguments: Option<String>,
}

#[derive(Default)]
pub struct ChatStreamParser {
    sse: SseParser,
    tools: BTreeMap<usize, (String, String, String)>,
    pub text: String,
    done: bool,
    finished: Option<String>,
    calls: Vec<ToolCall>,
    tool_bytes: usize,
    pub duplicate_terminations: usize,
}
impl ChatStreamParser {
    /// Returns deltas immediately, validated tools only after [DONE].
    pub fn push(&mut self, bytes: &[u8]) -> Result<Vec<String>, StreamError> {
        let mut tokens = Vec::new();
        for event in self.sse.push(bytes)? {
            if self.done {
                if event.data == "[DONE]" {
                    self.duplicate_terminations += 1;
                    continue;
                }
                return Err(StreamError::AfterDone);
            }
            if event.event == "error" {
                return Err(StreamError::Provider(
                    event.data.chars().take(500).collect(),
                ));
            }
            if event.data == "[DONE]" {
                if self.text.is_empty() && self.tools.is_empty() {
                    return Err(StreamError::Malformed("respuesta vacía".into()));
                }
                let mut ids = HashSet::new();
                for (id, name, args) in self.tools.values() {
                    if id.is_empty() || name.is_empty() || !ids.insert(id.clone()) {
                        return Err(StreamError::Malformed(
                            "identidad de herramienta ausente/duplicada".into(),
                        ));
                    }
                    let arguments = serde_json::from_str(args).map_err(|e| {
                        StreamError::Malformed(format!("argumentos de herramienta inválidos: {e}"))
                    })?;
                    self.calls.push(ToolCall {
                        id: id.clone(),
                        name: name.clone(),
                        arguments,
                    });
                }
                if self.finished.as_deref() == Some("tool_calls") && self.calls.is_empty()
                    || self.finished.as_deref() == Some("stop") && !self.calls.is_empty()
                {
                    return Err(StreamError::Malformed("finish_reason inconsistente".into()));
                }
                self.done = true;
                continue;
            }
            let value: serde_json::Value = serde_json::from_str(&event.data)
                .map_err(|e| StreamError::Malformed(e.to_string()))?;
            if let Some(error) = value.get("error") {
                return Err(StreamError::Provider(
                    error.to_string().chars().take(500).collect(),
                ));
            }
            let chunk: Chunk =
                serde_json::from_value(value).map_err(|e| StreamError::Malformed(e.to_string()))?;
            if chunk.choices.len() > 1 {
                return Err(StreamError::Malformed(
                    "sólo se admite choice 0 (n=1)".into(),
                ));
            }
            for choice in chunk.choices {
                if choice.index != 0 || self.finished.is_some() {
                    return Err(StreamError::Malformed(
                        "choice inválida o delta después de finish_reason".into(),
                    ));
                }
                if let Some(text) = choice.delta.content {
                    if self.text.len() + text.len() > MAX_RESPONSE_BYTES {
                        return Err(StreamError::BufferLimit);
                    }
                    if !text.is_empty() {
                        self.text.push_str(&text);
                        tokens.push(text);
                    }
                }
                for delta in choice.delta.tool_calls.unwrap_or_default() {
                    if delta.index >= MAX_TOOLS
                        || delta.kind.as_deref().is_some_and(|v| v != "function")
                    {
                        return Err(StreamError::Malformed(
                            "índice/tipo de herramienta inválido".into(),
                        ));
                    }
                    let entry = self.tools.entry(delta.index).or_default();
                    if let Some(id) = delta.id {
                        if !entry.0.is_empty() && entry.0 != id {
                            return Err(StreamError::Malformed("id de herramienta cambió".into()));
                        }
                        self.tool_bytes += id.len();
                        entry.0 = id;
                    }
                    if let Some(function) = delta.function {
                        if let Some(name) = function.name {
                            self.tool_bytes += name.len();
                            entry.1.push_str(&name);
                        }
                        if let Some(args) = function.arguments {
                            self.tool_bytes += args.len();
                            entry.2.push_str(&args);
                        }
                    }
                    if self.tool_bytes > MAX_RESPONSE_BYTES || entry.2.len() > MAX_EVENT_BYTES {
                        return Err(StreamError::BufferLimit);
                    }
                }
                if let Some(reason) = choice.finish_reason {
                    if reason != "stop" && reason != "tool_calls" {
                        return Err(StreamError::Provider(format!(
                            "respuesta incompleta: finish_reason={reason}"
                        )));
                    }
                    self.finished = Some(reason);
                }
            }
        }
        Ok(tokens)
    }
    pub fn is_done(&self) -> bool {
        self.done
    }
    pub fn finish(self) -> Result<(String, Vec<ToolCall>), StreamError> {
        self.sse.finish()?;
        if !self.done {
            return Err(StreamError::PrematureEof);
        }
        Ok((self.text, self.calls))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn fixture(text: &str) -> Vec<u8> {
        format!(
            "data: {}\r\n\r\ndata: [DONE]\r\n\r\n",
            serde_json::json!({"choices":[{"delta":{"content":text},"finish_reason":"stop"}]})
        )
        .into_bytes()
    }
    #[test]
    fn provider_fixtures_at_every_boundary() {
        for (fixture, expected) in [
            (
                include_bytes!("../../tests/fixtures/f1b2/openai.sse").as_slice(),
                "Español 🙂",
            ),
            (
                include_bytes!("../../tests/fixtures/f1b2/groq.sse").as_slice(),
                "niño",
            ),
            (
                include_bytes!("../../tests/fixtures/f1b2/openrouter.sse").as_slice(),
                "acción",
            ),
        ] {
            for split in 0..=fixture.len() {
                let mut parser = ChatStreamParser::default();
                parser.push(&fixture[..split]).unwrap();
                parser.push(&fixture[split..]).unwrap();
                assert_eq!(parser.finish().unwrap().0, expected);
            }
        }
    }

    #[test]
    fn every_byte_boundary_and_byte_at_a_time() {
        for text in ["Español", "niño", "acción", "🦀🙂🚀"] {
            let bytes = fixture(text);
            for split in 0..=bytes.len() {
                let mut p = ChatStreamParser::default();
                let mut tokens = p.push(&bytes[..split]).unwrap();
                tokens.extend(p.push(&bytes[split..]).unwrap());
                assert_eq!(tokens.concat(), text);
                assert_eq!(p.finish().unwrap().0, text);
            }
            let mut p = ChatStreamParser::default();
            for byte in bytes {
                p.push(&[byte]).unwrap();
            }
            assert_eq!(p.finish().unwrap().0, text);
        }
    }
    #[test]
    fn exhaustive_small_event_partitions() {
        let bytes = b"data:x\n\n";
        for mask in 0..(1 << (bytes.len() - 1)) {
            let mut p = SseParser::default();
            let mut events = Vec::new();
            let mut start = 0;
            for i in 1..bytes.len() {
                if mask & (1 << (i - 1)) != 0 {
                    events.extend(p.push(&bytes[start..i]).unwrap());
                    start = i;
                }
            }
            events.extend(p.push(&bytes[start..]).unwrap());
            assert_eq!(events.len(), 1);
            assert_eq!(events[0].data, "x");
            p.finish().unwrap();
        }
    }
    #[test]
    fn bom_and_persistent_event_id() {
        let bytes = "\u{feff}id: first\ndata: uno\n\ndata: dos\n\nid:\ndata: tres\n\n".as_bytes();
        for split in 0..=bytes.len() {
            let mut p = SseParser::default();
            let mut events = p.push(&bytes[..split]).unwrap();
            events.extend(p.push(&bytes[split..]).unwrap());
            assert_eq!(events[0].id.as_deref(), Some("first"));
            assert_eq!(events[1].id.as_deref(), Some("first"));
            assert_eq!(events[2].id.as_deref(), Some(""));
        }
    }

    #[test]
    fn sse_fields_comments_multiline_and_line_endings() {
        let mut p = SseParser::default();
        let events = p.push(b": keepalive\r\nid: abc\nretry: 20\nevent: message\rdata: one\rdata:  two\r\rdata: three\n\n").unwrap();
        assert_eq!(
            events[0],
            SseEvent {
                event: "message".into(),
                data: "one\n two".into(),
                id: Some("abc".into()),
                retry: Some(20)
            }
        );
        assert_eq!(events[1].data, "three");
        p.finish().unwrap();
    }
    #[test]
    fn interleaved_tools_and_fragmented_json() {
        let deltas = [
            serde_json::json!({"tool_calls":[{"index":1,"id":"b","type":"function","function":{"name":"read_","arguments":"{\"path\":"}},{"index":0,"id":"a","function":{"name":"open_app","arguments":"{\"name\":"}}]}),
            serde_json::json!({"tool_calls":[{"index":0,"function":{"arguments":"\"niño\"}"}},{"index":1,"function":{"name":"file","arguments":"\"acción🙂\"}"}}]}),
        ];
        let bytes = format!(
            "{}data: [DONE]\n\n",
            deltas
                .iter()
                .map(|delta| format!(
                    "data: {}\n\n",
                    serde_json::json!({"choices":[{"delta":delta}]})
                ))
                .collect::<String>()
        )
        .into_bytes();
        for split in 0..=bytes.len() {
            let mut p = ChatStreamParser::default();
            p.push(&bytes[..split]).unwrap();
            p.push(&bytes[split..]).unwrap();
            let calls = p.finish().unwrap().1;
            assert_eq!(calls[0].id, "a");
            assert_eq!(calls[0].arguments["name"], "niño");
            assert_eq!(calls[1].name, "read_file");
            assert_eq!(calls[1].arguments["path"], "acción🙂");
        }
    }
    #[test]
    fn protocol_failures_never_complete() {
        for bytes in [
            b"data: {bad}\n\n".as_slice(),
            b"event: error\ndata: failed\n\n",
            b"data: {\"error\":{\"message\":\"bad\"}}\n\n",
            b"data: {\"choices\":[{\"finish_reason\":\"length\"}]}\n\n",
            b"data: [DONE]\n\n",
        ] {
            assert!(ChatStreamParser::default().push(bytes).is_err());
        }
        let bytes = fixture("x");
        for end in 0..bytes.len() - 2 {
            let mut p = ChatStreamParser::default();
            p.push(&bytes[..end]).unwrap();
            assert!(p.finish().is_err());
        }
        let mut p = ChatStreamParser::default();
        p.push(&bytes).unwrap();
        p.push(b"data: [DONE]\n\n").unwrap();
        assert_eq!(p.duplicate_terminations, 1);
        assert!(matches!(
            p.push(b"data: {}\n\n"),
            Err(StreamError::AfterDone)
        ));
        let mut duplicate = bytes;
        duplicate.extend_from_slice(b"data: [DONE]\n\n");
        let mut p = ChatStreamParser::default();
        p.push(&duplicate).unwrap();
        assert_eq!(p.duplicate_terminations, 1);
        assert_eq!(p.finish().unwrap().0, "x");
    }
    #[test]
    fn invalid_tools_utf8_and_limits() {
        for args in ["", "{bad}", "[]", "null"] {
            let bytes = format!(
                "data: {}\n\ndata: [DONE]\n\n",
                serde_json::json!({"choices":[{"delta":{"tool_calls":[{"index":0,"id":"a","function":{"name":"read_file","arguments":args}}]}}]})
            );
            assert!(ChatStreamParser::default().push(bytes.as_bytes()).is_err());
        }
        assert!(matches!(
            SseParser::default().push(&vec![b'x'; MAX_EVENT_BYTES + 1]),
            Err(StreamError::BufferLimit)
        ));
        assert!(matches!(
            SseParser::default().push(b"data: \xff\n"),
            Err(StreamError::Utf8)
        ));
    }
    #[test]
    fn cancellation_discards_partial_character_and_event() {
        let mut p = ChatStreamParser::default();
        p.push(b"data: \xf0\x9f").unwrap();
        drop(p);
        let mut p = ChatStreamParser::default();
        p.push(b"data: {\"choices\":").unwrap();
        drop(p);
        let mut next = ChatStreamParser::default();
        next.push(&fixture("nuevo")).unwrap();
        assert_eq!(next.finish().unwrap().0, "nuevo");
    }
}
