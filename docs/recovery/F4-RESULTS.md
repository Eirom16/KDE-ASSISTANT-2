# F4 — Resultados y límites de evidencia

Fecha: 2026-10-18. Baseline `9544a58eba4b6adf6c6343c04db80813b084f76b`, rama `recovery/f4-voice-coordinator`, sin commits. **PARTIAL / NEEDS_FIXES para aceptar F4.** Los logs finales no eliminan los fallos intermedios ni el incidente de aislamiento.

## Comandos y regresiones

| Comando exacto | Resultado | Evidencia |
| --- | --- | --- |
| `python3 scripts/validate_f4.py --output docs/recovery/F4-EVIDENCE/FINAL` | PASS | [runner](F4-EVIDENCE/FINAL/results.json), [log](F4-EVIDENCE/FINAL/coordinator.log) |
| `cargo fmt --check` | PASS | [log](F4-EVIDENCE/final-checks/fmt.log) |
| `cargo clippy --locked --all-targets -- -D warnings` | PASS | [log](F4-EVIDENCE/final-checks/clippy.log) |
| `cargo clippy --locked --features legacy-fixtures --all-targets -- -D warnings` | PASS | [log](F4-EVIDENCE/final-checks/clippy-legacy.log) |
| `cargo build --locked --bin kde-assistant` | PASS | [log](F4-EVIDENCE/final-checks/build.log) |
| `python3 scripts/validate_f35.py --plasma` | PASS | [log](F4-EVIDENCE/regressions/f35.log); 10 tests loopback, 15 Qt, nueve capturas, reinicios backend/QML reales |
| `python3 scripts/validate_f2.py` | PASS | [log](F4-EVIDENCE/regressions/f2.log); incluye F1A/F1B/F2, permisos, consentimiento Qt, fmt/check/Clippy default+legacy |
| `python3 scripts/validate_f3.py --runtime-only --plasma` | PASS tras corrección | [log final](F4-EVIDENCE/regressions/review-f3.log), [JSON nativo](F4-EVIDENCE/regressions/f3-runtime-final.json). Primer intento falló por retirada accidental de registro DBus, corregida sin alterar runner/aserciones |
| `python3 scripts/validate_f1b3b.py --qml` | PASS | [log](F4-EVIDENCE/regressions/review-f1b3b.log); selección Rust y QML offscreen existentes |
| `KDA_F4_LOCAL_MODEL=/var/tmp/kda-f4-tiny-jaan_09m/ggml-tiny.bin cargo test --locked --test f4_worker -- --ignored --test-threads=1` | PASS, 1 | [log](F4-EVIDENCE/final-checks/worker.log); ejecutado con XDG privado y sin claves |

El ajuste final de campos wake deshabilitados y contraste del estado vocal pasó además 15 tests Settings QML y las capturas de voz sobre Wayland/OpenGL: [resultado](F4-EVIDENCE/final-ui/results.json), [voz](F4-EVIDENCE/final-ui/voice.log).

El runner F4 pasó dos veces consecutivas sobre el mismo directorio de salida ([registro](F4-EVIDENCE/final-validation.json)); el segundo intento comprueba que un log antiguo no adelanta la detección de readiness. Las últimas verificaciones Rust y F4 se repitieron tras restaurar DBus y ajustar indisponibilidad QML. No se repitieron conexiones Groq reales. Los contadores solapados de suites no se suman. El test opt-in del worker permanece ignored al ejecutar cargo test sin la elección explícita del modelo. Los dos externos F1A mantienen las exclusiones heredadas del runner; no se presentan como PASS.

La evidencia externa íntegra de F3.5 está en `/tmp/kda-f35-validation-l9mc29mp/`; F1B en `/tmp/kda-f1b3b-check-u1q056ah/`; F3 final en `/tmp/kda-f3-validation-6pav6kfw/`. Se copian sólo logs/resultados/capturas seguros, no perfiles, SQLite, tokens ni archivos de autenticación.

## F4 por categoría

