# F4-R2 — Resultados verificables

**STT_SELECTED provisional: Vosk / LIVE_PTT_PASS (flujos probados) / F4 NEEDS_FIXES.** Grabar/Detener/edición y prueba separada de Cancelar confirmadas por el usuario; evidencia técnica de reposo, no envío y reaping. F4 no aceptada en conjunto. Sin commits, push, merge o release.

## Evidencia y límites

| Categoría | Resultado | Alcance real |
|---|---|---|
| UNIT | Evaluador/grabador/guardas de PTT: 12 PASS; aislamiento HOME/TMP/XDG: 3 PASS | Incluye procesos sintéticos; no son grabaciones humanas |
| UNIT | VoiceCoordinator 12, procesos 3, selección STT 1, integridad Vosk 2, HTTP voz 2: PASS | Preparación/parada, identidad, cancelación y rutas; sin hardware |
| UNIT | CLI: 1 test / 20 combinaciones PASS | Modos Vosk/Whisper, aridad y modos desconocidos no crean perfil ni Backend |
| AUTOMATED_AUDIO_FIXTURE | Resampling/límites: 4 PASS; daemon PipeWire privado 48→16 kHz PASS | Tono 440,131 Hz, RMS 0,707151; **no micrófono físico** |
| AUTOMATED_AUDIO_FIXTURE | Inferencia offline sobre cuatro WAV humanos autorizados, 3 repeticiones/configuración | WER 17,5% Vosk / 27,5% completo / 30% reducido, 40 palabras únicas; no certificación de español general |
| AUTOMATED_AUDIO_FIXTURE | Workers reales offline: paridad nativa Vosk 4/4, timeout, integridad corrupta y cancelación Vosk/Whisper PASS | Dos tests opt-in; progreso CPU confirmado antes de cancelar y PID recogido; no botón físico |
| AUTOMATED_QML | Voz: 8 PASS, Main normal/cancel/hide PASS | Dictado no llama chat, borrador conservado, operaciones tardías rechazadas |
| AUTOMATED_QML | Configuración: 16 PASS; serialización y recarga explícita Vosk, scroll, teclado, claro/oscuro | Mock local con escritura de speech persistida; 9 capturas generales + 2 Vosk específicas |
| AUTOMATED_KWIN | Gates F3.5/F3 --plasma PASS sobre ventanas propias | No interacción humana ni consentimiento de atajos reales |
| LIVE_MICROPHONE | Cuatro tomas de corpus autorizadas: 8/10/10/15 s, procesos recogidos | Dos intentos cortos previos EOF/0 PCM/sin WAV; no PTT productivo |
| HUMAN_VERIFIED | Usuario confirmó las cuatro referencias pronunciadas | No se confunde con aceptación de los textos reconocidos, latencia o PTT |
| LIVE_MICROPHONE / HUMAN_VERIFIED | Primera prueba PTT Grabar/Detener y edición: PASS | Usuario: funcionó bastante rápido y pudo editar; captura y STT recogidos; sesión con cero mensajes. Rapidez cualitativa, sin cronómetro ni nuevo WER sobre texto editado |
| LIVE_MICROPHONE / HUMAN_VERIFIED | Cancelar físico: PASS en los ciclos probados | Dos recording→idle; último estado idle; cero mensajes/LLM y todos los procesos recogidos |
| NOT_TESTED | Desconexión física, otros dispositivos/acentos, Piper audible, Hey Jarvis | Sin autorización implícita a nuevas tomas o instalaciones |

[Capturas consentidas](F4-R2-EVIDENCE/corpus-plan.json), [comparación](F4-R2-BENCHMARKS.md), [workers reales](F4-R2-EVIDENCE/worker-native-final.log), [12 tests finales](F4-R2-EVIDENCE/spanish-observer-final.log), [gate F4](F4-R2-EVIDENCE/gates-final-3/f4/results.json), [QML final](F4-R2-EVIDENCE/qml-final-2/qml.log).

## Gates y comandos

Todo ejecutable de aplicación se lanzó con perfil privado validado. HOME/TMP y cuatro raíces XDG son propios y distintos; bus/audio inaccesibles salvo la excepción de infraestructura de escritorio de los gates --plasma y el grabador consentido. LLM mock loopback, sin claves reales. No se ejecutaron suites concurrentes.

