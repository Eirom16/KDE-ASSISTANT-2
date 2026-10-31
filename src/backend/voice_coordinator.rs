//! Sole production owner of voice operations. Dictation has no AI/tools/DB writer.
use super::voice_process::{self, Cancellation};
use crate::models::config::SpeechConfig;
use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::{
    path::PathBuf,
    sync::{Arc, Mutex},
    time::Duration,
};
use tokio::{io::AsyncReadExt, task::JoinHandle};
#[path = "background_voice.rs"]
mod background;
pub use background::BackgroundSnapshot;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum State {
    Disabled,
    Idle,
    Preparing,
    Listening,
    Recording,
    Transcribing,
    TranscriptReady,
    Thinking,
    Speaking,
    Cancelling,
    Error,
    WakeWaiting,
}
impl State {
    pub fn can_reach(self, next: Self) -> bool {
        use State::*;
        if self == next {
            return true;
        }
        if matches!(next, Cancelling | Error) {
            return !matches!(self, Disabled);
        }
        match self {
            Disabled => next == Idle,
            Idle | TranscriptReady | Error => matches!(next, Preparing | Disabled | Idle),
            Preparing => matches!(next, Recording | Speaking | Listening | Idle | WakeWaiting),
            Recording | Listening => matches!(next, Transcribing | Idle),
            Transcribing => next == TranscriptReady,
            Speaking | Cancelling => next == Idle,
            Thinking => matches!(next, Speaking | Idle),
            WakeWaiting => matches!(next, Preparing | Idle),
        }
    }
}
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct Identity {
    pub session_id: String,
    pub generation: String,
    pub turn_id: Option<String>,
}
#[derive(Debug, Clone, Serialize)]
pub struct Snapshot {
    pub state: State,
    pub identity: Option<Identity>,
    pub transcript: String,
    pub error: Option<String>,
}
struct Operation {
    cancel: Cancellation,
    stop: Cancellation,
    task: JoinHandle<()>,
}
struct Inner {
    snapshot: Snapshot,
    operation: Option<Operation>,
    session: Option<String>,
    scope: String,
    closed: bool,
    warm_turn: Option<Identity>,
}
pub struct VoiceCoordinator {
    background: background::Manager,
    inner: Mutex<Inner>,
    admission: tokio::sync::Mutex<()>,
    warm: super::warm_piper::WarmPiperManager,
}
impl Default for VoiceCoordinator {
    fn default() -> Self {
        Self {
            background: background::Manager::default(),
            inner: Mutex::new(Inner {
                snapshot: Snapshot {
                    state: State::Idle,
                    identity: None,
                    transcript: String::new(),
                    error: None,
                },
                operation: None,
                session: None,
                scope: uuid::Uuid::new_v4().to_string(),
                closed: false,
                warm_turn: None,
            }),
            admission: tokio::sync::Mutex::new(()),
            warm: super::warm_piper::WarmPiperManager::default(),
        }
    }
}
impl VoiceCoordinator {
    pub fn fast_snapshot(&self) -> super::warm_piper::Snapshot {
        self.warm.snapshot()
    }
    pub async fn configure(&self, cfg: &SpeechConfig) {
        let enabled = self.background_snapshot().enabled;
        self.background
            .stop(enabled, "Configuración cambiada; pulsa Reanudar")
            .await;
        let _admission = self.admission.lock().await;
        self.drain().await;
        self.warm.configure(cfg).await;
        let mut inner = self.inner.lock().unwrap_or_else(|e| e.into_inner());
        inner.scope = uuid::Uuid::new_v4().to_string();
        inner.warm_turn = None;
    }
    /// Admission only. Loading happens in the manager, never in the LLM stream.
    pub async fn prepare_for_turn(
        &self,
        session: &str,
        turn: &str,
        scope: &str,
        cfg: &SpeechConfig,
    ) {
        if !cfg.piper_fast || !cfg.auto_speak {
            return;
        }
        let _admission = self.admission.lock().await;
        {
            let mut inner = self.inner.lock().unwrap_or_else(|e| e.into_inner());
            if inner.closed
                || inner.session.as_deref() != Some(session)
                || inner.scope != scope
                || !matches!(
                    inner.snapshot.state,
                    State::Idle | State::TranscriptReady | State::Error
                )
            {
                return;
            }
            inner.warm_turn = Some(Identity {
                session_id: session.into(),
                generation: scope.into(),
                turn_id: Some(turn.into()),
            });
        }
        let _ = self.warm.prepare(cfg, session).await;
    }
    pub fn snapshot(&self) -> Snapshot {
        self.inner
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .snapshot
            .clone()
    }
    fn update(&self, id: &Identity, state: State, text: String, error: Option<String>) {
        let mut inner = self.inner.lock().unwrap_or_else(|e| e.into_inner());
        if inner.snapshot.identity.as_ref() == Some(id)
            && inner.snapshot.state != State::Cancelling
            && inner.snapshot.state.can_reach(state)
        {
            if inner.snapshot.state != state {
                static CLOCK: once_cell::sync::Lazy<std::time::Instant> =
                    once_cell::sync::Lazy::new(std::time::Instant::now);
                log::info!(
                    "voice_transition {}",
                    serde_json::json!({
                        "generation": id.generation, "from": inner.snapshot.state,
                        "to": state, "monotonic_ms": CLOCK.elapsed().as_secs_f64() * 1000.0,
                    })
                );
            }
            inner.snapshot.state = state;
            inner.snapshot.transcript = text;
            inner.snapshot.error = error;
        }
    }
    async fn drain(&self) {
        let op = {
            let mut inner = self.inner.lock().unwrap_or_else(|e| e.into_inner());
            let op = inner.operation.take();
            if let Some(op) = &op {
                op.cancel.cancel();
                inner.snapshot.state = State::Cancelling;
            }
            op
        };
        if let Some(op) = op {
            let _ = op.task.await;
        }
        let mut inner = self.inner.lock().unwrap_or_else(|e| e.into_inner());
        inner.snapshot = Snapshot {
            state: State::Idle,
            identity: None,
            transcript: String::new(),
            error: None,
        };
    }
    pub async fn select_session(&self, session: Option<String>) -> String {
        let _admission = self.admission.lock().await;
        {
            let inner = self.inner.lock().unwrap_or_else(|e| e.into_inner());
            if inner.session == session {
                return inner.scope.clone();
            }
        }
        self.drain().await;
        self.background.revoke(None);
        self.warm.revoke().await;
        let mut inner = self.inner.lock().unwrap_or_else(|e| e.into_inner());
        inner.session = session;
        inner.warm_turn = None;
        inner.scope = uuid::Uuid::new_v4().to_string();
        inner.scope.clone()
    }
    pub fn scope_for(&self, session: Option<&str>) -> Option<String> {
        let inner = self.inner.lock().unwrap_or_else(|e| e.into_inner());
        (inner.session.as_deref() == session && session.is_some()).then(|| inner.scope.clone())
    }
    pub async fn shutdown(&self) {
        self.background.stop(false, "Cierre").await;
        let _admission = self.admission.lock().await;
        self.drain().await;
        self.warm.shutdown().await;
        let mut inner = self.inner.lock().unwrap_or_else(|e| e.into_inner());
        inner.warm_turn = None;
        inner.closed = true;
        inner.snapshot.state = State::Disabled;
    }
    pub async fn cancel(&self, id: &Identity) -> Result<()> {
        let _admission = self.admission.lock().await;
        anyhow::ensure!(
            {
                let inner = self.inner.lock().unwrap_or_else(|e| e.into_inner());
                inner.snapshot.identity.as_ref() == Some(id)
                    || (inner.scope == id.generation
                        && inner.session.as_ref() == Some(&id.session_id))
            },
            "Operación de voz obsoleta"
        );
        self.background.revoke(Some(id));
        self.drain().await;
        self.warm.revoke().await;
        let mut inner = self.inner.lock().unwrap_or_else(|e| e.into_inner());
        inner.scope = uuid::Uuid::new_v4().to_string();
        inner.warm_turn = None;
        Ok(())
    }
    pub fn stop(&self, id: &Identity) -> Result<()> {
        let inner = self.inner.lock().unwrap_or_else(|e| e.into_inner());
        anyhow::ensure!(
            inner.snapshot.identity.as_ref() == Some(id),
            "Operación de voz obsoleta"
        );
        anyhow::ensure!(
            matches!(inner.snapshot.state, State::Preparing | State::Recording),
            "No hay captura activa"
        );
        if let Some(op) = &inner.operation {
            op.stop.cancel();
        }
        Ok(())
    }
    pub async fn start(
        self: &Arc<Self>,
        session: String,
        scope: String,
        cfg: SpeechConfig,
    ) -> Result<Snapshot> {
        self.start_operation(session, scope, cfg, false).await
    }
    pub async fn start_wake(
        self: &Arc<Self>,
        session: String,
        scope: String,
        cfg: SpeechConfig,
    ) -> Result<Snapshot> {
        anyhow::ensure!(
            cfg.wake_word_threshold.is_finite() && (0.1..=1.0).contains(&cfg.wake_word_threshold),
            "Umbral wake inválido"
        );
        self.start_operation(session, scope, cfg, true).await
    }
    async fn start_operation(
        self: &Arc<Self>,
        session: String,
        scope: String,
        cfg: SpeechConfig,
        wake: bool,
    ) -> Result<Snapshot> {
        anyhow::ensure!(
            !self.background_snapshot().enabled,
            "Pausa o desactiva la escucha de fondo antes de abrir otro micrófono"
        );
        anyhow::ensure!(
            (1..=120).contains(&cfg.max_recording_secs)
                && (1..=300).contains(&cfg.stt_timeout_secs),
            "Límites de grabación o STT inválidos"
        );
        anyhow::ensure!(
            cfg.stt_language.len() <= 16
                && !cfg.stt_language.is_empty()
                && cfg
                    .stt_language
                    .bytes()
                    .all(|b| b.is_ascii_alphabetic() || b == b'-'),
            "Idioma STT inválido"
        );
        anyhow::ensure!(
            cfg.vad_silence_rms.is_finite()
                && cfg.vad_silence_ms <= 10000
                && cfg.vad_min_record_ms <= 120000,
            "Configuración de silencio inválida"
        );
        let _admission = self.admission.lock().await;
        {
            let inner = self.inner.lock().unwrap_or_else(|e| e.into_inner());
            anyhow::ensure!(
                !inner.closed && inner.session.as_ref() == Some(&session) && inner.scope == scope,
                "Selecciona una conversación válida antes de dictar"
            );
            anyhow::ensure!(
                !self.background_snapshot().enabled,
                "La escucha de fondo es dueña del micrófono"
            );
            anyhow::ensure!(
                matches!(
                    inner.snapshot.state,
                    State::Idle | State::Error | State::TranscriptReady
                ),
                "Voz ocupada"
            );
        }
        self.drain().await;
        self.warm.revoke().await;
        let model = super::voice_stt::Selection::from_config(&cfg)?;
        let id = Identity {
            session_id: session,
            generation: scope,
            turn_id: None,
        };
        let cancel = Cancellation::default();
        let stop = Cancellation::default();
        let mut inner = self.inner.lock().unwrap_or_else(|e| e.into_inner());
        inner.scope = uuid::Uuid::new_v4().to_string();
        inner.warm_turn = None;
        inner.snapshot = Snapshot {
            state: State::Preparing,
            identity: Some(id.clone()),
            transcript: String::new(),
            error: None,
        };
        let owner = self.clone();
        let operation_id = id.clone();
        let task_cancel = cancel.clone();
        let task_stop = stop.clone();
        let task = tokio::spawn(async move {
            let result = async {
                if wake {
                    // Validate STT before arming, and revoke prepared Piper before any capture.
                    let checked = voice_process::run(
                        model.command(true)?,
                        &[],
                        4096,
                        Duration::from_secs(5),
                        &task_cancel,
                    )
                    .await?;
                    if let Some(error) = serde_json::from_slice::<Option<String>>(&checked)? {
                        anyhow::bail!(error);
                    }
                    let configured = cfg.wake_word_model_path.clone();
                    let (dir, runtime) = tokio::task::spawn_blocking(move || {
                        let dir = super::wakeword::models_dir(&configured)?;
                        super::wakeword::validate_models(&dir)?;
                        let runtime = super::wakeword_ml::ensure_onnx_lib()?;
                        Ok::<_, anyhow::Error>((dir, runtime))
                    })
                    .await??;
                    let detector =
                        super::wakeword::detector_command(&dir, &runtime, cfg.wake_word_threshold)?;
                    let capture = voice_process::capture_command(&cfg, &task_cancel).await?;
                    super::wakeword::wait(detector, capture, &task_cancel, || {
                        owner.update(&operation_id, State::WakeWaiting, String::new(), None);
                    })
                    .await?;
                    anyhow::ensure!(!task_cancel.is_cancelled(), "Operación cancelada");
                    owner.update(&operation_id, State::Preparing, String::new(), None);
                }
                owner
                    .dictate(&operation_id, cfg, model, &task_cancel, &task_stop)
                    .await
            }
            .await;
            match result {
                Ok(Some(text)) => owner.update(&operation_id, State::TranscriptReady, text, None),
                Ok(None) => owner.update(&operation_id, State::Idle, String::new(), None),
                Err(error) => owner.update(
                    &operation_id,
                    State::Error,
                    String::new(),
                    Some(error.to_string()),
                ),
            }
        });
        inner.operation = Some(Operation { cancel, stop, task });
        Ok(inner.snapshot.clone())
    }
    async fn dictate(
        &self,
        id: &Identity,
        cfg: SpeechConfig,
        model: super::voice_stt::Selection,
        cancel: &Cancellation,
        stop: &Cancellation,
    ) -> Result<Option<String>> {
        if stop.is_cancelled() {
            return Ok(None);
        }
        // Validate before opening the microphone; no downloader or remote fallback.
        let check = model.command(true)?;
        let checked = voice_process::run(check, &[], 4096, Duration::from_secs(5), cancel).await?;
        let error: Option<String> = serde_json::from_slice(&checked)?;
        if let Some(error) = error {
            anyhow::bail!(error);
        }
        anyhow::ensure!(!cancel.is_cancelled(), "Operación cancelada");
        if stop.is_cancelled() {
            return Ok(None);
        }
        let cmd = voice_process::capture_command(&cfg, cancel).await?;
        if stop.is_cancelled() {
            return Ok(None);
        }
        let audio = self.capture_pcm(id, &cfg, cmd, cancel, stop).await?;

        anyhow::ensure!(!cancel.is_cancelled(), "Operación cancelada");
        if audio.is_empty() && stop.is_cancelled() {
            return Ok(None);
        }
        self.update(id, State::Transcribing, String::new(), None);
        Ok(Some(
            self.transcribe_pcm(&audio, &cfg, model, cancel).await?,
        ))
    }
    async fn transcribe_pcm(
        &self,
        audio: &[u8],
        cfg: &SpeechConfig,
        model: super::voice_stt::Selection,
        cancel: &Cancellation,
    ) -> Result<String> {
        self.transcribe_with_command(audio, cfg, model.command(false)?, cancel)
            .await
    }
    async fn transcribe_with_command(
        &self,
        audio: &[u8],
        cfg: &SpeechConfig,
        cmd: tokio::process::Command,
        cancel: &Cancellation,
    ) -> Result<String> {
        if audio.len() < 3200 || pcm_rms(audio) < 0.0001 {
            return Ok(String::new());
        }
        let data = voice_process::run(
            cmd,
            audio,
            64 * 1024,
            Duration::from_secs(cfg.stt_timeout_secs.clamp(1, 300)),
            cancel,
        )
        .await?;
        let text: String = serde_json::from_slice(&data).context("Respuesta STT inválida")?;
        Ok(text)
    }
    async fn capture_pcm(
        &self,
        id: &Identity,
        cfg: &SpeechConfig,
        mut cmd: tokio::process::Command,
        cancel: &Cancellation,
        stop: &Cancellation,
    ) -> Result<Vec<u8>> {
        anyhow::ensure!(!cancel.is_cancelled(), "Operación cancelada");
        if stop.is_cancelled() {
            return Ok(Vec::new());
        }
        let mut child = cmd.spawn().context("PipeWire no disponible (pw-record)")?;
        let mut stdout = child.stdout.take().context("Captura sin salida PCM")?;
        drop(child.stdin.take());
        let capture = async {
            let mut audio = Vec::new(); let mut chunk = [0u8; 3200];
            let max_bytes = cfg.max_recording_secs.clamp(1, 120) as usize * 32000;
            let deadline = tokio::time::Instant::now() + Duration::from_secs(cfg.max_recording_secs.clamp(1, 120) + 3);
            let mut silent_samples = 0usize;
            loop {
                let count = tokio::select! { biased;
                    _ = cancel.cancelled() => anyhow::bail!("Operación cancelada"),
                    _ = stop.cancelled() => break,
                    _ = tokio::time::sleep_until(deadline) => break,
                    result = tokio::time::timeout(Duration::from_secs(3), stdout.read(&mut chunk)) => result.context("Micrófono sin datos: comprueba conexión y permisos de audio")??,
                };
                anyhow::ensure!(count > 0, "Micrófono desconectado, ocupado o acceso de audio denegado");
                let available = max_bytes.saturating_sub(audio.len());
                audio.extend_from_slice(&chunk[..count.min(available)]);
                self.update(id, State::Recording, String::new(), None);
                // PCM16 has fixed framing; retain a trailing byte for the next read.
                let aligned = audio.len() / 2 * 2;
                let frame = &audio[aligned.saturating_sub(count / 2 * 2)..aligned];
                let rms = pcm_rms(frame);
                if rms < cfg.vad_silence_rms.clamp(0.0001, 0.5) { silent_samples += frame.len() / 2; } else { silent_samples = 0; }
                if audio.len() >= max_bytes || (audio.len() >= cfg.vad_min_record_ms as usize * 32 && silent_samples >= cfg.vad_silence_ms as usize * 16) { break; }
            }
            audio.truncate(audio.len() / 2 * 2);
            Ok::<_, anyhow::Error>(audio)
        }.await;
        let _ = child.kill().await;
        let _ = child.wait().await;
        capture
    }
}
impl Drop for VoiceCoordinator {
    fn drop(&mut self) {
        if let Some(op) = &self
            .inner
            .get_mut()
            .unwrap_or_else(|e| e.into_inner())
            .operation
        {
            op.cancel.cancel();
        }
    }
}
pub fn pcm_rms(data: &[u8]) -> f32 {
    let count = data.len() / 2;
    if count == 0 {
        return 0.0;
    }
    (data
        .as_chunks::<2>()
        .0
        .iter()
        .map(|b| (i16::from_le_bytes([b[0], b[1]]) as f32 / 32768.0).powi(2))
        .sum::<f32>()
        / count as f32)
        .sqrt()
}
pub(crate) fn read_voice_metadata(path: &std::path::Path) -> Result<serde_json::Value> {
    use std::{io::Read, os::unix::fs::OpenOptionsExt};
    let file = std::fs::OpenOptions::new()
        .read(true)
        .custom_flags((rustix::fs::OFlags::NOFOLLOW | rustix::fs::OFlags::NONBLOCK).bits() as i32)
        .open(path)
        .context("Configuración de voz ausente o inválida")?;
    anyhow::ensure!(
        file.metadata()?.is_file(),
        "Configuración de voz no regular"
    );
    let mut data = Vec::new();
    file.take(65537).read_to_end(&mut data)?;
    anyhow::ensure!(data.len() <= 65536, "Configuración de voz demasiado grande");
    serde_json::from_slice(&data).context("Configuración de voz corrupta")
}
pub fn validate_model(path: &std::path::Path) -> Result<()> {
    use std::io::Read;
    let mut file = std::fs::File::open(path)
        .context("Modelo STT no instalado; instala manualmente un modelo Whisper multilingüe")?;
    anyhow::ensure!(
        file.metadata()?.is_file()
            && (1024 * 1024..=4 * 1024 * 1024 * 1024).contains(&file.metadata()?.len()),
        "Modelo STT corrupto o incompleto"
    );
    let mut header = [0; 4];
    file.read_exact(&mut header)?;
    anyhow::ensure!(
        &header == b"lmgg",
        "Formato de modelo STT inválido (se requiere Whisper GGML)"
    );
    Ok(())
}
/// Restricted subprocess mode: PCM16/16k mono in, JSON string out. No Backend or network.
pub fn stt_worker(model: PathBuf, language: String) -> Result<()> {
    use std::io::{Read, Write};
    validate_model(&model)?;
    let mut data = Vec::new();
    std::io::stdin()
        .take(120 * 32000 + 1)
        .read_to_end(&mut data)?;
    anyhow::ensure!(
        data.len() <= 120 * 32000 && data.len() % 2 == 0,
        "PCM fuera de límites"
    );
    let audio: Vec<f32> = data
        .as_chunks::<2>()
        .0
        .iter()
        .map(|b| i16::from_le_bytes([b[0], b[1]]) as f32 / 32768.0)
        .collect();
    let engine = super::stt::WhisperEngine::new(model, None)?;
    let text = engine.transcribe(
        &audio,
        if language == "auto" {
            None
        } else {
            Some(&language)
        },
    )?;
    anyhow::ensure!(text.len() <= 32000, "Transcripción fuera de límites");
    std::io::stdout().write_all(&serde_json::to_vec(&text)?)?;
    Ok(())
}

