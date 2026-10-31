# F4 — Contrato de voz

Fecha: 2026-10-18. Baseline `9544a58eba4b6adf6c6343c04db80813b084f76b`.
Rama `recovery/f4-voice-coordinator`. Contrato de la implementación parcial; no certifica hardware ni manos libres.

## Autoridad e identidades

`VoiceCoordinator` es el único propietario **productivo** de captura/inferencia/reproducción. No tiene cliente LLM, executor, credenciales ni escritor SQLite. Los controladores heredados permanecen para compatibilidad y fixtures, sin arranque ni rutas de mutación productivas. Python/Pipecat no se inicia.

`POST /api/voice/session` selecciona una sesión existente o deselecciona con null. Seleccionar la misma sesión es idempotente; cambiarla drena la operación anterior. Rust entrega un `scope` UUID. `/voice/start` y `/speak` requieren ese scope y lo consumen. Cancelar una reserva antes de recibir el acknowledgement también la revoca: un inicio tardío falla. Una petición repetida con un scope consumido no crea otra captura.

La identidad de operación contiene `session_id`, `generation` y `turn_id`. Dictado y lectura manual tienen `turn_id:null`: no inventan un turno conversacional. Autolectura lleva el Turn ID F1B existente y el scope tomado al iniciar ese turno. Un cambio de sesión/ocultación revoca ese scope y evita audio posterior de la respuesta antigua.

`stop`, `cancel` y el alias `/speak/stop` requieren la identidad exacta; datos desconocidos se rechazan. Los callbacks Rust comparan identidad y transición. QML tiene una generación local adicional y entrega cada transcripción una sola vez. La creación de la primera sesión también conserva esa generación: cancelar u ocultar durante la respuesta HTTP demorada impide iniciar captura después. Se vuelve a comprobar visibilidad/generación en Qt.callLater. El bearer local conserva el alcance de aplicación F2; esto no es una ACL contra otros procesos del mismo UID.

## Estados

| Origen | Destinos ordinarios admitidos |
| --- | --- |
| Disabled | Idle |
| Idle, TranscriptReady, Error | Preparing, Idle, Disabled |
| Preparing | Recording, Speaking, Listening, Idle |
| Recording, Listening | Transcribing, Idle |
| Transcribing | TranscriptReady |
| Speaking, Cancelling | Idle |
| Thinking | Speaking, Idle |
| WakeWaiting | Listening, Idle |

Fuera de Disabled se permite Error/Cancelling; las auto-transiciones son idempotentes. Una transición inválida no se aplica. `Thinking`, `Listening` y `WakeWaiting` reservan estados del contrato futuro; **no constituyen un segundo turno LLM ni prueban wake activo**. En F4 se usa Recording para PCM recibido. Preparing no afirma que el micrófono entregue datos. Cancelling permanece hasta recoger al worker.

## Captura y dictado

Semántica accesible: pulsar inicia; pulsar de nuevo termina y transcribe. Cancelar es una acción independiente. No depende de mantener presionado ni de atajos. El primer dictado crea una sesión vacía si hace falta; no crea mensajes hasta Enviar.

Rust supervisa `/usr/bin/pw-record`, sin shell, con stdout PCM16LE, mono, 16 kHz. PipeWire convierte canales/frecuencia con calidad 4. Se enumeran `Audio/Source` por `pw-dump` únicamente bajo acción de consulta/inicio. La selección usa `node.name`, no etiquetas ALSA antiguas. Un nombre guardado que falta da error; no se sustituye silenciosamente. Las propiedades `node.dont-reconnect`, `node.dont-fallback`, `node.dont-move` impiden mover una captura iniciada a otro destino bajo la política documentada de WirePlumber. El comportamiento físico ante desconexión todavía requiere prueba.

Lecturas de 3200 bytes; buffer de muestras limitado por `speech.max_recording_secs` (default 30, máximo 120; máximo 3,84 MB PCM). El reloj monotónico limita además duración de captura + preparación. Falta de datos durante tres segundos, EOF y fallo de proceso terminan con error. El endpoint de silencio usa RMS y `vad_silence_ms`/`vad_min_record_ms`; no se presenta como reconocimiento de habla ni WebRTC VAD. El buffer se libera al terminar/cancelar. El dispositivo se libera **antes** de STT.

El formato alternativo de fixtures convierte 44,1/48 kHz mediante sinc con filtro paso bajo. No reinterpreta bytes. Producción usa el resampler de PipeWire; ambos caminos tienen evidencia separada.

## STT y recursos