| Comando oficial | Resultado |
|---|---|
| `cargo fmt --check` | PASS |
| `cargo clippy --locked --all-targets -- -D warnings` | PASS, también repetido tras el ajuste del fixture SQLite |
| `cargo clippy --locked --features legacy-fixtures --all-targets -- -D warnings` | PASS, también repetido tras el ajuste del fixture SQLite |
| `cargo build --locked --bin kde-assistant --bin bench_voice` | PASS |
| `python3 -B scripts/validate_f4.py --output docs/recovery/F4-R2-EVIDENCE/gates-final-3/f4` | PASS |
| `python3 -B scripts/validate_f35.py --plasma` | PASS; QML repetido focalmente tras añadir capturas Vosk/espera de layout |
| `python3 -B scripts/validate_f2.py` | PASS: F1A, F1B1/B2/B3A/B3B, F2, HTTP, QML y gate-w1r |
| `python3 -B scripts/validate_f3.py --runtime-only --plasma` | PASS |
| `python3 -B scripts/validate_f1b3b.py --qml` | PASS tras corregir la espera de destrucción del fixture; sin omitir aserciones |

Orquestación exacta de la pasada: `python3 -B scripts/validate_f4_r2.py --output docs/recovery/F4-R2-EVIDENCE/gates-final-3`. [Registro](F4-R2-EVIDENCE/gates-final-3/results.json). Esa pasada registra el primer fallo F1B3B; el reemplazo final PASS está en [final-regressions.json](F4-R2-EVIDENCE/final-regressions.json) y [log completo](F4-R2-EVIDENCE/f1b3b-final.log). [Estado combinado y hashes de los nueve informes conservados](F4-R2-EVIDENCE/review-final.json).

El ajuste final de F1B3A sólo afecta al teardown del test: espera referencias compartidas y posee el destructor final antes de reabrir. Una Weak no actualizable no garantizaba que el descriptor flock hubiera cerrado. Mantiene plazo de tres segundos y todas las comprobaciones de UnknownOutcome/journal/permisos; no se reintenta ningún efecto. Los tres casos HTTP focales pasaron y después pasó el runner F1B3B completo. No se cambió código productivo de SQLite, F2 o permisos. [Focal](F4-R2-EVIDENCE/f1b3a-teardown-final.log).

El QML final se ejecutó con `validate_f35.qml_tests(repo, output, private_env, build_runner(...))`, mismo harness oficial, perfil privado y offscreen; 16 PASS. Se dejó asentar el layout después de mostrar los campos antes de forzar foco: la primera captura de prueba intentaba enfocar antes del polish y falló su aserción de viewport. No se eliminó esa aserción. [Oscuro](F4-R2-EVIDENCE/qml-final-2/r2-vosk-dark.png), [claro](F4-R2-EVIDENCE/qml-final-2/r2-vosk-light.png).

## Comandos de evaluación offline y hardware

```text
python3 -B scripts/test_f4_safety.py
python3 -B scripts/test_f4_spanish.py
python3 -B scripts/compare_f4_spanish.py --prepare
python3 -B scripts/record_f4_spanish.py --profile /var/tmp/kda-f4r2-corpus-gjw89ydm --receipt RECIBO_PRIVADO_DE_UN_USO
python3 -B scripts/compare_f4_spanish.py --profile /var/tmp/kda-f4r2-corpus-gjw89ydm --whisper-model /var/tmp/kda-f4-tiny-jaan_09m/ggml-tiny.bin --vosk-lib /var/tmp/kda-f4r2-vosk-pruy5scb/vosk/libvosk.so --vosk-model /var/tmp/kda-f4r2-vosk-pruy5scb/vosk-model-small-es-0.42 --runs 3
python3 -B scripts/benchmark_native_vosk_f4.py --profile /var/tmp/kda-f4r2-corpus-gjw89ydm --library /var/tmp/kda-f4r2-vosk-pruy5scb/vosk/libvosk.so --model /var/tmp/kda-f4r2-vosk-pruy5scb/vosk-model-small-es-0.42 --name native-before --runs 3
python3 -B scripts/benchmark_native_vosk_f4.py --profile /var/tmp/kda-f4r2-corpus-gjw89ydm --library /var/tmp/kda-f4r2-vosk-pruy5scb/vosk/libvosk.so --model /var/tmp/kda-f4r2-vosk-pruy5scb/vosk-model-small-es-0.42 --name native-after --runs 3
cargo test --offline --locked --test f4_worker -- --ignored --nocapture --test-threads=1
python3 -B scripts/live_f4_ptt.py --prepare --library /var/tmp/kda-f4r2-vosk-pruy5scb/vosk/libvosk.so --model /var/tmp/kda-f4r2-vosk-pruy5scb/vosk-model-small-es-0.42
```

