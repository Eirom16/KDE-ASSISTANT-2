# F4 — Informe de implementación parcial

Fecha: 2026-10-18. **Implementación: PARTIAL. Revisión: NEEDS_FIXES para aceptar F4 como experiencia usable.** El código y la evidencia se entregan para revisión técnica; no hay commits.

## Baseline

Se verificaron HEAD `9544a58eba4b6adf6c6343c04db80813b084f76b`, rama F3.5, ancestry de producción `691687b4437fef41db84c2f0b00c127e19828720`, un único worktree, índice vacío y ausencia de modificaciones tracked. Había informes históricos sin seguimiento; no se sobrescribieron. No existía rama F4; se creó `recovery/f4-voice-coordinator`. Sin reset, clean, stash, push, merge, release, F5 ni F6.

Se leyeron AGENTS, PLAN/PROMPT/Apple Design, contratos F1B/F2/F3/F3.5, cierre/validación F3.5, roadmap y auditoría histórica. F3.5 permanece consolidada. La confirmación personal de Groq proviene del usuario; su gate formal F3.5-W1 fue omitido por su decisión, sin convertirlo en certificación exhaustiva ni repetir llamadas reales.

## Auditoría F4-0

| Componente | Implementación/estado anterior | Dependencias y coste | Problema/evidencia | Decisión |
| --- | --- | --- | --- | --- |
| Captura | cpal/ALSA, sólo arrancaba con wake+Piper+Whisper+ONNX | Stream continuo; callbacks con asignaciones y mutex compartido | Dictado por sí solo no abría dispositivo; error de stream sólo log; fallback de mic seleccionado | Sustituir camino productivo por helper PipeWire supervisado; conservar cpal para fixtures/historia |
| Resampling | Interpolación lineal y media de canales | CPU proporcional a samples | Sin antialias; tasa cero inválida; canales cero podían dividir/panicar | PipeWire en producto; sinc acotado para conversión offline y tests |
| WhisperEngine | whisper-rs 0.16 / whisper.cpp, carga lazy cacheada | Base local 147.951.465 B; inferencia intensiva CPU | Sin timeout/cancelación de spawn_blocking; todos los cores; contexto retenido indefinidamente | Conservar motor, aislar proceso terminable; tres hilos; elección final pendiente |
| SpeechService/STT API | auto/api elegía Groq al encontrar key, fallback local | HTTP multipart más CPU de fallback | Consentimiento por key insuficiente; podía sumar fallos remotos y espera local | Ruta productiva estrictamente local; adaptador remoto histórico sin caller activo |
| VoicePipeline Rust | STT→LLM→tools→TTS incremental, op-id informativo | Tareas y estados independientes de TurnRegistry | Estados inválidos aplicados, sesión mutable, buffers sin límite global, callbacks obsoletos | Retirar de arranque y mutación HTTP; conservar historia/fixtures, no usar como autoridad |
| Sidecar main.py | Pipecat/LocalAudioTransport/GroqSTT/GroqLLM/Piper | Python, VAD y otro contexto LLM | Segundo sistema conversacional y endpoint manual tools; no era lanzado por main actual | No iniciar ni reenviar eventos/comandos; no sustituirlo por otro agente |
| spike_bot.py | Experimento de voz cloud separado | Pipecat/Groq/Piper | Otro programa experimental, fuera de producto | Aplazar/retener como historia, sin ejecución |
| PiperEngine | Descubrimiento local y CLI reutilizables | Sin binario ni voces aquí | WAV por timestamp, cleanup sólo happy path, wait/output sin límites, logs con texto; stop no mataba síntesis | Reutilizar inventario; sustituir ejecución productiva por stdout bounded y kill+wait |
| SpeechService playback | rodio con flag compartido mutable | Sink/hilo y buffers WAV duplicados | Otra llamada podía resetear stop; productor incremental abortado sin ownership integral | No usar en nuevo camino; pw-play bajo misma operación |
| openWakeWord | ort dinámico + mel→embedding→hey_jarvis | Tres ONNX y runtime ya en disco, sin cargar en startup | Modelo por nombre no valida detección; fallback heurístico histórico, umbrales diversos, sin corpus adverso | BLOCKED tras gates; ninguna escucha automática ni fallback energético productivo |
| Configuración | F3.5 guarda campos speech, varios consumidores legacy | Sin carga de motores al guardar | auto_listen/chimes/calibración prometían caminos inactivos; API opt-in insuficiente | Conectar campos necesarios, deshabilitar controles no disponibles y explicar límites |
| UI | Mic oculto por voice_enabled de wake; dictado insertaba al borrador | QML REST/SSE existente | Requests sin generación/identidad; sin cancelación STT real | VoiceController y estado real; conservar diseño/editor |
| Supervisor F3/F1B | QML propio, registry, cancelación y journal fiables en su alcance | Tokio/SQLite | Voz heredada no pertenecía al supervisor | Reutilizar contratos; nueva operación vocal independiente sin efectos LLM |

Había dos pipelines conversacionales completos distintos (Rust y sidecar), un spike Python adicional y un subcamino de dictado. Tras F4 hay un único camino vocal **productivo**, que introduce texto en el chat existente. No se ha borrado código histórico funcional sin sustituir su caller productivo y conservar regresiones.