Preflight en el mismo ejecutable, modo restringido `--voice-model-check`, sin Backend ni configuración personal. Comprueba archivo regular, header GGML, tamaño mínimo/máximo; no acredita integridad criptográfica de cualquier archivo elegido. Los modelos inventariados tienen SHA-256 y la descarga tiny autorizada se comparó con el hash del repositorio fuente. Un archivo con header correcto pero pesos corruptos falla después en el loader.

`--voice-stt-worker MODEL LANGUAGE` recibe PCM por stdin, devuelve una cadena JSON limitada y ejecuta Whisper local. No abre micrófono ni servidor HTTP. Whisper usa CPU, como máximo tres hilos, y contexto textual independiente. Español por defecto; `auto` permite autodetección. tiny/base/small o ruta absoluta GGML; otros nombres fallan, sin caer silenciosamente en base.

Un solo worker por operación; carga fría bajo demanda, sin prewarm, sin modelos grandes duplicados. El contexto se libera al terminar el proceso. Se prioriza cancelación durante carga/inferencia sobre caché persistente. Carga caliente sólo se mide en el benchmark. `stt_timeout_secs`: 90 por defecto, máximo 300; es un límite de contención, **no un presupuesto de latencia aceptable**. La selección de un motor suficientemente rápido sigue pendiente.

## TTS

Lectura final tras completar y persistir el turno. No hay síntesis incremental ni prompts adicionales. La lectura manual también pasa por el coordinador. Una operación, cola efectiva de profundidad uno, sin mezcla de respuestas. Se reutiliza descubrimiento de Piper; el camino nativo exige el ejecutable `piper-tts` y rechaza el nombre genérico `piper`, que también identifica otro programa Linux, hasta verificar una variante explícita; se sustituye su síntesis con archivos temporales por stdout PCM acotado (16 MiB). La voz `.onnx` requiere `.onnx.json`, sample rate válido y duración 0,5–2. Piper tiene timeout de 90 s; reproducción `pw-play` tiene 180 s. Sólo se abre salida al disponer de PCM y no estar cancelado.

Cancelar termina síntesis o reproducción y recoge al hijo; detener audio no afirma cancelar LLM. Una nueva lectura reemplaza la anterior una vez drenada. La UI puede parar antes de comenzar otra interacción de voz. No hay escucha durante TTS ni cancelación de eco automática. Compatibilidad real del binario Piper, selección de speakers heredada y latencia audible siguen pendientes; motor/voz no están instalados en este host.

## Procesos, salida y fallos

`voice_process` elimina el entorno heredado; sólo pasa el socket de sesión PipeWire. No pasa HOME, API keys ni bearer. Todos los argumentos van separados, sin shell. stdin/stdout tienen límites; stderr de motores no se registra ni devuelve. Timeouts/cancelación hacen kill + wait del **Child propio**, sin búsquedas ni señales por nombre. `kill_on_drop` es respaldo; PDEATHSIG enlaza al hilo Tokio creador para la muerte del controlador. No se promete supervisión de nietos arbitrarios de ejecutables ajenos; los helpers soportados procesan audio/inferencia directamente.

El cierre de main deshabilita admisión vocal y drena voz antes de esperar turnos/HTTP. Ocultar/cerrar la ventana y cambiar conversación cancelan/revocan voz. Error de voz conserva texto. No hay reinicio/reintento automático de motores ni de operaciones UnknownOutcome. Un reinicio de aplicación parte sin recursos vocales activos.

## Datos, red y F2

Audio crudo y transcripción pendiente viven en RAM, no en SQLite ni WAV temporales del producto. No se guardan muestras wake/calibración. Los WAV del benchmark son señales generadas en un perfil temporal, no grabaciones. No hay promesa de borrado forense de RAM, swap o copias externas.

El texto dictado se añade al borrador actual con un espacio, sin borrar texto previo. Se revisa/corrige con el editor existente; puede descartarse editándolo. Enviar utiliza `/api/chat` normal, TurnRegistry, journal y SQLite F1B. La transcripción que el usuario envía deliberadamente puede persistir como cualquier mensaje.

STT remoto está deshabilitado incluso si una configuración antigua dice auto/api o hay una clave Groq. No hay consentimiento implícito por credenciales. No se implementa todavía un consentimiento de subida de grabaciones; tampoco se anuncia esa opción como disponible.

No hay API de inyección de PCM/transcripciones/resultados de sidecar. Las rutas legacy `/voice/log`, `/voice/sidecar-event`, `/voice/calibration`, `/dictate/start` y `/dictate/stop` no están registradas. Los streams legacy son de lectura inactiva; no activan audio. Auth/Host/Origin permanecen vigentes.

F2 conserva exactamente system_info Green, read_file/create_file Yellow y edit_file Red, con las otras 19 bloqueadas. Ninguna palabra reconocida ejecuta acciones. Dictado no añade autorización, y no se implementa envío automático por wake.
