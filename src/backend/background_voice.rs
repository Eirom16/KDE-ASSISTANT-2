//! Session-scoped opt-in. Detection never creates chat messages or grants tool authority.
use super::*;
use crate::backend::{
    voice_stt,
    wake_audio::{Capture, Detector, Ring},
    wakeword,
};
type Decoder = Arc<dyn Fn() -> Result<tokio::process::Command> + Send + Sync>;
struct Sources {
    detector: tokio::process::Command,
    capture: tokio::process::Command,
    decoder: Decoder,
}
struct BufferedInput {
    ring: Arc<Ring>,
    cursor: u64,
    trigger: u64,
}
#[derive(Debug, Clone, Serialize)]
pub struct BackgroundSnapshot {
    pub enabled: bool,
    pub paused: bool,
    pub state: String,
    pub microphone_active: bool,
    pub error: Option<String>,
    pub capture_pid: Option<u32>,
    pub detector_pid: Option<u32>,
    pub activations: u64,
    pub activation: Option<Identity>,
    pub transcripts: Vec<Snapshot>,
}
struct Runner {
    cancel: Cancellation,
    task: JoinHandle<()>,
}
struct Data {
    snapshot: BackgroundSnapshot,
    runner: Option<Runner>,
    revision: u64,
}
pub(super) struct Manager {
    data: Mutex<Data>,
    control: tokio::sync::Mutex<()>,
}
impl Default for Manager {
    fn default() -> Self {
        Self {
            control: tokio::sync::Mutex::new(()),
            data: Mutex::new(Data {
                revision: 0,
                runner: None,
                snapshot: BackgroundSnapshot {
                    enabled: false,
                    paused: false,
                    state: "off".into(),
                    microphone_active: false,
                    error: None,
                    capture_pid: None,
                    detector_pid: None,
                    activations: 0,
                    activation: None,
                    transcripts: Vec::new(),
                },
            }),
        }
    }
}
impl Manager {
    fn snapshot(&self) -> BackgroundSnapshot {
        self.data
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .snapshot
            .clone()
    }
    fn mutate(&self, revision: u64, f: impl FnOnce(&mut BackgroundSnapshot)) {
        let mut d = self.data.lock().unwrap_or_else(|e| e.into_inner());
        if d.revision == revision {
            f(&mut d.snapshot);
        }
    }
    pub(super) fn revoke(&self, id: Option<&Identity>) {
        let mut d = self.data.lock().unwrap_or_else(|e| e.into_inner());
        d.snapshot
            .transcripts
            .retain(|s| id.is_some() && s.identity.as_ref() != id);
        if id.is_none() || d.snapshot.activation.as_ref() == id {
            d.snapshot.activation = None;
        }
    }
    async fn stop_locked(&self, enabled: bool, reason: &str) {
        let runner = {
            let mut d = self.data.lock().unwrap_or_else(|e| e.into_inner());
            d.revision += 1;
            d.snapshot.enabled = enabled;
            d.snapshot.paused = enabled;
            d.snapshot.state = if enabled { "paused" } else { "off" }.into();
            d.snapshot.error = if enabled { Some(reason.into()) } else { None };
            d.snapshot.activation = None;
            d.runner.take()
        };
        if let Some(runner) = runner {
            runner.cancel.cancel();
            let _ = runner.task.await;
        }
        let mut d = self.data.lock().unwrap_or_else(|e| e.into_inner());
        d.snapshot.microphone_active = false;
        d.snapshot.capture_pid = None;
        d.snapshot.detector_pid = None;
    }
    pub(super) async fn stop(&self, enabled: bool, reason: &str) {
        let _control = self.control.lock().await;
        self.stop_locked(enabled, reason).await;
    }
}
impl VoiceCoordinator {
    pub fn background_snapshot(&self) -> BackgroundSnapshot {
        self.background.snapshot()
    }
    pub async fn set_background(
        self: &Arc<Self>,
        action: &str,
        cfg: SpeechConfig,
    ) -> Result<BackgroundSnapshot> {
        anyhow::ensure!(
            ["enable", "resume", "pause", "disable"].contains(&action),
            "Acción de escucha inválida"
        );
        let _control = self.background.control.lock().await;
        {
            let inner = self.inner.lock().unwrap_or_else(|e| e.into_inner());
            if inner.closed {
                anyhow::ensure!(action == "disable", "Coordinador cerrado");
                return Ok(self.background.snapshot());
            }
        }
        let before = self.background.snapshot();
        if action == "enable" && before.enabled && !before.paused {
            return Ok(before);
        }
        if action == "resume" {
            anyhow::ensure!(before.enabled, "Activa primero Hey Jarvis");
        }
        if matches!(action, "enable" | "resume") {
            anyhow::ensure!(
                cfg.wake_word_threshold.is_finite()
                    && (0.1..=1.0).contains(&cfg.wake_word_threshold),
                "Umbral wake inválido"
            );
            anyhow::ensure!(
                (4..=120).contains(&cfg.max_recording_secs)
                    && (1..=300).contains(&cfg.stt_timeout_secs),
                "Dictado wake requiere un límite de 4 a 120 segundos"
            );
            anyhow::ensure!(
                cfg.vad_silence_rms.is_finite() && (160..=10000).contains(&cfg.vad_silence_ms),
                "Silencio wake inválido"
            );
            let inner = self.inner.lock().unwrap_or_else(|e| e.into_inner());
            anyhow::ensure!(
                !inner.closed && inner.session.is_some(),
                "Selecciona una conversación antes de activar Hey Jarvis"
            );
        }
        self.background
            .stop_locked(
                before.enabled || matches!(action, "enable" | "resume"),
                "Escucha pausada",
            )
            .await;
        let _admission = self.admission.lock().await;
        self.drain().await;
        self.warm.revoke().await;
        self.inner
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .warm_turn = None;
        if matches!(action, "pause" | "disable") {
            self.background
                .stop_locked(action == "pause" && before.enabled, "Pausa solicitada")
                .await;
            return Ok(self.background.snapshot());
        }
        let cancel = Cancellation::default();
        let signal = cancel.clone();
        let owner = self.clone();
        let mut d = self
            .background
            .data
            .lock()
            .unwrap_or_else(|e| e.into_inner());
        d.revision += 1;
        let revision = d.revision;
        d.snapshot.enabled = true;
        d.snapshot.paused = false;
        d.snapshot.state = "preparing".into();
        d.snapshot.error = None;
        let task = tokio::spawn(async move {
            let result = owner.background_loop(cfg, &signal, revision).await;
            if !signal.is_cancelled() {
                // A lost device/worker must also revoke ongoing STT/TTS, never retry capture silently.
                let _admission = owner.admission.lock().await;
                owner.drain().await;
                owner.warm.revoke().await;
                owner.background.revoke(None);
                owner.background.mutate(revision, |s| {
                    s.paused = true;
                    s.state = "error".into();
                    s.microphone_active = false;
                    s.capture_pid = None;
                    s.detector_pid = None;
                    s.error = Some(
                        result
                            .err()
                            .map(|e| e.to_string())
                            .unwrap_or_else(|| "Escucha terminada".into()),
                    );
                });
            }
        });
        d.runner = Some(Runner { cancel, task });
        drop(d);
        Ok(self.background.snapshot())
    }
    pub fn acknowledge_background(&self, generation: &str) {
        self.background
            .data
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .snapshot
            .transcripts
            .retain(|s| {
                s.identity
                    .as_ref()
                    .is_none_or(|id| id.generation != generation)
            });
    }
    fn background_eligible(&self) -> Option<String> {
        let d = self.inner.lock().unwrap_or_else(|e| e.into_inner());
        (!d.closed
            && d.warm_turn.is_none()
            && matches!(
                d.snapshot.state,
                State::Idle | State::TranscriptReady | State::Error
            ))
        .then(|| d.session.clone())
        .flatten()
    }
    async fn background_loop(
        self: &Arc<Self>,
        cfg: SpeechConfig,
        cancel: &Cancellation,
        revision: u64,
    ) -> Result<()> {
        let model = voice_stt::Selection::from_config(&cfg)?;
        let checked = voice_process::run(
            model.command(true)?,
            &[],
            4096,
            Duration::from_secs(5),
            cancel,
        )
        .await?;
        if let Some(error) = serde_json::from_slice::<Option<String>>(&checked)? {
            anyhow::bail!(error);
        }
        let configured = cfg.wake_word_model_path.clone();
        let (dir, runtime) = tokio::task::spawn_blocking(move || {
            let dir = wakeword::models_dir(&configured)?;
            wakeword::validate_models(&dir)?;
            Ok::<_, anyhow::Error>((dir, crate::backend::wakeword_ml::ensure_onnx_lib()?))
        })
        .await??;
        let selected_cfg = cfg.clone();
        let sources = Sources {
            detector: crate::backend::wake_audio::detector_command(
                &dir,
                &runtime,
                cfg.wake_word_threshold,
            )?,
            capture: voice_process::capture_command(&cfg, cancel).await?,
            decoder: Arc::new(move || {
                voice_stt::Selection::from_config(&selected_cfg)?.command(false)
            }),
        };
        self.background_sources(&cfg, cancel, revision, sources)
            .await
    }
    async fn background_sources(
        self: &Arc<Self>,
        cfg: &SpeechConfig,
        cancel: &Cancellation,
        revision: u64,
        sources: Sources,
    ) -> Result<()> {
        let mut detector = Detector::start(sources.detector, cancel).await?;
        let work = async {
            let command = sources.capture;
            anyhow::ensure!(!cancel.is_cancelled(), "Operación cancelada");
            let capture = Capture::start(command)?;
            self.background.mutate(revision, |s| {
                s.microphone_active = true;
                s.capture_pid = Some(capture.pid);
                s.detector_pid = detector.pid();
            });
            let result = self
                .background_frames(
                    cfg,
                    &capture.ring,
                    &mut detector,
                    cancel,
                    revision,
                    sources.decoder,
                )
                .await;
            capture.stop().await;
            result
        }
        .await;
        detector.stop().await;
        work
    }
    async fn background_frames(
        self: &Arc<Self>,
        cfg: &SpeechConfig,
        ring: &Arc<Ring>,
        detector: &mut Detector,
        cancel: &Cancellation,
        revision: u64,
        decoder: Decoder,
    ) -> Result<()> {
        let mut cursor = ring.cursor(0);
        let mut armed = true;
        let mut quiet = 0u32;
        let mut cooldown = 0u32;
        loop {
            let pcm = ring.read(&mut cursor, cancel).await?;
            let session = self.background_eligible();
            if session.is_none() {
                armed = false;
                quiet = 0;
                cooldown = 25;
                self.background
                    .mutate(revision, |s| s.state = "busy".into());
                cursor = ring.cursor(0);
                continue;
            }
            if !armed {
                self.background
                    .mutate(revision, |s| s.state = "rearming".into());
                cooldown = cooldown.saturating_sub(1);
                if pcm_rms(&pcm) < cfg.vad_silence_rms.clamp(0.0001, 0.5) {
                    quiet += 1;
                } else {
                    quiet = 0;
                }
                if cooldown > 0 || quiet < 8 {
                    continue;
                }
                detector.reset(cancel).await?;
                cursor = ring.cursor(0);
                armed = true;
                continue;
            }
            self.background
                .mutate(revision, |s| s.state = "listening".into());
            let score = detector.score(&pcm, cancel).await?;
            if score < cfg.wake_word_threshold {
                continue;
            }
            anyhow::ensure!(
                self.background.snapshot().transcripts.len() < 8,
                "Ocho borradores pendientes: escucha pausada hasta revisarlos"
            );
            let session = session.context("Conversación ausente")?;
            // Cursor is anchored to the classified frame, not to delayed STT load completion.
            let start = cursor.saturating_sub(25); // Two seconds of pre-roll, inside the four-second ring.
            if let Ok(id) = self
                .begin_buffered(
                    session,
                    cfg.clone(),
                    BufferedInput {
                        ring: ring.clone(),
                        cursor: start,
                        trigger: cursor,
                    },
                    decoder.clone(),
                )
                .await
            {
                self.background.mutate(revision, |s| {
                    s.activations += 1;
                    s.activation = Some(id);
                    s.state = "busy".into();
                });
            }
            armed = false;
            cooldown = 25;
            quiet = 0;
        }
    }
    async fn begin_buffered(
        self: &Arc<Self>,
        session: String,
        cfg: SpeechConfig,
        input: BufferedInput,
        decoder: Decoder,
    ) -> Result<Identity> {
        let _admission = self.admission.lock().await;
        let background = self.background_snapshot();
        anyhow::ensure!(background.enabled && !background.paused, "Escucha revocada");
        anyhow::ensure!(
            self.background_eligible().as_deref() == Some(session.as_str()),
            "Voz ocupada o sesión obsoleta"
        );
        self.drain().await;
        self.warm.revoke().await;
        let id = Identity {
            session_id: session,
            generation: uuid::Uuid::new_v4().to_string(),
            turn_id: None,
        };
        let cancel = Cancellation::default();
        let stop = Cancellation::default();
        let task_cancel = cancel.clone();
        let task_stop = stop.clone();
        let task_id = id.clone();
        let owner = self.clone();
        let mut inner = self.inner.lock().unwrap_or_else(|e| e.into_inner());
        inner.scope = uuid::Uuid::new_v4().to_string();
        inner.warm_turn = None;
        inner.snapshot = Snapshot {
            state: State::Recording,
            identity: Some(id.clone()),
            transcript: String::new(),
            error: None,
        };
        let task = tokio::spawn(async move {
            let result = async {
                let audio = buffered_pcm(
                    &input.ring,
                    input.cursor,
                    input.trigger,
                    &cfg,
                    &task_cancel,
                    &task_stop,
                )
                .await?;
                owner.update(&task_id, State::Transcribing, String::new(), None);
                owner
                    .transcribe_with_command(&audio, &cfg, decoder()?, &task_cancel)
                    .await
            }
            .await;
            match result {
                Ok(text) => {
                    owner.update(
                        &task_id,
                        State::TranscriptReady,
                        strip_activation(&text),
                        None,
                    );
                    let inner = owner.inner.lock().unwrap_or_else(|e| e.into_inner());
                    if inner.snapshot.identity.as_ref() == Some(&task_id)
                        && inner.snapshot.state == State::TranscriptReady
                    {
                        owner
                            .background
                            .data
                            .lock()
                            .unwrap_or_else(|e| e.into_inner())
                            .snapshot
                            .transcripts
                            .push(inner.snapshot.clone());
                    }
                }
                Err(e) => owner.update(&task_id, State::Error, String::new(), Some(e.to_string())),
            }
        });
        inner.operation = Some(Operation { cancel, stop, task });
        Ok(id)
    }
}
async fn buffered_pcm(
    ring: &Ring,
    mut cursor: u64,
    trigger: u64,
    cfg: &SpeechConfig,
    cancel: &Cancellation,
    stop: &Cancellation,
) -> Result<Vec<u8>> {
    let mut pcm = Vec::new();
    let mut silence = 0usize;
    let maximum = cfg.max_recording_secs as usize * 32000;
    loop {
        let frame = tokio::select! { biased;
            _ = cancel.cancelled() => anyhow::bail!("Interacción cancelada"),
            _ = stop.cancelled() => break,
            value = ring.read(&mut cursor, cancel) => value?,
        };
        if pcm_rms(&frame) < cfg.vad_silence_rms.clamp(0.0001, 0.5) {
            silence += frame.len() / 2;
        } else {
            silence = 0;
        }
        let take = frame.len().min(maximum.saturating_sub(pcm.len()));
        pcm.extend_from_slice(&frame[..take]);
        if pcm.len() >= maximum
            || (cursor >= trigger + 3
                && pcm.len() >= cfg.vad_min_record_ms as usize * 32
                && silence >= cfg.vad_silence_ms as usize * 16)
        {
            break;
        }
    }
    Ok(pcm)
}
fn strip_activation(text: &str) -> String {
    let mut words = text.split_whitespace();
    let normalize = |s: &str| s.trim_matches(|c: char| !c.is_alphabetic()).to_lowercase();
    let skip = match (words.next().map(normalize), words.next().map(normalize)) {
        (Some(a), Some(b)) if a == "hey" && b == "jarvis" => 2,
        (Some(a), _) if a == "jarvis" => 1,
        _ => 0,
    };
    if skip == 0 {
        text.trim().into()
    } else {
        text.split_whitespace()
            .skip(skip)
            .collect::<Vec<_>>()
            .join(" ")
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn fixture(code: &str) -> tokio::process::Command {
        let mut cmd = voice_process::command("/usr/bin/python3");
        cmd.args(["-c", code]);
        cmd
    }
    fn cfg() -> SpeechConfig {
        let mut c = crate::models::Config::first_run().speech;
        c.max_recording_secs = 4;
        c.vad_min_record_ms = 200;
        c.vad_silence_ms = 320;
        c.wake_word_threshold = 0.8;
        c
    }
    async fn until(condition: impl Fn() -> bool) {
        tokio::time::timeout(Duration::from_secs(8), async {
            while !condition() {
                tokio::time::sleep(Duration::from_millis(5)).await;
            }
        })
        .await
        .unwrap();
    }
    fn sources() -> Sources {
        Sources {
            detector: fixture("import sys,struct\ni=sys.stdin.buffer;o=sys.stdout.buffer;o.write(b'R');o.flush()\nwhile True:\n c=i.read(1)\n if not c:break\n if c==b'X':o.write(b'R');o.flush();continue\n assert c==b'F'\n b=i.read(2560);assert len(b)==2560\n score=1.0 if b[:2]==struct.pack('<h',1234) else 0.0\n o.write(struct.pack('<f',score));o.flush()"),
            capture: fixture("import os,time,struct\nfor n in range(700):\n v=1234 if n in (15,160) else 1500 if 16<=n<=22 or 161<=n<=167 else 0\n os.write(1,struct.pack('<h',v)*1280);time.sleep(.01)"),
            decoder: Arc::new(|| Ok(fixture("import sys,json,time\nb=sys.stdin.buffer.read();assert b'\\xdc\\x05'*(1280*7) in b,'post-wake command lost';time.sleep(.03);print(json.dumps('Hey Jarvis abre el navegador'))"))),
        }
    }
    async fn start_fixture(owner: &Arc<VoiceCoordinator>, sources: Sources) {
        owner.select_session(Some("s".into())).await;
        let mut d = owner.background.data.lock().unwrap();
        d.revision += 1;
        let rev = d.revision;
        d.snapshot.enabled = true;
        d.snapshot.paused = false;
        let cancel = Cancellation::default();
        let signal = cancel.clone();
        let o = owner.clone();
        let task = tokio::spawn(async move {
            let _ = o.background_sources(&cfg(), &signal, rev, sources).await;
        });
        d.runner = Some(Runner { cancel, task });
    }
    #[tokio::test]
    async fn two_hands_free_interactions_share_capture_rearm_and_pause_reaps_both_children() {
        let o = Arc::new(VoiceCoordinator::default());
        assert!(!o.background_snapshot().enabled);
        start_fixture(&o, sources()).await;
        until(|| o.background_snapshot().transcripts.len() == 1).await;
        let first = o.background_snapshot();
        let id = first.transcripts[0].identity.clone().unwrap();
        assert_eq!(first.transcripts[0].transcript, "abre el navegador");
        assert_eq!(first.activations, 1);
        o.acknowledge_background(&id.generation);
        until(|| {
            o.background_snapshot().activations == 2
                && o.background_snapshot().transcripts.len() == 1
        })
        .await;
        let second = o.background_snapshot();
        assert_eq!(first.capture_pid, second.capture_pid);
        assert_eq!(first.detector_pid, second.detector_pid);
        assert_ne!(
            id.generation,
            second.transcripts[0].identity.as_ref().unwrap().generation
        );
        assert_eq!(second.transcripts[0].transcript, "abre el navegador");
        assert_eq!(second.activations, 2);
        o.set_background("pause", cfg()).await.unwrap();
        let paused = o.background_snapshot();
        assert!(paused.enabled && paused.paused && !paused.microphone_active);
        for pid in [first.capture_pid, first.detector_pid]
            .into_iter()
            .flatten()
        {
            assert!(!std::path::Path::new(&format!("/proc/{pid}")).exists());
        }
        assert_eq!(
            paused.transcripts.len(),
            1,
            "Pause must not lose an undelivered draft"
        );
        o.set_background("disable", cfg()).await.unwrap();
        assert!(!o.background_snapshot().enabled);
        o.shutdown().await;
        assert!(o.set_background("enable", cfg()).await.is_err());
    }
    #[tokio::test]
    async fn cancel_one_interaction_preserves_background_authority_and_allows_the_next_activation()
    {
        let o = Arc::new(VoiceCoordinator::default());
        start_fixture(&o, sources()).await;
        until(|| o.background_snapshot().activations == 1).await;
        let id = o.background_snapshot().activation.unwrap();
        o.cancel(&id).await.unwrap();
        assert!(o.background_snapshot().enabled);
        assert!(o.background_snapshot().transcripts.is_empty());
        until(|| o.background_snapshot().activations == 2).await;
        o.set_background("disable", cfg()).await.unwrap();
        assert!(!o.background_snapshot().microphone_active);
        o.shutdown().await;
    }
    #[tokio::test]
    async fn device_eof_closes_input_and_detector_instead_of_reconnecting() {
        let o = Arc::new(VoiceCoordinator::default());
        o.select_session(Some("s".into())).await;
        {
            let mut d = o.background.data.lock().unwrap();
            d.snapshot.enabled = true;
        }
        let mut s = sources();
        s.capture = fixture("import os;os.write(1,b'\\0'*2560)");
        assert!(o
            .background_sources(&cfg(), &Cancellation::default(), 0, s)
            .await
            .is_err());
        let seen = o.background_snapshot();
        for pid in [seen.capture_pid, seen.detector_pid].into_iter().flatten() {
            assert!(!std::path::Path::new(&format!("/proc/{pid}")).exists());
        }
        o.shutdown().await;
    }
    #[test]
    fn only_the_activation_prefix_is_removed() {
        assert_eq!(
            strip_activation("Hey, Jarvis. Abre el navegador"),
            "Abre el navegador"
        );
        assert_eq!(strip_activation("Busca Hey Jarvis"), "Busca Hey Jarvis");
        assert_eq!(strip_activation("Hey Jarvis"), "");
    }
}
