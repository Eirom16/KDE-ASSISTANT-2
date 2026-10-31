# F4-R — Resultados de validación

**Rendimiento: IMPROVED en experimentos offline. Estado F4: NEEDS_FIXES.** El default productivo no fue cambiado a un motor/contexto sin precisión validada. Sin commits ni aceptación física. Las cifras no son presupuestos ni certificación de voz española.

## Gates finales

Todos se ejecutaron en serie, Cargo offline, CARGO_BUILD_JOBS=2, sin compilación pesada concurrente con inferencia. Perfiles privados y datos/credenciales sintéticos. Los gates Plasma conservan acceso al display/bus de sesión únicamente para sus propias ventanas; no hay registro de atajos ni captura física.

| Comando exacto | Resultado | Log |
| --- | --- | --- |
| `cargo fmt --check` | PASS, 0.968 s | [fmt](F4-R-EVIDENCE/gates-final/fmt.log) |
| `cargo clippy --locked --all-targets -- -D warnings` | PASS, 7.794 s | [clippy](F4-R-EVIDENCE/gates-final/clippy.log) |
| `cargo clippy --locked --features legacy-fixtures --all-targets -- -D warnings` | PASS, 8.100 s | [clippy-legacy](F4-R-EVIDENCE/gates-final/clippy-legacy.log) |
| `cargo build --locked --bin kde-assistant --bin bench_voice` | PASS, 0.817 s | [build](F4-R-EVIDENCE/gates-final/build.log) |
| `KDA_F4_LOCAL_MODEL=/var/tmp/kda-f4-tiny-jaan_09m/ggml-tiny.bin cargo test --locked --test f4_worker -- --ignored --test-threads=1 --nocapture` | PASS, 4.944 s | [worker](F4-R-EVIDENCE/gates-final/worker.log) |
| `python3 scripts/validate_f4.py --output docs/recovery/F4-R-EVIDENCE/gates-final/f4` | PASS, 39.598 s | [f4](F4-R-EVIDENCE/gates-final/f4.log) |
| `python3 scripts/validate_f35.py --plasma` | PASS, 23.218 s | [f35](F4-R-EVIDENCE/gates-final/f35.log) |
| `python3 scripts/validate_f2.py` | PASS, 72.718 s | [f2](F4-R-EVIDENCE/gates-final/f2.log) |
| `python3 scripts/validate_f3.py --runtime-only --plasma` | PASS, 32.300 s | [f3](F4-R-EVIDENCE/gates-final/f3.log) |
| `python3 scripts/validate_f1b3b.py --qml` | PASS, 38.030 s | [f1b3b](F4-R-EVIDENCE/gates-final/f1b3b.log) |

[Registro final](F4-R-EVIDENCE/gates-final/results.json), [driver](F4-R-EVIDENCE/gate-final-driver.log). La primera ejecución completa también pasó; tras instrumentar la cancelación se repitieron los gates sobre el código final. Se conserva la historia en `gates/`, `f4-first/` y sus logs. No se relajaron aserciones heredadas ni se cambió el catálogo F2. Las exclusiones/ignored heredadas siguen identificadas por sus runners: PASS de la selección no equivale a pruebas externas o humanas omitidas.

F3.5 usó proveedores mock en loopback. No se repitió Groq real ni se reutilizaron claves personales. El W1 humano omitido anteriormente por decisión del usuario continúa sin convertirse en aceptación exhaustiva.

## Clasificación de evidencia

| Categoría | Resultado verificable | Límite |
| --- | --- | --- |
| UNIT | CLI: 1 test con 14 variantes; salida antes de Backend y perfil sin archivos creados | No usa el perfil real para demostrar aislamiento |
| UNIT | 3 tests Python: env privado, omisiones/escapes/symlinks/permisos/credenciales/sockets; helpers rechazan antes de spawn/listen | No es un sandbox contra otros procesos del mismo UID |
| UNIT | 11 coordinador + 3 supervisor + 2 HTTP | No se suman otra vez al total de suites heredadas |
| AUTOMATED_AUDIO_FIXTURE | 4 tests resampling/modelos, fixture PipeWire propio y backend HTTP auténtico sin modelo | No es micrófono físico ni habla |
| AUTOMATED_AUDIO_FIXTURE | Whisper real: cancelaciones temporizadas 30/500 ms, timeout 1 s; además probe de inferencia confirmado | Temporizar por sí solo no identifica una etapa; el probe aporta señal/progreso independientes |
| AUTOMATED_AUDIO_FIXTURE | Vosk 2 ejecuciones, cada una con n=10 por señal, más terminación/reaping | Sólo señales sin habla, no motor productivo integrado |
| AUTOMATED_QML | 6 casos funcionales (8 PASS con init/cleanup), más Main real con HTTP demorado: positivo 1 inicio; cancel/hide 0; borrador conservado y chat 0 | Servidor simulado, sin audio físico |
| AUTOMATED_KWIN | F3/F3.5: ventanas propias, activación/salida/recursos sobre Wayland | Sin entrada humana ni consentimiento de atajo real |
| HUMAN_VERIFIED | Sin evidencia humana nueva de voz | Confirmación previa de Groq no valida F4-R |
| LIVE_MICROPHONE / LIVE_TTS / LIVE_WAKE_WORD | NOT_TESTED | Sin dispositivos físicos, voces ni escucha activados |
| BLOCKED / NOT_TESTED | O3/release sin CMake, corpus de habla/WER, tiny cuantizado y sherpa nativo | No se convierten en PASS mediante carga de archivos o tiempos sintéticos |

