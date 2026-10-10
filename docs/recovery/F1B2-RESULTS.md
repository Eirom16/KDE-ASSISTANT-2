# F1B-2 — Resultados verificables

Fecha: 2026-10-10. Baseline `600a363ee3a9d64d9ea2adbdbda5448efacc7e8a`; rama `recovery/f1b2-conversation-engine`. **IMPLEMENTED / READY_FOR_REVIEW** para el contrato escrito. Resultados previos a consolidación local; SHA/árboles exactos en F1B2-COMMIT-VALIDATION. Sin push ni merge.

## Validación final de la revisión de código

`python3 scripts/validate_f1b2.py --qml`: **exit 0**. Evidencia del árbol de trabajo auditado: `/tmp/kda-f1b2-check-3z4avo14/`, con results.json y logs. XDG_CONFIG_HOME/DATA_HOME/CACHE_HOME y TMPDIR privados, umask 077; claves y proxies retirados; Cargo offline. Fixtures loopback/SQLite temporal, sin modelos, micrófono ni proveedor real. Los logs se conservan; los directorios de configuración/datos de las pruebas se recogen. La validación no certifica un sandbox general de egress.

| Comando exacto | Exit | Resultado del runner padre | Log |
| --- | --- | --- | --- |
| `cargo fmt --check` | 0 | exit 0 | `fmt.log` |
| `cargo check --locked --all-targets` | 0 | exit 0 | `check.log` |
| `cargo clippy --locked --all-targets -- -D warnings` | 0 | exit 0 | `clippy.log` |
| `cargo test --locked --test f1a_security` | 0 | ok. 19 passed; 0 failed; 1 ignored; 0 measured; 0 filtered out; finished in 0.20s | `f1a-security.log` |
| `cargo test --locked --test f1b1_text_startup -- --test-threads=1` | 0 | ok. 13 passed; 0 failed; 1 ignored; 0 measured; 0 filtered out; finished in 1.26s | `f1b1.log` |
| `cargo test --locked --test f1b2_conversation -- --test-threads=1` | 0 | ok. 20 passed; 0 failed; 3 ignored; 0 measured; 0 filtered out; finished in 2.27s | `f1b2-conversation.log` |
| `cargo test --locked --lib -- --test-threads=1 --skip backend::wakeword_ml::tests::download_and_extract_onnx_runtime --skip backend::tool_executor::tests::show_image_huge_url_is_rejected_before_download` | 0 | ok. 150 passed; 0 failed; 0 ignored; 0 measured; 2 filtered out; finished in 3.69s | `unit-safe.log` |
| `cargo test --locked --lib -- --exact --test-threads=1 backend::http_server::tests::dictate_roundtrip_empty_transcript_releases_flag backend::http_server::tests::dictate_start_conflicts_with_voice_pipeline backend::http_server::tests::dictate_stop_without_start_conflicts backend::speech_service::tests::piper_engine_initializes backend::voice_pipeline::tests::auto_barge_ignora_mic_alto_si_tts_aun_no_suena backend::voice_pipeline::tests::barge_in_transita_interrupted_a_listening backend::voice_pipeline::tests::dictado_activo_aborta_wait_for_speech_rapido backend::voice_pipeline::tests::dictado_activo_bloquea_start_listening backend::voice_pipeline::tests::emit_state_mapea_a_strings_legados backend::voice_pipeline::tests::signal_broadcast_state backend::voice_pipeline::tests::signal_level_throttled backend::voice_pipeline::tests::wait_for_speech_times_out_quiet` | 0 | ok. 12 passed; 0 failed; 0 ignored; 0 measured; 140 filtered out; finished in 1.16s | `piper-inherited-now-lazy.log` |
| `cargo test --locked --test app_paths` | 0 | ok. 2 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s | `app-paths.log` |
| `cargo test --locked --test f1b1_text_startup qml_offscreen -- --ignored --nocapture` | 0 | ok. 1 passed; 0 failed; 0 ignored; 0 measured; 13 filtered out; finished in 2.65s | `qml-offscreen.log` |
| `cargo test --locked --test f1b2_conversation qml_ -- --ignored --nocapture --test-threads=1` | 0 | ok. 3 passed; 0 failed; 0 ignored; 0 measured; 20 filtered out; finished in 7.20s | `qml-f1b2-review.log` |

