# F1B-3B — Validación de los commits locales

Fecha: 2026-10-13. **PASS / CONSOLIDATED**. Baseline `d8a6972b5bc3977408c8d0956fce3196623d5458`; rama `recovery/f1b3b-history-retention`. Recibo posterior a los commits, sin versionar por autorreferencia; complementa los tres informes versionados.

## Proveniencia exacta

| Caso | Commit final | Árbol obtenido con git write-tree antes del commit | Archivo Git SHA256 | Evidencia de gates |
| --- | --- | --- | --- | --- |
| A | `b07240f936b0abde098c42d1c32a0189955026b2` | `ccd12e739b70f1fc850b11c8157b06913c8521cc` | `7aa64ac536a9fee7962c481579543a0f8fb6afa03962f72f2028bb4e6dc7b186` | `/tmp/kda-f1b3a-check-qz16x0jv` |
| B | `e00fc1f77322b8fd1dace4c9c4858327a141f82a` | `2ee7dd18e778b10a8344aa42bef7eb61f11fd82b` | `2e31e167c46dfa8ce48034584c6aed59d1a3c0757e86d10d268e07c7715ac6bf` | `/tmp/kda-f1b3b-check-jipsj57o` |

Antes de cada commit: paths staged exactos, diff binario completo revisado, git diff --cached --check PASS, patrón de credenciales sin matches y revisión de literals/fixtures. Sin credenciales personales en el diff; los tokens de fixtures son ficticios. Se hizo git archive del tree staged, se extrajo en directorio 0700 privado y se ejecutó allí, sin superponer fuentes unstaged. Después se compararon los 229 archivos de A y los 236 de B, blobs y bits ejecutables con sus árboles Git; no hubo modificaciones. HEAD tree coincide exactamente con el tree validado para cada commit; padres y rama anterior comprobados.

Archives/manifests/diffs completos: `/tmp/kda-f1b3b-consolidation-f24ll7kk/A.tar`, `B.tar`, `A-staged-manifest.json`, `B-staged-manifest.json`, `A-archive-manifest.json`, `B-archive-manifest.json`, `A-staged.patch`, `B-staged.patch`. Los SHA256 por archivo cambiando están incluidos abajo para que el recibo no dependa sólo de /tmp. El runner F1B3B verificó además su manifiesto de 127 fuentes durante ejecución.

No existe el runner ni suite nuevos en A: se usó **python3 scripts/validate_f1b3a.py --qml**, agrupando todos los 11 gates disponibles. Su campo JSON base es una etiqueta heredada de F1B3A; el baseline y tree de esta auditoría son los del recibo Git anterior, no esa etiqueta. En B se usó exactamente **python3 scripts/validate_f1b3b.py --qml**, 13 gates, con todas las fuentes F1B3B del índice. No se copió el runner nuevo dentro del archivo A ni se fingió ejecutar una suite ausente.

## Resultados por comando

Celdas de tests: **PASS / FAIL / IGNORED / FILTERED**, en ese orden. — es gate sin tests. Los números proceden del resumen final del proceso padre; hijos no sumados.

