# F4-R5 — Voz rápida opcional

Implementación: **IMPLEMENTED**. Rendimiento: **IMPROVED**. Seguridad y lifecycle: **PASS en el alcance automatizado**. Aceptación humana R5: **NOT_TESTED**. F4 global: **PARTIAL**, pendiente de revisión.

Rama `recovery/f4-voice-coordinator`; HEAD F3.5 `9544a58eba4b6adf6c6343c04db80813b084f76b`, un worktree e índice vacío. Se conservan las modificaciones anteriores y los veinte informes F4/R/R2/R3/R4 inventariados por SHA-256 en [baseline](F4-R5-EVIDENCE/baseline.json). No commits, push, merge, release, F5/F6 ni wake word.

## Arquitectura integrada

`VoiceCoordinator` conserva la autoridad única. Posee un `WarmPiperManager` perezoso, que serializa preparación, reserva, validación, síntesis y evicción. Sólo ese actor posee el proceso Piper preparado. El proceso no recibe sesión, turno, bearer, proveedor, herramientas ni socket de escritorio: únicamente control de inferencia versionado, identidades opacas y texto final autorizado. Rust conserva la correspondencia con sesión/turno/scope.

Modo OFF conserva la ruta R4 desechable: validación completa, carga, PCM completo, `pw-play`, kill/wait y liberación. Modo ON permite reutilizar una sesión ONNX verificada y preparar durante un turno LLM con autolectura. Activar el switch, iniciar Backend, iniciar sesión o abrir ventana no crea el actor ni carga el modelo. Sin autolectura, una acción manual Leer permite carga bajo demanda.

No se envían tokens parciales a Piper. El backend publica el turno completado y luego admite su texto final para lectura. No hay prompts adicionales. Si el LLM termina antes de Ready, la voz espera asíncronamente; la conversación escrita ya puede completarse. Si cambia sesión/configuración o se cancela el turno, se revoca la preparación y ninguna respuesta antigua puede reproducirse.

Se reutiliza de R4 la API real `PiperVoice.synthesize`, las comprobaciones SHA-256, ownership, permisos, identidad de inodos y descriptores fijados, las opciones acústicas/CPU y la reproducción bufferizada. El experimento R4 de dos solicitudes/TTL 3 s permanece separado. El nuevo protocolo `managed-v1` no acepta workers experimentales ni stream progresivo. La entrega por frases descartada en R4 no se conectó a producción.

## Retención y consumo

Propuesta inicial: TTL de inactividad 30 s, validado 15..60, sin control adicional en QML. Empieza en Ready y se reinicia tras síntesis exitosa, no tras reproducción ni consulta de estado. La vida máxima es 2×TTL desde Ready; una solicitud activa conserva su deadline acotado. Se recoge el worker al expirar, apagar modo, cambiar contexto, cerrar aplicación o perder integridad/salud. El reproductor de PCM completo válido puede continuar después de una expiración normal del modelo; una revocación efectiva cancela la operación.

La política usa una envolvente R4 medida de 256 MiB: no prepara con menos de tres envolventes disponibles; PSI positivo actúa como alerta temprana dentro de dos márgenes de admisión. El monitor a 1 Hz revoca ante presión o RSS superior a dos envolventes. El criterio inicial «cualquier PSI positivo» se revisó tras capturar un fallback con PSI 0,17% y MemAvailable 4.997.476 KiB: un historial residual con amplia capacidad impedía preparar. Se conserva esa evidencia, sin forzar presión ni cambiar el sistema. Los factores son decisiones conservadoras de admisión/evicción derivadas de la medición, no límites duros del heap ni garantía frente a OOM/cgroups restringidos.

Iniciar PTT drena Piper antes de STT; mientras capture/transcriba no se admite preparación anticipada. No se mantiene una sesión por conversación ni dos motores ONNX preparados. CPU y memoria reales, TTL candidatos y coste de coexistencia con QML se separan en BENCHMARKS.

