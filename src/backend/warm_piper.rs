//! One lazy actor per VoiceCoordinator. It alone owns the prepared TTS process.
use super::{
    piper_protocol,
    piper_resource_watch::ResourceWatch,
    voice_process::{self, Cancellation},
    voice_tts::Selection,
};
use crate::models::config::SpeechConfig;
use anyhow::{Context, Result};
use serde::Serialize;
use std::{
    path::PathBuf,
    sync::{
        atomic::{AtomicU64, Ordering},
        Arc, Mutex, OnceLock,
    },
    time::{Duration, Instant},
};
use tokio::{
    process::{Child, ChildStdin, ChildStdout},
    sync::{mpsc, oneshot},
    task::JoinHandle,
};

// R4 measured 255.5 MiB. Reserve one model, one scratch envelope and one
// envelope for UI/desktop allocation growth; this is a prewarm admission policy,
// not a promise of an ONNX heap bound. R5 measures the policy on this host.
const ENVELOPE_KIB: u64 = 256 * 1024;
const MIN_AVAILABLE_KIB: u64 = 3 * ENVELOPE_KIB;
const MAX_WORKER_RSS_KIB: u64 = 2 * ENVELOPE_KIB;
const ENGINE_TIMEOUT: Duration = Duration::from_secs(90);

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum State {
    Disabled,
    Cold,
    Preparing,
    Ready,
    Synthesizing,
    Expiring,
    Stopping,
    Fallback,
    Error,
}
#[derive(Clone, Serialize)]
pub struct Snapshot {
    pub enabled: bool,
    pub phase: Option<String>,
    pub state: State,
    pub generation: Option<String>,
    pub worker_pid: Option<u32>,
    pub ttl_secs: u64,
    pub prepares: u64,
    pub reuses: u64,
    pub revocations: u64,
    pub completed: u64,
    pub ready_ms: Option<f64>,
    pub last_synthesis_ms: Option<f64>,
    pub last_engine_ms: Option<f64>,
    pub last_stop_ms: Option<f64>,
    pub reason: Option<String>,
}
impl Default for Snapshot {
    fn default() -> Self {
        Self {
            enabled: false,
            phase: None,
            state: State::Disabled,
            generation: None,
            worker_pid: None,
            ttl_secs: 30,
            prepares: 0,
            reuses: 0,
            revocations: 0,
            completed: 0,
            ready_ms: None,
            last_synthesis_ms: None,
            last_engine_ms: None,
            last_stop_ms: None,
            reason: None,
        }
    }
}
#[derive(Clone, PartialEq, Eq)]
struct Key {
    runtime: PathBuf,
    model: PathBuf,
    scale: u32,
    speaker: u32,
    ttl: u64,
}
impl Key {
    fn new(selected: &Selection, cfg: &SpeechConfig) -> Self {
        Self {
            runtime: selected.runtime.clone(),
            model: selected.model.clone(),
            scale: selected.scale.to_bits(),
            speaker: selected.speaker,
            ttl: cfg.piper_fast_ttl_secs,
        }
    }
}
struct Control {
    snapshot: Snapshot,
    authorized: Option<Key>,
    stop: Cancellation,
    preparing: bool,
    request: Option<String>,
}
struct Shared {
    control: Mutex<Control>,
    epoch: AtomicU64,
}
impl Shared {
    fn change(&self, f: impl FnOnce(&mut Control)) {
        f(&mut self.control.lock().unwrap_or_else(|e| e.into_inner()));
    }
    fn snapshot(&self) -> Snapshot {
        self.control
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .snapshot
            .clone()
    }
    fn retire(&self, epoch: u64) {
        let mut control = self.control.lock().unwrap_or_else(|e| e.into_inner());
        // Do not cancel a token already rotated by a concurrent explicit revoke.
        if self
            .epoch
            .compare_exchange(
                epoch,
                epoch.wrapping_add(1),
                Ordering::SeqCst,
                Ordering::SeqCst,
            )
            .is_ok()
        {
            control.stop.cancel();
            control.stop = Cancellation::default();
            control.preparing = false;
        }
    }
    fn current(&self, epoch: u64) -> bool {
        self.epoch.load(Ordering::SeqCst) == epoch
    }
}
struct Actor {
    tx: mpsc::Sender<Message>,
    lifetime: Cancellation,
    task: tokio::sync::Mutex<Option<JoinHandle<()>>>,
}
pub struct WarmPiperManager {
    shared: Arc<Shared>,
    actor: OnceLock<Actor>,
}
impl Default for WarmPiperManager {
    fn default() -> Self {
        Self {
            shared: Arc::new(Shared {
                control: Mutex::new(Control {
                    snapshot: Snapshot::default(),
                    authorized: None,
                    stop: Cancellation::default(),
                    preparing: false,
                    request: None,
                }),
                epoch: AtomicU64::new(0),
            }),
            actor: OnceLock::new(),
        }
    }
}
#[derive(Clone)]
struct Plan {
    selected: Selection,
    key: Key,
    session: String,
    epoch: u64,
    stop: Cancellation,
}
enum Message {
    Prepare(Plan),
    Synthesize {
        plan: Plan,
        request: String,
        text: String,
        answer: oneshot::Sender<Result<Vec<u8>>>,
    },
    Reset(oneshot::Sender<()>),
}
impl WarmPiperManager {
    pub fn operation_guard(&self, cancel: &Cancellation) -> RevocationGuard {
        let revoked = self
            .shared
            .control
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .stop
            .clone();
        let cancel = cancel.clone();
        RevocationGuard(tokio::spawn(async move {
            revoked.cancelled().await;
            cancel.cancel();
        }))
    }
    pub fn snapshot(&self) -> Snapshot {
        self.shared.snapshot()
    }
    fn actor(&self) -> &Actor {
        self.actor.get_or_init(|| {
            let (tx, rx) = mpsc::channel(2);
            let lifetime = Cancellation::default();
            let task = tokio::spawn(run_actor(rx, self.shared.clone(), lifetime.clone()));
            Actor {
                tx,
                lifetime,
                task: tokio::sync::Mutex::new(Some(task)),
            }
        })
    }
    pub async fn configure(&self, cfg: &SpeechConfig) {
        let selected = Selection::from_config(cfg).ok();
        self.shared.change(|c| {
            c.snapshot.enabled = cfg.piper_fast;
            c.snapshot.ttl_secs = cfg.piper_fast_ttl_secs;
            c.authorized = selected.as_ref().map(|s| Key::new(s, cfg));
        });
        self.revoke().await;
    }
    pub async fn revoke(&self) {
        self.shared.epoch.fetch_add(1, Ordering::SeqCst);
        self.shared.change(|c| {
            c.stop.cancel();
            c.stop = Cancellation::default();
            c.preparing = false;
            c.snapshot.revocations += 1;
            c.snapshot.state = State::Stopping;
        });
        if let Some(actor) = self.actor.get() {
            let (tx, rx) = oneshot::channel();
            if actor.tx.send(Message::Reset(tx)).await.is_ok() {
                let _ = rx.await;
            }
        } else {
            cold(&self.shared);
        }
    }
    pub async fn shutdown(&self) {
        self.revoke().await;
        self.shared.change(|c| {
            c.snapshot.enabled = false;
            c.snapshot.state = State::Disabled;
        });
        if let Some(actor) = self.actor.get() {
            actor.lifetime.cancel();
            if let Some(task) = actor.task.lock().await.take() {
                let _ = task.await;
            }
        }
    }
    fn plan(&self, cfg: &SpeechConfig, session: &str) -> Result<Option<Plan>> {
        if !cfg.piper_fast {
            return Ok(None);
        }
        let selected = Selection::from_config(cfg)?;
        let key = Key::new(&selected, cfg);
        let control = self
            .shared
            .control
            .lock()
            .unwrap_or_else(|e| e.into_inner());
        anyhow::ensure!(
            control.snapshot.enabled && control.authorized.as_ref() == Some(&key),
            "Configuración de voz rápida obsoleta"
        );
        anyhow::ensure!((15..=60).contains(&key.ttl), "TTL de voz rápida inválido");
        Ok(Some(Plan {
            selected,
            key,
            session: session.into(),
            epoch: self.shared.epoch.load(Ordering::SeqCst),
            stop: control.stop.clone(),
        }))
    }
    pub async fn prepare(&self, cfg: &SpeechConfig, session: &str) -> Result<()> {
        let Some(plan) = self.plan(cfg, session)? else {
            return Ok(());
        };
        let skip = {
            let mut c = self
                .shared
                .control
                .lock()
                .unwrap_or_else(|e| e.into_inner());
            if c.preparing || matches!(c.snapshot.state, State::Ready | State::Synthesizing) {
                true
            } else {
                c.preparing = true;
                false
            }
        };
        if skip {
            return Ok(());
        }
        if self.actor().tx.try_send(Message::Prepare(plan)).is_err() {
            self.shared.change(|c| c.preparing = false);
        }
        Ok(())
    }
    /// None is a policy fallback before dispatch. Engine/IPC/integrity failures
    /// are errors and are never automatically replayed through the cold route.
    pub async fn synthesize(
        &self,
        cfg: &SpeechConfig,
        session: &str,
        text: String,
        cancel: &Cancellation,
    ) -> Result<Option<Vec<u8>>> {
        let Some(plan) = self.plan(cfg, session)? else {
            return Ok(None);
        };
        let request = uuid::Uuid::new_v4().to_string();
        {
            let mut c = self
                .shared
                .control
                .lock()
                .unwrap_or_else(|e| e.into_inner());
            anyhow::ensure!(c.request.is_none(), "Piper preparado ocupado");
            c.request = Some(request.clone());
        }
        let (tx, rx) = oneshot::channel();
        let msg = Message::Synthesize {
            plan,
            request: request.clone(),
            text,
            answer: tx,
        };
        let work = async {
            self.actor()
                .tx
                .send(msg)
                .await
                .context("Gestor Piper detenido")?;
            rx.await.context("Piper perdió su respuesta")?
        };
        let result = tokio::select! {biased;
            _=cancel.cancelled()=> {self.revoke().await;Err(anyhow::anyhow!("Operación cancelada"))},
            value=tokio::time::timeout(ENGINE_TIMEOUT,work)=>match value {
                Ok(value)=>value,Err(_)=>{self.revoke().await;Err(anyhow::anyhow!("Tiempo de Piper agotado"))}
            },
        };
        self.shared.change(|c| {
            if c.request.as_ref() == Some(&request) {
                c.request = None;
            }
        });
        match result {
            Ok(pcm) if pcm.is_empty() => Ok(None),
            value => value.map(Some),
        }
    }
}
pub struct RevocationGuard(JoinHandle<()>);
impl Drop for RevocationGuard {
    fn drop(&mut self) {
        self.0.abort();
    }
}
impl Drop for WarmPiperManager {
    fn drop(&mut self) {
        self.shared.change(|c| c.stop.cancel());
        if let Some(actor) = self.actor.get() {
            actor.lifetime.cancel();
        }
    }
}
fn cold(shared: &Shared) {
    shared.change(|c| {
        c.snapshot.state = if c.snapshot.enabled {
            State::Cold
        } else {
            State::Disabled
        };
        c.snapshot.generation = None;
        c.snapshot.worker_pid = None;
        c.preparing = false;
    });
}

