# F1B-3A — Journal persistente y recuperación

Fecha: 2026-10-10. Estado: **IMPLEMENTED** para el contrato de operaciones de herramientas descrito en [F1B3A-JOURNAL-CONTRACT](F1B3A-JOURNAL-CONTRACT.md). Recomendación: **READY_FOR_REVIEW**, con los límites de almacenamiento y autoridad indicados abajo. Sin F1B-3B.

## Baseline y preparación

Proyecto `Eirom16/KDE-ASSISTANT-2`, workspace `/home/eirom/KDE-ASSISTANT-2`. HEAD inicial de implementación y revisión: **`27f2db9430bcbc7c074472383fc8acbc142b5f89`**, baseline obligatorio. Rama anterior `recovery/f1b2-conversation-engine`; rama creada **`recovery/f1b3a-persistence`**. El árbol tracked/staged inicial estaba limpio; se conservaron los **17 documentos/parches sin versionar** preexistentes. Se leyeron AGENTS, los tres documentos de referencia y los seis documentos de F1B2/recuperación pedidos. Se inspeccionaron almacenamiento, turnos, HTTP, executor, auditoría y migraciones históricas.

No se ejecutaron reset, clean ni stash. La revisión posterior autoriza sólo dos commits locales; sus SHA/árboles y validaciones se registran en F1B3A-COMMIT-VALIDATION y F1B3A-CLOSURE. No push, merge ni release. Cargo.toml/Cargo.lock permanecen idénticos al baseline. No hubo delegación a otros agentes, cambios de configuración personal ni acceso a proveedores/modelos/micrófono reales.

## Archivos de la entrega

| Archivo | Cambio |
| --- | --- |
| `src/backend/operation_journal.rs` (nuevo) | Máquina persistente, transacciones/rollback, recovery, inputs/retries, metadatos y pruebas de crash dentro de las rutas reales. |
| `migrations/003_operation_journal.sql` (nuevo) | Cinco tablas aditivas, índices, CHECKs y triggers de identidad/transiciones/enlaces; marcador incremental idempotente. |
| `src/backend/session_manager.rs` | Arranque transaccional, apertura privada con flock, WAL/FULL comprobados, busy_timeout, publicación rechazada si hay incertidumbre; fixtures adaptados. |
| `src/backend/conversation_turn.rs` | Despacho registrado bajo el mutex que arbitra cancelación, con drenado y registro del resultado antes de check tardío. |
| `src/backend/ai_service.rs` | Prepared antes de aprobación, denegación obligatoria y uso del executor con operación/turno. Conserva validación batch, parser y deduplicación existentes. |
| `src/backend/tool_executor.rs` | Journal obligatorio en el executor del Backend; resultados/auditoría redaccionada atómicos; invocaciones heredadas reciben turnos propios. Mantiene comprobaciones de configuración/SafePath. |
| `src/backend/http_server.rs` | Usuario/turno/input atómicos; settle de pendientes con terminal; bloqueo de regenerate con efectos en cualquier intento; consulta autenticada de operaciones; assertions del test de drenado. |
| `src/backend/mod.rs` | Exporta el módulo nuevo; conserva wiring de Backend con almacenamiento compartido. |
| `tests/f1b3a_persistence.rs` (nuevo) | 26 casos ordinarios y worker ignorado invocado por los padres: procesos SIGKILL, SQLite, errores, executor real privado y HTTP local. |
| `tests/f1b1_text_startup.rs` | El fixture Qt libera su backend y espera la desaparición real del owner SQLite antes de arrancar el ejecutable. |
| `tests/f1b2_conversation.rs` | El test de reinicio espera la liberación real de los owners HTTP anteriores antes de abrir el backend nuevo. No debilita el flock. |
| `scripts/validate_f1b3a.py` (nuevo) | Gates exactos con XDG/TMPDIR privados, claves/proxies retirados, Cargo offline y logs persistidos; Qt opt-in. |
| `docs/recovery/F1B3A-JOURNAL-CONTRACT.md` | Semántica normativa de persistencia, consentimiento, estados, recovery, reintentos y límites. |
| `docs/recovery/F1B3A-RESULTS.md` | Resultados exactos, logs, matriz de crash y fallos intermedios. |
| `docs/recovery/F1B3A-REPORT.md` | Este informe. |

