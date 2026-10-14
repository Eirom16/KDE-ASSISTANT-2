# F2 — Resultados y cobertura

Rama `recovery/f2-permission-engine`; baseline `991f5c78dddf0b4f208bf5683604db2cb5c51112`. Auditoría de consolidación del 14 de octubre de 2026. El runner usó Cargo offline, SQLite y XDG temporales privados, Qt offscreen y ninguna clave de proveedor. No usó modelos, micrófono, documentos personales ni servicios externos.

## Comandos y estados

| Comando del runner `python3 scripts/validate_f2.py` | Estado | PASS | FAIL | IGNORED | FILTERED |
| --- | --- | ---: | ---: | ---: | ---: |
| `cargo fmt --check` | PASS | — | — | — | — |
| `cargo check --locked --all-targets` | PASS | — | — | — | — |
| `cargo clippy --locked --all-targets -- -D warnings` | PASS | — | — | — | — |
| `cargo clippy --locked --features legacy-fixtures --all-targets -- -D warnings` | PASS | — | — | — | — |
| `cargo test --locked --features legacy-fixtures --test f1a_security -- --test-threads=1` | PASS | 19 | 0 | 1 | 0 |
| `cargo test --locked --test f1b1_text_startup -- --test-threads=1` | PASS | 13 | 0 | 1 | 0 |
| `cargo test --locked --test f1b2_conversation -- --test-threads=1` | PASS | 20 | 0 | 3 | 0 |
| `cargo test --locked --test f1b3a_persistence -- --test-threads=1` | PASS | 26 | 0 | 1 | 0 |
| `cargo test --locked --features legacy-fixtures --test f1b3b_storage -- --test-threads=1` | PASS | 33 | 0 | 3 | 0 |
| `cargo test --locked --test f2_permissions -- --test-threads=1` | PASS | 17 | 0 | 0 | 0 |
| `cargo test --locked --lib f2_http_consent -- --test-threads=1` | PASS | 1 | 0 | 0 | 160 |
| `qmltestrunner -import . -input tests/qml/tst_f2_consent.qml -o -,txt` con `QT_QPA_PLATFORM=offscreen` | PASS | 4 | 0 | 0 | 0 |
| `python3 scripts/validate_gate_w1r.py` | PASS | 11 | 0 | 0 | 0 |

`FILTERED` en la invocación HTTP indica los 160 tests unitarios no seleccionados por ese filtro, no fallos. Los `IGNORED` son exclusiones heredadas de las suites F1. Los logs del runner viven en un directorio temporal privado que se elimina al terminar; el resumen se imprimió en stdout. Hubo un primer intento de auditoría con FAIL de formato y una regresión F1B-3A por cambiar la auditoría de recuperación; se corrigieron ambos sin alterar la semántica heredada. El runner F1B-3B independiente en el primer archivo staged B reprodujo un fallo intermitente F1B-1: el fixture abría dos backends sucesivos sin soltar explícitamente las referencias del primero. Se corrigió el teardown y el test específico pasó cinco ejecuciones consecutivas. La tabla recoge el rerun completo final con todos los gates PASS.

Comandos de regresión solicitados, ejecutados **también por separado**:

| Comando | Estado | Alcance |
| --- | --- | --- |
| `python3 scripts/validate_f1b3b.py --qml` | PASS | fmt, check, clippy, F1A, F1B-1/2/3A/3B, unitarios seguros, rutas app y QML offscreen; todos exit 0 |
| `python3 scripts/validate_gate_w1r.py` | PASS | 11 PASS, 0 FAIL |

Ensayo adicional con Wayland real y XDG privado: `qmltestrunner` con `QT_QPA_PLATFORM=wayland` y `QT_QPA_PLATFORMTHEME=generic` tuvo **4 PASS, 0 FAIL**. Con el tema KDE del entorno, el runner abortó al inicializar `KDEPlasmaPlatformTheme6.so`; por tanto la integración específica de Plasma/KWin sigue **NOT_TESTED**. No se ejecutó backend real ni hubo operaciones de archivo en este ensayo QML.

## Los 35 escenarios exigidos

`PASS` significa que una prueba automática ejercitó el control indicado. `NOT_TESTED` indica ausencia de ejecución del comportamiento subyacente. Algunos escenarios comparten un mismo test F2. Los nombres citados son funciones de `tests/f2_permissions.rs` salvo indicación.