pub fn pressure(memory: &str, psi: &str) -> Result<bool> {
    let available = memory
        .lines()
        .find_map(|line| {
            line.strip_prefix("MemAvailable:")
                .and_then(|value| value.split_whitespace().next())
                .and_then(|v| v.parse::<u64>().ok())
        })
        .context("MemAvailable no disponible")?;
    let stall = psi
        .lines()
        .next()
        .and_then(|line| {
            line.split_whitespace()
                .find_map(|field| field.strip_prefix("avg10="))
        })
        .context("PSI de memoria no disponible")?
        .parse::<f64>()?;
    // A small historical PSI value with ample capacity caused false evictions
    // on this host (0.17%, 4.76 GiB available). Use it as an early warning
    // within two admission margins; always reject insufficient capacity.
    Ok(available < MIN_AVAILABLE_KIB
        || !stall.is_finite()
        || (stall > 0. && available < 2 * MIN_AVAILABLE_KIB))
}
async fn memory_pressure() -> Result<bool> {
    pressure(
        &tokio::fs::read_to_string("/proc/meminfo").await?,
        &tokio::fs::read_to_string("/proc/pressure/memory").await?,
    )
}
// A reaped/exited child is handled by Child::wait, not by memory revocation.
fn live_rss(status: &str) -> Result<Option<u64>> {
    let state = status
        .lines()
        .find_map(|line| {
            line.strip_prefix("State:")
                .and_then(|s| s.split_whitespace().next())
        })
        .context("Estado de proceso ausente")?;
    if matches!(state, "Z" | "X") {
        return Ok(None);
    }
    let rss = status
        .lines()
        .find_map(|line| {
            line.strip_prefix("VmRSS:")
                .and_then(|v| v.split_whitespace().next())
                .and_then(|v| v.parse::<u64>().ok())
        })
        .context("RSS no disponible")?;
    Ok(Some(rss))
}
struct Worker {
    child: Child,
    input: ChildStdin,
    output: ChildStdout,
    plan: Plan,
    generation: String,
    idle_until: tokio::time::Instant,
    life_until: tokio::time::Instant,
    watch: JoinHandle<()>,
    monitor: JoinHandle<()>,
    cancel: Cancellation,
}
impl Worker {
    async fn stop(&mut self, shared: &Shared, state: State) {
        shared.change(|c| c.snapshot.state = state);
        let start = Instant::now();
        self.cancel.cancel();
        let _ = self.child.kill().await;
        let _ = self.child.wait().await;
        self.watch.abort();
        let _ = (&mut self.watch).await;
        self.monitor.abort();
        let _ = (&mut self.monitor).await;
        shared.change(|c| {
            c.snapshot.worker_pid = None;
            c.snapshot.generation = None;
            c.snapshot.last_stop_ms = Some(start.elapsed().as_secs_f64() * 1000.);
        });
    }
    fn equivalent(&self, plan: &Plan) -> bool {
        self.plan.epoch == plan.epoch
            && self.plan.key == plan.key
            && self.plan.session == plan.session
            && !self.plan.stop.is_cancelled()
            && !self.cancel.is_cancelled()
    }
}
async fn start(plan: Plan, shared: &Arc<Shared>) -> Result<Option<Worker>> {
    if memory_pressure().await.unwrap_or(true) {
        shared.change(|c| {
            c.snapshot.state = State::Fallback;
            c.snapshot.reason = Some("memory_pressure".into());
            c.preparing = false;
        });
        return Ok(None);
    }
    anyhow::ensure!(
        shared.current(plan.epoch) && !plan.stop.is_cancelled(),
        "Preparación revocada"
    );
    shared.change(|c| {
        c.snapshot.state = State::Preparing;
        c.snapshot.phase = Some("load".into());
        c.snapshot.reason = None;
        c.snapshot.prepares += 1;
    });
    let selected = plan.selected.clone();
    let watch = tokio::task::spawn_blocking(move || ResourceWatch::new(&selected)).await??;
    anyhow::ensure!(
        shared.current(plan.epoch) && !plan.stop.is_cancelled(),
        "Preparación revocada"
    );
    let cancel = Cancellation::default();
    let mut cmd = plan.selected.managed_command()?;
    let stop = cancel.clone();
    let revoked = plan.stop.clone();
    let health = shared.clone();
    let watch_task = tokio::spawn(async move {
        tokio::select! {biased;_=revoked.cancelled()=>{},_=watch.changed()=>{
        health.change(|c| c.snapshot.reason=Some("resource_changed_or_watch_failed".into()));
        revoked.cancel();}}
        stop.cancel();
    });
    let mut child = match cmd.spawn() {
        Ok(child) => child,
        Err(error) => {
            watch_task.abort();
            let _ = watch_task.await;
            return Err(error.into());
        }
    };
    let pid = child.id().context("Piper sin PID")?;
    let generation = uuid::Uuid::new_v4().to_string();
    let stop = cancel.clone();
    let revoked = plan.stop.clone();
    let health = shared.clone();
    let monitor = tokio::spawn(async move {
        let mut missing = 0;
        loop {
            tokio::time::sleep(Duration::from_secs(1)).await;
            let memory = tokio::fs::read_to_string(format!("/proc/{pid}/status")).await;
            let sample = match memory {
                Ok(body) => live_rss(&body),
                Err(error) if error.kind() == std::io::ErrorKind::NotFound => break,
                Err(error) => Err(error.into()),
            };
            let too_large = match sample {
                Ok(None) => break,
                Ok(Some(rss)) => {
                    missing = 0;
                    rss > MAX_WORKER_RSS_KIB
                }
                Err(_) => {
                    missing += 1;
                    if missing < 2 {
                        continue;
                    }
                    true
                }
            };
            let pressure = memory_pressure().await.unwrap_or(true);
            if too_large || pressure {
                health.change(|c| {
                    c.snapshot.reason = Some(
                        if too_large {
                            "rss_limit_or_metrics_missing"
                        } else {
                            "memory_pressure"
                        }
                        .into(),
                    )
                });
                revoked.cancel();
                stop.cancel();
                break;
            }
        }
    });
    shared.change(|c| {
        c.snapshot.worker_pid = Some(pid);
        c.snapshot.generation = Some(generation.clone());
    });
    let input = child.stdin.take().context("Piper sin stdin")?;
    let output = child.stdout.take().context("Piper sin stdout")?;
    let now = tokio::time::Instant::now();
    let mut worker = Worker {
        child,
        input,
        output,
        plan,
        generation,
        idle_until: now,
        life_until: now,
        watch: watch_task,
        monitor,
        cancel,
    };
    let begin = Instant::now();
    let load = async {
        piper_protocol::send(
            &mut worker.input,
            serde_json::json!({"protocol":1,"kind":"init",
            "generation":worker.generation,"ttl_secs":worker.plan.key.ttl}),
        )
        .await?;
        piper_protocol::ready(&mut worker.output, &worker.generation).await
    };
    let result = tokio::select! {biased;
        _=worker.cancel.cancelled()=>Err(anyhow::anyhow!("Preparación Piper revocada o recursos modificados")),
        result=tokio::time::timeout(ENGINE_TIMEOUT,load)=>match result {
            Ok(value)=>value, Err(error)=>Err(anyhow::anyhow!(error).context("Piper no confirmó Ready")),
        },
    };
    if let Err(error) = result {
        shared.retire(worker.plan.epoch);
        worker.stop(shared, State::Stopping).await;
        return Err(error);
    }
    if !shared.current(worker.plan.epoch)
        || worker.plan.stop.is_cancelled()
        || worker.cancel.is_cancelled()
    {
        worker.stop(shared, State::Stopping).await;
        anyhow::bail!("Piper preparado obsoleto")
    }
    let now = tokio::time::Instant::now();
    worker.idle_until = now + Duration::from_secs(worker.plan.key.ttl);
    worker.life_until = now + Duration::from_secs(worker.plan.key.ttl * 2);
    shared.change(|c| {
        c.snapshot.state = State::Ready;
        c.snapshot.phase = None;
        c.snapshot.ready_ms = Some(begin.elapsed().as_secs_f64() * 1000.);
        c.preparing = false;
    });
    Ok(Some(worker))
}
async fn synthesize(
    worker: &mut Worker,
    request: &str,
    text: &str,
    shared: &Shared,
) -> Result<Vec<u8>> {
    anyhow::ensure!(
        shared.current(worker.plan.epoch)
            && !worker.plan.stop.is_cancelled()
            && !worker.cancel.is_cancelled(),
        "Worker Piper revocado"
    );
    shared.change(|c| {
        c.snapshot.state = State::Synthesizing;
        c.snapshot.phase = Some("validation".into());
    });
    let reservation = piper_protocol::reserve(
        &mut worker.output,
        &mut worker.input,
        &worker.generation,
        request,
    );
    tokio::select! {biased;
        _=worker.cancel.cancelled()=>anyhow::bail!("Reserva Piper revocada"),
        result=tokio::time::timeout(Duration::from_secs(15),reservation)=>result.context("Reserva Piper sin respuesta")??,
    }
    // Full cryptographic verification before EVERY reuse. Watches also revoke
    // changes during load/inference/idle, including same-size replacements.
    let mut check = worker.plan.selected.command(true)?;
    check.env_clear();
    let validation =
        voice_process::run(check, &[], 4096, Duration::from_secs(15), &worker.cancel).await?;
    let error: Option<String> = serde_json::from_slice(&validation)?;
    if let Some(error) = error {
        anyhow::bail!("{error}")
    }
    let work = piper_protocol::synthesize(
        &mut worker.output,
        &mut worker.input,
        &worker.generation,
        request,
        text,
        || shared.change(|c| c.snapshot.phase = Some("inference".into())),
    );
    let start = Instant::now();
    let result = tokio::select! {biased;
        _=worker.cancel.cancelled()=>Err(anyhow::anyhow!("Síntesis Piper revocada o recursos modificados")),
        result=tokio::time::timeout(ENGINE_TIMEOUT,work)=>result.context("Piper no terminó la síntesis")?,
    }?;
    anyhow::ensure!(
        shared.current(worker.plan.epoch)
            && !worker.plan.stop.is_cancelled()
            && !worker.cancel.is_cancelled(),
        "Resultado Piper revocado"
    );
    worker.idle_until = tokio::time::Instant::now() + Duration::from_secs(worker.plan.key.ttl);
    shared.change(|c| {
        c.snapshot.state = State::Ready;
        c.snapshot.phase = None;
        c.snapshot.completed += 1;
        c.snapshot.last_engine_ms = Some(result.synthesis_ms);
        c.snapshot.last_synthesis_ms = Some(start.elapsed().as_secs_f64() * 1000.);
    });
    Ok(result.pcm)
}
async fn run_actor(mut rx: mpsc::Receiver<Message>, shared: Arc<Shared>, lifetime: Cancellation) {
    let mut worker: Option<Worker> = None;
    loop {
        let message = if let Some(current) = worker.as_mut() {
            tokio::select! {biased;
                _=lifetime.cancelled()=>None,
                _=current.cancel.cancelled()=>{
                    shared.retire(current.plan.epoch);
                    current.stop(&shared,State::Stopping).await;worker=None;
                    shared.change(|c|{c.snapshot.state=State::Error;c.snapshot.reason.get_or_insert_with(||"revoked_or_resource_pressure".into());c.preparing=false;});
                    continue;
                },
                status=current.child.wait()=>{
                    if !status.as_ref().is_ok_and(|s|s.success()) {shared.retire(current.plan.epoch);}
                    current.stop(&shared,State::Stopping).await;worker=None;
                    shared.change(|c|{c.snapshot.state=State::Cold;c.snapshot.reason=Some(if status.is_ok_and(|s|s.success()) {"worker_expired"}else{"worker_died"}.into());});
                    continue;
                },
                _=tokio::time::sleep_until(current.idle_until.min(current.life_until))=>{
                    current.stop(&shared,State::Expiring).await;worker=None;cold(&shared);continue;
                },
                message=rx.recv()=>message,
            }
        } else {
            tokio::select! {biased;_=lifetime.cancelled()=>None,message=rx.recv()=>message}
        };
        let Some(message) = message else { break };
        match message {
            Message::Reset(answer) => {
                if let Some(ref mut current) = worker {
                    current.stop(&shared, State::Stopping).await;
                }
                worker = None;
                cold(&shared);
                let _ = answer.send(());
            }
            Message::Prepare(plan) => {
                if !shared.current(plan.epoch) || plan.stop.is_cancelled() {
                    continue;
                }
                if worker.as_ref().is_some_and(|w| w.equivalent(&plan)) {
                    shared.change(|c| c.preparing = false);
                    continue;
                }
                if let Some(ref mut current) = worker {
                    current.stop(&shared, State::Stopping).await;
                }
                worker = None;
                let epoch = plan.epoch;
                match start(plan, &shared).await {
                    Ok(value) => worker = value,
                    Err(_) => {
                        shared.retire(epoch);
                        shared.change(|c| {
                            c.snapshot.state = State::Error;
                            c.snapshot
                                .reason
                                .get_or_insert_with(|| "prepare_failed".into());
                            c.preparing = false;
                        });
                    }
                }
            }
            Message::Synthesize {
                plan,
                request,
                text,
                answer,
            } => {
                if !shared.current(plan.epoch) || plan.stop.is_cancelled() {
                    let _ = answer.send(Err(anyhow::anyhow!("Solicitud Piper revocada")));
                    continue;
                }
                let expired = worker
                    .as_ref()
                    .is_some_and(|w| tokio::time::Instant::now() >= w.idle_until.min(w.life_until));
                if worker.as_ref().is_some_and(|w| !w.equivalent(&plan)) || expired {
                    if let Some(ref mut current) = worker {
                        current.stop(&shared, State::Expiring).await;
                    }
                    worker = None;
                }
                let reused = worker.is_some();
                if worker.is_none() {
                    match start(plan, &shared).await {
                        Ok(value) => worker = value,
                        Err(error) => {
                            let _ = answer.send(Err(error));
                            shared.change(|c| {
                                c.snapshot.state = State::Error;
                                c.preparing = false;
                            });
                            continue;
                        }
                    }
                }
                let Some(ref mut current) = worker else {
                    let _ = answer.send(Ok(Vec::new()));
                    continue;
                };
                if reused {
                    shared.change(|c| c.snapshot.reuses += 1);
                }
                let result = synthesize(current, &request, &text, &shared).await;
                if result.is_err() {
                    current.stop(&shared, State::Stopping).await;
                    worker = None;
                    shared.change(|c| {
                        c.snapshot.state = State::Error;
                        c.snapshot
                            .reason
                            .get_or_insert_with(|| "synthesis_failed".into());
                    });
                }
                let _ = answer.send(result);
            }
        }
    }
    if let Some(mut current) = worker {
        current.stop(&shared, State::Stopping).await;
    }
    cold(&shared);
}

