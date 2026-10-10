# F1B-3A — Resultados verificables

Fecha: 2026-10-10. Baseline `27f2db9430bcbc7c074472383fc8acbc142b5f89`; rama `recovery/f1b3a-persistence`. Estado **IMPLEMENTED / READY_FOR_REVIEW** dentro del [contrato](F1B3A-JOURNAL-CONTRACT.md). Consolidación local documentada por separado en F1B3A-CLOSURE; sin push/merge/release.

## Ejecución final

`python3 scripts/validate_f1b3a.py --qml`: **exit 0**, once gates aprobados tras las tres correcciones de producción y teardown Qt. Evidencia `/tmp/kda-f1b3a-check-qtjt5337/`: results.json, logs y source-manifest.json. La ejecución previa de implementación cacguus5 pasó nueve gates sin Qt (22 tests); gyo5tr0d pasó once (25 tests) antes de corregir la denegación manual. Ninguna sustituye los 26 tests finales.

XDG_CONFIG_HOME/DATA_HOME/CACHE_HOME y TMPDIR privados; umask 077, claves/proxies retirados, Cargo offline. Datos temporales recogidos, logs conservados. Fixtures SQLite 0700, archivos sintéticos y mocks loopback; no sandbox general de egress ni hardware/proveedores reales.

| Comando exacto | Exit | PASS / FAIL / IGNORED / FILTERED (runner padre) | Tiempo | Log |
| --- | --- | --- | --- | --- |
| `cargo fmt --check` | 0 | PASS | 0.7 s | `fmt.log` |
| `cargo check --locked --all-targets` | 0 | PASS | 11.339 s | `check.log` |
| `cargo clippy --locked --all-targets -- -D warnings` | 0 | PASS | 12.314 s | `clippy.log` |
| `cargo test --locked --test f1a_security` | 0 | test result: ok. 19 passed; 0 failed; 1 ignored; 0 measured; 0 filtered out; finished in 0.11s | 41.529 s | `f1a-security.log` |
| `cargo test --locked --test f1b1_text_startup -- --test-threads=1` | 0 | test result: ok. 13 passed; 0 failed; 1 ignored; 0 measured; 0 filtered out; finished in 0.84s | 7.524 s | `f1b1.log` |
| `cargo test --locked --test f1b2_conversation -- --test-threads=1` | 0 | test result: ok. 20 passed; 0 failed; 3 ignored; 0 measured; 0 filtered out; finished in 2.01s | 9.658 s | `f1b2-conversation.log` |
| `cargo test --locked --test f1b3a_persistence -- --test-threads=1` | 0 | test result: ok. 26 passed; 0 failed; 1 ignored; 0 measured; 0 filtered out; finished in 5.95s | 10.177 s | `f1b3a-persistence.log` |
| `cargo test --locked --lib -- --test-threads=1 --skip backend::wakeword_ml::tests::download_and_extract_onnx_runtime --skip backend::tool_executor::tests::show_image_huge_url_is_rejected_before_download` | 0 | test result: ok. 156 passed; 0 failed; 1 ignored; 0 measured; 2 filtered out; finished in 4.01s | 33.274 s | `unit-safe.log` |
| `cargo test --locked --test app_paths` | 0 | test result: ok. 2 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s | 2.347 s | `app-paths.log` |
| `cargo test --locked --test f1b1_text_startup qml_offscreen -- --ignored --nocapture` | 0 | test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 13 filtered out; finished in 2.73s | 3.127 s | `qml-offscreen.log` |
| `cargo test --locked --test f1b2_conversation qml_ -- --ignored --nocapture --test-threads=1` | 0 | test result: ok. 3 passed; 0 failed; 0 ignored; 0 measured; 20 filtered out; finished in 6.72s | 7.119 s | `qml-f1b2-review.log` |

Los runners hijos no se suman otra vez. El worker ignored del target nuevo es invocado explícitamente por los diez tests padres de SIGKILL; el worker unitario también es invocado por sus cuatro padres. No son pruebas funcionales pendientes ni PASS individuales: los padres comprueban checkpoint, señal 9, wait y recuperación. Los seis nuevos unitarios ordinarios están incluidos en los 156 unitarios seguros; no se suman por separado.

## Matriz de interrupción y recuperación

Todos los siguientes escenarios: **PASS**, SIGKILL exclusivamente sobre el hijo creado por el test. No se mata por nombre, PID buscado ni process group. Cada padre vuelve a abrir la misma base tres veces, comprueba integridad, conservación de mensajes, estado persistido y ausencia de replay. Los fixtures de transacción no invocan herramientas del sistema.