## Esquema y decisiones

Tablas nuevas: `operation_journal`, `operation_events`, `turn_recovery`, `turn_inputs`, `recovery_migrations`. La nueva migración está dentro de la transacción de inicialización junto con los ALTER históricos; no toca el esquema de mensajes de F1B-2 ni borra sesiones. `operation_events` permite distinguir los tiempos de preparación, consentimiento, despacho y resultado. El journal enlaza cada operación a su turno/sesión y a una fila de auditoría; el resultado guarda estado y tamaño, la auditoría guarda duración. No se guardan argumentos, destinos, errores completos, documentos ni claves en los registros nuevos.

Estados: **Prepared -> Authorized -> Dispatched -> Succeeded/Failed/UnknownOutcome**; Prepared -> Denied; Prepared/Authorized -> Abandoned. Terminales inmutables. Authorized registra consentimiento del caller/política existente, no un permiso definitivo de F2. Dispatched significa que el executor pudo empezar, no prueba el efecto. Failed no prueba rollback. Succeeded conserva la semántica de éxito del executor, no certifica la finalización de procesos externos.

Se mantiene el punto de publicación F1B-2: sólo Turn.complete ganador publica los mensajes finales y Completed juntos. La preparación inicial ahora añade usuario/turno/input en una transacción; los resultados intermedios del journal sobreviven incluso si no se publica una respuesta completa. Los resultados tardíos se guardan en la operación original y siguen suprimidos al cancelar; no se reasignan a un turno nuevo.

Al abrir: flock exclusivo de instancia, migración, recovery atómico y después servicio. Prepared/Authorized -> Abandoned; Dispatched -> UnknownOutcome; turnos pendientes -> Failed con snapshot del estado SQLite previo. Los resultados terminales y mensajes ya publicados permanecen. No hay llamadas externas en recovery ni reapertura de permisos. Registry nuevo vacío evita bloqueos perpetuos. Un recovery fallido impide servir y conserva los pendientes para otro inicio.

Regenerate vincula todos sus intentos al mismo usuario: si alguno pudo ejecutar una operación, requiere solicitud nueva después de inspección manual. No se implementa replay de Operation ID. Este enlace corrige también el caso de un intento regenerado fallido cuyo usuario seguía perteneciendo al turno original.

## Verificación y garantías

Ejecución tras revisión: `python3 scripts/validate_f1b3a.py --qml`, exit 0, once gates; logs y resultados en `/tmp/kda-f1b3a-check-qtjt5337/`. Incluye los comandos Qt opt-in solicitados. Comandos, conteos exactos, exclusiones y matriz en [F1B3A-RESULTS](F1B3A-RESULTS.md). Los SIGKILL sólo apuntan a hijos propios, con checkpoint y wait; no a procesos del escritorio. Las operaciones externas de pruebas se limitan a archivos sintéticos privados o mocks loopback.

Se verifican conservación de mensajes, integrity_check/foreign_key_check, no duplicación de efectos, ausencia de replay durante tres reinicios consecutivos, transiciones honestas, liberación de sesiones y no éxito ante fallos de escritura. Se prueban RAISE(ABORT/ROLLBACK), busy_timeout real, cuota de páginas que devuelve SQLITE_FULL, fallo diferido de COMMIT, unwind, escritura inicial/final de auditoría y recuperación fallida. Cuatro cortes cfg(test) detienen las rutas reales durante migración, commit de intención, auditoría y recuperación; diez escenarios adicionales detienen procesos hijos entre fases o dentro de transacciones físicas.

El test de drenado F1B-2 conserva 202/409 mientras ejecuta, observa Dispatched sin transacción abierta y después conserva Succeeded aunque el turno sea Cancelled. No hay segunda herramienta ni respuesta final. Las regresiones originales mantienen SEC-01/02, UUID backend, secuencias, estados terminales, SSE incremental, validación de herramientas y ausencia de reintentos automáticos.

## Defectos reproducidos en la revisión final

