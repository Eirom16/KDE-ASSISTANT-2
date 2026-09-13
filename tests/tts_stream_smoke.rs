//! Smoke test manual del TTS streaming por oración (plan §11).
//!
//! Requiere piper-tts instalado, un modelo en
//! `~/.local/share/kde-assistant/models/piper/` y un dispositivo de audio.
//! Por eso está `#[ignore]`: se ejecuta a mano con
//!
//! ```bash
//! cargo test --test tts_stream_smoke -- --ignored --nocapture
//! ```

use kde_assistant_lib::backend::speech_service::SpeechService;
use kde_assistant_lib::models::Config;
use std::sync::Arc;
use std::time::Instant;
use tokio::sync::{mpsc, RwLock};

#[tokio::test]
#[ignore]
async fn speak_streaming_reproduce_oraciones_en_orden() {
    let cfg = Arc::new(RwLock::new(Config::default()));
    let speech = SpeechService::new(cfg)
        .await
        .expect("SpeechService debe inicializar");
    if !speech.is_available().await {
        eprintln!("piper no disponible; saltando smoke test");
        return;
    }

    let (tx, rx) = mpsc::channel::<String>(4);
    let t_start = Instant::now();
    let feeder = tokio::spawn(async move {
        let sentences = [
            "Primera oración del asistente.",
            "Segunda oración, un poco más larga para notar el encadenamiento.",
            "Tercera y última oración. Fin de la prueba.",
        ];
        for (i, s) in sentences.iter().enumerate() {
            // Simular la llegada progresiva del LLM.
            tokio::time::sleep(std::time::Duration::from_millis(400)).await;
            eprintln!(
                "[smoke] enviando oración {} a los {}ms",
                i + 1,
                t_start.elapsed().as_millis()
            );
            tx.send(s.to_string()).await.expect("canal TTS abierto");
        }
    });

    let result = speech.speak_streaming(rx).await;
    feeder.await.expect("feeder");
    result.expect("speak_streaming sin error");
    eprintln!("[smoke] terminado en {}ms", t_start.elapsed().as_millis());
}