| Categoría | Evidencia real | Lo que no demuestra |
| --- | --- | --- |
| UNIT | 11 coordinador + 3 procesos + 2 handlers; estados, reservas revocables, IDs obsoletos, silencio, captura acotada, errores, cancelación con procesos sintéticos, entorno sin claves, salida limitada y PID exacto recogido | No prueba micrófono, oído, precisión ni detección wake |
| AUTOMATED_AUDIO_FIXTURE | 4 tests de conversión/modelos: duración y seno 1 kHz a 16/44,1/48 kHz, rechazo de alias 12 kHz, canales/rates inválidos y archivos corruptos/ausentes | La referencia sinc offline no es una certificación del driver físico |
| AUTOMATED_AUDIO_FIXTURE | Daemon PipeWire privado sin ALSA/udev/Pulse/Bluetooth/session manager; seno SPA 440 Hz a 48 kHz; `pw-record` genera 66.164 B PCM16 mono 16 kHz; frecuencia medida 440,13 Hz, RMS ~0,707; procesos desaparecen | No es LIVE_MICROPHONE ni un dictado español completo |
| AUTOMATED_AUDIO_FIXTURE | Backend auténtico, perfil XDG sintético: sin hijos al arrancar, modos internos sin config/UI, auth 401, IDs erróneos 404/409, JSON de inyección 422, modelo ausente visible, sin mensajes dictados ni petición LLM, cancelación y rechazo de replay; después un chat escrito al mock funciona incluso con stt_backend=api | No prueba la ruta positiva completa captura→Whisper de habla→edición |
| AUTOMATED_AUDIO_FIXTURE | Worker Whisper real con tiny autorizado y PCM generado: cancelaciones tras 30/500 ms y timeout de 1 s; mismo supervisor productivo; sin hijos tras retornar | Los instantes no instrumentan la frontera exacta carga/inferencia. No atribuir cada cancelación a una etapa concreta sólo por tiempo |
| AUTOMATED_QML | 6 casos funcionales, 8 PASS incluyendo init/cleanup: estados, borrador conservado/editable, ausencia de autoenvío, cancelación antes del ack, scope conocido, cambio de sesión y resultados duplicados, error, mic deshabilitado sin modelo y texto disponible | Requests de VoiceController se simulan en estos tests; no es un usuario hablando |
| AUTOMATED_QML | Main.qml de producción con HTTP demorado: caso positivo inicia una captura simulada; cancelar u ocultar antes de crear la primera sesión inicia cero capturas y conserva borrador. [Tres escenarios](F4-EVIDENCE/FINAL/main-lifecycle.json) | El servidor es sintético y nunca abre audio; no es aceptación de micrófono |
| AUTOMATED_QML | Capturas claras/oscuras de grabación, transcripción y error; inspección visual local; color principal del token Theme para estado legible | El render offscreen software no pinta ColorOverlay correctamente; se verificó también en Wayland/OpenGL, sin cambiar Octicon |
| AUTOMATED_KWIN | F3 y F3.5 observan exclusivamente ventanas propias en Plasma Wayland | No es aceptación humana de teclado ni registro real de atajos |
| HUMAN_VERIFIED | Sólo declaración previa del usuario de que Groq F3.5 funciona; gate F3.5-W1 omitido por decisión suya | No es evidencia humana F4/audio |
| LIVE_MICROPHONE / LIVE_TTS / LIVE_WAKE_WORD | NOT_TESTED | No se encendió micrófono físico ni salida audible ni escucha wake |
| NOT_SUPPORTED | Global PTT registrado, barge-in por voz con eco, TTS incremental, STT remoto en F4 | No se anuncian como operativos |
| BLOCKED | Aceptación de rendimiento/precisión española, Piper real, wake tras sus gates | No se sustituyen por tests de carga |

Logs del runner: [coordinador](F4-EVIDENCE/FINAL/coordinator.log), [procesos](F4-EVIDENCE/FINAL/process.log), [HTTP](F4-EVIDENCE/FINAL/native-http.log), [audio](F4-EVIDENCE/FINAL/audio.log), [QML](F4-EVIDENCE/FINAL/qml.log), [PipeWire](F4-EVIDENCE/FINAL/pipewire.json), [runtime](F4-EVIDENCE/FINAL/runtime.json). Capturas nativas: [oscuro/grabación](F4-EVIDENCE/visual-plasma/dark-recording.png), [claro/transcripción](F4-EVIDENCE/visual-plasma/light-transcribing.png).

## Cobertura de los 17 requisitos

