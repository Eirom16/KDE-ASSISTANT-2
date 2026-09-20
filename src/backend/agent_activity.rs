//! Shared telemetry for voice and text agents. No prompts or file contents.
use serde::Serialize;
use std::collections::{HashMap, VecDeque};
use std::sync::{Arc, Mutex};
use std::time::Instant;
use tokio::sync::broadcast;

#[derive(Clone, Serialize)]
pub struct Run {
    pub id: String,
    pub model: String,
    pub channel: String,
    pub status: String,
    pub started_at: String,
    pub duration_ms: u64,
    pub tools: u64,
}

#[derive(Default)]
struct Data {
    active: HashMap<String, Run>,
    active_since: HashMap<String, Instant>,
    recent: VecDeque<Run>,
    completed: u64,
    errors: u64,
    cancelled: u64,
}

pub struct AgentActivity {
    data: Mutex<Data>,
    pub events: broadcast::Sender<serde_json::Value>,
}

impl Default for AgentActivity {
    fn default() -> Self {
        Self {
            data: Mutex::new(Data::default()),
            events: broadcast::channel(256).0,
        }
    }
}

impl AgentActivity {
    pub fn begin(self: &Arc<Self>, model: &str, voice: bool) -> RunGuard {
        let run = Run {
            id: uuid::Uuid::new_v4().to_string(),
            model: model.into(),
            channel: if voice { "voice" } else { "chat" }.into(),
            status: "running".into(),
            started_at: chrono::Utc::now().to_rfc3339(),
            duration_ms: 0,
            tools: 0,
        };
        let mut data = self.data.lock().unwrap_or_else(|e| e.into_inner());
        data.active.insert(run.id.clone(), run.clone());
        data.active_since.insert(run.id.clone(), Instant::now());
        drop(data);
        let _ = self
            .events
            .send(serde_json::json!({"type":"run", "phase":"start", "run_id":run.id}));
        RunGuard {
            activity: self.clone(),
            run,
            start: Instant::now(),
            finished: false,
        }
    }

    pub fn snapshot(&self) -> serde_json::Value {
        let data = self.data.lock().unwrap_or_else(|e| e.into_inner());
        let active = data
            .active
            .values()
            .cloned()
            .map(|mut run| {
                if let Some(started) = data.active_since.get(&run.id) {
                    run.duration_ms = started.elapsed().as_millis() as u64;
                }
                run
            })
            .collect::<Vec<_>>();
        serde_json::json!({ "scope":"since_start", "active":active,
            "recent":data.recent, "completed":data.completed, "errors":data.errors, "cancelled":data.cancelled })
    }
}

pub struct RunGuard {
    activity: Arc<AgentActivity>,
    run: Run,
    start: Instant,
    finished: bool,
}

impl RunGuard {
    pub fn tool(&mut self, id: &str, name: &str, phase: &str) {
        if phase == "start" {
            self.run.tools += 1;
            self.activity
                .data
                .lock()
                .unwrap_or_else(|e| e.into_inner())
                .active
                .insert(self.run.id.clone(), self.run.clone());
        }
        let scene = match name {
            "find_file" | "find_document" | "web_search" => "search",
            "open_app" | "open_url" | "focus_app" | "close_app" => "openapp",
            "create_file" | "edit_file" | "read_file" | "open_file" | "copy_file"
            | "preview_document" => "file",
            "show_image" => "download",
            "system_info" | "network_status" | "volume" | "brightness" | "media" | "kdeconnect"
            | "list_open_apps" => "system",
            "notify" | "remind_in" => "success",
            "screenshot" => "screenshot",
            _ => "system",
        };
        let _ = self
            .activity
            .events
            .send(serde_json::json!({"type":"intent", "run_id":self.run.id,
            "tool_call_id":id, "tool":name, "scene":scene, "phase":phase}));
    }

    pub fn finish(&mut self, status: &str) {
        if self.finished {
            return;
        }
        self.finished = true;
        self.run.status = status.into();
        self.run.duration_ms = self.start.elapsed().as_millis() as u64;
        let mut data = self.activity.data.lock().unwrap_or_else(|e| e.into_inner());
        data.active.remove(&self.run.id);
        data.active_since.remove(&self.run.id);
        match status {
            "completed" => data.completed += 1,
            "error" => data.errors += 1,
            _ => data.cancelled += 1,
        }
        data.recent.push_front(self.run.clone());
        data.recent.truncate(100);
        let _ = self
            .activity
            .events
            .send(serde_json::json!({"type":"run", "phase":status, "run_id":self.run.id}));
    }
}

impl Drop for RunGuard {
    fn drop(&mut self) {
        self.finish("cancelled");
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn aborted_run_is_closed_and_events_keep_identity() {
        let activity = Arc::new(AgentActivity::default());
        let mut rx = activity.events.subscribe();
        let mut run = activity.begin("test", true);
        assert_eq!(activity.snapshot()["active"].as_array().unwrap().len(), 1);
        run.tool("call-1", "find_file", "start");
        run.tool("call-1", "find_file", "error");
        drop(run);
        let events: Vec<_> = std::iter::from_fn(|| rx.try_recv().ok()).collect();
        assert_eq!(events[1]["tool_call_id"], events[2]["tool_call_id"]);
        assert_eq!(events[1]["scene"], "search");
        assert_eq!(activity.snapshot()["cancelled"], 1);
        assert_eq!(activity.snapshot()["active"].as_array().unwrap().len(), 0);
    }

    #[test]
    fn active_snapshot_reports_elapsed_duration() {
        let activity = Arc::new(AgentActivity::default());
        let _run = activity.begin("test", false);
        std::thread::sleep(std::time::Duration::from_millis(2));
        let snapshot = activity.snapshot();
        assert!(snapshot["active"][0]["duration_ms"].as_u64().unwrap() > 0);
    }
}