## Integridad, cancelación y recuperación

La carga verifica modelo/configuración y los 1799 recursos aprobados. Cada síntesis preparada repite hashes completos antes de usar la sesión. Inotify complementa, nunca sustituye, la verificación: recursos, directorios relevantes, binario, alias de Python y ejecutable canónico. Modelo/config se cargan mediante los descriptores/inodos verificados R4. No hay caché basada sólo en nombre/mtime/tamaño.

Pipes privados con protocolo v1, controles acotados, UUID de generación y solicitud, PCM16LE mono 22050 Hz, frames de hasta 32 KiB, total 16 MiB, contadores y SHA-256 terminal. Sólo Done válido permite reproducción de PCM completo. EOF, error, timeout, identidad antigua, corrupción y exceso invalidan el resultado. El actor recoge hijo, watcher y monitor. Los errores de motor/integridad/IPC no se reejecutan automáticamente por la ruta fría; fallback de política ocurre antes de despachar inferencia.

Se corrigieron dos casos de autoridad temporal: limpiar el turno de preparación pendiente al admitir una lectura nueva impide que una cancelación LLM antigua revoque la lectura posterior; retirar una generación por salud/integridad cancela su token y crea uno nuevo, sin envenenar la siguiente solicitud explícita. La rotación no cancela un token ya renovado por una revocación concurrente. Además, la señal del registry despierta también ante Completed: ese estado queda excluido de la rama de revocación, preservando la autolectura legítima después del cierre del turno.

El entorno mínimo no es un sandbox contra un atacante del mismo UID. Se conservan los límites R3/R4 sobre importaciones por paths y confianza en el intérprete del sistema; no se promete transacción atómica de todo el filesystem, resistencia absoluta al mismo UID ni prohibición de syscalls de red.

## Configuración y código

`SpeechConfig.piper_fast=false` y `piper_fast_ttl_secs=30`; serde migra campos ausentes a OFF/30 y valida límites. F3.5 conserva revisión, escritura privada/atómica, redacción de credenciales y recarga. En Voz aparece «Voz rápida (experimental)», con aviso orientativo de aproximadamente 310 MB y de recursos ausentes. No fuerza autolectura. Un modo activo puede apagarse aunque falten recursos. Theme, foco, teclado y componentes existentes se conservan.

Cambios: `warm_piper.rs`, `piper_protocol.rs`, `piper_resource_watch.rs`, `piper_worker.py`, `voice_tts.rs`, `voice_coordinator.rs`, integración de `mod.rs`/`http_server.rs`/dispatch estricto `main.rs`, `config.rs`, SettingsDialog/VoiceController y pruebas Rust/QML. Runners: `validate_f4_r5.py`, `benchmark_f4r5.py` y grupos adicionales en `validate_f4.py`. Sin nuevas dependencias descargadas.

Contrato completo: [VOICE-FAST-CONTRACT](F4-R5-VOICE-FAST-CONTRACT.md). Evidencia y fallos conservados: [RESULTS](F4-R5-RESULTS.md). Comparación válida y límites: [BENCHMARKS](F4-R5-BENCHMARKS.md). Pendientes: [ISSUES](F4-R5-ISSUES.md).

## REUSE FIRST: procedencia y decisiones

Se registra la política permanente en AGENTS.md. Consulta breve de documentación primaria y del código ya instalado; sin clones, paquetes o descargas adicionales. No se reescriben componentes funcionales por introducir esta política.