1. Transiciones: UNIT, tabla efectiva rechaza incompatibles.
2. Inicio/parada PTT: AUTOMATED_QML + capture_pcm sintético; helper PipeWire real separado. Flujo físico positivo pendiente.
3. Cancelar captura: UNIT con worker sintético y stop de capture_pcm; LIVE pendiente.
4. Cancelar STT: UNIT + worker Whisper real cancelado; sin sesión física.
5. Cambio de sesión durante STT: UNIT/QML descartan resultado viejo y drenan operación; captura española completa pendiente.
6. TTS interrumpida: surrogate de proceso bajo estado Speaking; Piper real NOT_TESTED.
7. Fallo de dispositivo: EOF/ausencia de datos sintéticos; desconexión física NOT_TESTED.
8. Modelo ausente/corrupto: fixtures y API auténtica sin modelo. Corrupción completa de pesos con header válido no es certificación de integridad de cualquier modelo.
9. Timeout: helper sintético y Whisper real, PID recogido.
10. Resampling: señales y frecuencia a tres tasas; PipeWire 48→16 separado.
11. Buffers/límites: audio y stdout bounded; no acumulación de cola.
12. Sin envío remoto: módulo sin cliente remoto, routing local y mock sólo recibe el chat escrito deliberado; no credenciales reales en esos tests.
13. Estado visual: Qt Test + capturas claras/oscuras nativas.
14. Sin autoenvío: SignalSpy cero envíos y conversación sin mensajes tras dictado fallido.
15. Sin herramientas desde voz: coordinador sin dependencia del executor/LLM y catálogo F2 regresado.
16. Sin huérfanos: PID exacto del fixture, workers Whisper, daemon/recorder privados y lifecycle F3. No se certifica cualquier wrapper/nieto externo Piper ausente.
17. Texto sin voz: mock chat completo después del error STT, más regresiones F1B/F3.5.

## Benchmark exploratorio en N5000

Host observado: Pentium Silver N5000, cuatro cores, x86_64, sin AVX/CUDA asumidos; Linux CachyOS 7.2.7. Ejecutable Cargo **dev**; whisper-rs-sys genera C++ RelWithDebInfo/O2 con WHISPER_DEBUG. No se construyó release para esta comparación y no se vació page cache. «Fría» significa contexto de modelo aún sin cargar en ese proceso, no disco frío certificado. Los motores no están aceptados por estos datos.

Fixture propio: seno de 440 Hz, tres segundos, WAV PCM16 mono 48 kHz. Ambos modelos devolvieron una etiqueta de música, **no una transcripción española**. Base se midió mientras había compilaciones; tiny después, sin compilaciones concurrentes. No es una comparación controlada suficiente ni un corpus de habla. n=5 produce p50/p95 descriptivos; no hay suficientes muestras para fijar SLO ni certificar colas de latencia.

| Medida | Base existente | Tiny autorizado |
| --- | --- | --- |
| Carga fría contexto, n=1 | 270,911 ms | 368,806 ms |
| Conversión/WAV, n=1 | 72,594 ms, referencia lineal anterior del binario | 1.020,745 ms, sinc offline actual; producción usa PipeWire |
| Inferencias calientes, ms, n=5 | 83579,530; 115346,917; 100294,156; 81410,747; 93810,461 | 39890,550; 36346,884; 44877,465; 45098,032; 34015,221 |
| p50 / p95 descriptivos | 93,810 / 115,347 s | 39,891 / 45,098 s |
| CPU usuario / sistema | 1219,121 / 3,897 s en 475,049 s de pared | 496,152 / 2,766 s en 201,792 s de pared |
| CPU media, % de un core (derivada) | ~257,45 % | ~247,24 % |
| Pico RSS/PSS muestreado | 335324 / 332083 KiB | 228688 / 224702 KiB |
| TTT integrado de usuario, p50/p95 | NOT_TESTED | NOT_TESTED |
| Habla/WER | NOT_TESTED | NOT_TESTED |

El tiempo de carga comparativamente pequeño frente a inferencia apoya liberar el modelo por operación en esta primera implementación; no justifica mantener dos motores calientes. Tiny consumió menos RAM y resultó más rápido en estas condiciones, pero sigue siendo muy lento. No se cambia automáticamente el modelo del usuario ni se selecciona un motor definitivo. Hace falta comparar alternativas pequeñas con habla española y build optimizado; no se inventa su velocidad.

