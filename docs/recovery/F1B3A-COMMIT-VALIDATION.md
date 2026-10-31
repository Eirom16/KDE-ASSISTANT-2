# F1B-3A — Validación de commits locales

Fecha: 2026-10-10. Estado **CONSOLIDATED / READY_FOR_REVIEW**. Rama `recovery/f1b3a-persistence`.
Baseline obligatorio `27f2db9430bcbc7c074472383fc8acbc142b5f89`. Sólo dos commits locales; sin push, merge, release, amend ni modificación de fases previas. F1B-3B no iniciado.

## Identidades verificadas

| Commit | SHA | Padre real | Árbol Git |
| --- | --- | --- | --- |
| A — Journal y recuperación persistente | `690c5d65b48a2fb19564c842a12677b129822a70` | `27f2db9430bcbc7c074472383fc8acbc142b5f89` | `0160bbadb6fcf91472c6bea162104a986b110f00` |
| B — Pruebas y documentación | `d8a6972b5bc3977408c8d0956fce3196623d5458` | `690c5d65b48a2fb19564c842a12677b129822a70` | `5f724bb939fc0e95d4fe197b2e519b8f5e02b6fd` |

Se verificaron HEAD/padre/tree contra los objetos staged antes y después de cada commit. `rev-list --count baseline..HEAD` = **2**. La rama anterior permanece en el baseline. Identidad de commit Codex configurada sólo por comando; no se alteró configuración personal persistente.

## Manifiestos exactos

A: 10 archivos, implementación completa y dos adaptaciones mínimas de fixtures heredados necesarias para respetar el flock y validar el commit por sí solo. No contiene la nueva batería ni checkpoints. B: 7 archivos, exclusivamente pruebas/hooks/script/documentación. Los dos Rust comunes sólo añaden cfg(test)/assertions en B; se comparó el código de producción al retirar esos bloques y es byte a byte idéntico a A. En total, **15 archivos distintos** cambian frente al baseline.

### Commit A

| Archivo | Responsabilidad |
| --- | --- |
| `migrations/003_operation_journal.sql` | Esquema aditivo, índices, CHECKs y triggers; version=3. |
| `src/backend/ai_service.rs` | Intento/denegación persistidos y dispatch/result por Operation/Turn ID. |
| `src/backend/conversation_turn.rs` | Persistencia del despacho bajo el lock de cancelación; drenado obligatorio. |
| `src/backend/http_server.rs` | A: turn/input, settle, regenerate, API de operaciones y denegación manual. B: assertions de resultado del turno cancelado. |
| `src/backend/mod.rs` | Exportación del módulo; el Backend mantiene un único executor with_audit. |
| `src/backend/operation_journal.rs` | A: transacciones, estados, recuperación, inputs/retries y validación de sesión. B: cuatro checkpoints cfg(test), cuatro tests SIGKILL, FULL/COMMIT/unwind y worker ignorado. |
| `src/backend/session_manager.rs` | Apertura/flock, migraciones atómicas, WAL/FULL, publicación y fixtures in-memory coherentes. |
| `src/backend/tool_executor.rs` | Journal y resultados obligatorios en el wiring de producción; auditoría redaccionada. |
| `tests/f1b1_text_startup.rs` | Adaptación mínima de teardown del fixture Qt para liberar la instancia SQLite. |
| `tests/f1b2_conversation.rs` | Adaptación mínima de teardown del fixture de reinicio para liberar owners HTTP. |

### Commit B

| Archivo | Responsabilidad |
| --- | --- |
| `docs/recovery/F1B3A-JOURNAL-CONTRACT.md` | Contrato normativo y límites de recuperación/autoridad/privacidad. |
| `docs/recovery/F1B3A-REPORT.md` | Implementación y defectos corregidos en revisión. |
| `docs/recovery/F1B3A-RESULTS.md` | Resultados de workspace, crash matrix y fallos antes/después. |
| `scripts/validate_f1b3a.py` | Gates con XDG/SQLite privados, Cargo offline y Qt opt-in. |
| `src/backend/http_server.rs` | A: turn/input, settle, regenerate, API de operaciones y denegación manual. B: assertions de resultado del turno cancelado. |
| `src/backend/operation_journal.rs` | A: transacciones, estados, recuperación, inputs/retries y validación de sesión. B: cuatro checkpoints cfg(test), cuatro tests SIGKILL, FULL/COMMIT/unwind y worker ignorado. |
| `tests/f1b3a_persistence.rs` | 26 tests ordinarios y worker: crash, fallos de escritura, identidad, privacidad y HTTP privado. |

## Procedimiento staged e independencia

Para ambos commits: lista exacta comparada con manifiesto; diff staged completo revisado; `git diff --cached --check` exit 0; inspección manual de escrituras/valores y patrones de credenciales privadas sin hallazgos. Los textos PRIVATE_* y synthetic-fixture de tests son datos sintéticos, no credenciales reales. No se añadieron archivos de configuración personal, SQLite, logs, modelos ni artefactos compilados.