#[cfg(test)]
mod tests {
    use super::*;
    #[tokio::test]
    async fn default_and_configuring_never_preload_or_create_actor() {
        let manager = WarmPiperManager::default();
        let mut cfg = crate::models::config::Config::default().speech;
        assert_eq!(manager.snapshot().state, State::Disabled);
        manager.configure(&cfg).await;
        assert!(manager.actor.get().is_none());
        cfg.piper_fast = true;
        manager.configure(&cfg).await;
        assert_eq!(manager.snapshot().state, State::Cold);
        assert!(manager.actor.get().is_none());
        let cancelled = Cancellation::default();
        let _guard = manager.operation_guard(&cancelled);
        manager.revoke().await;
        tokio::task::yield_now().await;
        assert!(cancelled.is_cancelled());
        cfg.piper_fast = false;
        manager.configure(&cfg).await;
        assert_eq!(manager.snapshot().state, State::Disabled);
        assert!(manager.actor.get().is_none());
        manager.shutdown().await;
    }
    #[test]
    fn memory_policy_is_fail_closed_and_reserves_measured_envelopes() {
        assert!(!pressure("MemAvailable: 4194304 kB", "some avg10=0.00").unwrap());
        assert!(pressure("MemAvailable: 786431 kB", "some avg10=0.00").unwrap());
        assert!(!pressure("MemAvailable: 4194304 kB", "some avg10=0.01").unwrap());
        assert!(pressure("MemAvailable: 1048576 kB", "some avg10=0.01").unwrap());
        assert!(!pressure("MemAvailable: 1048576 kB", "some avg10=0.00").unwrap());
        assert!(pressure("MemAvailable: 4194304 kB", "some avg10=NaN").unwrap());
        assert!(pressure("", "some avg10=0.00").is_err());
        assert!(pressure("MemAvailable: 4194304 kB", "").is_err());
    }
    #[tokio::test]
    async fn duplicate_preparations_share_one_queue_slot_and_revocation_epoch() {
        let manager = WarmPiperManager::default();
        let mut cfg = crate::models::config::Config::default().speech;
        cfg.piper_fast = true;
        cfg.piper_runtime = "/nonexistent/r5-private-runtime".into();
        cfg.piper_model = "/nonexistent/r5-model.onnx".into();
        manager.configure(&cfg).await;
        let (a, b) = tokio::join!(manager.prepare(&cfg, "s"), manager.prepare(&cfg, "s"));
        a.unwrap();
        b.unwrap();
        assert_eq!(manager.actor().tx.capacity(), 1);
        let old = manager.plan(&cfg, "s").unwrap().unwrap();
        manager.revoke().await;
        assert!(!manager.shared.current(old.epoch));
        assert!(old.stop.is_cancelled());
        assert!(manager.snapshot().worker_pid.is_none());
        manager.shutdown().await;
    }
    #[tokio::test]
    async fn health_revocation_rotates_token_without_poisoning_next_request() {
        let manager = WarmPiperManager::default();
        let cfg = crate::models::config::Config::default().speech;
        manager.configure(&cfg).await;
        let epoch = manager.shared.epoch.load(Ordering::SeqCst);
        let old = manager.shared.control.lock().unwrap().stop.clone();
        manager.shared.retire(epoch);
        assert!(old.is_cancelled());
        let fresh = manager.shared.control.lock().unwrap().stop.clone();
        assert!(!fresh.is_cancelled());
        manager.shared.retire(epoch);
        assert!(!fresh.is_cancelled());
        manager.revoke().await;
        let newest = manager.shared.control.lock().unwrap().stop.clone();
        manager.shared.retire(epoch);
        assert!(!newest.is_cancelled());
        manager.shutdown().await;
    }
    #[test]
    fn zombie_metrics_are_not_a_memory_failure_of_the_next_generation() {
        assert_eq!(live_rss("State: Z (zombie)\n").unwrap(), None);
        assert_eq!(live_rss("State: X (dead)\n").unwrap(), None);
        assert_eq!(
            live_rss("State: S (sleeping)\nVmRSS: 250000 kB").unwrap(),
            Some(250000)
        );
        assert!(live_rss("State: S (sleeping)\n").is_err());
        assert!(live_rss("").is_err());
    }
}
