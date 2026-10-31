# F1B-2 — Validación de commits locales

Fecha: 2026-10-10. Resultado: **CONSOLIDATED**, revisión satisfactoria del código real. Sin push, merge, release ni F1B-3.

## Identidad exacta

| Operación | Commit | Padre | Árbol validado antes de commit |
| --- | --- | --- | --- |
| Baseline F1B1 | `600a363ee3a9d64d9ea2adbdbda5448efacc7e8a` | `ddb3bce7610c1e770ab1d90c21e1050b3e37c40e` | Sin modificación |
| A — motor | `74b2ca4f5c80c057f6403c68cef30dd0950d8a4c` | `600a363ee3a9d64d9ea2adbdbda5448efacc7e8a` | `1f97039c5f125412be4594579d59c71b5bb815ba` |
| B — tests/documentación | `27f2db9430bcbc7c074472383fc8acbc142b5f89` | `74b2ca4f5c80c057f6403c68cef30dd0950d8a4c` | `771abb173c5e924df55aa7428228d8a1a4eb308c` |

Rama: `recovery/f1b2-conversation-engine`. `recovery/f1b1-text-startup` sigue en baseline. `git rev-list baseline..HEAD` contiene exactamente A/B. No se usaron reset, clean, stash ni sobrescritura de documentos ajenos.

A se exportó desde `git write-tree` mediante `git archive`; B también, antes de crear cada commit. A no contiene tests/f1b2_conversation, fixtures f1b2 ni script F1B2; compila sin esos archivos. A incluye la adaptación de patrón de agent_smoke y el fixture unitario heredado de HTTP/SQLite necesarios para all-targets. B añade sólo regiones de tests a cinco fuentes (incluyendo hooks cfg(test)), sin cambios de producción respecto a A. Las proyecciones sin los nuevos tests se compararon con A; los hooks de tool_executor fueron inspeccionados individualmente como cfg(test).

Las copias sólo tienen archivos del árbol Git; se compartió Cargo target como caché de compilación, no como fuente. Configuración, SQLite y mocks son privados. Cada script retiró claves y proxies, usó XDG/TMPDIR privados, umask 077 y Cargo offline. Se verificó el mismo hash de árbol antes y después de cada commit. Directorio de trabajo de evidencia: `/tmp/kda-f1b2-consolidation-4nbkqwpe`.

## Gates del árbol A

`python3 scripts/validate_f1b1.py --qml`, **exit 0**, sobre `/tmp/kda-f1b2-consolidation-4nbkqwpe/tree-a`. Script heredado del baseline (su campo base histórico no identifica este árbol; la identidad comprobada es la tabla anterior). Evidencia: `/tmp/kda-f1b1-check-zzrvgpxs`.

| Comando exacto | Exit | Resultado del runner padre | Tiempo del comando | Log |
| --- | --- | --- | --- | --- |
| `cargo fmt --check` | 0 | exit 0 | 0.644 s | `/tmp/kda-f1b1-check-zzrvgpxs/fmt.log` |
| `cargo check --locked --all-targets` | 0 | exit 0 | 11.106 s | `/tmp/kda-f1b1-check-zzrvgpxs/check.log` |
| `cargo clippy --locked --all-targets -- -D warnings` | 0 | exit 0 | 14.246 s | `/tmp/kda-f1b1-check-zzrvgpxs/clippy.log` |
| `cargo test --locked --test f1a_security` | 0 | ok. 19 passed; 0 failed; 1 ignored; 0 measured; 0 filtered out; finished in 0.14s | 46.734 s | `/tmp/kda-f1b1-check-zzrvgpxs/f1a-security.log` |
| `cargo test --locked --test f1b1_text_startup -- --test-threads=1` | 0 | ok. 13 passed; 0 failed; 1 ignored; 0 measured; 0 filtered out; finished in 0.82s | 8.572 s | `/tmp/kda-f1b1-check-zzrvgpxs/f1b1.log` |
| `cargo test --locked --lib -- --test-threads=1 --skip backend::wakeword_ml::tests::download_and_extract_onnx_runtime --skip backend::tool_executor::tests::show_image_huge_url_is_rejected_before_download` | 0 | ok. 134 passed; 0 failed; 0 ignored; 0 measured; 2 filtered out; finished in 1.59s | 33.159 s | `/tmp/kda-f1b1-check-zzrvgpxs/unit-safe.log` |
| `cargo test --locked --lib -- --exact --test-threads=1 backend::http_server::tests::dictate_roundtrip_empty_transcript_releases_flag backend::http_server::tests::dictate_start_conflicts_with_voice_pipeline backend::http_server::tests::dictate_stop_without_start_conflicts backend::speech_service::tests::piper_engine_initializes backend::voice_pipeline::tests::auto_barge_ignora_mic_alto_si_tts_aun_no_suena backend::voice_pipeline::tests::barge_in_transita_interrupted_a_listening backend::voice_pipeline::tests::dictado_activo_aborta_wait_for_speech_rapido backend::voice_pipeline::tests::dictado_activo_bloquea_start_listening backend::voice_pipeline::tests::emit_state_mapea_a_strings_legados backend::voice_pipeline::tests::signal_broadcast_state backend::voice_pipeline::tests::signal_level_throttled backend::voice_pipeline::tests::wait_for_speech_times_out_quiet` | 0 | ok. 12 passed; 0 failed; 0 ignored; 0 measured; 124 filtered out; finished in 1.17s | 1.879 s | `/tmp/kda-f1b1-check-zzrvgpxs/piper-inherited-now-lazy.log` |
| `cargo test --locked --test app_paths` | 0 | ok. 2 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s | 2.546 s | `/tmp/kda-f1b1-check-zzrvgpxs/app-paths.log` |
| `cargo test --locked --test f1b1_text_startup qml_offscreen -- --ignored --nocapture` | 0 | ok. 1 passed; 0 failed; 0 ignored; 0 measured; 13 filtered out; finished in 2.84s | 3.275 s | `/tmp/kda-f1b1-check-zzrvgpxs/qml-offscreen.log` |

