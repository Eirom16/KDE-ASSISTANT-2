# F1B-2 — Cierre de revisión y consolidación

Fecha: 2026-10-10. Estado: **CONSOLIDATED**. Revisión real satisfactoria; **READY_FOR_REVIEW** con los límites expresos del contrato. No se inició F1B-3.

## Identidad y cambios

Rama `recovery/f1b2-conversation-engine`. Baseline `600a363ee3a9d64d9ea2adbdbda5448efacc7e8a`; padre `ddb3bce7610c1e770ab1d90c21e1050b3e37c40e`, abuelo `ca82e722935e8076f526a484655f2f95f87b5d4c`. No había commits F1B2 previos al iniciar revisión. La rama anterior permanece en baseline.

- **A — motor:** `74b2ca4f5c80c057f6403c68cef30dd0950d8a4c`; padre baseline; 11 archivos: parser byte incremental, turnos/supervisión/cancel, integración HTTP/approvals, publicación SQLite y QML; adaptaciones mínimas de callers/fixtures heredados para compilar independientemente.
- **B — tests/documentación:** `27f2db9430bcbc7c074472383fc8acbc142b5f89`; padre A; 14 archivos: unitarios nuevos y hooks cfg(test), integración/fixtures HTTP-SSE y QML, script y los tres informes.

Manifiestos exactos, hashes de cada archivo, árbol/padre/SHA y comandos de validación previa a cada commit: [F1B2-COMMIT-VALIDATION](F1B2-COMMIT-VALIDATION.md). 20 archivos únicos del diff real, sin historia ajena. Cada commit se validó desde una copia git archive del árbol staged; A no depende de B sin versionar. Cargo.toml/Cargo.lock permanecen iguales.

## Defectos reproducidos y correcciones acotadas

| Defecto antes de revisión | Cambio mínimo | Regresión y resultado final |
| --- | --- | --- |
| Regenerar elimina assistants locales y luego el backend rechaza | Conserva historial y muestra repetición explícita mediante mensaje nuevo; cero requests regenerate del botón/retry | qml_regenerate_preserves_history_and_explains_restriction PASS |
| Stop marca cancelación efectiva antes de respuesta, ignora 202 | Conserva identidad/SSE/ocupación; muestra Cancelando hasta terminal, acknowledgment no determina Cancelled | qml_cancel_202_remains_cancelling_until_terminal PASS |
| RELEASE fallido deja publicación abierta con respuesta y estado completed dentro de transacción | RELEASE forma parte del resultado y fallo recoge transacción exterior propia | publication_release_failure_rolls_back_and_allows_next_turn PASS |
| SQLite RAISE(ROLLBACK) oculta causa buscando savepoint desaparecido | Detecta autocommit y conserva causa original; error de cleanup agrega ambas causas | publication_statement_rollback_preserves_original_error PASS |
| dispatch permite efectos de un turno terminal y su espera puede quedarse pendiente | check/dispatch rechazan terminales; cancellation despierta ante terminal | terminal_turn_never_dispatches_or_waits_for_cancellation PASS |
| Handle terminado con panic conserva estado Running al consultar | Registry diagnostica supervisor terminado sin terminal, marca Failed y recoge slot sólo al terminar handle | finished_supervisor_without_terminal_is_diagnosed_and_releases_slot PASS |
| Lista de sesiones recibida tarde cambia selección nueva | Guards de generación/selección en lista y creación, junto con historial/eventos; abandonar sesión invalida antes de abortar SSE | qml_obsolete_session_list_cannot_replace_new_selection PASS |

Evidencia previa fallida y posterior aprobada, errores del harness y validaciones intermedias: [F1B2-RESULTS](F1B2-RESULTS.md). Adicionalmente publication_failure_is_terminal_without_done_and_other_session_succeeds verifica el pipeline HTTP completo: Failed con causa original, sin done/assistant y sesión independiente válida. No se implementaron funciones nuevas ni refactorizaciones generales en la revisión.

## Auditoría de las garantías

- **Parser:** buffers de bytes limitados durante recepción; UTF-8 estricto sin lossy; Español/niño/acción/emojis en todos los cortes de byte y fragmentación de un byte. Eventos y múltiples tools intercalados se reconstruyen por índice. DONE requerido; errores/EOF/argumentos rotos no producen éxito ni dispatch. Batch completo se valida contra tools ofrecidas antes de efectos; IDs locales backend y un dispatch por llamada válida. Fixtures OpenAI/Groq/OpenRouter sintéticos; no nuevos proveedores/protocolos. Sin retry implícito de 429 ni replay automático de efectos.
- **Cancelación:** un supervisor retenido por turno, HTTP/forward/summary inline. Cancelar antes de despacho impide efectos; después del despacho drena la operación. La prueba con executor/SafePath real observa 202/false y segundo turno 409 durante espera; después crea sólo el primer archivo temporal, conserva su auditoría, impide segunda tool y termina Cancelled. No se promete rollback de efectos ya iniciados.
- **Recursos:** el mock TCP real observa EOF/reset en cancel antes de headers/primer token, durante tokens/UTF-8/evento y timeout; un intento LATE-A no llega a B. Se comprueban tareas terminadas, actividad/approvals vacías y ausencia de éxito en SQLite. Desconexión del cliente usa lease/closed para cancelar incluso antes del primer token. Un efecto heredado bloqueado puede permanecer Cancelling/202; no se fuerza abort sobre irreversible para declarar éxito.
- **Turnos/estados:** UUID backend distinto de Session ID, secuencia/estado en cada evento. Rechazo 409 de concurrencia en sesión, control separado entre sesiones, cancel UUID viejo no cancela B. Terminal lógico único, sin done cancelado ni aprobación tardía que reactive. Panic del work produce error; diagnóstico adicional detecta owner terminado sin terminal. SIGKILL/panic=abort/procesos externos no se certifican.
- **SQLite:** Turn.complete mantiene lock de estado -> DB durante publicación síncrona; cancelar ganador impide callback/persistencia. No hay await bajo esos locks ni camino inverso DB -> registry/turno. Network/aprobaciones son independientes, aunque las operaciones SQLite breves compartan mutex. SAVEPOINT contiene mensajes finales y completed; falla RELEASE/statement implica rollback/error, nunca Completed RAM. Migración aditiva conserva filas/NULL legacy y marca abandonados Failed tras reinicio; sin reanudación de acciones.
- **QML/backend:** todos los callers ejecutables hallados de chat/complete/regenerate/cancel inspeccionados; identities/eventos y callbacks se filtran por generación/sesión/turno. Stop conserva Cancelling y no interpreta 202 como final. Cambiar sesión libera body e invalida callbacks. Se conservan SEC-01/SEC-02, arranque escrito sin Piper/modelos y endpoints existentes con incompatibilidades explícitas. Voz no se reconstruye.