| Corte solicitado | Test/evidencia | Resultado recuperado |
| --- | --- | --- |
| 1. Antes de guardar intención | `crash_before_intent` | Ninguna operación; turno Failed; mensaje anterior completo conservado; cero archivo externo. |
| 2. Intención guardada, antes de despachar | `crash_after_intent`, `crash_after_authorization` | Prepared/Authorized -> Abandoned; cero efecto; permisos no reanudados. |
| 3. Despacho, antes de resultado | `crash_after_dispatch` | Dispatched -> UnknownOutcome; archivo sintético mantiene exactamente una escritura; cero replay. |
| 4. Resultado guardado, antes de finalizar turno | `crash_after_result_before_turn_finalization` | Succeeded intacto, turno Failed; no assistant final fabricado; efecto intacto. |
| 5. Transacción SQLite | `crash_during_sqlite_transaction`, unitario `sigkill_before_journal_commit` | Marcador/mensaje sin COMMIT revertidos; el corte en atomic real revierte operación+anchor/evento juntos; el Prepared/Authorized confirmado se abandona. |
| 6. Cancelación | `crash_during_cancellation` | Cancelling persistido -> Failed; operación despachada -> UnknownOutcome; efecto conservado una vez. |
| 7. Auditoría | `crash_during_audit_transaction`, unitario `sigkill_during_actual_audit_write` | Success de auditoría sin COMMIT revertido, Dispatched -> UnknownOutcome; no falso resultado conocido. |
| 8. Recuperación posterior | Unitario `sigkill_during_actual_recovery` | Corte después de cambiar operaciones, antes de finalizar turnos/COMMIT: permanece Dispatched al abrir SQLite crudo; siguiente recovery confirma UnknownOutcome una vez. |
| Migración | Unitario `sigkill_during_actual_migration` | DDL dentro de migración real sin COMMIT desaparece; siguiente apertura instala esquema/version=3 una vez; mensaje anterior intacto. |
| Despacho sin efecto observable | `dispatched_marker_without_an_effect_is_still_unknown` | UnknownOutcome también con cero archivo: el marcador no demuestra ejecución. |
| Error de executor registrado | `persisted_failure_survives_without_claiming_effect_rollback` | Failed intacto tras crash; fixture conserva su efecto previo para demostrar que Failed no significa rollback. |

Son diez procesos de crash de integración y cuatro de las rutas unitarias reales por ejecución de los targets finales. Hooks de migración/audit/commit/recovery se compilan sólo con cfg(test) en el ejecutable de unitarios y se arman sólo dentro del hijo. El worker de integración opera los métodos de almacenamiento reales entre fases y usa transacciones SQLite físicas para los cortes internos genéricos. El executor controlado de esos cortes escribe un archivo privado; no pretende certificar syscalls de KDE. El executor real/SafePath se ejerce separadamente en los casos de errores y HTTP.

## Inyección de errores y resultados observados

