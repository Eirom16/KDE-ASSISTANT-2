# F1B-1 — Resultados verificables

Fecha: 2026-10-10. Base obligatoria: `ca82e722935e8076f526a484655f2f95f87b5d4c`. Rama: `recovery/f1b1-text-startup`. Resultado del alcance de texto: **IMPLEMENTED / READY_FOR_REVIEW**. Integración gráfica KDE Plasma Wayland: **NOT_TESTED**.

## Método

Ejecutado `python3 scripts/validate_f1b1.py --qml`, exit **0**. Cada comando usa XDG_CONFIG_HOME, XDG_DATA_HOME, XDG_CACHE_HOME y TMPDIR privados, umask 077, claves de entorno retiradas y Cargo offline con lock. F1B1 reejecuta cada caso con mutación de entorno en un proceso aislado, nunca dentro del runner compartido. SQLite es real y temporal; los proveedores son mocks Axum en loopback, con credenciales ficticias. No se instala ni simula Piper. La UI usa el módulo privado real qml.auth. Los fixtures se eliminan; se conservan logs y results.json.

La evidencia final está en `/tmp/kda-f1b1-check-gqxb7pa7/` (0700). Sus rutas temporales no son entregables permanentes; el script y las pruebas permiten reproducirlas. El puerto 8765 debe estar libre para los tests del ejecutable y QML. Si ya hay un servicio, el fixture falla sin cerrarlo. El resto del mock/backend usa puertos efímeros con Host explícito de la autoridad de producción.

## Comandos finales

Tiempos de pared incluyen compilación; no son métricas de rendimiento del producto.

| Comando | Exit | Resultado | Segundos |
| --- | --- | --- | --- |
| `cargo fmt --check` | 0 | PASS | 0.575 |
| `cargo check --locked --all-targets` | 0 | PASS | 4.903 |
| `cargo clippy --locked --all-targets -- -D warnings` | 0 | PASS, sin warnings | 5.795 |
| `cargo test --locked --test f1a_security` | 0 | 19 passed; 0 failed; 1 ignored | 21.41 |
| `cargo test --locked --test f1b1_text_startup -- --test-threads=1` | 0 | 13 passed; 0 failed; 1 ignored (Qt opt-in) | 6.565 |
| `cargo test --locked --lib -- --test-threads=1 --skip backend::wakeword_ml::tests::download_and_extract_onnx_runtime --skip backend::tool_executor::tests::show_image_huge_url_is_rejected_before_download` | 0 | 137 passed; 0 failed; 2 filtered out | 15.398 |
| `cargo test --locked --lib -- --exact --test-threads=1 <12 nombres abajo>` | 0 | 12 passed; 0 failed; 127 filtered out | 1.543 |
| `cargo test --locked --test app_paths` | 0 | 2 passed; 0 failed | 1.85 |
| `cargo test --locked --test f1b1_text_startup qml_offscreen -- --ignored --nocapture` | 0 | 1 passed; 0 failed; 13 filtered out; binario y Main.qml offscreen | 3.115 |

También se ejecutó `python3 scripts/validate_f1a.py --mounts`, exit **0**, con la selección F1A original: 125 unitarios seguros aprobados y 14 filtrados, 19 tests de seguridad aprobados/1 ignored por defecto, 2 app_paths aprobados. El opt-in `search_mount_boundaries_remain_closed` pasó: **1 passed; 0 failed; 19 filtered out** en namespace privado. Evidencia: `/tmp/kda-f1a-check-icwyxbdp/`. Es la misma prueba reejecutada en hijo y padre; no cuenta como dos. La ejecución final F1B1 sobre los últimos cambios aprobó otra vez los 19 tests de seguridad y los cinco unitarios F1A incluidos en los 137.

No se ejecutó ni se declara aprobada `cargo test --all-targets` completa.

## Catorce casos F1B1 únicos

