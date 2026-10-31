# Plan de recuperación propuesto

Fecha 2026-10-10. Baseline `main@12da960ed4722401f1cb42b7a96d9cef912cf90a` con cambios locales previos conservados. Estado de Fase 0: **auditoría PARTIAL** por validaciones fallidas y flujos hardware/externos pendientes. **Todas las fases de implementación siguen pendientes; ninguna se inició ni se marca terminada.**

Diagnóstico y evidencias por ID: [REPORT.md](REPORT.md). Pruebas: [RESULTS.md](RESULTS.md). Diseño recomendado: [ARCHITECTURE.md](ARCHITECTURE.md). Se alinea el orden con la épica [#1](https://github.com/Eirom16/KDE-ASSISTANT-2/issues/1): estabilizar texto antes de tools/escritorio/voz y distribución. No se modificó el seguimiento remoto.

## Reglas de ejecución futura

- Conservar cambios del usuario; acordar cómo integrar su corrección de hotkey_listener antes de editar ese archivo.
- Mantener datos y migraciones compatibles; fixtures separados de config/SQLite/archivos reales.
- No usar tests de carga como prueba funcional ni registrar skipped como validación de proveedor/audio.
- Una fase sólo se acepta con comandos, logs redactados, revisión del diff y pruebas de sus condiciones de salida; los criterios siguientes son objetivos propuestos, no resultados actuales.
- No publicar una versión con herramientas si queda SEC-01/02 o un bypass equivalente. La ruta anterior insegura debe quedar inaccesible al migrar.

## F1 — Base reproducible y producto de texto confiable

Prioridad **P0/P1**. Depende de revisar esta auditoría y definir alcance mínimo. Responsabilidad: mantenedor de build/aplicación. No necesita voz ni integración externa para las primeras pruebas.

Trabajo:

1. Incorporar, preservando autoría y contexto, correcciones del commit que no compila y formato pendiente (REL-01).
2. Versionar lock de aplicación, fijar toolchain/MSRV y dependencias nativas documentadas; CI offline para tests puros. Marcar explícitamente tests de red/modelos/hardware (MAINT-02).
3. Inicialización de texto independiente de Piper, micro, modelos y wake (REL-02). No descargar modelos al arrancar sin selección/consentimiento. Error/ausencia de voz debe ser estado de capacidad, no fallo global.
4. Parser SSE incremental con bytes UTF-8, validación de fin/error/tool args (REL-03). Adaptadores OpenRouter/Groq/OpenAI/custom, credenciales opcionales para servidores locales según contrato.
5. Identidad de turno y scope de tareas; cancelación durante request/desconexión; persistencia transaccional de usuario, assistant, tools y estado cancelado/parcial (REL-04/05). Sesiones y configuración atómica con errores visibles (SEC-07).
6. Cerrar de inmediato fallback permisivo de config y rutas de escape antes de cualquier prueba con tools reales (SEC-01/02). Puede hacerse dentro de F1 como requisito de contención, sin abrir F2.

Condiciones de finalización:

- Checkout limpio del commit aceptado pasa `cargo fmt --check`, `cargo check --all-targets`, `cargo test --all-targets`, `cargo clippy --all-targets -- -D warnings` con dependencias conocidas; todos los skips opt-in identificados. CI conserva resultados del commit exacto.
- Ejecutable inicia UI/API y permite dos turnos de texto con Piper/modelos ausentes, red sin acceso a descargas y configuración de voz desactivada. No crea procesos de voz ni conexiones de descarga.
- Mocks comprueban UTF-8 fragmentado en cada límite de byte, EOF/error, tools fragmentadas y fin de turno; Unicode persiste sin corrupción.
- Cancelar o cerrar cliente detiene los workers propios del turno; ningún evento final antiguo se aplica al nuevo turno. Un servidor mock observa cierre de conexión; no sólo retorno 200 del endpoint.
- CRUD de sesiones, reinicio, config inválida/fallo de escritura y error SQLite tienen resultados explícitos; rollback conserva estado previo y sus errores llegan a UI.
- Sesión/prompt de un turno son inmutables durante su ejecución; historia de herramientas no queda huérfana tras límite, error o cancelación.
- Repro SEC-01 ya no ejecuta tools deshabilitadas bajo lock; fixtures SEC-02 no cambian ningún archivo fuera de la zona autorizada.

## F2 — Límite de autorización y herramientas seguras

Prioridad **P0/P1**. Depende de F1 y de decisión sobre política manual/voz. Responsabilidad: seguridad + servicios de herramientas.

Trabajo:

1. Política central y executor que exige capacidad válida. Catálogo por turno/origen; herramienta ausente nunca se ejecuta. El flag global y flags individuales tienen semántica inequívoca (SEC-03/08).
2. Consentimiento vinculado a argumentos, paths resueltos, efecto, sesión y turno, no ID arbitrario del modelo. Registro de aprobación antes de evento; no colisiones entre sesiones.
3. Filesystem relativo a directorio autorizado, no symlink/TOCTOU, creación exclusiva; backup y destino derivado incluidos en decisión. Append no pierde datos ante error/competencia (SEC-02).
4. IPC falla cerrado sin token; bootstrap y assets privados, GET config redactado, comparación de autoridad exacta (SEC-04/05/06). Definir alcance frente a otros usuarios y procesos del mismo UID.
5. Supervisión de hijos, límites de bytes/tiempo, red y redirects según política, cancelación, kill+wait, auditoría redaccionada (SEC-08/09). Retirar limpieza por nombre de procesos ajenos.
6. Reclasificar efectos destructivos: overwrite/close/SMS/share, incluso en voz. Un clic manual puede expresar intención, pero el origen debe ser verificable y separado del agente.

Condiciones de finalización:

- Matriz green/yellow/red × texto/voz/manual × flags global/individual × aprobación/denegación/timeout/cancelación pasa con executor, HTTP y agente simulado.
- Reproducciones SEC-01 a SEC-07 devuelven rechazo o resultado seguro; tests no modifican targets exteriores ni permiten bearer vacío.
- Carrera de reemplazo de symlink y destinos `.bak`/hijo de directorio no escapan en múltiples intentos; la implementación revisada fija el objeto por descriptor, no sólo repite canonicalize.
- Dos sesiones con mismo ID propuesto por modelo no comparten consentimiento; cambio de argumentos después de aprobar se rechaza.
- Mock de prompt injection no puede ampliar catálogo/permisos ni omitir confirmación; esto acredita el límite mecánico, no inmunidad lingüística universal.
- Cliente sin token recibe 401; origen/host preparado se rechaza; secretos no aparecen en GET config, assets de lectura amplia, stdout/stderr ni auditoría. Prueba de permisos atraviesa directorios con un usuario de prueba cuando sea viable.
- Proceso dummy cancelado/timeout no permanece vivo y se hace wait; URLs/respuestas/documentos sintéticos excediendo límites se detienen durante lectura con memoria acotada.
- Resultado de herramienta y consentimiento auditables incluso si falla el siguiente paso del LLM; fallos de ledger tienen política explícita.

## F3 — KDE/Wayland y ciclo de vida verificables

Prioridad **P1**. Depende de F1 y F2 para probar acciones con efectos. Responsabilidad: integración escritorio.

Trabajo:

1. Instancia única y puerto/socket adquiridos antes de trabajo pesado; errores de servidor y procesos reportados al usuario.
2. Rutas instaladas independientes de CWD; lanzar/supervisar Main y overlay por PID propio. Retirar pkill por patrones (DES-01/SEC-09).
3. Definir atajos nativos Wayland mediante KGlobalAccel/portal o mecanismo probado; rdev sólo X11 si se conserva. Registrar verdaderamente interfaz DBus si se ofrece --send-msg.
4. Tray DBusMenu, cierre a bandeja, salir explícito, SIGINT/SIGTERM y crash de UI/overlay. Handshake de overlay y fallback de avatar.
5. Adaptadores media/volumen/brillo/red/KDEConnect con detección de dependencias; no fingir blur mediante transparencia.

Condiciones de finalización:

- Pruebas en Plasma Wayland y X11/XWayland documentan Qt/compositor/plugin exactos; atajos press/release funcionan con una app nativa enfocada, sin privilegios root ni acceso raw al teclado innecesario.
- Clic derecho del tray importa DBusMenu; no popup fantasma al inicio; cerrar oculta, abrir restaura y salir termina.
- `--send-msg` tiene receptor y entrega verificable o se retira del contrato; instancia duplicada no mata otra aplicación.
- Ejecutar desde un directorio distinto encuentra QML/Octicons/licencias; paquete instalado arranca con el mismo resultado.
- Después de salir/SIGTERM/crash, en menos de 5 s no quedan hijos propios, listener, captura ni tareas; un proceso qml6 de prueba ajeno permanece intacto.
- Caída de overlay se detecta y activa fallback o comunica indisponibilidad; no queda personaje ausente por flag inicial.
- Herramientas KDE soportadas pasan contra sesión real con fixtures/destinos de prueba; ausentes devuelven capacidad no disponible. Acciones destructivas usan apps dummy sin trabajo real.

## F4 — Voz opcional con una sola autoridad

Prioridad **P1**. Depende de supervisor F1, política F2 e integración PTT F3. Responsabilidad: audio/voz. Decisión recomendada: **Rust es coordinador; sidecar Python no forma parte del producto** (VOI-03).

Trabajo:

1. Un VoiceCoordinator con estados válidos, op-id/session-id inmutables, reloj monotónico y cancelación de todos los workers (VOI-01).
2. PTT local y dictado antes de wake; micro seleccionado y formato resampleado; wake desactivado no impide captura PTT (VOI-02).
3. STT Whisper local lazy; motor remoto sólo con elección explícita y explicación de envío. No inferir consentimiento por encontrar Groq key.
4. TTS Piper supervisado y errores propagados; decidir persistente vs por utterance midiendo coste. Cola acotada, reproducción, stop, barge-in y eco con límites.
5. Audio callback sin I/O/inferencia ni locks contenciosos, worker dedicado para inferencia; ONNX ML opcional, sin activación heurística si modelo no disponible (PERF-01).
6. Eliminar autoridad duplicada/ramas sidecar de producción tras conservar historia técnica; sin dos propietarios del micro/contexto/permisos.

Condiciones de finalización:

- Texto continúa operativo sin audio/Piper/ONNX. Ninguna descarga silenciosa; estado de disponibilidad visible.
- PTT release concluye captura, dictado no llama LLM/TTS, modo local no abre conexiones STT remotas. Test de red lo confirma.
- Dataset de grabaciones de prueba consentidas mide WER y tiempos por etapa p50/p95 en hardware declarado; se fija presupuesto antes de aceptar. No se reutilizan números históricos sin medición.
- Casos silencio, ruido, micro desconectado, cambio de dispositivo, tasa 44.1/48/16 kHz, error STT/Piper, cancelación y barge-in terminan en estados válidos y sin respuesta vieja.
- Cambio de sesión durante voz conserva la sesión del turno; tools quedan en historial completo y aplican la misma política que texto.
- Wake real mide falsos positivos/negativos con dataset y umbral documentados; falta ML deshabilita wake sin fallback de energía y conserva PTT.
- Pruebas de audio reales y fixtures offline separadas; una prueba usa un único propietario de captura. No hay sidecar LLM concurrente.

## F5 — UI/avatar y coste de reposo

Prioridad **P1/P2**. Depende de contratos de eventos F1/F4 y overlay F3. Responsabilidad: UI.

Trabajo: extraer clientes y estado de Main/SettingsDialog; conservar sistema Cupertino/Octicons/atribución blobatar; eliminar parsers/estado duplicados y polling que pueda perder eventos; identidad/secuencia de eventos, cancelación visible, estados de error y teclado/accesibilidad. Desacoplar animación de timers de negocio (MAINT-01, DES-01).

Condiciones de finalización:

- Harness de avatar y pruebas de UI contra backend fixture cubren chat, cancelación, config, sesión, tool approval y errores; cada estado visible corresponde a turno vigente.
- QML lint no introduce nuevos warnings; se triagean los 44 actuales y se fija un baseline justificado. Smoke offscreen y prueba visual Plasma separados.
- Sin avatar duplicado al arrancar/cambiar sesión/reconectar; flujo teclado/foco y tray probado en sesión real.
- Se miden app completa en reposo 60 s y arranque frío/caliente con versiones/flags/modelos declarados; se fija presupuesto de CPU/RSS/latencia según hardware. Las cifras offscreen de F0 no son ese presupuesto.
- Memoria de SSE/eventos y grabación tiene límites; repetir turnos/reconexiones no produce crecimiento sostenido ni hijos huérfanos.

## F6 — Distribución y mantenimiento

Prioridad **P2**, salvo build reproducible ya P1. Depende de todas las capacidades incluidas en la versión, no de features diferidas.

Trabajo: instalación/desktop entry/assets, docs actualizadas y matriz soportada; manifiestos/licencias, política de descargas/checksums/runtime dinámico, logs redactados y migraciones/backup; releases construidos y probados por CI. Mantener biblioteca Qt y runtime coherentes.

Condiciones de finalización:

- Checkout limpio + lock produce build/test/package en entorno documentado; smoke de instalación desde CWD ajeno y arranque sin modelos.
- Todos los P0 cerrados con reproducciones de regresión y P1 del alcance aceptado sin BLOCKED ocultos; features diferidas desactivadas y documentadas.
- Desinstalar no borra datos del usuario; actualizar conserva sesiones/config con migración probada y rollback documentado.
- Documentación distingue soportado/experimental/planificado y ofrece pruebas opt-in seguras; licencias/assets de terceros conservados.
- No secretos en artefacto/logs; publicación/commit/push sólo con autorización correspondiente, fuera de esta auditoría.

## Decisiones pendientes del mantenedor

| Decisión | Recomendación | Evidencia/dependencia |
| --- | --- | --- |
| Estrategia A/B/C | B, reconstrucción parcial con entregas incrementales | P0 del límite executor y turnos, componentes reutilizables; ARCHITECTURE. |
| Alcance v0.1 | Texto estable; tools sólo tras F2; voz opt-in y posterior | REL-02, validación actual falla por Piper. |
| Autoridad de voz/Python | Rust único; archivar sidecar experimental | VOI-01/03, no lifecycle Rust activo y duplicación Python. |
| Semántica de manual y voz | Consentimiento por efecto; no auto universal de amarillas destructivas | SEC-03, copy overwrite/close/SMS. |
| Política local/remota STT | Local por defecto, remoto explícito | VOI-02, auto→Groq contradice promesa. |
| IPC y adversario | Mantener contrato REST/SSE inicialmente; evaluar socket Unix/bootstrap privado y scopes | SEC-04/05/06; token no aísla mismo UID. |
| Matriz escritorio | Plasma Wayland objetivo; X11 fallback declarado, versiones fijadas | DES-01, Qt build/runtime difieren. |
| Presupuesto/hardware voz | Definir corpus, WER y p95 antes de decidir TTS persistente | No hay medidas reales de voz en F0. |

No se requiere resolver estas decisiones para leer/revisar los cuatro entregables; sí antes de cambios que dependan de ellas. Este documento propone criterios, no autoriza ejecutar Fase 1.
