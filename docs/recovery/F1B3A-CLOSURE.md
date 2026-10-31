# F1B-3A — Cierre de revisión y consolidación local

Fecha: 2026-10-10. Estado **CONSOLIDATED**. Recomendación **READY_FOR_REVIEW**. F1B-3B no iniciado.
Baseline `27f2db9430bcbc7c074472383fc8acbc142b5f89`; rama `recovery/f1b3a-persistence`.

- A `690c5d65b48a2fb19564c842a12677b129822a70` — journal/migración, integración de producción y compatibilidad de teardown de dos fixtures heredados; padre baseline, árbol `0160bbadb6fcf91472c6bea162104a986b110f00`.
- B `d8a6972b5bc3977408c8d0956fce3196623d5458` — tests de crash/errores/regresión, hooks cfg(test), script y tres informes; padre A, árbol `5f724bb939fc0e95d4fe197b2e519b8f5e02b6fd`.

Manifiestos exactos, responsabilidades de los 10 + 7 archivos, comandos y resultados por árbol: [F1B3A-COMMIT-VALIDATION](F1B3A-COMMIT-VALIDATION.md). En total 15 archivos cambian. Ningún commit previo se reescribió; sólo commits locales, sin push/merge/release.

## Revisión del código e invariantes

Se examinaron los diffs completos y las rutas relevantes de los ocho archivos pedidos: operation_journal, migración 003, SessionManager, Turn/Registry, AiService, ToolExecutor, HTTP y Backend. Se rastrearon constructores y callers, transacciones y límites de locks; no se sustituyó la revisión por los informes iniciales.

| Área | Resultado verificado |
| --- | --- |
| Cobertura productiva | Backend::new construye with_audit y comparte ese executor con HTTP/voz. No hay caller productivo de ToolExecutor::new: sólo fixtures y smoke opt-in. execute_for_turn y execute heredado confirman Operation ID/intención y despacho antes de sondear el efecto; denegaciones LLM/manuales se registran sin efectos. La librería conserva new sin DB para fixtures: no se afirma que consumidores Rust alternativos tengan este contrato automáticamente. |
| Persistencia obligatoria | Fallos de intención/despacho impiden ejecutar; fallo de resultado/audit no devuelve tool_result ni permite otro request LLM o done. Si settle también falla, queda evidencia pendiente, sin éxito ficticio; recuperación posterior debe confirmar sus cambios o Backend::new falla. |
| Locks | Registry -> Turn; despacho/publicación Turn -> DB. Preparación/autorización/resultado liberan DB antes de adquirir Turn. No ciclo DB -> Turn/Registry ni await de red, aprobación, LLM o executor bajo DB/transacción. flock es no bloqueante, adquirido al abrir y conservado hasta drop. |
| Contención | busy_timeout 5000 ms probado con writer aislado: falla sin efecto y se reutiliza la conexión. SQLite síncrono puede bloquear workers y retrasar cancelación antes de su timeout async; no hay cota general HTTP de 2 s ni límite para fsync/hardware. No se reescribe la arquitectura de DB. |
| Transacciones | Estado/evento/audit final juntos; COMMIT fallido y rollback automático se manejan; cleanup conserva causas y unwind intenta rollback. Apretar un error de disco no permite afirmar resultado confirmado. La prueba de cuota devuelve SQLITE_FULL real; FK diferida falla en COMMIT. |
| Esquema | Migración aditiva/idempotente, cinco tablas, índices y cuatro triggers activos. Sonda SQL privada adicional rechazó dueño inválido, transición inválida, cambio de identidad, evento huérfano y consentimiento inválido (5 PASS). FK global legacy permanece desactivada; los enlaces nuevos se validan por triggers/APIs. |
| Recovery | Prepared/Authorized -> Abandoned; Dispatched -> UnknownOutcome; terminales conocidos intactos; snapshot persistido y turnos abandonados Failed. Rollback del recovery fallido conserva el estado; storage inutilizable impide iniciar servicio. No replay ni respuesta inventada. Registry nuevo libera sesiones. |
| Privacidad/API | Nuevas auditorías [redacted], metadatos allowlist sin args/destinos/resultados/keys/prompts. Sesiones arbitrarias inexistentes se rechazan al iniciar intención. GET requiere middleware bearer/host/origin, igualdad SQL parametrizada y límite 1..500. Consultas de sesión inexistente/inyección devuelven vacío. Bearer autoriza toda la aplicación; no ACL por sesión. |
| Retry | turn_inputs bloquea todos los intentos despachados, incluso antiguos, inciertos, fallidos/cancelados y usuario legacy NULL turn_id, tras tres reinicios. Última fila user se vuelve a comprobar en transacción. Solicitud nueva recibe IDs nuevos y no altera outcomes anteriores. Efectos históricos pre-journal no se reconstruyen. |
| Consentimiento | ToolCtx.decided sigue siendo declaración del caller, nunca capacidad fiable. Se conservan flags/SafePath y SEC-01/02; permisos centrales vinculados al efecto quedan en F2. |

## Defectos y pruebas antes/después