impl VoiceCoordinator {
    pub async fn speak(
        self: &Arc<Self>,
        session: String,
        turn: Option<String>,
        scope: Option<String>,
        text: String,
        cfg: SpeechConfig,
    ) -> Result<Snapshot> {
        anyhow::ensure!(
            !text.trim().is_empty() && text.len() <= 16000,
            "Texto de voz vacío o demasiado largo"
        );
        let _admission = self.admission.lock().await;
        {
            let inner = self.inner.lock().unwrap_or_else(|e| e.into_inner());
            anyhow::ensure!(
                !inner.closed && inner.session.as_ref() == Some(&session),
                "Conversación de voz obsoleta"
            );
            anyhow::ensure!(
                scope.as_ref().is_none_or(|s| *s == inner.scope),
                "Operación de voz obsoleta"
            );
            anyhow::ensure!(
                matches!(
                    inner.snapshot.state,
                    State::Idle | State::TranscriptReady | State::Error | State::Speaking
                ),
                "Voz ocupada"
            );
        }
        self.drain().await;
        let id = Identity {
            session_id: session,
            turn_id: turn,
            generation: scope.unwrap_or_else(|| uuid::Uuid::new_v4().to_string()),
        };
        let cancel = Cancellation::default();
        let task_cancel = cancel.clone();
        let owner = self.clone();
        let operation_id = id.clone();
        let mut inner = self.inner.lock().unwrap_or_else(|e| e.into_inner());
        inner.scope = uuid::Uuid::new_v4().to_string();
        inner.warm_turn = None;
        inner.snapshot = Snapshot {
            state: State::Preparing,
            identity: Some(id),
            transcript: String::new(),
            error: None,
        };
        let task = tokio::spawn(async move {
            let result = owner
                .render_and_play(&operation_id, text, cfg, &task_cancel)
                .await;
            match result {
                Ok(()) => owner.update(&operation_id, State::Idle, String::new(), None),
                Err(e) => {
                    owner.warm.revoke().await;
                    owner.update(
                        &operation_id,
                        State::Error,
                        String::new(),
                        Some(e.to_string()),
                    );
                }
            }
        });
        inner.operation = Some(Operation {
            cancel,
            stop: Cancellation::default(),
            task,
        });
        Ok(inner.snapshot.clone())
    }
    async fn render_and_play(
        &self,
        id: &Identity,
        text: String,
        cfg: SpeechConfig,
        cancel: &Cancellation,
    ) -> Result<()> {
        let selected = super::voice_tts::Selection::from_config(&cfg)?;
        anyhow::ensure!(
            selected.paths_present(),
            "Piper o voz no instalados en las rutas configuradas"
        );
        // One disposable worker verifies all resources; no separate duplicate preflight.
        // Full PCM is retained: Piper's sentence chunks can leave measurable supply gaps.
        let started = std::time::Instant::now();
        let _fast_guard = cfg.piper_fast.then(|| self.warm.operation_guard(cancel));
        let fast = self
            .warm
            .synthesize(&cfg, &id.session_id, text.clone(), cancel)
            .await?;
        let mut generated = None;
        let pcm = if let Some(pcm) = fast {
            pcm
        } else {
            let mut output = voice_process::run_observed(
                selected.command(false)?,
                text.as_bytes(),
                16 * 1024 * 1024,
                Duration::from_secs(90),
                cancel,
            )
            .await?;
            let data = std::mem::take(&mut output.data);
            generated = Some(output);
            data
        };
        anyhow::ensure!(
            !pcm.is_empty() && pcm.len() % 2 == 0,
            "Piper no generó audio PCM válido"
        );
        let pcm_complete_ms = started.elapsed().as_secs_f64() * 1000.0;
        anyhow::ensure!(!cancel.is_cancelled(), "Operación cancelada");
        let mut play = voice_process::command("/usr/bin/pw-play");
        play.args([
            "--raw",
            "--format",
            "s16",
            "--channels",
            "1",
            "--rate",
            "22050",
            "-",
        ]);
        self.update(id, State::Speaking, String::new(), None);
        let playback =
            voice_process::run_observed(play, &pcm, 1024, Duration::from_secs(180), cancel).await?;
        log::info!(
            "voice_tts_timing {}",
            serde_json::json!({
                "route": if generated.is_some() {"single-validation-buffered"} else {"managed-buffered"}, "pcm_bytes": pcm.len(),
                "pcm_complete_received_ms": pcm_complete_ms,
                "player_spawn_ms": pcm_complete_ms + playback.spawn_ms,
                "worker_process": generated, "playback_process": playback,
                "playback_complete_ms": started.elapsed().as_secs_f64() * 1000.0,
            })
        );
        Ok(())
    }
    pub async fn cancel_turn(&self, session: Option<&str>, turn: &str) {
        let _admission = self.admission.lock().await;
        let matches = self.snapshot().identity.as_ref().is_some_and(|id| {
            Some(id.session_id.as_str()) == session && id.turn_id.as_deref() == Some(turn)
        });
        let pending = {
            let mut inner = self.inner.lock().unwrap_or_else(|e| e.into_inner());
            let matched = inner.warm_turn.as_ref().is_some_and(|id| {
                Some(id.session_id.as_str()) == session && id.turn_id.as_deref() == Some(turn)
            });
            if matched {
                inner.warm_turn = None;
            }
            matched
        };
        if matches {
            self.drain().await;
        }
        if matches || pending {
            self.warm.revoke().await;
        }
    }
}

