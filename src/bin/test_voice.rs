//! Test del pipeline de voz: espeak-ng TTS + chime + audio capture
//! Ejecutar: cargo run --bin test_voice

use anyhow::Result;
use kde_assistant_lib::backend::chime_player::ChimePlayer;
use kde_assistant_lib::backend::speech_service::{play_wav_bytes, SpeechService};
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::RwLock;

use kde_assistant_lib::models::Config;

#[tokio::main]
async fn main() -> Result<()> {
    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("info")).init();

    log::info!("Test pipeline de voz (piper-tts)");

    // 1. Listar modelos piper disponibles
    let cfg = Arc::new(RwLock::new(Config::default()));
    let speech = SpeechService::new(cfg.clone()).await?;
    let models = speech.list_models().await;
    log::info!("Modelos piper disponibles: {models:?}");
    if !speech.is_available().await {
        log::warn!("piper-tts no esta disponible. {}", speech.help_message());
        return Ok(());
    }

    // 2. Probar TTS
    let text = "Hola, soy KDE Assistant. La fase 5 de voz con piper esta completa. La calidad es mucho mejor.";
    log::info!("Sintetizando: {text}");
    let wav = speech.synthesize(text).await?;
    log::info!("TTS genero {} bytes WAV", wav.len());
    play_wav_bytes(&wav)?;
    log::info!("Reproduccion TTS completa");

    // 3. Probar chimes
    let chimes = ChimePlayer::new().await?;
    log::info!("Reproduciendo chime de activacion");
    chimes.play_activate();
    std::thread::sleep(Duration::from_millis(500));
    log::info!("Reproduciendo chime de proceso");
    chimes.play_process();
    std::thread::sleep(Duration::from_millis(500));
    log::info!("Reproduciendo chime de desactivacion");
    chimes.play_deactivate();

    log::info!("Todos los tests pasaron");
    Ok(())
}