| Comando exacto | Fuentes finales | Archive staged A | Archive staged B |
| --- | --- | --- | --- |
| `cargo fmt --check` | PASS (exit 0) | PASS (exit 0) | PASS (exit 0) |
| `cargo check --locked --all-targets` | PASS (exit 0) | PASS (exit 0) | PASS (exit 0) |
| `cargo clippy --locked --all-targets -- -D warnings` | PASS (exit 0) | PASS (exit 0) | PASS (exit 0) |
| `cargo test --locked --test f1a_security` | PASS 19/0/1/0 | PASS 19/0/1/0 | PASS 19/0/1/0 |
| `cargo test --locked --test f1b1_text_startup -- --test-threads=1` | PASS 13/0/1/0 | PASS 13/0/1/0 | PASS 13/0/1/0 |
| `cargo test --locked --test f1b2_conversation -- --test-threads=1` | PASS 20/0/3/0 | PASS 20/0/3/0 | PASS 20/0/3/0 |
| `cargo test --locked --test f1b3a_persistence -- --test-threads=1` | PASS 26/0/1/0 | PASS 26/0/1/0 | PASS 26/0/1/0 |
| `cargo test --locked --test f1b3b_storage -- --test-threads=1` | PASS 33/0/3/0 | NOT_TESTED: no disponible en A | PASS 33/0/3/0 |
| `cargo test --locked --lib -- --test-threads=1 --skip backend::wakeword_ml::tests::download_and_extract_onnx_runtime --skip backend::tool_executor::tests::show_image_huge_url_is_rejected_before_download` | PASS 156/0/1/2 | PASS 156/0/1/2 | PASS 156/0/1/2 |
| `cargo test --locked --test app_paths` | PASS 2/0/0/0 | PASS 2/0/0/0 | PASS 2/0/0/0 |
| `cargo test --locked --test f1b1_text_startup qml_offscreen -- --ignored --nocapture` | PASS 1/0/0/13 | PASS 1/0/0/13 | PASS 1/0/0/13 |
| `cargo test --locked --test f1b2_conversation qml_ -- --ignored --nocapture --test-threads=1` | PASS 3/0/0/20 | PASS 3/0/0/20 | PASS 3/0/0/20 |
| `cargo test --locked --test f1b3b_storage qml_ -- --ignored --nocapture --test-threads=1` | PASS 2/0/0/34 | NOT_TESTED: no disponible en A | PASS 2/0/0/34 |

A: 236 Rust + 4 Qt PASS; B/fuentes finales: 269 Rust + 6 Qt PASS por ejecución. Cero FAIL finales. IGNORED Qt normales se ejecutan en gates opt-in; workers crash/cache sólo mediante sus padres. Mount namespace F1A IGNORED, dos unitarios externos FILTERED, funcionalidad remota NOT_TESTED. Los FILTERED 13/20/34 de selecciones Qt son tests fuera de ese filtro, no fallos.

XDG_CONFIG_HOME/XDG_CACHE_HOME/XDG_DATA_HOME/TMPDIR privados; umask 077, claves/proxies retirados, Cargo offline, SQLite sintético, proveedores loopback y Qt offscreen. Cargo target compartido sólo como caché de compilación, fuera de los archives; CARGO_MANIFEST_DIR y archivos ejecutados pertenecen al archive correspondiente. No modelos/micrófono/proveedores reales ni borrados personales. No se presenta la selección como sandbox general de egress.

## Correcciones y revisiones de invariantes

Reproducciones con exit 101 antes y exit 0 después: workspace sustituido con publish/cleanup indebidamente aceptados; descarga por pathname que seguía symlink y sobrescribía target sintético. Logs before-/after- y JSON en `/tmp/kda-f1b3b-consolidation-f24ll7kk`. PDF sintético, render de texto/office, limpieza tras fallo, symlink inactivo con recuperación posterior, leases activos, referencias y cuotas PASS. Import de fixture incorrecto y Clippy por import sin uso corregidos; dos unitarios de render adaptados al ownership exigido. Iteraciones fallidas se documentan en F1B3B-RESULTS; no se contabilizan como PASS.

Historial inspeccionado: pending IDs por batch con resultados exactos, causalidad por ID, ventanas completas, system esencial y errores antes de provider/persistencia; intentos conocidos deben estar Completed. NULL legacy intactos, incertidumbre no convertida en resultado exitoso. Herramientas, entrada y max_tokens forman presupuesto; charge usa writer contador con multiplicación/suma saturadas y input_budget valida respuesta antes de restar. Ventana u32 limita el presupuesto efectivo en este target Linux de 64 bits. SQL octet_length evita materializar payload overflow antes de su guard; SUM SQLite que no puede representarse devuelve error. Strings escapadas se cargan conservadoramente dos veces; no se certifica tokenizer/template remoto ni se asume compatibilidad ilimitada de configuraciones/modelos custom.