No se reprodujo un huérfano de audio del proyecto antiguo con hardware. Su ownership permitía persistencia de procesos/buffers en errores/cancelación; se distingue ese riesgo estático de un incidente demostrado. Los tests nuevos comprueban reaping de un hijo conocido. Los ajustes completos y su conexión se detallan en el contrato: idioma, modelo, dispositivo, autolectura y duración se usan; wake/escucha continua/chimes/API/speaker heredado no están aceptados como funcionalidades reales.

## Arquitectura y cambios

Nuevo `voice_coordinator.rs`: máquina explícita, identidad por sesión/generación/turno, reserva revocable, captura única, límites, cancelación y snapshot. `voice_process.rs`: subprocess sin shell, entorno mínimo, límites, deadline, kill+wait y vínculo al padre. El mismo ejecutable tiene modos restringidos de validación/inferencia que no inicializan Backend, Qt, credenciales ni SQLite.

Main ya no inicia wake ni captura con --enable-voice. El coordinador se drena antes del cierre de turnos. HTTP añade start/status/stop/cancel con identidad y retira mutaciones legacy. Autolectura se inicia tras persistir un turno completo y lleva su identidad; no retrasa ni modifica el texto. Rust conserva todas las sesiones/permisos/herramientas.

QML incorpora VoiceController, estados y cancelación del micrófono, indicador textual, edición/inserción en borrador y descarte de resultados tardíos/duplicados. Configuración conserva F3.5 y sólo adapta campos vocales. El tray deja de anunciar un PTT global no registrado. No hay rediseño, WebEngine, nuevos iconos externos ni atajos globales habilitados.

Las rutas modificadas incluyen main/mod/http_server, speech_service/stt/audio_capture/config, Main/InputBar/SettingsDialog/TrayMenu/qmldir; nuevos coordinador/procesos/VoiceController, benchmark/corpus/runner, fixtures y tests. No se cambia el motor de permisos ni el esquema SQLite. Se verifica también Main.qml real contra un servidor demorado: la primera sesión inicia una sola captura simulada; cancelar/ocultar antes del acknowledgement produce cero inicios.

API elegida: PipeWire local a través de sus herramientas nativas `pw-record`, `pw-dump`, `pw-play`, sin X11. La CLI es un helper de audio, no un servidor ni agente. Evita agregar otro binding nativo antes de validar captura. [Formato y resampling](https://docs.pipewire.org/page_man_pw-cat_1.html), [política de destino WirePlumber](https://pipewire.pages.freedesktop.org/wireplumber/policies/linking.html).

No se añadieron dependencias Cargo ni paquetes globales. Se reutilizan Tokio, serde, whisper-rs, hound y el inventario Piper. Las dependencias de voz antiguas siguen compilándose; no se afirma reducción del tamaño del binario ni empaquetado F6.

## Etapas y gates

| Subfase de entrega | Estado | Funciona con evidencia | Pendiente/gate |
| --- | --- | --- | --- |
| F4-A coordinador/PTT | PARTIAL | Lifecycle, scope revocable, cancelación, audio sintético PipeWire, dictado editable/QML, texto sin voz | Captura física, desconexión/ocupación/permisos reales y experiencia completa de usuario |
| F4-B STT local | PARTIAL | Modelo local, validación, worker terminable, límites, benchmarks sintéticos, errores sin tocar chat | Precisión española, WER hablado, presupuesto y motor suficientemente rápido; comparación tiny offline separada |
| F4-C Piper | PARTIAL | Integración final de turno y ownership/cancelación preparados; fixtures de proceso y error de ausencia | Piper/voz no instalados; síntesis real, reproducción, oído humano y métricas no aceptados |
| F4-D Hey Jarvis | BLOCKED | Auditoría de adaptador y modelos existentes; arranque deshabilitado | Gates anteriores, modelo/procedencia/detección real, ruido/eco/FP/FN/coste de escucha |

No se declara STT real de habla terminado, Piper escuchado ni wake usable. No se activó micrófono físico. La autorización adicional recibida sólo permite descargar tiny para benchmark offline: se verificó el SHA y se guardó en un directorio privado, sin instalarlo en el perfil personal ni cambiar defaults.

## Limitación e incidente de aislamiento

Durante el diagnóstico se omitió por error el aislamiento XDG al ejecutar una opción interna que aún no estaba conectada. El binario arrancó el perfil personal y Qt. Se terminó únicamente esa instancia propia (backend 10857, Qt 10864), sin micrófono, atajos ni llamadas de chat observadas. El arranque leyó configuración y abrió SQLite; también pudo ejecutar recuperación/mantenimiento y regenerar bootstrap privado. **No hay snapshot previo que permita certificar ausencia de cambios en ese perfil.** No se intentó rollback ni se leyó su contenido para reconstruirlo.

Se corrigió el dispatcher, se rechazan otros modos internos desconocidos antes de Backend, y el runner comprueba con XDG privado que esos modos no crean config/DB/UI. Este incidente invalida una afirmación de aislamiento universal de toda la sesión; las ejecuciones automatizadas posteriores documentadas sí usan perfiles separados. Véase F4-ISSUES y F4-RESULTS.

Para aceptar el producto faltan rendimiento y hardware. El objetivo de voz suficientemente rápida en el N5000 permanece abierto; no se sustituye por una declaración de éxito basada en fixtures.