| Caso | Evidencia y resultado |
| --- | --- |
| `first_run_without_piper_whisper_onnx_or_wake` | Config nueva restrictiva; todas las tools desactivadas, allowlist vacía, voz/memoria/personaje apagados; captura None, Piper sin inicializar, Whisper ausente, ML sin cargar; sin directorios models/lib. PASS. |
| `existing_configuration_is_preserved` | Conserva bytes, clave, modelo Groq anterior, flag de herramienta y wake habilitado; ningún servicio de audio/ML se activa al crear Backend. PASS. |
| `corrupt_configuration_is_recoverable_and_never_replaced` | Error explícito; JSON permanece intacto y no se crea DB; corregir el fixture permite reintentar. PASS. |
| `missing_api_key_never_contacts_provider` | UI/API informa has_api_key=false; el chat SSE devuelve error; mock registra cero peticiones. PASS. |
| `invalid_tokens_fail_closed` | Bearer esperado vacío rechazado; API 401; serve rechaza token; bootstrap rechaza server.token existente vacío sin sobrescribirlo. PASS. |
| `hostile_authorities_are_rejected_over_http` | Ocho Hosts y nueve Origins preparados rechazados con 403; null y autoridades loopback completas admitidas; ausencia de Host rechazada. PASS. |
| `private_files_redaction_and_failed_update_preserve_secrets` | Config/backup/token/QML 0600; directorio QML 0700; GET no devuelve secretos; guardar vista redactada conserva claves; backup symlink provoca 500, conserva RAM/disco y archivo exterior. PASS. |
| `exclusive_bootstrap_only_tolerates_an_existing_winner` | Publicación exclusiva real conserva ganador sin temporales sobrantes; helper rechaza EIO inyectado incluso con destino existente. No inyecta syscall fsync. PASS. |
| `redacted_updates_preserve_legacy_and_provider_credentials` | Mapa activo vacío o diferente de legacy, cambio de proveedor, guardado/relectura de disco y reemplazo explícito: claves preservadas y resolución correcta. PASS. |
| `model_catalog_uses_selected_provider_credentials_and_url` | Dos mocks de catálogo: la clave del proveedor seleccionado sólo llega a su URL; el mock anterior recibe cero peticiones. PASS. |
| `two_turns_with_mock_and_persistence_after_backend_restart` | Health 200, configura proveedor por API, crea sesión, SSE token/done en dos turnos, cuatro mensajes reales; segundo request contiene el primer intercambio; tras destruir servidor/backend y recrearlos, mismos mensajes y token. PASS. |
| `startup_has_no_download_or_global_process_cleanup_calls` | Guard estático de los entrypoints de descarga/prewarm y limpieza global eliminados. Se complementa con ausencia de models/lib en arranque dinámico. PASS. |
| `real_binary_lifecycle_and_critical_failure_leave_no_listener_or_foreign_victim` | Ejecutable real: puerto ocupado no crea config; corrupción y token inválido terminan con errores en stderr y no dejan listener; health exige 200 y SIGINT funciona; qml6 ausente termina con error y libera puerto; guard termina/recoge hijo propio; proceso independiente del fixture sobrevive. PASS. |
| `qml_offscreen_settings_and_two_visible_turns` | Opt-in ejecutado: Main.qml muestra falta de clave; SettingsDialog guarda proveedor/modelo/key ficticia; crea sesión y muestra first/reply-one/second/reply-two. Después lanza el ejecutable real con su UI: permanece vivo, recoge su QML y libera 8765 con SIGINT, conserva vivo otro qml6/Main.qml del fixture. Identifica argv del hijo; una UI con error real de carga también termina backend/libera 8765. PASS offscreen. |

Los doce casos predeterminados reejecutados en hijos y el harness Qt no se cuentan dos veces por mostrar resultado hijo/padre. Son **13 tests predeterminados más 1 opt-in Qt**; el guard estático no necesita hijo.

Marcadores Qt finales:

```text
F1B1_UI_PASS; QML terminado y recogido; cuatro mensajes persistidos
F1B1_BINARY_UI_PASS; hijo QML propio recogido; QML ajeno sigue vivo; puerto liberado
F1B1_QML_FAILURE_PASS; fallo real de carga QML visible; puerto liberado; QML ajeno vivo
```

## Piper heredado y exclusiones