Retención inspeccionada: BEGIN IMMEDIATE, checks antes de DELETE, dependientes antes de padres, parametrización exacta, guards, consentimiento y rollback. Registry se mantiene durante borrado; no hay callback inverso SQLite -> Registry en callers inspeccionados. Scheduler conserva globals. Recursos se fijan por descriptor e identidad, se crean con EXCL y se reclaman antes de borrar; rechazo conservador de ownership desconocido. API/QML muestran allowlists de metadata, redacción sólo en respuesta de audit, páginas reemplazadas de 50 y generaciones contra callbacks tardíos. No replay ni reintento automático nuevo.

## Parche y equivalencia

Diff Git binario/full-index `d8a6972b5bc3977408c8d0956fce3196623d5458` -> `e00fc1f77322b8fd1dace4c9c4858327a141f82a`, exclusivamente 26 archivos; SHA256 `b44a3910eefe2d8b15ddafffa7852faf20cd79bd1ddcb9aa9793d23fe7033ad8`, 193463 bytes. git apply --check PASS sobre archive limpio de baseline. Aplicación real PASS, 236 contenidos SHA256 iguales a archive B. Con GIT_INDEX_FILE/GIT_WORK_TREE privados se reconstruyó tree `2ee7dd18e778b10a8344aa42bef7eb61f11fd82b`, idéntico a HEAD. No reset, checkout de ramas previas ni mutación del índice real durante esa comprobación.

## Estado final y NOT_TESTED

Tracked/staged limpios; 20 documentos históricos sin versionar idénticos a su snapshot inicial. Sólo se añadieron los tres entregables post-commit sin versionar (cierre, validación, parche). No push/merge/release/F2/F3. Plasma/tray/voz real, tokenizadores/proveedores externos, procesos QML huérfanos, garantías contra escritores arbitrarios del mismo UID, quota física/global, mount namespace y fallos físicos/forenses: NOT_TESTED o fuera de la garantía. Véase F1B3B-CLOSURE para aceptación real pendiente.

### Manifest staged A