| Defecto real | Antes | Corrección mínima | Después |
| --- | --- | --- | --- |
| Retry aceptaba trailing tool tras cambiar el historial | Regresión retry_rechecks_unanswered_input_inside_transaction: FAIL, exit 101 | Validar última fila/role dentro de BEGIN IMMEDIATE y enlazar su ID exacto | PASS; cero turno insertado al rechazar |
| Manual aceptaba sesión inexistente/persistía metadato arbitrario | nonexistent_session_cannot_dispatch_or_persist_untrusted_metadata: FAIL, 101 | Exigir sesión existente dentro de begin_turn | PASS; cero journal/efecto |
| Manual rojo devolvía 403 sin journal | http_manual_denial_is_persisted_and_write_failure_is_closed: FAIL, 101, cero filas | Confirmar Denied antes de 403; devolver 500 si falla su registro | PASS; cero Dispatched/efecto aun con trigger fallido |
| Qt retenía backend al lanzar otra instancia | qml_offscreen: FAIL, 101 después del primer harness PASS | Drop del backend y esperar último owner DB; mantener flock | PASS, incluida recogida del ejecutable/QML propios |

Logs anteriores en `/tmp/kda-f1b3a-review-rz85yxmf/` y `/tmp/kda-f1b3a-check-ni5kftf8/`. Gates del workspace final `/tmp/kda-f1b3a-check-qtjt5337/` y de B staged `/tmp/kda-f1b3a-check-9s246nz9/`: once PASS cada uno, sin fallos finales. A staged `/tmp/kda-f1b2-check-veoc4tl8/`: once PASS sobre su padre real. Los dos fixtures de reinicio corregidos durante implementación también se conservan, sin relajar exclusión de instancia.

## Resultados finales y crash matrix

Formato/check/clippy: PASS, exit 0. B aislado: F1A 19 PASS/1 IGNORED; F1B1 13/1; F1B2 20/3; F1B3A 26/1; unitarios 156/1/2 FILTERED; app_paths 2/0. Qt opt-in: F1B1 1 PASS/13 FILTERED y F1B2 3 PASS/20 FILTERED, sin IGNORED ni FAIL en opt-ins. Los workers ignorados se ejecutan desde sus padres; los montajes y dos tests de red/descarga permanecen NOT_TESTED. No se suman de nuevo los runners hijos ni reruns Piper.

| Interrupción | Recuperación comprobada | Estado |
| --- | --- | --- |
| Antes de intención | Sin operación/efecto; turno Failed | PASS |
| Prepared/Authorized antes de dispatch | Abandoned; cero efecto | PASS |
| Dispatched antes de resultado | UnknownOutcome con una escritura privada, sin replay | PASS |
| Marcador confirmado antes del efecto | UnknownOutcome también con cero escritura | PASS |
| Resultado antes del terminal del turno | Succeeded/Failed intacto; turno Failed; no respuesta final inventada | PASS |
| Transacción SQLite/intención sin COMMIT | Rollback de journal/audit/eventos/mensaje no confirmado | PASS |
| Cancelación | Efecto previo conservado; UnknownOutcome al crash, o Succeeded si resultado fue registrado antes | PASS |
| Escritura de auditoría | Audit final sin COMMIT revertido; UnknownOutcome | PASS |
| Migración real | DDL sin COMMIT revertido; siguiente arranque instala una vez | PASS |
| Recuperación real | Cambios sin COMMIT revertidos; siguiente recovery idempotente | PASS |

14 procesos hijos terminados con SIGKILL en B (10 integración + 4 hooks unitarios), cada uno creado por su test, esperado hasta checkpoint, terminado por Child::kill y recogido con wait/señal 9. Tres reaperturas por escenario: SQLite íntegro, mensajes completados conservados, cero repetición externa y registry disponible. No señales a procesos reales del usuario. Véanse [resultados completos](F1B3A-RESULTS.md) y [contrato](F1B3A-JOURNAL-CONTRACT.md).

## Límites y riesgos restantes

WAL + synchronous FULL comprobados; commit no garantiza todos los dispositivos/VFS ni power loss. No exactly-once ni transacción compartida con filesystem/DBus/red. Failed no significa rollback de efectos parciales; Succeeded conserva la semántica del executor. UnknownOutcome mantiene identidad, herramienta/timeline/audit, sin argumentos/destino exacto: requiere inspección externa y una solicitud nueva explícita para repetir.

La auditoría histórica y mensajes mantienen su política anterior. No se certifica anonimato de nombres/tamaños/timing, ni ACL de sesiones; F2 pendiente. flock sólo protege backends cooperantes; un escritor del mismo UID puede ignorarlo. El borrado legacy de sesiones no purga journal y puede dejar turn_inputs históricos sin mensaje; no se afirma integridad referencial universal después de esos borrados. Retención/poda y su conciliación pertenecen a F1B-3B. Scheduler de recordatorios, audio y coordinación global de voz conservan lifecycle anterior; no se certificaron entregas exactly-once. Hardware/KDE/Plasma, proveedores reales, micrófono/modelos y fallos físicos no probados.

## Parche y Git final

[F1B3A-CHANGES.patch](F1B3A-CHANGES.patch): 135554 bytes, SHA256 `81b7a6da179824ab2b058ef9b21cf6fa21a908ef660aabe291bfe4f95d8579b6`. git apply --check/aplicación sobre baseline limpio: exit 0. Sus 224 archivos y el índice reconstruido coinciden exactamente con el árbol final `5f724bb939fc0e95d4fe197b2e519b8f5e02b6fd`. Excluye los informes posteriores de cierre/validación/parche y los documentos históricos ajenos.

Tracked limpio, staged vacío. Se conservan los 17 documentos históricos sin versionar con hashes iniciales idénticos, más los tres entregables de cierre sin versionar. No reset/clean/stash ni terceros commits; Cargo.toml/Cargo.lock y rama F1B2 sin cambios.

**Puede iniciarse F1B-3B en una tarea separada**, preservando este contrato y abordando retención/contexto según su alcance aprobado. Este trabajo no la inicia ni reconstruye permisos F2.