Los recibos concretos de captura y los manifests por muestra están privados; cada toma se hizo tras permiso y preparación explícitos, y cada referencia se confirmó después. Se archivó cada comparación sin sobrescribir datos de la siguiente. La última manifest selecciona long. Para el test opt-in se establecieron explícitamente `KDA_F4_LOCAL_MODEL`, `KDA_F4_VOSK_LIBRARY`, `KDA_F4_VOSK_MODEL` y `KDA_F4_VOSK_CORPUS` con las rutas anteriores, dentro de un entorno privado, Cargo offline y jobs=2. Los benchmarks se ejecutaron después de acabar compilación; comparador verifica ausencia de compiladores.

Primer build --locked requirió registrar libloading ya existente como dependencia raíz del lock y falló antes de cambiar paquetes; se añadió sólo esa línea y se construyó offline/locked. Primer Clippy detectó int_plus_one; expresión equivalente corregida. Primer gate con TMPDIR anidado falló por socket >108 B; daemon diagnóstico privado confirmó la causa, fixture con ruta corta pasó. Los logs fallidos se conservan; no se contabilizan como PASS.

## Recursos y privacidad

Reposo del backend de texto en gate F4: tres segundos, RSS 26320 KiB / PSS 23510 KiB, CPU un núcleo muestreada 0,0%; sin hijos de voz. Observación breve y cuantizada, no promesa de consumo exactamente cero. Texto escrito funciona con modelo ausente y todos los servicios de voz apagados; el mock recibió sólo la petición de texto deliberada. [Runtime](F4-R2-EVIDENCE/gates-final-3/f4/runtime.json).

CPU/RSS/PSS, carga, etapas y percentiles de cada motor están en BENCHMARKS. Existe una primera prueba PTT física con valoración cualitativa positiva; no hay aún latencia cronometrada completa ni Piper audible. No se inventan cifras. El p50 nativo largo 3,758 s incluye verificación/carga/inferencia/liberación, no captura ni preflight independiente ni QML.

Ningún WAV humano, hipótesis completa, DB o token se añadió a Git. El corpus humano, hipótesis y logs privados se eliminaron al cerrar la comparación offline ([cleanup](F4-R2-EVIDENCE/cleanup-corpus.json)); Vosk y ambos perfiles PTT también se eliminaron al cerrar ([registro final](F4-R2-EVIDENCE/cleanup-final.json)); usuario amplió explícitamente el uso Vosk para PTT y autorizó una primera prueba física (no Cancelar ni nuevas tomas). El usuario abrió el helper y realizó Grabar/Detener y edición, con captura de pantalla aportada. Backend en transcript_ready, cero mensajes y sólo qml6 como hijo al inspeccionar; no quedaban captura ni worker. [Evidencia sanitizada](F4-R2-EVIDENCE/live-ptt-first.json). Cancelar físico también fue confirmado en la prueba separada, con su propio registro; no se repite captura sin permiso. La limpieza ya se realizó, sin prometer borrado forense. No se tocó el modelo tiny previo ni el perfil personal.

## Primera prueba PTT humana