Los 12 heredados Piper están incluidos en los 150 unitarios; no se cuentan dos veces. F1A tiene 1 ignored, F1B1 1 ignored y F1B2 3 ignored en la selección ordinaria. QML opt-in se ejecutó aparte: 1 heredado y 3 nuevos aprobados. Los runners hijos no se suman de nuevo. Árboles staged A/B, hashes y validación previa a cada commit: [F1B2-COMMIT-VALIDATION](F1B2-COMMIT-VALIDATION.md).

## Casos F1B-2 HTTP/TCP

Los **20 casos HTTP: PASS**, 0 failed. La selección del archivo deja **3 ignored QML**, ejecutados aparte. Cada uno se ejecuta en un proceso con XDG privado. Los subprocesos también imprimen su resultado; no se suman otra vez al contador del runner padre.

| Test | Resultado y observación |
| --- | --- |
| `cancel_before_first_token_closes_connection` | PASS; Cancelled, EOF observado por proveedor, sin done, 1 usuario, 0 tasks/actividad AI activos. |
| `cancel_before_headers_and_during_incomplete_utf8_or_event` | PASS; tres requests: headers pendientes, bytes parciales de emoji, evento parcial; todos cierran y no publican token/done. |
| `cancel_tokens_then_b_isolated_from_late_a` | PASS; A parcial se cancela, mock intenta LATE-A tras EOF; B sigue activo pese al cancel tardío de A; ni respuesta ni contexto/SQLite se contaminan. |
| `approval_cancel_releases_waiter_and_late_decision` | PASS; espera pasa de 1 a 0, UUID local distinto del proveedor, resolver aprobación tardía devuelve false, archivo no creado. |
| `simultaneous_same_session_rejected_before_provider` | PASS; dos POST simultáneos producen 200/409, sólo un request al mock y un usuario; regenerate simultáneo también 409; AppState adicional comparte la misma reserva Backend. |
| `cancellation_is_scoped_to_session_and_turn` | PASS; cancel con sesión incorrecta 404; A cierra mientras B mantiene socket y completa; no se cancela otra sesión. |
| `client_disconnect_cancels_idle_provider` | PASS; drop del body SSE produce EOF en proveedor y 0 turnos activos, sin assistant completado. |
| `provider_timeout_releases_connection` | PASS; timeout inyectado 150 ms: provider_timeout/Failed, EOF y sin respuesta completada; tasks/actividad liberados. |
| `malformed_truncated_error_and_abrupt_streams_never_succeed` | PASS; cinco streams: JSON inválido, DONE ausente, error SSE, chunked truncado y tool parcial. Códigos respectivos: invalid_provider_stream, invalid_provider_stream, provider_error, provider_transport, invalid_provider_stream; sin done/assistant completado. |
| `fragmented_success_persists_turn_identity` | PASS; Español/niño/acción/emojis con HTTP de un byte; 2 mensajes con el mismo UUID y estado SQLite completed. |
| `interleaved_tools_execute_once_with_matching_local_ids` | PASS; dos read_file privados, fragmentos intercalados; IDs locales assistant/tool coinciden; 2 auditorías y 5 mensajes, sin duplicar ejecuciones. |
| `malformed_tool_schema_never_reaches_approval_or_executor` | PASS; required ausente termina en error; sin aprobación, tool_result, auditoría, archivo ni done. |
| `rate_limit_has_no_implicit_retry` | PASS; 429 terminal; sólo un request registrado, ningún reintento. |
| `cancelled_turn_survives_backend_restart_without_false_reply` | PASS; conserva usuario/Cancelled tras recrear backend; sin active tasks ni assistant parcial. |
| `old_database_migration_preserves_rows_and_marks_abandoned_turns` | PASS; esquema antiguo sin turn_id preserva sesión/mensaje y NULL legacy; ejecución abandonada Cancelling cambia a Failed al reiniciar. |
| `complete_endpoint_returns_error_status_and_never_fake_success` | PASS; parsing falla con 502 y state Failed, sin campo response de éxito ni assistant. |
| `complete_request_identity_query_and_cancellation` | PASS; GET identidad de complete en ejecución, cancel dirigido, complete retorna 409/Cancelled, EOF en proveedor y supervisor recogido. |
| `cancellation_during_supervised_summary_closes_its_http_request` | PASS; petición del resumen queda abierta y se cierra al cancelar; no se publica la candidate response como assistant; quedan 60 mensajes previos y el usuario actual. |
| `regeneration_rejects_completed_history_without_replaying_tools` | PASS; nueva identidad y turn/error coherentes, conserva los dos mensajes previos y no contacta nuevamente al proveedor. |
| `publication_failure_is_terminal_without_done_and_other_session_succeeds` | PASS; trigger SQLite RAISE(ROLLBACK) produce Failed/error con causa original, sin done ni assistant; handles se liberan y otra sesión publica correctamente. |