## Validación final de los árboles exactos

A: 9 gates heredados, todos exit 0; 19 seguridad (1 ignored), 13 arranque (1 ignored), 134 unitarios seguros (2 filtered), 12 Piper del mismo conjunto, 2 app_paths y 1 Qt heredado separado. Tests nuevos F1B2 NOT_PRESENT en A, no PASS implícito.

B: `python3 scripts/validate_f1b2.py --qml`, **exit 0**, 11 comandos internos aprobados, incluidos fmt/check --locked all-targets/clippy -D warnings y los tres targets de integración obligatorios.

| Selección B | Passed | Failed | Ignored | Filtered |
| --- | --- | --- | --- | --- |
| f1a_security | 19 | 0 | 1 | 0 |
| f1b1_text_startup ordinario | 13 | 0 | 1 | 0 |
| f1b2_conversation ordinario | 20 | 0 | 3 Qt | 0 |
| lib segura | 150 | 0 | 0 | 2 externos |
| Piper heredados, incluidos en lib | 12 | 0 | 0 | 140 |
| app_paths | 2 | 0 | 0 | 0 |
| Qt F1B1 opt-in | 1 | 0 | 0 | 13 |
| Qt F1B2 opt-in | 3 | 0 | 0 | 20 |

Evidencia A `/tmp/kda-f1b1-check-zzrvgpxs`, B `/tmp/kda-f1b2-check-s6e3td5o`; validación de working tree `/tmp/kda-f1b2-check-3z4avo14`. No se suman runners hijos ni subconjuntos. Mounts opt-in, dos externos filtrados, proveedor real, micrófono, modelos/descargas, TTS/hardware y Plasma Wayland NOT_RUN/NOT_TESTED. Todos los datos/credenciales de tests son temporales/sintéticos. Los resultados originales aprobados se conservan como históricos y no sustituyen esta revisión.

## Compatibilidad y pendientes de F1B-3

Cancel exige Session/Turn coincidentes: clientes session-only reciben rechazo, sin fallback al turno nuevo. Complete devuelve 502 al fallar/409 al cancelarse. Regenerate rechaza historial respondido; el botón/retry de QML explica mensaje nuevo explícito y conserva historial. El endpoint conserva retry explícito del último usuario sin respuesta para clientes conscientes de posibles efectos previos; no certifica exactly-once entre intentos. DONE/JSON/UTF-8/tools inválidos dejan de aceptarse como éxito, se retiró retry automático 429 y done espera mantenimiento opcional supervisado. Documentación README histórica no se modifica; CONTRACT define el API vigente. AgentActivity cuenta invocaciones ReAct, no el terminal/publicación del turno.

F1B-3 pendiente: journal/transacciones de usuario-intento-auditoría-efectos, recuperación de intermedios/crash, durabilidad/fsync/fallos que impidan incluso rollback, retención/poda de turnos/parciales y ventanas de historial con parejas tools completas. No se garantiza exactly-once externo tras crash ni rollback de archivos/apps/notificaciones/audio/recordatorios ya ejecutados. Supervisión global de procesos/voz/SIGKILL sigue fuera de F1B-2. No se inició trabajo de esa fase.

## Parche y Git final

[F1B2-CHANGES.patch](F1B2-CHANGES.patch): diff real baseline -> B, 254505 bytes, SHA256 `10c2d45640c04bcb5ccd6697ccaa1b90608a5cb6be2dd3d9c366f1f6b16dc235`. `git apply --check`: exit 0 en copia limpia; aplicación real exit 0 y equivalencia byte a byte de los 20 archivos cambiados y los 217 archivos del árbol completo. Tres warnings de LF LF en fixtures SSE son framing válido; no se eliminaron los terminadores.

Árbol tracked/staged limpio. Permanecen sin versionar exactamente 14 documentos históricos preexistentes (hashes preservados) más estos tres entregables posteriores a commits: CLOSURE, COMMIT-VALIDATION y CHANGES.patch. Se excluyen de commits/patch la historia ajena y la evidencia de cierre autorreferente, siguiendo el cierre F1B1. No hay tercer commit. Ninguna rama anterior se modificó; sin push, merge ni release. Identidad Codex por comando, como baseline, sin alterar configuración personal.