A: F1B2 nuevos **NOT_PRESENT**, no se cuentan como PASS. Se ejecutaron las regresiones disponibles, incluida Qt heredada. 134 unitarios seguros: se retiran tres tests de helpers obsoletos del baseline y todavía no existen los 16 nuevos de B. Dos externos filtered out, 1 ignored de seguridad y 1 QML ignored en F1B1 ordinario. QML se ejecutó por separado y pasó 1.

## Gates del árbol B

`python3 scripts/validate_f1b2.py --qml`, **exit 0**, sobre `/tmp/kda-f1b2-consolidation-4nbkqwpe/tree-b`. Evidencia: `/tmp/kda-f1b2-check-s6e3td5o`. Incluye todos los comandos obligatorios y selección unitaria segura.

| Comando exacto | Exit | Resultado del runner padre | Tiempo del comando | Log |
| --- | --- | --- | --- | --- |
| `cargo fmt --check` | 0 | exit 0 | 0.686 s | `/tmp/kda-f1b2-check-s6e3td5o/fmt.log` |
| `cargo check --locked --all-targets` | 0 | exit 0 | 12.124 s | `/tmp/kda-f1b2-check-s6e3td5o/check.log` |
| `cargo clippy --locked --all-targets -- -D warnings` | 0 | exit 0 | 12.865 s | `/tmp/kda-f1b2-check-s6e3td5o/clippy.log` |
| `cargo test --locked --test f1a_security` | 0 | ok. 19 passed; 0 failed; 1 ignored; 0 measured; 0 filtered out; finished in 0.12s | 42.356 s | `/tmp/kda-f1b2-check-s6e3td5o/f1a-security.log` |
| `cargo test --locked --test f1b1_text_startup -- --test-threads=1` | 0 | ok. 13 passed; 0 failed; 1 ignored; 0 measured; 0 filtered out; finished in 0.77s | 7.901 s | `/tmp/kda-f1b2-check-s6e3td5o/f1b1.log` |
| `cargo test --locked --test f1b2_conversation -- --test-threads=1` | 0 | ok. 20 passed; 0 failed; 3 ignored; 0 measured; 0 filtered out; finished in 1.95s | 10.426 s | `/tmp/kda-f1b2-check-s6e3td5o/f1b2-conversation.log` |
| `cargo test --locked --lib -- --test-threads=1 --skip backend::wakeword_ml::tests::download_and_extract_onnx_runtime --skip backend::tool_executor::tests::show_image_huge_url_is_rejected_before_download` | 0 | ok. 150 passed; 0 failed; 0 ignored; 0 measured; 2 filtered out; finished in 3.80s | 33.899 s | `/tmp/kda-f1b2-check-s6e3td5o/unit-safe.log` |
| `cargo test --locked --lib -- --exact --test-threads=1 backend::http_server::tests::dictate_roundtrip_empty_transcript_releases_flag backend::http_server::tests::dictate_start_conflicts_with_voice_pipeline backend::http_server::tests::dictate_stop_without_start_conflicts backend::speech_service::tests::piper_engine_initializes backend::voice_pipeline::tests::auto_barge_ignora_mic_alto_si_tts_aun_no_suena backend::voice_pipeline::tests::barge_in_transita_interrupted_a_listening backend::voice_pipeline::tests::dictado_activo_aborta_wait_for_speech_rapido backend::voice_pipeline::tests::dictado_activo_bloquea_start_listening backend::voice_pipeline::tests::emit_state_mapea_a_strings_legados backend::voice_pipeline::tests::signal_broadcast_state backend::voice_pipeline::tests::signal_level_throttled backend::voice_pipeline::tests::wait_for_speech_times_out_quiet` | 0 | ok. 12 passed; 0 failed; 0 ignored; 0 measured; 140 filtered out; finished in 1.15s | 1.855 s | `/tmp/kda-f1b2-check-s6e3td5o/piper-inherited-now-lazy.log` |
| `cargo test --locked --test app_paths` | 0 | ok. 2 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s | 2.203 s | `/tmp/kda-f1b2-check-s6e3td5o/app-paths.log` |
| `cargo test --locked --test f1b1_text_startup qml_offscreen -- --ignored --nocapture` | 0 | ok. 1 passed; 0 failed; 0 ignored; 0 measured; 13 filtered out; finished in 2.84s | 3.256 s | `/tmp/kda-f1b2-check-s6e3td5o/qml-offscreen.log` |
| `cargo test --locked --test f1b2_conversation qml_ -- --ignored --nocapture --test-threads=1` | 0 | ok. 3 passed; 0 failed; 0 ignored; 0 measured; 20 filtered out; finished in 6.88s | 7.289 s | `/tmp/kda-f1b2-check-s6e3td5o/qml-f1b2-review.log` |