pub(crate) fn piper_audio_options(metadata: &serde_json::Value, speaker: u32) -> Result<u64> {
    let rate = metadata["audio"]["sample_rate"]
        .as_u64()
        .context("Frecuencia Piper ausente")?;
    anyhow::ensure!((8000..=48000).contains(&rate), "Frecuencia Piper inválida");
    let speakers = metadata["num_speakers"]
        .as_u64()
        .context("Número de hablantes Piper ausente")?;
    anyhow::ensure!(
        u64::from(speaker) < speakers,
        "Hablante Piper no disponible"
    );
    Ok(rate)
}

#[cfg(test)]
mod tests {
    use super::*;
    fn identity(session: &str, generation: &str) -> Identity {
        Identity {
            session_id: session.into(),
            generation: generation.into(),
            turn_id: None,
        }
    }
    fn fixture(code: &str) -> tokio::process::Command {
        let mut cmd = voice_process::command("/usr/bin/python3");
        cmd.args(["-c", code]);
        cmd
    }
    fn prepare(owner: &VoiceCoordinator, id: &Identity, state: State) {
        let mut inner = owner.inner.lock().unwrap();
        inner.session = Some(id.session_id.clone());
        inner.snapshot = Snapshot {
            state,
            identity: Some(id.clone()),
            transcript: String::new(),
            error: None,
        };
    }
    #[test]
    fn transition_table_rejects_incompatible_states() {
        use State::*;
        assert!(!Disabled.can_reach(Recording));
        assert!(!Idle.can_reach(Transcribing));
        assert!(!Recording.can_reach(Speaking));
        assert!(Preparing.can_reach(WakeWaiting));
        assert!(WakeWaiting.can_reach(Preparing));
        assert!(!WakeWaiting.can_reach(Transcribing));
        assert!(!WakeWaiting.can_reach(Speaking));
        assert!(!Transcribing.can_reach(Recording));
        for (a, b) in [
            (Idle, Preparing),
            (Preparing, Recording),
            (Recording, Transcribing),
            (Transcribing, TranscriptReady),
            (Speaking, Cancelling),
            (Cancelling, Idle),
            (Error, Preparing),
        ] {
            assert!(a.can_reach(b));
        }
    }
    #[test]
    fn piper_metadata_is_bounded_and_rejects_non_regular_inputs() {
        let path = std::env::temp_dir().join(format!("f4-piper-{}", uuid::Uuid::new_v4()));
        use std::os::unix::fs::OpenOptionsExt;
        let file = std::fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .mode(0o600)
            .open(&path)
            .unwrap();
        file.set_len(65537).unwrap();
        assert!(read_voice_metadata(&path)
            .unwrap_err()
            .to_string()
            .contains("grande"));
        std::fs::remove_file(&path).unwrap();
        assert!(read_voice_metadata(std::path::Path::new("/dev/null"))
            .unwrap_err()
            .to_string()
            .contains("regular"));
    }
    #[test]
    fn piper_rejects_invalid_speakers_and_sample_rates_before_execution() {
        let mut metadata = serde_json::json!({
            "audio": {"sample_rate": 22050}, "num_speakers": 2
        });
        assert_eq!(piper_audio_options(&metadata, 0).unwrap(), 22050);
        assert_eq!(piper_audio_options(&metadata, 1).unwrap(), 22050);
        assert!(piper_audio_options(&metadata, 2).is_err());
        assert!(piper_audio_options(&metadata, u32::MAX).is_err());
        metadata["num_speakers"] = serde_json::json!(0);
        assert!(piper_audio_options(&metadata, 0).is_err());
        metadata["num_speakers"] = serde_json::json!(1);
        for rate in [
            serde_json::json!(7999),
            serde_json::json!(48001),
            serde_json::json!(22050.5),
            serde_json::json!("22050"),
            serde_json::Value::Null,
        ] {
            metadata["audio"]["sample_rate"] = rate;
            assert!(piper_audio_options(&metadata, 0).is_err());
        }
    }
    #[tokio::test]
    async fn obsolete_results_never_replace_current_session() {
        let owner = VoiceCoordinator::default();
        let old = identity("old", "old");
        prepare(&owner, &old, State::Transcribing);
        owner.select_session(Some("new".into())).await;
        owner.update(
            &old,
            State::TranscriptReady,
            "old private text".into(),
            None,
        );
        assert!(owner.snapshot().transcript.is_empty());
        assert_eq!(owner.snapshot().state, State::Idle);
        assert!(owner.cancel(&old).await.is_err());
    }
    #[tokio::test]
    async fn cancel_before_start_revokes_reserved_generation_and_shutdown_blocks_admission() {
        let owner = Arc::new(VoiceCoordinator::default());
        let scope = owner.select_session(Some("s".into())).await;
        owner.cancel(&identity("s", &scope)).await.unwrap();
        assert!(owner
            .start(
                "s".into(),
                scope,
                crate::models::Config::first_run().speech.clone()
            )
            .await
            .is_err());
        let scope = owner.select_session(Some("s".into())).await;
        owner.shutdown().await;
        assert!(owner
            .start(
                "s".into(),
                scope,
                crate::models::Config::first_run().speech.clone()
            )
            .await
            .is_err());
        assert_eq!(owner.snapshot().state, State::Disabled);
    }
    #[tokio::test]
    async fn missing_model_fails_before_any_capture_and_text_backend_remains_available() {
        let owner = Arc::new(VoiceCoordinator::default());
        let scope = owner.select_session(Some("s".into())).await;
        let mut cfg = crate::models::Config::first_run().speech;
        cfg.stt_model = "/nonexistent/f4-model.bin".into();
        owner.start("s".into(), scope, cfg).await.unwrap();
        tokio::time::sleep(Duration::from_millis(50)).await;
        assert_eq!(owner.snapshot().state, State::Error);
        assert!(owner.snapshot().error.is_some());
        owner.shutdown().await;
    }
    #[tokio::test]
    async fn stop_during_preparing_returns_idle_without_model_or_capture() {
        let owner = Arc::new(VoiceCoordinator::default());
        let scope = owner.select_session(Some("s".into())).await;
        let mut cfg = crate::models::Config::first_run().speech;
        cfg.stt_model = "/nonexistent/f4-model.bin".into();
        let started = owner.start("s".into(), scope, cfg).await.unwrap();
        assert_eq!(started.state, State::Preparing);
        owner.stop(started.identity.as_ref().unwrap()).unwrap();
        // The current-thread runtime has not polled the operation before stop.
        tokio::time::sleep(Duration::from_millis(10)).await;
        let stopped = owner.snapshot();
        assert_eq!(stopped.state, State::Idle);
        assert!(stopped.error.is_none());
        assert!(stopped.transcript.is_empty());
        owner.shutdown().await;
    }
    #[tokio::test]
    async fn capture_is_bounded_and_device_eof_is_an_error() {
        let owner = VoiceCoordinator::default();
        let id = identity("s", "g");
        prepare(&owner, &id, State::Preparing);
        let mut cfg = crate::models::Config::first_run().speech;
        cfg.max_recording_secs = 1;
        let bytes = owner
            .capture_pcm(
                &id,
                &cfg,
                fixture("import os; os.write(1,b'\\x00\\x10'*50000)"),
                &Cancellation::default(),
                &Cancellation::default(),
            )
            .await
            .unwrap();
        assert_eq!(bytes.len(), 32000);
        assert_eq!(owner.snapshot().state, State::Recording);
        let result = owner
            .capture_pcm(
                &id,
                &cfg,
                fixture("pass"),
                &Cancellation::default(),
                &Cancellation::default(),
            )
            .await;
        assert!(result.unwrap_err().to_string().contains("Micrófono"));
    }
    #[tokio::test]
    async fn silence_endpoint_and_stop_release_capture() {
        let owner = Arc::new(VoiceCoordinator::default());
        let id = identity("s", "g");
        prepare(&owner, &id, State::Preparing);
        let mut cfg = crate::models::Config::first_run().speech;
        cfg.vad_silence_ms = 100;
        cfg.vad_min_record_ms = 100;
        let result = owner
            .capture_pcm(
                &id,
                &cfg,
                fixture("import os,time; os.write(1,b'\\0'*16000); time.sleep(5)"),
                &Cancellation::default(),
                &Cancellation::default(),
            )
            .await
            .unwrap();
        assert!((3200..=16000).contains(&result.len()));
        let stop = Cancellation::default();
        stop.cancel();
        let result = owner
            .capture_pcm(
                &id,
                &cfg,
                fixture("import time; time.sleep(5)"),
                &Cancellation::default(),
                &stop,
            )
            .await
            .unwrap();
        assert!(result.is_empty());
    }
    async fn operation_cancellation(state: State) {
        let owner = Arc::new(VoiceCoordinator::default());
        let id = identity("s", "g");
        prepare(&owner, &id, state);
        let cancel = Cancellation::default();
        let child_cancel = cancel.clone();
        let task_owner = owner.clone();
        let task_id = id.clone();
        let task = tokio::spawn(async move {
            let result = voice_process::run(
                fixture("import time; time.sleep(10)"),
                &[],
                128,
                Duration::from_secs(15),
                &child_cancel,
            )
            .await;
            assert!(result.is_err());
            task_owner.update(&task_id, State::TranscriptReady, "stale".into(), None);
        });
        owner.inner.lock().unwrap().operation = Some(Operation {
            cancel,
            stop: Cancellation::default(),
            task,
        });
        owner.cancel(&id).await.unwrap();
        assert_eq!(owner.snapshot().state, State::Idle);
        assert!(owner.snapshot().transcript.is_empty());
        assert!(owner.inner.lock().unwrap().operation.is_none());
    }
    #[tokio::test]
    async fn cancellation_during_capture() {
        operation_cancellation(State::Recording).await;
    }
    #[tokio::test]
    async fn wake_cancellation_revokes_results_and_reaps_operation() {
        operation_cancellation(State::WakeWaiting).await;
    }
    #[tokio::test]
    async fn wake_rejects_duplicates_tts_prewarm_stale_scope_and_shutdown() {
        let owner = Arc::new(VoiceCoordinator::default());
        let scope = owner.select_session(Some("s".into())).await;
        let mut cfg = crate::models::Config::first_run().speech;
        cfg.piper_fast = true;
        cfg.auto_speak = true;
        let id = identity("s", &scope);
        prepare(&owner, &id, State::WakeWaiting);
        assert!(owner
            .start_wake("s".into(), scope.clone(), cfg.clone())
            .await
            .is_err());
        assert!(owner
            .start("s".into(), scope.clone(), cfg.clone())
            .await
            .is_err());
        assert!(owner
            .speak(
                "s".into(),
                None,
                Some(scope.clone()),
                "No debe hablar".into(),
                cfg.clone()
            )
            .await
            .is_err());
        owner.prepare_for_turn("s", "t", &scope, &cfg).await;
        assert!(owner.inner.lock().unwrap().warm_turn.is_none());
        assert_eq!(owner.fast_snapshot().prepares, 0);
        owner.select_session(Some("new".into())).await;
        owner.update(&id, State::TranscriptReady, "obsoleto".into(), None);
        assert!(owner.snapshot().transcript.is_empty());
        assert!(owner
            .start_wake("s".into(), scope.clone(), cfg.clone())
            .await
            .is_err());
        let scope = owner.scope_for(Some("new")).unwrap();
        owner.shutdown().await;
        assert!(owner.start_wake("new".into(), scope, cfg).await.is_err());
        assert_eq!(owner.snapshot().state, State::Disabled);
    }
    #[tokio::test]
    async fn cancellation_during_stt_load_and_inference() {
        operation_cancellation(State::Transcribing).await;
    }
    #[tokio::test]
    async fn interruption_of_tts_generation_or_playback() {
        operation_cancellation(State::Speaking).await;
    }
    #[tokio::test]
    async fn invalid_transition_is_rejected_without_overwriting_text() {
        let owner = VoiceCoordinator::default();
        let id = identity("s", "g");
        prepare(&owner, &id, State::Recording);
        owner.update(&id, State::Speaking, "bad".into(), None);
        assert_eq!(owner.snapshot().state, State::Recording);
        assert!(owner.snapshot().transcript.is_empty());
    }
    #[tokio::test]
    async fn old_llm_cancellation_cannot_revoke_new_manual_speech() {
        let owner = Arc::new(VoiceCoordinator::default());
        let scope = owner.select_session(Some("s".into())).await;
        owner.inner.lock().unwrap().warm_turn = Some(Identity {
            session_id: "s".into(),
            generation: scope.clone(),
            turn_id: Some("old".into()),
        });
        let cfg = crate::models::config::Config::default().speech;
        let current = owner
            .speak("s".into(), None, Some(scope), "Texto sintético".into(), cfg)
            .await
            .unwrap();
        assert!(owner.inner.lock().unwrap().warm_turn.is_none());
        let revocations = owner.fast_snapshot().revocations;
        owner.cancel_turn(Some("s"), "old").await;
        assert_eq!(owner.fast_snapshot().revocations, revocations);
        assert_eq!(owner.snapshot().identity, current.identity);
        owner.shutdown().await;
    }
    #[tokio::test]
    async fn prewarm_is_ineligible_during_stt_or_without_auto_read_or_scope() {
        let owner = Arc::new(VoiceCoordinator::default());
        let scope = owner.select_session(Some("s".into())).await;
        let mut cfg = crate::models::config::Config::default().speech;
        cfg.piper_fast = true;
        owner.configure(&cfg).await;
        let scope = owner.scope_for(Some("s")).unwrap_or(scope);
        owner.prepare_for_turn("s", "t", &scope, &cfg).await;
        assert!(owner.inner.lock().unwrap().warm_turn.is_none());
        cfg.auto_speak = true;
        owner.prepare_for_turn("s", "t", "old-scope", &cfg).await;
        assert!(owner.inner.lock().unwrap().warm_turn.is_none());
        owner.inner.lock().unwrap().snapshot.state = State::Transcribing;
        owner.prepare_for_turn("s", "t", &scope, &cfg).await;
        assert!(owner.inner.lock().unwrap().warm_turn.is_none());
        assert_eq!(owner.fast_snapshot().prepares, 0);
        owner.shutdown().await;
    }
}