| Caso | Evidencia y conclusión |
| --- | --- |
| INSERT de operación o anchor fallido | `insert_and_dispatch_failures_never_execute`: RAISE(ABORT) devuelve error, cero archivo y conexión reutilizable. |
| Marcador de despacho fallido | Mismo test con RAISE(ROLLBACK): nunca sondea el executor, cero efecto. |
| Actualización de resultado o audit final fallida | `result_or_audit_failure_never_reports_success`: efecto privado ocurre, retorno Err; estado UnknownOutcome, success audit false y mensajes anteriores intactos. |
| Error de disco simulado | `simulated_disk_full_fails_closed_and_rolls_back`: trigger de escritura fallida revierte anchor+intención; no dispatch; al retirar el fallo la misma conexión ejecuta correctamente. El texto del trigger es una simulación, no un SQLITE_IOERR físico. |
| SQLITE_FULL real aislado | Unitario `full_and_commit_errors_rollback_and_release_connection`: max_page_count privado + zeroblob excede cuota y devuelve `ErrorCode::DiskFull`; rollback/autocommit comprobados. No se llena un disco real. |
| COMMIT fallido | Mismo unitario: FK diferida falla en COMMIT; cero fila child, autocommit y siguiente transacción válida. Publicación F1B-2 también conserva sus pruebas de RELEASE/RAISE(ROLLBACK). |
| Unwind | `transaction_unwind_rolls_back_before_supervisor_continues`: panic controlado recogido, cero facts y conexión fuera de transacción. |
| SQLITE_BUSY real | `busy_database_times_out_without_effect_and_recovers`: writer externo mantiene BEGIN IMMEDIATE; timeout ~5 s, cero efecto; después de ROLLBACK la invocación funciona. |
| Recovery fallido | `recovery_error_rolls_back_all_changes_and_next_start_can_recover`: trigger en snapshot hace fallar apertura; estado/evento Unknown sin confirmar revierten; siguiente apertura recupera. |
| Reinicios repetidos | Todos los padres de SIGKILL reabren tres veces; no se duplica UnknownOutcome, snapshot ni marcador de migración y no se tocan targets externos. |
| Instancia duplicada | `second_backend_cannot_recover_live_operations`: segunda apertura falla con flock; después de cerrar la primera, abre correctamente. |
| Migración legacy | `legacy_migration_preserves_messages_and_explicit_storage_settings`: conserva mensajes/NULL turn_id y un único marcador en tres aperturas; WAL activo. Apertura productiva verifica FULL=2. |
| Consentimiento/metadatos | `privacy_denials_transitions_and_schema_are_enforced`: Denied sin efecto, UUID backend, transiciones terminales rechazadas, enlaces inválidos rechazados; argumentos `[redacted]`, sin contenido/destino en API. |
| Cancel antes del despacho | `cancellation_before_dispatch_is_persisted_as_abandoned`: cero archivo; Authorized -> Abandoned durante settle. |
| Intentos regenerate | `retries_keep_the_original_user_link_and_cannot_hide_effects`: efecto incierto de retry impide otro regenerate aunque el usuario conserve Turn ID original; nuevo mensaje explícito tiene identidad independiente. |

## Integración HTTP y F1B-2

- `http_success_and_authenticated_journal_match_turn`: SSE real del router + proveedor local; una creación sintética, dos requests LLM, journal Succeeded en el Turn ID del stream, cuatro mensajes publicados; GET sin bearer 401 y GET autorizado devuelve sólo metadatos. Reinicio conserva el resultado y no duplica el archivo.
- `http_result_write_failure_has_no_tool_success_or_done`: creación ya ocurrió, trigger bloquea su registro final; sólo un request LLM, SSE error sin tool_result/done, journal UnknownOutcome, sólo usuario persistido; reinicio no repite efectos.
- Test existente `f1b2_cancel_dispatched_tool_drains_effect_and_blocks_next_tool`, reforzado: antes de liberar la barrera hay Dispatched confirmado y autocommit; 202/409 conservados; después Succeeded ligado al turno cancelado, una auditoría, cero segunda herramienta/assistant/done.
- Los 20 tests F1B2 ordinarios pasan, incluidos UUID/SSE/terminales, cancelación antes/durante red/aprobación, sesiones paralelas, A tardío, malformed batch, timeout, 429 sin retry, complete/regenerate y migración/publicación. Los 19 SEC-01/02 y 13 arranque/texto también pasan.

## Exclusiones explícitas

- F1A: 1 ignored de montajes opt-in, **NOT_TESTED**.
- F1B1: 1 ignored en selección ordinaria; opt-in Qt ejecutado aparte: **1 PASS, 0 FAIL, 0 IGNORED, 13 FILTERED**.
- F1B2: 3 ignored en selección ordinaria; opt-in Qt ejecutado aparte: **3 PASS, 0 FAIL, 0 IGNORED, 20 FILTERED**. No se cambió QML; se verifica el harness offscreen, no Plasma.
- Unitarios: 2 filtered, `download_and_extract_onnx_runtime` y `show_image_huge_url_is_rejected_before_download`, **NOT_TESTED** por red/descarga heredada. Los tests Piper heredados incluidos son lazy y no ejercitan voz real.
- Proveedores reales, micrófono, audio/TTS, modelos, herramientas del escritorio, Plasma Wayland, pérdidas de alimentación y fallos físicos/VFS: **NOT_TESTED**.
- No se ejecutó `cargo test --all-targets` indiscriminado, para no activar smokes externos. `check/clippy --all-targets` sí compilaron esos targets.

## Fallos intermedios y correcciones

No se presentan intentos incompletos como validación final:

1. Check inicial: E0716 por temporal de Message en begin_turn, corregido con binding de usuario y ubicación posterior al build del contexto. Compilación del fixture inicial: E0283 por `.into()` redundante en create_session, corregido. Una assertion nueva usó `id` fuera de scope (E0425); se corrigió a `turn.id`.
2. `/tmp/kda-f1b3a-check-08iemsm8`: fmt/check exit 0, clippy exit 101 por let_and_return en executor; los siguientes gates no corrieron. Se retornó directamente el match.
3. `/tmp/kda-f1b3a-check-ofimz1xe`: fmt/check/clippy/F1A/F1B1 exit 0; F1B2 exit 101 (19 passed, 1 failed, 3 ignored). El fixture de reinicio retenía owners HTTP anteriores después de abortar serve; flock detectó backend aún vivo. Ahora espera la desaparición real del Arc anterior antes de reabrir, manteniendo el bloqueo de instancia. Gates posteriores de ese intento no ejecutados.
4. `/tmp/kda-f1b3a-check-cdsajci7`: gates hasta F1B2 aprobados; F1B3A exit 101 (20 passed, 2 failed, 1 ignored). Los dos fixtures HTTP nuevos repetían el mismo error de teardown en su reapertura; ahora cierran el client y esperan la liberación del DB owner. Unitarios/app_paths de ese intento no ejecutados.
5. La ejecución de implementación `/tmp/kda-f1b3a-check-cacguus5` aprueba los nueve gates sobre el código final, incluido el test de exclusión de segunda instancia y los tres fixtures de reinicio corregidos. Los resultados unitarios/intermedios focales anteriores se usaron para desarrollo, no sustituyen esta ejecución completa.


## Revisión final: antes y después

- Dos regresiones nuevas sobre el código anterior: `retry_rechecks_unanswered_input_inside_transaction` y `nonexistent_session_cannot_dispatch_or_persist_untrusted_metadata`, cada una **0 PASS, 1 FAIL, 0 IGNORED, 24 FILTERED, exit 101**. Logs `*-before.log` y regressions-before.json en `/tmp/kda-f1b3a-review-rz85yxmf/`. La primera aceptaba trailing tool; la segunda ejecutaba y persistía una sesión arbitraria. Correcciones limitadas a validar sesión y última fila/role dentro de la transacción de inicio/retry. Después: ambos **PASS** en la selección completa de 26.
- Primera batería de revisión `/tmp/kda-f1b3a-check-ni5kftf8/`: nueve gates Rust **PASS**, 24 tests F1B3A **PASS**; Qt F1B1 **0 PASS, 1 FAIL, 0 IGNORED, 13 FILTERED, exit 101** al lanzar el ejecutable con el Backend del fixture aún vivo. Qt F1B2 **NOT_TESTED** en ese intento, porque el script paró en el fallo. Teardown corregido liberando el owner; segunda batería completa gyo5tr0d **PASS** con ambos opt-ins.
- Nueva cobertura `legacy_user_retry_remains_blocked_after_repeated_restarts`: usuario legacy NULL turn_id vinculado a un retry despachado/cancelado sigue bloqueado tras tres aperturas; UnknownOutcome y mensajes intactos. **PASS**.
- Los dos fixtures HTTP ahora consultan sesión inexistente e inyección SQL codificada: HTTP 200 y lista vacía, sin ampliar acceso; bearer ausente 401, contenido privado ausente. **PASS**. El límite se inspecciona en el clamp 1..500 del método SQL parametrizado; no se afirma una prueba de carga con 500 filas.

- Tercera regresión de producción `http_manual_denial_is_persisted_and_write_failure_is_closed`: antes **0 PASS, 1 FAIL, 0 IGNORED, 26 FILTERED, exit 101** (journal vacío tras 403). Log `http_manual_denial_is_persisted_and_write_failure_is_closed-before.log` en la evidencia de revisión. Después **PASS**: Denied persistido antes de 403; trigger que impide Denied produce HTTP 500, no más eventos Dispatched ni efectos. La batería final qtjt5337 repite todos los gates y opt-ins.

## Límites de la evidencia

WAL/FULL y commit confirmado no equivalen a exactly-once externo ni durabilidad universal ante pérdida física. La ventana entre marcador y resultado se conserva como UnknownOutcome. Failed puede conservar efectos parciales; Succeeded representa la semántica del executor, no verificación independiente. ToolCtx sigue sin ser una capacidad; F2 pendiente. La consulta nueva es API/SQLite, sin pantalla de recuperación ni retención/contexto de F1B-3B. El contrato detalla estos límites y la política de solicitud manual nueva después de inspección.