## Regresiones de UI

Tres pruebas opt-in sobre Main.qml real y servidor loopback: `qml_regenerate_preserves_history_and_explains_restriction`, `qml_cancel_202_remains_cancelling_until_terminal`, `qml_obsolete_session_list_cannot_replace_new_selection`: **3 PASS**. Verifican historial intacto y cero requests regenerate, Cancel 202 con SSE aún abierto/ocupado e identidad retenida hasta terminal, y selección/mensajes nuevos intactos pese al callback retrasado de lista.

## Parser y máquina de estados

Estas **16 nuevas pruebas PASS** están incluidas en los **150 unitarios seguros**, no son otro contador de ejecuciones independientes.

| Test unitario | Evidencia |
| --- | --- |
| `sse_parser::tests::every_byte_boundary_and_byte_at_a_time` | Todos los cortes de dos partes y fragmentación de un byte para Español, niño, acción y tres emojis; texto idéntico, sin replacement. |
| `sse_parser::tests::exhaustive_small_event_partitions` | Todas las 128 particiones posibles de los 8 bytes de data:x LF LF. |
| `sse_parser::tests::interleaved_tools_and_fragmented_json` | Todos los cortes de dos partes; dos calls por índice, nombres y argumentos multibyte intercalados reconstruidos. |
| `sse_parser::tests::sse_fields_comments_multiline_and_line_endings` | LF/CRLF/CR mezclados, comentarios, id/retry/event y data multilínea. |
| `sse_parser::tests::bom_and_persistent_event_id` | BOM partido en cualquier corte y Last Event ID persistente/reseteable. |
| `sse_parser::tests::provider_fixtures_at_every_boundary` | Todos los cortes de fixtures sintéticos OpenAI, Groq y OpenRouter; usage/metadata/comentarios compatibles. |
| `sse_parser::tests::protocol_failures_never_complete` | Todos los prefijos incompletos del fixture pequeño, JSON/evento error/finish_reason length, ausencia DONE, DONE duplicado idempotente y datos posteriores rechazados. |
| `sse_parser::tests::invalid_tools_utf8_and_limits` | Argumentos vacíos/rotos/array/null rechazados; UTF-8 inválido y buffer excesivo dan error. |
| `sse_parser::tests::cancellation_discards_partial_character_and_event` | Descartar parser con emoji/evento parcial no contamina el siguiente parser; integración TCP comprueba cancelación real. |
| `conversation_turn::tests::cancelled_before_dispatch_never_polls_effect` | Contador de efectos 0; terminal inmutable y rechazo de transición a Running. |
| `conversation_turn::tests::completion_and_cancellation_have_one_linearization_winner` | 50 carreras con threads unidos; persistencia ocurre exactamente cuando gana Completed; cancelar ganador impide el callback y termina Cancelled. |
| `http_server::tests::f1b2_cancel_dispatched_tool_drains_effect_and_blocks_next_tool` | HTTP mock + executor/SafePath real en SQLite memoria/XDG privado: 202/false mientras drena, 1 task activo y sesión bloqueada; al liberar crea primer archivo privado, no el segundo, una auditoría y ningún assistant/done, después Cancelled. |