Estos **12 nombres exactos**, que en F1A fallaban todos por constructor obligatorio de Piper, ahora pasan sin instalarlo:

```text
backend::http_server::tests::dictate_roundtrip_empty_transcript_releases_flag
backend::http_server::tests::dictate_start_conflicts_with_voice_pipeline
backend::http_server::tests::dictate_stop_without_start_conflicts
backend::speech_service::tests::piper_engine_initializes
backend::voice_pipeline::tests::auto_barge_ignora_mic_alto_si_tts_aun_no_suena
backend::voice_pipeline::tests::barge_in_transita_interrupted_a_listening
backend::voice_pipeline::tests::dictado_activo_aborta_wait_for_speech_rapido
backend::voice_pipeline::tests::dictado_activo_bloquea_start_listening
backend::voice_pipeline::tests::emit_state_mapea_a_strings_legados
backend::voice_pipeline::tests::signal_broadcast_state
backend::voice_pipeline::tests::signal_level_throttled
backend::voice_pipeline::tests::wait_for_speech_times_out_quiet
```

El nombre heredado `piper_engine_initializes` comprueba ahora el constructor diferido de SpeechService; no acredita sintetizar con un motor real. Los otros once comprueban controladores, exclusión de dictado, señales y estados; no necesitan modelos ni hardware. Todos están incluidos en los 137 unitarios seguros y se repitieron aparte para verificar el grupo histórico. `command -v piper-tts` y `command -v piper` devolvieron exit 1 en este entorno. Whisper/openWakeWord se prueban con XDG_DATA_HOME vacío y ORT_DYLIB_PATH apuntando a un archivo inexistente. No se consultaron ni modificaron modelos personales.

Permanecen excluidos exactamente estos dos tests de la selección unitaria:

- `backend::wakeword_ml::tests::download_and_extract_onnx_runtime`: puede descargar una biblioteca nativa.
- `backend::tool_executor::tests::show_image_huge_url_is_rejected_before_download`: conserva la exclusión externa de F1A.

No se ejecutaron los opt-in `tests/tts_stream_smoke.rs::speak_streaming_reproduce_oraciones_en_orden` (Piper, voz y salida de audio) ni `tests/wake_resample_sim.rs::resample_44100_vs_directo` (runtime y modelos ONNX/audio). `tests/agent_smoke.rs::chat_with_openrouter` requiere proveedor remoto y no acredita esta fase. Los binarios test_voice/test_stt/test_wakeword tampoco se ejecutaron. **Voz real, micrófono, modelos instalados, proveedores remotos y Plasma Wayland: NOT_TESTED.**

## Correcciones durante validación y límites

La primera selección nueva dio 7 passed / 3 failed: encontró una carrera de SIGINT antes del registro del handler, un caso de conservación de clave legacy junto a mapa parcial y una expectativa incorrecta de tools vacío frente a campo omitido por serde. Se corrigieron los dos problemas de implementación y la expectativa. El primer harness Qt falló por import absoluto y luego por usar una autoridad efímera que el nuevo control Host rechazaba; se corrigió el fixture a URI file y puerto real 8765. Clippy detectó dos guards de tests sobre await; se acotaron por bloques. Los resultados finales de la tabla proceden del árbol corregido.

El mock SSE usa texto ASCII y eventos completos. No verifica UTF-8 fragmentado ni cancelación profunda. REL-03/REL-04 siguen pendientes para F1B-2. No hay sandbox de egress ni captura general de tráfico: la evidencia de ausencia de descarga combina entrypoints eliminados, constructores diferidos y filesystem vacío después del arranque. Las peticiones de conversación verificadas sólo van a loopback. No se presenta un smoke offscreen como validación del compositor, bandeja, atajos o overlay reales.

## Consolidación local posterior

La tabla corresponde a la repetición tras corregir los defectos de bootstrap, conservación de credenciales y catálogo detectados durante revisión. Los SHA, árboles staged exactos compilados sobre sus padres y la validación final de los commits se registran en F1B1-COMMIT-VALIDATION.md. No se amplía esta evidencia a fallos reales de fsync ni Plasma Wayland.