| Archivo | Blob Git | SHA256 del contenido |
| --- | --- | --- |
| `migrations/004_storage_retention.sql` | `1eff673a64864318d72f54676997cc6b4b9f0e59` | `1c6bbdd5e86059d85301ca8edb570f7f1d816017ad96d96b8106443272ba9f2b` |
| `qml/Main.qml` | `73fcc03490ef1e5d9da7844b744959428273c6e8` | `e94c0f3e1d4413e7b797f432793a8a40c048b625a7e45374bdd151adb7f25937` |
| `qml/components/RecoveryHistory.qml` | `499396a871fbdb5617d9826a2e0a89cbd824910a` | `22deb4e686010c3ff0bab07c6f64a98bbdb42c288aef7548c1d1cb551804158a` |
| `qml/qmldir` | `fbf449a43c53750a72e64cc3a903355f6c9d6a6a` | `2d54626f32aa96cb3ed48689069af6140666e3a3399f3762a09ee3b073e1facd` |
| `src/backend/ai_service.rs` | `99ab37b8de00fc921421e92cde9ab8e4986f45e7` | `d6595a912ec7952310c9c5a3cbfd799204cede47f2938e3caed0f39c88211915` |
| `src/backend/conversation_turn.rs` | `a0d43260f191b1329314b2e48efb6613a787ae81` | `d67a1870b4cf646d1b07c6594ff006298ce6a12c70586e5610c26aedd9ea3b68` |
| `src/backend/history_context.rs` | `cc5cb48812968a443a99206df4027d8b1312d67e` | `718270fa4e7f968733a73faa59b31e261dffc53b6c6696471f243da62caecd8b` |
| `src/backend/http_server.rs` | `97098eb8e716a6ecfd6be6c99f590fba608496a9` | `ed86e6d3c9fa4cfd0cb3acd854169ec7a071eae4017f185e24bed8b950e97c1d` |
| `src/backend/mod.rs` | `1bf1ab354610f27980c1e2e2ab562c723dd58326` | `8961459a1bf840bb41e9afb2bb95135c0ee77d6fea754a6deb43f2928fef9358` |
| `src/backend/operation_journal.rs` | `8057cdf43e742a263552bdb518501891e6b6732d` | `0a5fbc32a8eac38da3f384136af1135bada62b849f1ad10beb81a0274292ee7b` |
| `src/backend/resource_cache.rs` | `8928745150cec4b58b59cfd7e1db42a60db55e41` | `3506330e8f05f08b6d2d8d1414b5806d31fbe92d5ee10a8b8cec354c2f31e091` |
| `src/backend/safe_fs.rs` | `00d891b366a5c904a3b3d11a0e613bb895ab185f` | `a971421c3ac2c1d9303f56e97a105e8a8579788e3b319d085e05b68edaaf8d4d` |
| `src/backend/session_manager.rs` | `ccff021b179acaad33c752eba8bdb2a8ec96de9a` | `87fdedbb03d301a00af07085d670bba1e37b5eac4460ea6849e2cbd781021d44` |
| `src/backend/storage_retention.rs` | `6ab940d3e246f755dc67100e8bb9281309019989` | `1d9819da80993ad4bf3edc8a7170ee5fb6988848de6004fe85f1d575713d9e57` |
| `src/backend/tool_executor.rs` | `a56bf64235a3d9783cede01779ff668a0f66f2db` | `d9a7c8fb8d2975c30eaba98f7a6ab2f10f823d71084889f6e64949d135254689` |
| `src/backend/voice_pipeline.rs` | `254d0fcdc0eb6f219cfb3a76250ac7770e574f95` | `d0302822231da5d3fefc6cbb82547dfca2624fd1f24ade17261d78ea49660216` |
| `src/models/config.rs` | `6e63c4154ca9388dfbb13f4fcad76058548999fb` | `37e2f637faa3bab8d4759ea16b8b88ec4825d0768dca466aa61bf9559cae4ebf` |
| `tests/f1b1_text_startup.rs` | `8af24b90b50c5676cccffbdb72f45c81d71be6f7` | `980eaf1a8cfcc9c3affa8bc5c36a4fbc06976c98b0cf7e364e6fda5219e75f89` |
| `tests/f1b3a_persistence.rs` | `e28edc2557cad6ba8f92636ac7c0534079baefa3` | `83eb31d16490a82b20be524af4771fb1e756420674b2cce71296dd8c93b065d0` |

### Manifest staged B

| Archivo | Blob Git | SHA256 del contenido |
| --- | --- | --- |
| `docs/recovery/F1B3B-REPORT.md` | `af15e1155749c3982d11e0720d0bd3016fcbcb72` | `6d1d3a8d4fda0c1908de8444af6c4e897ecc3daea5fffd6d94ddbf980304c902` |
| `docs/recovery/F1B3B-RESULTS.md` | `b62d524b3b1e17c40cd45263ad50b07d5498d803` | `ed8dac2a72f67f39bcd57c29653c07dfd3de6510c0fe70c30030b40c99d40fd2` |
| `docs/recovery/F1B3B-STORAGE-CONTRACT.md` | `3670e226856ec45f9051214340c4eefb2c97be69` | `39687970b7474a7b63788b12e04f7d8ee69ae7b3e71a56990e4a800a907a8011` |
| `scripts/validate_f1b3b.py` | `7237d25bfd3762f78c253bfd9ca32c1fad7e06cc` | `30b4d79e52c0283cff36b71dff63c3e0c3c558be8b33b6776231771530c00418` |
| `tests/f1b3b_storage.rs` | `077a85599615c94d63e0743bf12a072cf3465fef` | `2c986b7799621deb2d3ad193dc97215bb24cbb5210509eed98bc511f28d2d54b` |
| `tests/qml/F1B3BMain.qml` | `afd7be62da66018a703699a4d6fa8560f2e595e0` | `14e2cefb12d180bb187ddf9953dd6b48ea752c4965f6b798b07f53f392258c3c` |
| `tests/qml/F1B3BRecovery.qml` | `80b448663143cdde3896b1a31ed4fd71743c9985` | `860e39c2ca8d4eb90eae22df4771aceda56d65b65900b235c78cfd2eab6b1edb` |