Cada árbol se exportó con `git archive <staged-tree>` a un directorio 0700 propio, sin archivos sin versionar del workspace ni dependencia de los informes históricos locales. Cargo usó el lockfile exportado y el caché de crates/target existente, offline; todo el código, SQL y fixtures requeridos procedían del árbol Git. A se construyó sobre el baseline real; B sobre A ya validado. No se usó el script F1B3A inexistente en A.

### Árbol A aislado — PASS

Exportación: `/tmp/kda-f1b3a-review-rz85yxmf/tree-a-final`. Comando: `CARGO_TARGET_DIR=/home/eirom/KDE-ASSISTANT-2/target python3 scripts/validate_f1b2.py --qml`.
Evidencia `/tmp/kda-f1b2-check-veoc4tl8/`: results.json y logs. El campo base del script histórico de A describe F1B2; el padre real comprobado de A es el baseline F1B3A de la tabla anterior.

| Comando exacto | Exit | Runner padre / clasificación | Tiempo | Log |
| --- | --- | --- | --- | --- |
| `cargo fmt --check` | 0 | PASS | 0.853 s | `fmt.log` |
| `cargo check --locked --all-targets` | 0 | PASS | 16.724 s | `check.log` |
| `cargo clippy --locked --all-targets -- -D warnings` | 0 | PASS | 21.825 s | `clippy.log` |
| `cargo test --locked --test f1a_security` | 0 | test result: ok. 19 passed; 0 failed; 1 ignored; 0 measured; 0 filtered out; finished in 0.29s | 62.63 s | `f1a-security.log` |
| `cargo test --locked --test f1b1_text_startup -- --test-threads=1` | 0 | test result: ok. 13 passed; 0 failed; 1 ignored; 0 measured; 0 filtered out; finished in 0.94s | 12.728 s | `f1b1.log` |
| `cargo test --locked --test f1b2_conversation -- --test-threads=1` | 0 | test result: ok. 20 passed; 0 failed; 3 ignored; 0 measured; 0 filtered out; finished in 2.32s | 15.989 s | `f1b2-conversation.log` |
| `cargo test --locked --lib -- --test-threads=1 --skip backend::wakeword_ml::tests::download_and_extract_onnx_runtime --skip backend::tool_executor::tests::show_image_huge_url_is_rejected_before_download` | 0 | test result: ok. 150 passed; 0 failed; 0 ignored; 0 measured; 2 filtered out; finished in 3.87s | 44.041 s | `unit-safe.log` |
| `cargo test --locked --lib -- --exact --test-threads=1 backend::http_server::tests::dictate_roundtrip_empty_transcript_releases_flag backend::http_server::tests::dictate_start_conflicts_with_voice_pipeline backend::http_server::tests::dictate_stop_without_start_conflicts backend::speech_service::tests::piper_engine_initializes backend::voice_pipeline::tests::auto_barge_ignora_mic_alto_si_tts_aun_no_suena backend::voice_pipeline::tests::barge_in_transita_interrupted_a_listening backend::voice_pipeline::tests::dictado_activo_aborta_wait_for_speech_rapido backend::voice_pipeline::tests::dictado_activo_bloquea_start_listening backend::voice_pipeline::tests::emit_state_mapea_a_strings_legados backend::voice_pipeline::tests::signal_broadcast_state backend::voice_pipeline::tests::signal_level_throttled backend::voice_pipeline::tests::wait_for_speech_times_out_quiet` | 0 | test result: ok. 12 passed; 0 failed; 0 ignored; 0 measured; 140 filtered out; finished in 1.19s | 1.926 s | `piper-inherited-now-lazy.log` |
| `cargo test --locked --test app_paths` | 0 | test result: ok. 2 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s | 2.578 s | `app-paths.log` |
| `cargo test --locked --test f1b1_text_startup qml_offscreen -- --ignored --nocapture` | 0 | test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 13 filtered out; finished in 3.20s | 3.648 s | `qml-offscreen.log` |
| `cargo test --locked --test f1b2_conversation qml_ -- --ignored --nocapture --test-threads=1` | 0 | test result: ok. 3 passed; 0 failed; 0 ignored; 0 measured; 20 filtered out; finished in 6.96s | 7.388 s | `qml-f1b2-review.log` |

### Árbol B aislado — PASS

Exportación: `/tmp/kda-f1b3a-review-rz85yxmf/tree-b`. Comando: `CARGO_TARGET_DIR=/home/eirom/KDE-ASSISTANT-2/target python3 scripts/validate_f1b3a.py --qml`.
Evidencia `/tmp/kda-f1b3a-check-9s246nz9/`: results.json y logs. El campo base del script histórico de A describe F1B2; el padre real comprobado de A es el baseline F1B3A de la tabla anterior.