Cuatro regresiones adicionales incluidas: `conversation_turn::tests::terminal_turn_never_dispatches_or_waits_for_cancellation` (0 efectos en Cancelled/Failed/Completed, espera despierta), `conversation_turn::tests::finished_supervisor_without_terminal_is_diagnosed_and_releases_slot` (panic inyectado, Failed y reserva liberada), `session_manager::tests::publication_release_failure_rolls_back_and_allows_next_turn` (FK diferida falla al RELEASE, rollback/autocommit y siguiente turno válido), `session_manager::tests::publication_statement_rollback_preserves_original_error` (RAISE(ROLLBACK), causa conservada, sin respuesta persistida).

La exhaustividad distingue todos los cortes de dos partes para mensajes semánticos de la enumeración de todas las multiparticiones para un evento mínimo; no se afirma explorar 2^N particiones de fixtures largos.

## Matriz de requisitos de integración

| Requisito obligatorio | Tests que lo demuestran |
| --- | --- |
| 1. Antes del primer token | cancel_before_first_token + cancel_before_headers. |
| 2. Durante tokens | cancel_tokens_then_b. |
| 3. Durante aprobación | approval_cancel. |
| 4. Durante tool y efectos iniciados | f1b2_cancel_dispatched_tool (unitario de integración HTTP/SQLite/executor). |
| 5. B después de cancelar A | cancel_tokens_then_b. |
| 6. A tardío no contamina B | cancel_tokens_then_b, mock intenta LATE-A. |
| 7. Otra sesión continúa | cancellation_is_scoped. |
| 8. Desconexión | client_disconnect. |
| 9. Timeout | provider_timeout. |
| 10. Parsing no produce éxito falso | malformed_truncated + complete_endpoint_returns_error. |
| 11. Reinicio después de cancelación | cancelled_turn_survives_backend_restart + old_database_migration. |
| 12. Solicitudes simultáneas misma sesión | simultaneous_same_session, incluyendo regenerate y registro compartido. |

## Liberación observada

El mock HTTP/1.1 usa Connection: close y lee el socket después de entregar headers/tokens (o antes de headers). En cancelación/timeout registra read == 0 o cierre/reset y luego intenta escribir LATE-A. Los tests esperan ese indicador, revisan `running_tasks()==0`, actividad AI vacía donde corresponde, esperas 0 y estado/mensajes SQLite. Es evidencia de la conexión loopback real, no un 200 del endpoint. En HTTP/2 real, liberar una petición puede resetear su stream sin cerrar una conexión compartida con otras sesiones; no se promete destruir el pool ni los sockets idle.

La herramienta despachada es distinta: antes de recoger su finalización se observa 202 y active_count 1, no una falsa liberación. Su efecto privado se conserva; no hay rollback. Tras drenar ya no se despacha la segunda tool. La barrera cfg(test) se instala después de autorización SafePath y antes del I/O; prueba el ownership de una operación ganadora del despacho, sin falsear permisos.

## Ignorados, excluidos y no ejecutados

- F1A: **1 ignored**, `search_mount_boundaries_remain_closed`; montaje opt-in **NOT_RUN en F1B2**, no PASS por inferencia de fases anteriores.
- F1B1 por defecto: **1 ignored**, QML opt-in; se ejecutó aparte y pasó **1**.
- Unitarios: **2 filtered out**, `backend::wakeword_ml::tests::download_and_extract_onnx_runtime` y `backend::tool_executor::tests::show_image_huge_url_is_rejected_before_download`. **NOT_RUN**, excluidos por su dependencia de red/descarga externa heredada; no se cuentan como PASS.
- Smokes de proveedor/modelos/audio, micrófono, herramientas destructivas del escritorio, descargas reales: **NOT_RUN**.
- QML/KDE: prueba **offscreen PASS**; Plasma Wayland real, tray/atajos/compositor y voz/TTS reales **NOT_TESTED**.