B: 20 casos HTTP passed, 3 Qt ignored en la selección ordinaria; esos 3 se ejecutaron por separado y passed. Unitarios 150 passed/2 filtered. F1A 19 passed/1 ignored; F1B1 13 passed/1 ignored y su Qt aparte 1 passed/13 filtered. Piper 12 son un subconjunto ya incluido en los unitarios; no se suman dos veces. App_paths 2 passed. No se ejecutaron mounts opt-in, las dos pruebas externas filtradas, smokes reales, micrófono, modelos/descargas, Plasma Wayland ni TTS real. Ignorados/filtrados/no presentes no se presentan como PASS.

## Manifiestos exactos

Hashes SHA256 de bytes, junto con los árboles Git anteriores. Archivos repetidos entre A/B reciben en B únicamente tests. Históricos de recuperación excluidos.

### A — 11 archivos

| Archivo | SHA256 en A |
| --- | --- |
| `qml/Main.qml` | `63485f186944ed8b697522ced2d8e71c2cc4061e3a6c08cc0d913aa8c2a2348a` |
| `src/backend/ai_service.rs` | `8726a74e7d61406e1cdde7c58d71dcd3ea27756c271b6d97eb504c0992f2c383` |
| `src/backend/approvals.rs` | `27ca19bd73e76dee8dbac3ef9595c553fe40fa6b04a681cc428b90db373e974a` |
| `src/backend/conversation_turn.rs` | `0937c9e10497e936aa2c554e216102610b38971b93e562420c14cb8ab098adfa` |
| `src/backend/http_server.rs` | `00e23008355b2b73a385d27948eb8fff3db7060fa568600bfec8e378a5640f83` |
| `src/backend/mod.rs` | `788720adb238f8b5cb54f23a39c460bc89d6b04e5e337d69f1f9f71839a1fd59` |
| `src/backend/session_manager.rs` | `44af2f7541bc6f66fd4cc65a544a8108f1a9d0e6b82f4c81876d417d59e584a2` |
| `src/backend/sse_parser.rs` | `f6143bfecb775a78702759aaeebe3efa2d003a8662f440ac7f48138523039cd0` |
| `src/backend/tool_executor.rs` | `44bf44708908cf7b46e0595d1590c5c2beb272f855ec15ada0aba9f516f20fd0` |
| `src/models/message.rs` | `3a273dd5010783c2d695bb96bc651d976e9fd1a094ebc02f3c94bdca3877ddb5` |
| `tests/agent_smoke.rs` | `e9ba43bad47261e6a63285799a768f2369496573e889caf10076b8a0268612d9` |

### B — 14 archivos