| Comando exacto | Exit | Runner padre / clasificación | Tiempo | Log |
| --- | --- | --- | --- | --- |
| `cargo fmt --check` | 0 | PASS | 0.812 s | `fmt.log` |
| `cargo check --locked --all-targets` | 0 | PASS | 14.096 s | `check.log` |
| `cargo clippy --locked --all-targets -- -D warnings` | 0 | PASS | 18.626 s | `clippy.log` |
| `cargo test --locked --test f1a_security` | 0 | test result: ok. 19 passed; 0 failed; 1 ignored; 0 measured; 0 filtered out; finished in 0.20s | 60.349 s | `f1a-security.log` |
| `cargo test --locked --test f1b1_text_startup -- --test-threads=1` | 0 | test result: ok. 13 passed; 0 failed; 1 ignored; 0 measured; 0 filtered out; finished in 0.87s | 10.515 s | `f1b1.log` |
| `cargo test --locked --test f1b2_conversation -- --test-threads=1` | 0 | test result: ok. 20 passed; 0 failed; 3 ignored; 0 measured; 0 filtered out; finished in 2.20s | 12.714 s | `f1b2-conversation.log` |
| `cargo test --locked --test f1b3a_persistence -- --test-threads=1` | 0 | test result: ok. 26 passed; 0 failed; 1 ignored; 0 measured; 0 filtered out; finished in 6.03s | 12.959 s | `f1b3a-persistence.log` |
| `cargo test --locked --lib -- --test-threads=1 --skip backend::wakeword_ml::tests::download_and_extract_onnx_runtime --skip backend::tool_executor::tests::show_image_huge_url_is_rejected_before_download` | 0 | test result: ok. 156 passed; 0 failed; 1 ignored; 0 measured; 2 filtered out; finished in 4.08s | 42.198 s | `unit-safe.log` |
| `cargo test --locked --test app_paths` | 0 | test result: ok. 2 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s | 2.641 s | `app-paths.log` |
| `cargo test --locked --test f1b1_text_startup qml_offscreen -- --ignored --nocapture` | 0 | test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 13 filtered out; finished in 3.09s | 3.556 s | `qml-offscreen.log` |
| `cargo test --locked --test f1b2_conversation qml_ -- --ignored --nocapture --test-threads=1` | 0 | test result: ok. 3 passed; 0 failed; 0 ignored; 0 measured; 20 filtered out; finished in 7.02s | 7.467 s | `qml-f1b2-review.log` |

A no tiene todavía los seis unitarios nuevos ni el target F1B3A: **NOT_PRESENT**, no pruebas fallidas ni ignoradas. El rerun Piper de A (12 PASS) ya está incluido en sus 150 unitarios; no se suma dos veces. B reproduce todos los gates finales y sus 14 cortes SIGKILL (10 integración + 4 unitarios).

IGNORED ordinarios: montajes F1A, Qt F1B1/F1B2 y dos workers. Qt se ejecuta aparte con 1 + 3 PASS; los workers son invocados por padres supervisados y terminados con SIGKILL, no PASS independientes. FILTERED seguros: dos unitarios con red/descarga, NOT_TESTED; los filtros de los opt-ins sólo seleccionan sus casos Qt. Plasma, voz/hardware, proveedores reales, power loss y fallos físicos/VFS: NOT_TESTED.

## Prueba del parche completo

`F1B3A-CHANGES.patch` = `git diff --binary 27f2db9430bcbc7c074472383fc8acbc142b5f89 d8a6972b5bc3977408c8d0956fce3196623d5458`. **135554 bytes**, SHA256 `81b7a6da179824ab2b058ef9b21cf6fa21a908ef660aabe291bfe4f95d8579b6`.
Contiene los 15 archivos de A/B y sus archivos nuevos; excluye los 17 documentos históricos sin versionar, datos de usuario, credenciales, DB, logs, cachés, builds y estos informes posteriores autorreferentes.

Se exportó el baseline limpio a `/tmp/kda-f1b3a-review-rz85yxmf/patch-baseline`, se ejecutaron `git apply --check` y `git apply`: **exit 0** ambos. Comparación byte a byte de los **224 archivos** de HEAD, y construcción de índice nuevo incluyendo sus modos/rutas: árbol aplicado `5f724bb939fc0e95d4fe197b2e519b8f5e02b6fd` = árbol final `5f724bb939fc0e95d4fe197b2e519b8f5e02b6fd`. No se requiere ningún documento sin versionar para aplicar el parche o compilar.

Evidencia auxiliar privada: commit-a.json, commit-b.json, manifests SHA256, staged diffs, regresiones-before y patch-validation.json en `/tmp/kda-f1b3a-review-rz85yxmf/`. No se incluyen logs privados en la entrega.

## Estado final

Tracked limpio; índice vacío. Sólo permanecen sin versionar los 17 documentos históricos conservados más F1B3A-CLOSURE.md, F1B3A-COMMIT-VALIDATION.md y F1B3A-CHANGES.patch. Sus hashes históricos coinciden con el inventario inicial. Cargo.toml/Cargo.lock sin cambios. No tercer commit.