Backend sin UI y sin motores, una ventana de 3,002 s: RSS 26596 KiB, PSS 23870 KiB y 0 ticks CPU observados (0,0 % redondeado). Es una observación corta y cuantizada de ese proceso, no promesa de cero CPU ni coste de la aplicación completa. No se fijó presupuesto numérico de producto.

Comandos:

```text
cargo build --locked --bin bench_voice
python3 scripts/voice_corpus.py docs/recovery/F4-EVIDENCE/corpus
# Primer experimento, wrapper de muestreo /proc descrito en JSON:
target/debug/bench_voice /home/eirom/.local/share/kde-assistant/models/ggml-base.bin docs/recovery/F4-EVIDENCE/corpus/tone.wav 5 docs/recovery/F4-EVIDENCE/bench-base-debug.json
# Runner reproducible de la comparación autorizada:
python3 scripts/benchmark_f4.py --model /var/tmp/kda-f4-tiny-jaan_09m/ggml-tiny.bin --runs 5 --output docs/recovery/F4-EVIDENCE/bench-tiny
```

[Base tiempos](F4-EVIDENCE/bench-base-debug.json), [base proceso](F4-EVIDENCE/bench-base-debug-process.json), [tiny tiempos](F4-EVIDENCE/bench-tiny/benchmark.json), [tiny proceso](F4-EVIDENCE/bench-tiny/process.json), [autorización/fuente/revisión/hash tiny](F4-EVIDENCE/tiny-download.json). Tiny: revisión `5359861c739e955e79d9a303bcbc70fb988958b1`, 77.691.713 bytes, SHA-256 `be07e048e1e599ad46341c8d2a135645097a538221678b7acdd1b1919c6e1b21`. Se descargó únicamente ese modelo y no se instaló en el perfil personal.

## Corpus y precisión

`scripts/voice_corpus.py` genera silencio, ruido determinista y seno; textos de referencia cortos/largos/números/apps/consulta natural, propios y CC0. El [manifest](F4-EVIDENCE/corpus/manifest.json) marca explícitamente las grabaciones humanas ausentes. No se utilizó habla personal ni TTS como sustituto de una persona. WER implementado/verificado con ejemplos de texto; **WER de reconocimiento de español no disponible**. No se atribuye precisión a los modelos desde una señal o tests unitarios.

Inicio TTS, primer PCM Piper, primer audio audible, cola real y coste Piper: NOT_TESTED. Wake TP/FP/FN, latencia, CPU/RSS de escucha y activación por propia TTS: NOT_TESTED/BLOCKED. No se pide instalar voces/paquetes ni activar hardware durante esta entrega.

## Fallos intermedios e incidente

Se conservan logs de validación inicial/final y primera regresión F3. El fallo de reaping inicial pertenecía al marcador PID publicado antes de escribirlo; se corrigió el fixture con rename atómico y se mantuvo la comprobación del PID exacto. El daemon inicial sin negociación de puertos fue un fixture inválido, no evidencia de micrófono físico fallido. Reutilizar un directorio de evidencia reveló otro fallo del fixture: el log en append podía contener readiness antiguo antes de generar el token nuevo. Se trunca al lanzar cada instancia; los dos intentos finales pasan con el mismo directorio. No se relajaron aserciones del producto ni de suites heredadas.

El diagnóstico de un dispatcher incompleto arrancó una instancia del **perfil personal** fuera del aislamiento: F4-INC-01 en [ISSUES](F4-ISSUES.md). Se terminó su backend propio; no hubo audio ni atajos, ni chat de proveedor observado. El arranque abrió config/SQLite y pudo alterar recuperación/metadata/bootstrap; no se dispone de estado previo para afirmar que no cambió. No se oculta ese hecho bajo los PASS posteriores ni se hizo rollback especulativo.

No hubo root, paquetes globales, cambios a atajos KDE, aplicaciones ajenas, commits, push, merge, release ni inicio de otra fase. La única descarga aprobada fue tiny. Las pruebas físicas y aceptación de rendimiento/precisión siguen pendientes de revisión y autorización separadas.