| Archivo | SHA256 en B |
| --- | --- |
| `docs/recovery/F1B2-CONTRACT.md` | `c3fdd9cccdbadf3d8a43ff93f164cdafe4833c8eebe72f320f5f91cfc6a94aba` |
| `docs/recovery/F1B2-REPORT.md` | `47b7a82d17245fc8674e5c21e204c2e774ec0c024a91a88190264f7e0f374cd3` |
| `docs/recovery/F1B2-RESULTS.md` | `8514222ed34e52ec79fe3195dc9b40403b4029d65e433e8c2661dbb3cba527fc` |
| `scripts/validate_f1b2.py` | `b0574b76fde413de09c6f69ca40230c311d26c9d7419f3a0dea4d50944ab833e` |
| `src/backend/conversation_turn.rs` | `827df529cc0202ef1824d515ab70c8ddf5d699269c9331b025b3104a526d670e` |
| `src/backend/http_server.rs` | `711867d1d9787fa8b82fe57b213c7fecc29b2472f9903e4c8c2406af4fa85a85` |
| `src/backend/session_manager.rs` | `8c122bdb2ea226850a26d9a3d4a82fd6beecee4bca2afc0f17262acdb3cd726c` |
| `src/backend/sse_parser.rs` | `c2f646c724523418e777133491e8b652b23b074b54286d97b7b9ed4f58b4a597` |
| `src/backend/tool_executor.rs` | `5f55d89bf7af396d60d3da303f12617394d23f3efbb98319bbf66ed0edc3b336` |
| `tests/f1b2_conversation.rs` | `b39394e17201505e2132baf5bb0f786baafaa538766d19042f2d6ee849f2f9ee` |
| `tests/fixtures/f1b2/groq.sse` | `b950d8ae272b019785c494de584d7d50166ba9fe24eb26a7c63f77237b1f588a` |
| `tests/fixtures/f1b2/openai.sse` | `7b894c9803896d0e6c23fc536caa2a46f777ea0042010d381d4731118edbf47b` |
| `tests/fixtures/f1b2/openrouter.sse` | `903cc9101d8ef58ff657d7cc6860c84b88f6f56d7d20e956f603f7d751c74f42` |
| `tests/qml/F1B2Review.qml` | `4ce8a16b7bee0e896e23e92eae9223202589643c9a04ae7e30082e2f7f62731a` |

## Revisión del diff y anomalías registradas

- Manifiestos staged exactos, padres/ramas/árboles comprobados; 20 archivos únicos entre A/B. Producción inspeccionada para ownership, órdenes de locks, arbitraje SQLite/cancel, lifecycle de approvals, parsers/proveedores y callers QML. Sin ampliación de dependencias: Cargo.toml/lock idénticos al baseline.
- Inspección manual de literales/fixtures y búsqueda de patrones de claves sk-, AIza y bloques PRIVATE KEY: 0 coincidencias sensibles en los diffs staged. Claves/IDs de fixtures son sintéticos y usan loopback; no se incluyeron configs, tokens privados, DB ni logs. No se afirma que un regex sea un detector universal de secretos.
- Primer intento de commit A: **exit 128**, Author identity unknown, HEAD intacto. Se comprobó que los tres commits anteriores usan Codex <codex@localhost>; ambos commits nuevos usan esa identidad mediante `git -c user.name=Codex -c user.email=codex@localhost`, sin modificar configuración Git local/global o personal.
- Primer `git diff --cached --check` de B: **exit 2**, únicamente por LF LF al EOF de los tres fixtures SSE. No se borraron esos terminadores del protocolo. `git -c core.whitespace=-blank-at-eof diff --cached --check`: exit 0. La preparación inicial de B se interrumpió antes de exportar/validar y el wrapper posterior devolvió KeyError por tree_b_path ausente; no se ejecutó ningún gate de ese intento. La exportación completa y sus 11 gates posteriores sí pasaron.
- Fallos reproducidos, pruebas inicialmente fallidas y errores del harness se conservan y separan en F1B2-RESULTS. La ejecución original aprobada no sustituyó la auditoría: ésta reprodujo siete defectos y corrigió cada uno con regresión. No se ocultaron fallos ni se amplió selección a proveedores reales.

## Parche reproducible

[F1B2-CHANGES.patch](F1B2-CHANGES.patch) es exactamente `git diff --binary --full-index 600a363ee3a9d64d9ea2adbdbda5448efacc7e8a 27f2db9430bcbc7c074472383fc8acbc142b5f89`: **20 archivos**, incluidos nuevos de producción/tests/fixtures/script/los tres informes. SHA256 `10c2d45640c04bcb5ccd6697ccaa1b90608a5cb6be2dd3d9c366f1f6b16dc235`, **254505 bytes**.

`git apply --check` sobre copia limpia del baseline: **exit 0**, sin stderr. Aplicación real: **exit 0**, con tres warnings de línea vacía EOF de fixtures SSE, conservadas como protocolo. Comparación byte a byte: los 20 archivos cambiados coinciden con B y todos los **217 archivos del árbol completo** coinciden; no hay archivos añadidos ajenos ni pérdida de baseline. Evidencia `/tmp/kda-f1b2-consolidation-4nbkqwpe/patch-validation.json`.

Se excluyen el parche mismo, CLOSURE y COMMIT-VALIDATION (evidencia posterior a commits, evitando autorreferencia), los 14 documentos históricos sin versionar, datos/configs/credenciales/logs/modelos y artefactos de compilación. Los tres entregables de cierre quedan disponibles como archivos sin versionar; no hay tercer commit. El árbol tracked/staged final está limpio; sólo esos tres entregables y los 14 históricos permanecen untracked.