[Runner F4](F4-R-EVIDENCE/gates-final/f4/results.json), [runtime](F4-R-EVIDENCE/gates-final/f4/runtime.json), [PipeWire](F4-R-EVIDENCE/gates-final/f4/pipewire.json), [Main](F4-R-EVIDENCE/gates-final/f4/main-lifecycle.json), [F3](F4-R-EVIDENCE/gates-final/f3-results.json), [F3.5](F4-R-EVIDENCE/gates-final/f35-results.json).

## Cancelación y ownership

El probe publica atómicamente una marca tras cargar contexto/crear estado y justo antes de whisper_full; se observaron 3 ticks CPU posteriores antes de cancelar con `voice_process::run`. Cancel→retorno con kill+wait: 15.601 ms, n=1, PID 68746 ausente al retornar. [JSON](F4-R-EVIDENCE/confirmed-inference-cancel.json). No se presenta un percentil ni una aceptación audible de esa muestra. Los modos del worker principal se prueban además por separado con PCM generado, límites y timeout. El observer productivo no hace archivos ni logs.

Vosk fue terminado después de procesar un bloque PCM; el hijo exacto se recogió antes de retornar, sin procesos permanentes. Ambas pruebas son automatizadas de inferencia local; no cancelan ni certifican un turno LLM. Las regresiones confirman que voz fallida no rompe el chat escrito, y F2 conserva sus permisos.

## Recursos y latencia

Tiempos crudos, CPU, RSS/PSS, percentiles/desviación/MAD y condiciones en [BENCHMARKS](F4-R-BENCHMARKS.md), [resumen Whisper](F4-R-EVIDENCE/whisper-summary.json), [Vosk final](F4-R-EVIDENCE/vosk-final/benchmark.json) y [proceso Vosk](F4-R-EVIDENCE/vosk-final/process.json).

- Tiny completo canónico: p50 40,833 s, p95 40,954 s, n=3. Contexto 256: 5,468/5,636 s, n=3. No se cambió el default ni se certificó precisión desde el tono.
- Vosk final, n=10/señal: tono 376,836/410,453 ms; silencio 351,020/379,941 ms; ruido 470,124/475,887 ms. Texto siempre vacío, WER hablado NOT_TESTED. Cada ejecución se informa separadamente.
- Conversión, carga y creación de estado se separan de inferencia/texto disponible internamente. TTT integrado del usuario y primer audio audible: NOT_TESTED.

Backend sin UI/motores durante 3.003 s: RSS/PSS 26260/22305 KiB y 0.00 % de un core observado. Muestreo corto cuantizado, sin promesa de CPU cero. No había hijos vocales al arrancar.

| Muestra F3 final | Proceso | RSS/PSS (KiB) | CPU % un core |
| --- | --- | --- | --- |
| visible_idle, 3.021 s | backend | 27808/23648 | 0.00 |
| visible_idle, 3.021 s | QML | 187720/157361 | 12.58 |
| hidden_idle, 3.024 s | backend | 27808/23648 | 0.00 |
| hidden_idle, 3.024 s | QML | 187768/157409 | 2.31 |

Se muestran backend y UI por separado: el coste visible de QML no se atribuye a un modelo de voz apagado ni se oculta con una medida del backend solo. Estas ventanas breves no fijan un presupuesto de producto.

## Recursos autorizados y fallos no ocultados

Sólo se descargaron los dos artefactos Vosk aprobados. Runtime contrastado con SHA PyPI, modelo con tamaño/SHA registrado; no hubo pip ni instalación global. [Inventario/cleanup](F4-R-EVIDENCE/vosk-download.json) confirma que se eliminaron descarga y extracción privadas. Tiny existente se conservó, sin tocar defaults ni instalarlo en el perfil personal.

El intento de native Release falló con exit 101 por CMake ausente ([log](F4-R-EVIDENCE/native-release-build.log)). No hay resultados O3. La solicitud de descarga de CMake sigue sin autorización específica; «continúa» se atendió progresando en trabajo independiente. No se descargó CMake ni otro modelo/voz/binario. Los builds/gates PASS usan el cache existente, no acreditan un clean release.

No se volvió a abrir ni inspeccionar metadatos del perfil personal, ni se intentó rollback de F4-INC-01. La incertidumbre histórica se mantiene; [REPORT](F4-R-REPORT.md) documenta las guardas preventivas. Ningún PASS actual certifica que el arranque personal anterior no modificó archivos.

No se hicieron pruebas con audio personal, micrófono, altavoz ni wake, ni se usaron API keys reales. No hubo root, paquetes globales, GlobalShortcuts, cambios F2, commits, push, merge, release, F5 ni F6. Los cinco informes F4 se conservan intactos; revisión de hashes/HEAD/índice en [source-review](F4-R-EVIDENCE/source-review.json).
