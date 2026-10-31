# F4-R3 — Incidencias, límites y gates

**PARTIAL / LIVE_TTS NOT_TESTED / NEEDS_FIXES**. Fecha: 2026-10-19.

| ID | Estado | Evidencia y límite |
|---|---|---|
| R3-AUTH | Descarga/instalación offline autorizadas | «Lo autorizo», recibo ligado al hash del inventario. Permiso audible T1 pendiente; nunca implica micrófono |
| R3-ENTRY | Corregido, real offline PASS | Entry point del wheel es piper.__main__:main. Adaptador usa API de ese paquete en intérprete explícito/verificado, sin PATH ni nombre genérico piper |
| R3-SPEAKER | Integrado, UNIT PASS; PCM real con speaker 0 | Hablantes 0/1 válidos, QML persiste selección. Síntesis real de speaker 1 y preferencia/inteligibilidad humana pendientes |
| R3-CAPABILITY | Corregido; gates finales PASS | Disponibilidad de recursos explícitos, lectura manual deshabilitada si faltan. piper_initialized describe carga en Backend, que no conserva modelo; no confundir Preparing STT con Piper cargado |
| R3-CPU | Real offline PASS | Piper 1.8.0/ORT 1.31.0 CPU funcionan en N5000. Sin CUDA/AVX supuesto; Python 3.14/x86_64 evaluados, otras combinaciones NOT_SUPPORTED |
| R3-PERF | Deuda abierta | Medición final: carga ONNX ≈4,7–4,9 s, síntesis corta ≈1,1 s; preflight+worker emparejados ≈8,16–9,01 s antes de HTTP/salida. No hay presupuesto de aceptación inventado. BASIC/DISABLED no ayudan; prepacking varía poco con n=3. Inicio audible/inteligibilidad pendientes |
| R3-CANCEL | Automatizado real, aceptación humana pendiente | Kill+wait de procesos propios, sesión y reproducción a null sink PASS. Primera cancelación a 1,5 s no acredita que terminara la carga; no se etiqueta como inferencia probada. Medición directa post-carga final: n=3, kill+wait p50 32 ms, todos recogidos |
| R3-LICENSE | Documentado, distribución pendiente | Motor GPL-3.0-or-later, no Apache-2.0. Ficha de voz: corpus CC BY 3.0/derivación lessac; repo MIT. Preservar avisos y resolver condiciones específicas antes de distribuir; no se inicia F6 |
| R3-INTEGRITY | Implementado | Pin de voz/config y 1799 recursos de wheels aprobados; `.pth`/sitecustomize/bytecode cache evitados. No se presenta como sandbox frente al mismo UID o verificación atómica de todos los pathnames |
| R3-PCM | Real offline PASS | PCM16LE mono 22.050 Hz; límites stdin 16.000 B/stdout 16 MiB, timeouts/cancelación conservados. Peor caso de memoria de una sola frase de 16.000 B no medido; el límite de salida no equivale a un RLIMIT del heap ONNX |
| R3-HUMAN | NOT_TESTED | T1–T5 por separado; cada una exige permiso y respuesta humana. No LIVE_TTS_PASS por generar PCM o consumirlo en null sink |
| R3-PLATFORM | Limitación técnica | Adaptador pinneado al venv CPython 3.14 y voz sharvard evaluados; no instala automáticamente motores/voces ni acepta distribuciones arbitrarias |
| R3-CLEANUP | Evaluación abierta | PCM instrumentales eliminados. Paquetes/voz/venv/perfiles temporales conservados hasta cerrar evaluación; después se eliminan sin prometer borrado forense |

Conservar chat escrito y PTT R2, sin nuevas capturas. Sin cambios F2, herramientas restringidas, consentimiento remoto, wake word, atajos globales, perfil personal o commits. No se declara F4 completa.

## Apéndice T1 — Gate de escucha

Preparación vigente y recursos verificados. La autorización previa cubre instalación/síntesis offline; la reproducción audible requiere una confirmación nueva. **T1 NOT_TESTED** hasta concederla y realizar la toma. No se declarará LIVE_TTS_PASS sin finalización técnica correcta y confirmación humana de escucha; problemas audibles se clasificarán PARTIAL o FAILED según evidencia.

Quedan pendientes, en orden: permiso explícito, comprobación no invasiva de salida, ajuste de volumen por el usuario, una reproducción de la frase prevista y respuesta humana sobre escucha completa/claridad/naturalidad/volumen. Sin T2–T5, wake o F5; sin commits.

### T1 — Autorización, salida y ejecución técnica (valoración humana pendiente)

El usuario autorizó explícitamente «si, lo autorizo» y después confirmó «Volumen cómodo; reproducir ahora». Se consultaron únicamente la salida predeterminada y su volumen con wpctl inspect/get-volume, sin modificarlos: Audio Interno Estéreo analógico / ALC257 Analog. Antes del ajuste manual del usuario figuraba Volume: 1.15; no se impuso ni cambió volumen, mute, ruta o dispositivo. [Autorización](F4-R3-EVIDENCE/T1/audible-authorization.json), [salida](F4-R3-EVIDENCE/T1/output-check.json), [preparación física confirmada](F4-R3-EVIDENCE/T1/readiness.json).

Una única ejecución del escenario preparado: código de salida 0, sin error técnico observado; Preparing 79,4 ms → Speaking 8038,4 ms → Idle 11883,7 ms desde el reloj del launcher. Pared del comando completo 12,241 s. La ventana observada de Speaking fue ≈3,845 s; no es una medición del instante acústico de primer/último audio. No se calculan p50/p95 de esta observación.

[Procesos y ejecución](F4-R3-EVIDENCE/T1/technical-run.json), [estados de reproducción](F4-R3-EVIDENCE/T1/playback-observation.json). Se observaron Backend 11837, preflight 11845, worker 11846 (exec Rust→Python 3.14, mismo PID) y reproductor 11855 (pw-play resuelve a pw-cat). Todos recogidos. El backend volvió a idle, la sesión sigue sin mensajes y no hubo archivos PCM/WAV, micrófono, descarga o proveedor remoto. El recibo de una sola toma quedó consumido; no se repitió la reproducción ni se inició T2–T5.

Finalización técnica correcta; **clasificación LIVE_TTS pendiente de respuesta humana** sobre escucha completa, claridad, naturalidad, volumen y posibles defectos. No se declara LIVE_TTS_PASS por el estado idle o el exit 0.

### T1 — Resultado humano y problema abierto de latencia

- **T1 LIVE_TTS_PASS**, con finalización técnica correcta y confirmación humana de reproducción perfecta, sin cortes/distorsión. [Evidencia](F4-R3-EVIDENCE/T1/human-confirmation.json).
- **R3-PERF continúa ABIERTO**: el usuario identifica la espera de ocho segundos como único problema. No se acepta esa espera como resuelta ni se amplían timeouts para ocultarla. Optimización posterior sujeta a la siguiente revisión; no se ejecutan nuevos benchmarks en T1.
- R3-HUMAN queda resuelto sólo para escuchar la frase de T1; T2 Detener, T3 nueva reproducción, T4 cambio de sesión y T5 chat mock con aceptación humana permanecen NOT_TESTED.
- Estado global **PARTIAL / F4 NEEDS_FIXES**. Instalación/perfil privados conservados temporalmente para revisión, sin actividad vocal ni audio crudo. Sin micrófono, nuevas descargas, cambios KDE/personal, commits o fases adicionales. Se espera revisión.