El usuario informó «Funcionó bastante rápido, y pude editar el texto» y aportó captura del borrador con el aviso de revisión. Se registra HUMAN_VERIFIED para edición y valoración cualitativa; no se puntúa WER sobre un borrador que pudo ser corregido. API del perfil privado confirmó transcript_ready, texto no vacío y cero mensajes. Sólo qml6 seguía como hijo del backend: pw-record y el worker ya habían terminado. El launcher terminó sin producir su resumen final; la causa no se determinó. No hay contador final de llamadas al mock ni latencia exacta recuperable. Después se comprobó que los tres PID conocidos (launcher/backend/QML) ya no existían y que el perfil no contenía PCM/WAV. No se reconstruyeron transiciones ni resultados inexistentes.

El primer intento de ejecutar el comando tuvo dos erratas de ruta, `Live`/`live` y `2xad`/`2xa4`; Python salió antes de iniciar la aplicación. No es un fallo del motor ni una toma de audio; se indicó el comando correcto sin consumir un nuevo consentimiento.

## Corrección del observador de prueba tras el PTT

`live_f4_ptt.py` ahora guarda atómicamente un resumen sin transcripción al cambiar de estado y antes/después de cleanup, vacía el log por línea y sólo marca reaping cuando termina. Así no depende exclusivamente del cierre final para conservar observaciones. Un fixture de proceso sin audio simula la pérdida del observador después de transcript_ready: comprueba registro parcial anterior al fallo y recolección del proceso exacto al salir. No reescribe la evidencia ausente de la primera prueba.

`python3 -B scripts/test_f4_spanish.py`: 12 PASS. Después, `python3 -B scripts/validate_f4.py --output docs/recovery/F4-R2-EVIDENCE/f4-observer-final`: PASS completo, serial y privado, sin modelos/hardware. [Log focal](F4-R2-EVIDENCE/spanish-observer-final.log), [gate afectado](F4-R2-EVIDENCE/f4-observer-final/results.json). Los demás gates anteriores no se repitieron por un cambio exclusivo del helper Python; no hubo cambios productivos Rust/QML.

Los comandos históricos de comparación/test opt-in requieren volver a aportar un corpus consentido: los cuatro WAV, hipótesis y logs privados ya se eliminaron. Los resúmenes sanitizados, WER y métricas permanecen. Vosk/perfiles temporales se eliminaron al cerrar la evaluación PTT autorizada; Tras la solicitud separada de Cancelar, el usuario indicó continua; se preparó un recibo para esa prueba manual, con captura únicamente mediante acción física. El usuario realizó esa prueba y confirmó que funcionó correctamente; resultado documentado abajo. Sin promesa de borrado forense.

## Cierre de Cancelar y limpieza

[LIVE_MICROPHONE / HUMAN_VERIFIED](F4-R2-EVIDENCE/live-ptt-cancel.json): el usuario respondió «funciono correctamente» al flujo de Cancelar. El registro del launcher muestra tres ciclos iniciados físicamente por el usuario: dos recording→idle y uno recording→transcribing→transcript_ready. No se oculta el ciclo adicional ni se puntúa su texto como corpus. Último estado idle, cero peticiones al mock, cero mensajes, ended/backend_reaped/children_reaped true y ningún archivo PCM/WAV. No se automatizaron clics o grabaciones.

El ciclo auxiliar registró 1451,779 ms entre observar transcribing y transcript_ready, **n=1**, monitor 100 ms. No mide clic físico→texto editable ni latencia de Cancelar; no tiene p50/p95. Los aproximadamente tres segundos grabando antes de volver a idle incluyen la espera del usuario y no se reportan como demora de cancelación.

Corpus privado eliminado primero; después runtime/modelo Vosk, archivos descargados y ambos perfiles PTT eliminados por directorio exacto validado. [Limpieza corpus](F4-R2-EVIDENCE/cleanup-corpus.json), [limpieza final](F4-R2-EVIDENCE/cleanup-final.json). Modelo tiny preexistente intacto, sin configuración personal inspeccionada o cambiada. No queda instalación permanente Vosk: la selección experimental existe en código, su uso posterior requerirá recursos locales configurados expresamente.

Resultado R2: STT_SELECTED (Vosk provisional), LIVE_PTT_PASS para los flujos probados, F4 NEEDS_FIXES por TTS audible y límites de precisión/rendimiento documentados. Gates focales/regresiones PASS; no hubo cambios productivos tras las pruebas físicas. Sin commits, push, merge o release. Se espera revisión técnica.