## Fallos intermedios y regresiones

Durante implementación, check detectó E0505 (borrow/move del sender en simple_completion) y warnings/helpers sin uso; se corrigieron usando un único futuro propietario y retirando helpers obsoletos. Clippy inicial rechazó dead_code y luego pasó.

La ejecución `/tmp/kda-f1b2-check-fl3qy0yc/` terminó **exit 1**: fmt exit 0 y check **exit 101**, E0027 en el patrón de `tests/agent_smoke.rs` tras añadir success. Los siguientes gates de ese intento **NO SE EJECUTARON**; no se declaran PASS. El smoke se adaptó con `..` y la ejecución final anterior y esta final completa comprobaron all-targets. En la implementación original no hubo tests Rust fallidos en las selecciones ejecutadas; los fallos de compilación/lint no se ocultan.

Las ejecuciones completas intermedias `/tmp/kda-f1b2-check-wqef84qk/`, `/tmp/kda-f1b2-check-c1r6ksl4/`, `/tmp/kda-f1b2-check-rphw8ofb/` y `/tmp/kda-f1b2-check-_ehz32cq/` también terminaron exit 0, pero corresponden a distintas revisiones de implementación; los resultados finales de este documento se toman exclusivamente de `/tmp/kda-f1b2-check-83mezqaf/`.

No se detectó regresión en SEC-01/SEC-02, texto sin Piper/modelos, bootstrap, persistencia happy path o flujo QML offscreen de F1B1. Hay cambios de compatibilidad deliberados de cancel/complete/regenerate y supervisión del mantenimiento opcional, documentados en CONTRACT y REPORT. No se afirma durabilidad ni exactly-once externo; siguen pendientes de F1B-3.

## Fallos reproducidos durante la revisión final

- `/tmp/f1b2-audit-before-db.log`: exit 101, 0 passed/2 failed/150 filtered; RELEASE fallido deja transacción abierta y rollback del trigger oculta causa. Después: `/tmp/f1b2-audit-after-db.log`, exit 0, 2 passed/150 filtered.
- `/tmp/f1b2-audit-before-turn.log`: exit 101, 2 passed/2 failed/148 filtered; terminal aceptado por dispatch y handle con panic permanece Running. Después: `/tmp/f1b2-audit-after-turn.log`, exit 0, 4 passed/148 filtered. El panic inyectado es intencional y el test comprueba su diagnóstico, no efectos externos deshechos.
- `/tmp/f1b2-audit-before-qml2.log`: exit 101, 0 passed/2 failed/19 filtered; Stop afirma finalización antes de acknowledgment y Regenerar borra historial. Después: `/tmp/f1b2-audit-after-qml.log`, exit 0, 2 passed/19 filtered.
- `/tmp/f1b2-audit-before-stale2.log`: exit 101, 0 passed/1 failed/22 filtered; callback de lista reemplaza selección nueva. Después: `/tmp/f1b2-audit-after-stale.log`, exit 0, 3 passed/20 filtered.
- Fallos del harness, no del producto: `/tmp/f1b2-audit-before-qml.log` exit 101 por token fixture inválido (corregido a 64 hex sintéticos); `/tmp/f1b2-audit-before-stale.log` pasó pero NO constituye evidencia: no se había conectado la ruta de lista retrasada y había warning de variable no usada. Se corrigió el mock y se reprodujo el fallo real en before-stale2. Ninguno se presenta como PASS de producto.
- `/tmp/kda-f1b2-check-noimrgwu/`: exit 0 sobre revisión intermedia antes del guard final de lista/creación; sus 2 Qt nuevos no sustituyen los 3 de la revisión final. La implementación inicial `/tmp/kda-f1b2-check-83mezqaf/` también pasó sobre un diff que esta revisión demostró insuficiente; se conserva como evidencia histórica.

No se oculta ningún fallo. Los resultados que acreditan la consolidación se obtienen de las copias exactas staged, documentadas separadamente; no se infiere PASS de pruebas ignoradas o no presentes en A.