| Recurso reutilizado | Procedencia / licencia | Integración y modificaciones |
|---|---|---|
| Piper 1.8.0, API Python | [OHF-Voice/Piper](https://github.com/OHF-Voice/piper1-gpl/blob/main/docs/API_PYTHON.md), GPL-3.0-or-later; distribución privada autorizada R3 | Se reutilizan carga, fonemización y síntesis existentes; el wrapper añade IPC acotado e identidad. Sin modificar el sintetizador ni crear un servidor HTTP. Voz/licencia e inventario exactos se conservan en R3. |
| ONNX Runtime 1.31.0 | Microsoft / MIT, runtime privado R3 | Una única sesión CPU, parámetros previos. Sin implementación propia de inferencia ni nuevos backends. |
| Tokio 1.53.2 | [Tokio Notify](https://docs.rs/tokio/1.53.2/tokio/sync/struct.Notify.html), MIT; versión bloqueada y caché existente | Actor, canales acotados, procesos, timers y notificaciones. Notify sustituye el polling del primitivo de cancelación conservando su API y flag terminal; no se desarrolla un mecanismo de notificación propio. Tests de carreras y múltiples waiters. |
| rustix 1.1.5 | [Bytecode Alliance](https://github.com/bytecodealliance/rustix), Apache-2.0 WITH LLVM-exception OR Apache-2.0 OR MIT | API inotify existente para identidad y revocación, con AsyncFd de Tokio; adaptación Linux pequeña y específica. Sin dependencia de un watcher multiplataforma adicional. |
| sha2 0.10.9 | [RustCrypto](https://github.com/RustCrypto/hashes), MIT OR Apache-2.0 | Hashes completos y terminales; no criptografía propia. |
| PipeWire / pw-play y componentes QML F3.5 | APIs y componentes ya presentes en el proyecto | Reproducción existente de PCM completo, controles Theme/SettingsToggle, guardado y revisión. Sin nuevo reproductor, servidor de audio o sistema de configuración. |

La lógica nueva se limita a la autoridad y política específicas del producto: sesión/turno/generación, consentimiento, reserva validada, TTL y revocación. Se consultó también el [servidor HTTP oficial Piper](https://github.com/OHF-Voice/piper1-gpl/blob/main/docs/API_HTTP.md): no aporta esas garantías y ampliaría IPC/superficie de acceso. Se reutiliza su motor mediante la API Python, conservando el supervisor Rust probado y adaptando el experimento R4. Un pool genérico tampoco sustituye la autoridad de sesión y permisos. La compatibilidad y coste se evalúan en los benchmarks reales del N5000, no por popularidad. Las dependencias GPL/avisos de voz requieren revisión de distribución antes de F6; esta entrega no autoriza empaquetado.

Piper 1.8.0 / ONNX Runtime 1.31.0 / CPython 3.14, voz es_ES-sharvard-medium, hablante 0 y escala 1 en benchmarks. Licencias y avisos R3 se conservan; no se certifica empaquetado F6. T1 R3 conserva exclusivamente su PASS histórico. R5 audible y T2–T5 humanos NOT_TESTED; sin micrófono, altavoces, servicios IA reales, shortcuts o modificaciones de F2. Las 19 herramientas siguen bloqueadas.

## Cierre para revisión

La comparación integrada alternada (n=5 por caso) registra Speaking en null sink p50 7,280 s R4 frío, 7,850 s primera lectura R5, 1,696 s Ready y 1,677 s consecutiva. Preparación durante LLM mock: final → Speaking p50 1,695 s, frente a 6,979 s con preparación apagada (series mock agrupadas, dispersión conservada). No son tiempos acústicos.

Modo OFF y migración OFF preservados. TTL inicial propuesto 30 s (15/30/60 evaluados), sin residencia permanente; máximo RSS retenido observado 297 MiB en la serie corta. Todos los gates requeridos cerraron PASS tras corregir el lint del watcher y acortar perfiles de los arneses; intentos fallidos preservados. Se eliminaron 68 perfiles sintéticos y sus copias de recursos/configuraciones; quedan evidencia escalar/capturas propias y baseline para revisión. La instalación Piper anterior permanece íntegra y no fue instalada nuevamente. Los veinte informes anteriores conservan sus hashes.

No commits, push, merge, release ni fases nuevas. No micrófono, sonido físico o APIs reales. Se espera revisión técnica antes de cualquier prueba audible o consolidación.