1. `begin_retry_turn` aceptaba un historial que dejó de terminar en user después del precheck HTTP. Test `retry_rechecks_unanswered_input_inside_transaction`: FAIL (exit 101) antes; PASS después. Se comprueba la última fila y su role dentro de BEGIN IMMEDIATE y se usa ese ID exacto; el rechazo no inserta turno.
2. `begin_turn` aceptaba un session_id inexistente desde ToolCtx manual y lo persistía, ejecutando un archivo privado. Test `nonexistent_session_cannot_dispatch_or_persist_untrusted_metadata`: FAIL (101) antes; PASS después. Se exige existencia de sesión dentro de la transacción inicial; cero efecto/metadatos en rechazo. No se cambia la autoridad de consentimiento.
3. La ruta manual roja devolvía 403 sin journal. `http_manual_denial_is_persisted_and_write_failure_is_closed`: FAIL (101) antes; PASS después. Se confirma Denied/audit/evento antes de 403; fallo de persistencia devuelve 500 y cero efectos.
4. Qt F1B1 retenía el Backend del fixture al lanzar otra instancia contra la misma DB: primer opt-in FAIL (101), UI inicial PASS, ejecutable no arrancaba por la exclusión de instancia. Se libera el backend y se espera su último owner; opt-in completo PASS. El flock no se debilita.

Evidencia anterior en `/tmp/kda-f1b3a-review-rz85yxmf/` y `/tmp/kda-f1b3a-check-ni5kftf8/`. El gate final repite SEC-01/02, F1B-2 y Qt. Se añadió cobertura de usuario legacy sin Turn ID, intentos inciertos cancelados y tres reinicios; GET autenticado con sesión inexistente/inyección permanece vacío. No se halló un ciclo registry/turn/DB ni await externo bajo DB. La contención síncrona puede retrasar workers/cancelación; el contrato precisa esa limitación sin prometer latencia general de 2 s.

## Riesgos y límites pendientes

- **Ventana SQLite/efecto externo:** no hay exactly-once ni atomicidad compartida. UnknownOutcome puede corresponder a cero, uno o un efecto parcial. Repetir explícitamente puede duplicarlo.
- **Almacenamiento físico:** WAL/FULL depende de VFS, OS y dispositivo. SIGKILL no certifica power loss, controladoras ni todos los SQLITE_IOERR. Un disco que impida rollback puede impedir también recuperación; el backend falla cerrado.
- **Autoridad:** ToolCtx sigue siendo declarativo; F2 debe construir permisos centrales y consentimiento vinculado al efecto. No se amplían flags, privilegios o herramientas.
- **Operaciones heredadas:** herramientas manuales/de voz usan turnos de invocación propios, sin cambiar la supervisión global o el pipeline de voz. Audio/auto-speak y entregas del scheduler no reciben este lifecycle. No se probaron servicios KDE/hardware reales.
- **Visibilidad/retención:** se añade API de metadatos y timeline SQLite, no una pantalla de recuperación. API acotada a las 500 operaciones recientes; inspección más antigua requiere SQLite. No se podan operaciones ni se reconstruyen parciales. F1B-3B no comenzó.
- **Concurrencia:** flock protege backends cooperantes, no manipulación de la base por otro proceso del mismo UID. SQLite sigue síncrono dentro de workers Tokio, con busy_timeout de 5 s por contención, que puede retrasar cancelación y no limita hardware/mutex; no existe un actor dedicado de DB nuevo.
- **Privacidad:** nuevas auditorías redaccionadas; filas históricas y mensajes mantienen la política anterior. Metadatos de herramienta/tamaño/timing también pueden ser sensibles.
- **Validación externa:** proveedores remotos, voz, micrófono, modelos, power loss, Plasma y montajes opt-in quedan explícitamente no ejecutados. Qt offscreen F1B1 (1) y F1B2 (3) sí pasan; no certifican Plasma.

Con esos límites, **READY_FOR_REVIEW** del primer bloque F1B-3. El criterio principal se cumple: la recuperación conserva lo conocido y representa como incierto lo que no puede demostrar, sin fabricar éxito ni repetir operaciones.