| # | Escenario | Estado y evidencia |
| ---: | --- | --- |
| 1 | Green permitido | PASS — `green_is_auto_and_caller_approval_is_not_a_capability` |
| 2 | Lectura sensible no Green | PASS — mismo test y `policy_denies_secrets_network_unknown_and_oversize` |
| 3 | Yellow sin aprobación | PASS — `yellow_approval_is_bound_to_arguments_resource_and_single_dispatch` |
| 4 | Yellow con aprobación | PASS — mismo test |
| 5 | Red sin aprobación | PASS — `red_requires_exact_consent_and_edits_approved_inode` |
| 6 | Red con aprobación exacta | PASS — mismo test y copia `.bak` |
| 7 | Argumentos cambiados | PASS — `yellow_approval_is_bound_to_arguments_resource_and_single_dispatch` |
| 8 | Recurso cambiado | PASS — mismo test e `inode_or_symlink_replacement_cannot_reuse_consent` |
| 9 | Consentimiento de otra operación | PASS — `two_pending_consents_do_not_authorize_each_other` |
| 10 | Expiración | PASS — `forged_ids_expiration_and_cancellation_cannot_approve` |
| 11 | Reutilización de un solo uso | PASS — `yellow_approval_is_bound_to_arguments_resource_and_single_dispatch` |
| 12 | Doble clic | PASS — segunda decisión rechazada en el mismo test; endpoint usa transición atómica `pending` |
| 13 | Sesión incorrecta | PASS — `forged_ids_expiration_and_cancellation_cannot_approve` |
| 14 | Turn ID incorrecto | PASS — mismo test |
| 15 | Operation ID inexistente | PASS — mismo test |
| 16 | Turno cancelado | PASS — `approved_but_cancelled_turn_does_not_dispatch` |
| 17 | QML cerrado en espera | PASS — `tst_f2_consent.qml` verifica cierre/denegación local y descarte al cambiar de sesión; `approvals::tests` verifica que el waiter caído se retira. Cierre real del proceso QML: NOT_TESTED |
| 18 | Reinicio con aprobación pendiente | PASS — `denied_expired_and_pending_requests_do_not_survive_restart` |
| 19 | Fallo SQLite antes de despacho | PASS — `sqlite_dispatch_failure_never_executes_and_marker_recovers_unknown` |
| 20 | SIGKILL después del marcador | PASS — prueba de interrupción heredada F1B-3A en `validate_f1b3b.py`; F2 prueba la recuperación del marcador y `UnknownOutcome` sin un SIGKILL nuevo |
| 21 | Dos aprobaciones concurrentes | PASS — `two_pending_consents_do_not_authorize_each_other` usa dos tareas simultáneas |
| 22 | Prompt injection en resultado | PASS — `prompt_injection_in_tool_data_cannot_create_a_capability` |
| 23 | `ToolCtx.decided` falsificado | PASS — `green_is_auto_and_caller_approval_is_not_a_capability` |
| 24 | Ruta heredada sin autoridad | PASS — `ToolExecutor::new` eliminado del build predeterminado; `legacy_fixture` exige feature opt-in. La ruta HTTP manual conserva autoridad F2 y `all_nineteen_legacy_tools_remain_denied_with_flags_enabled` verifica catálogo, plan y despacho manual |
| 25 | Race symlink/inode | PASS — `inode_or_symlink_replacement_cannot_reuse_consent` |
| 26 | URL prohibida/redirección | PASS para denegación previa a red — `disabled_network_and_process_adapters_cannot_contact_loopback_or_spawn`. Redirección efectiva: NOT_TESTED porque el adaptador está deshabilitado |
| 27 | Timeout de proceso | NOT_TESTED — no se despachan adaptadores de procesos; el test confirma que la ruta productiva no crea hijo |
| 28 | Resultado grande | PASS — `oversized_result_is_bounded_and_audited_as_failure`; `system_info` limita salida a 32 000 caracteres |
| 29 | Denegación auditada | PASS — `denied_expired_and_pending_requests_do_not_survive_restart` y test loopback |
| 30 | Cancelación sin reejecución | PASS — `approved_but_cancelled_turn_does_not_dispatch` y recuperación F1B-3A |
| 31 | Herramienta no catalogada | PASS — `policy_denies_secrets_network_unknown_and_oversize` |
| 32 | Reinicios repetidos | PASS — `denied_expired_and_pending_requests_do_not_survive_restart` abre SQLite dos veces |
| 33 | Borrado con operación pendiente | PASS — `session_deletion_rejects_pending_and_sessions_are_isolated` |
| 34 | Evento confiable/no confiable | PASS — `f2_http_consent` usa servidor loopback privado: sin bearer recibe 401; con bearer y call falsificada recibe 404. La prueba directa verifica la aprobación válida y el replay. Inyección de evento dentro de QML: NOT_TESTED |
| 35 | Aislamiento de conversaciones | PASS — `session_deletion_rejects_pending_and_sessions_are_isolated` y `two_pending_consents_do_not_authorize_each_other` |

## Cobertura que falta para reactivar herramientas

- Redirecciones HTTP, DNS rebinding, bytes y timeouts de red: **NOT_TESTED**, porque `web_search`, `show_image` remoto y `open_url` no pueden despacharse en F2.
- Timeout y ownership de hijos, límites stdout/stderr: **NOT_TESTED**, porque `open_app`, `close_app` y demás adaptadores de procesos no pueden despacharse.
- Proveedor real, Plasma Wayland, diálogo visual en una sesión KDE real, modelos y micrófono: **NOT_TESTED**.
- No hubo tests externos ni solicitudes a direcciones públicas. El test de loopback crea un servidor privado y comprueba que no recibe conexión.

Regresiones añadidas en esta auditoría: `all_nineteen_legacy_tools_remain_denied_with_flags_enabled`, `create_collision_and_backup_collision_preserve_existing_data`, `uncertain_edit_result_is_audited_without_replay`, argumentos inesperados y metadatos Green acotados, y `test_session_detach_discards_old_consent` QML. La creación usa temporal exclusivo y publicación atómica `NOREPLACE`; un fallo tras iniciar una edición se registra `UnknownOutcome`.

Resultado de ingeniería: **IMPLEMENTED para el catálogo reducido**. La reactivación de cualquiera de las 19 herramientas bloqueadas requiere sus propios controles y pruebas antes de cambiar la política central.
