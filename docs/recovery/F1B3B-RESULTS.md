# F1B-3B — Resultados reproducibles

Fecha: 2026-10-12. **IMPLEMENTED / READY_FOR_REVIEW**, dentro del alcance de historial, almacenamiento y recuperación. Baseline: `d8a6972b5bc3977408c8d0956fce3196623d5458`; rama `recovery/f1b3b-history-retention`. Consolidación local posterior documentada en F1B3B-CLOSURE/F1B3B-COMMIT-VALIDATION; sin push, merge ni release.

## Ejecución final

Comando: `python3 scripts/validate_f1b3b.py --qml`.

Evidencia final local: `/tmp/kda-f1b3b-check-4js24vaa/`, con `results.json`, `source-manifest.json` y un log por gate. Transcript `/tmp/kda-f1b3b-consolidation-f24ll7kk/final-sources-third.log`. Estos archivos son evidencia temporal privada, no entregables publicados ni volcados de SQLite personal. El manifiesto SHA256 incluye 127 archivos Rust/QML/SQL/Python/JS/qmldir y coincide con el código final; no hubo cambios de fuentes durante los gates. Los tres documentos entregables contienen el resultado durable y la descripción de cobertura.

El harness usa umask 077, XDG_CONFIG_HOME/XDG_CACHE_HOME/XDG_DATA_HOME/TMPDIR privados, SQLite sintético, mocks loopback y fixtures propias. Retira claves API, proxies y flags heredados KDA_F1; Cargo offline. El runner limpia únicamente sus directorios de fixture. No se descargaron modelos, no se abrió micrófono y no se borraron conversaciones/archivos personales. No constituye un sandbox general de egress; la selección y los mocks evitan los tests externos conocidos.

Todos los comandos de la siguiente tabla terminaron con exit 0. Los números corresponden al resumen final del proceso padre; los hijos controlados que repiten un test o generan una fixture no se suman otra vez. FILTERED de hijos tampoco se atribuye al padre.

| Gate / comando exacto | Estado | PASS | FAIL | IGNORED | FILTERED |
| --- | --- | ---: | ---: | ---: | ---: |
| `cargo fmt --check` | PASS | — | — | — | — |
| `cargo check --locked --all-targets` | PASS | — | — | — | — |
| `cargo clippy --locked --all-targets -- -D warnings` | PASS | — | — | — | — |
| `cargo test --locked --test f1a_security` | PASS | 19 | 0 | 1 | 0 |
| `cargo test --locked --test f1b1_text_startup -- --test-threads=1` | PASS | 13 | 0 | 1 | 0 |
| `cargo test --locked --test f1b2_conversation -- --test-threads=1` | PASS | 20 | 0 | 3 | 0 |
| `cargo test --locked --test f1b3a_persistence -- --test-threads=1` | PASS | 26 | 0 | 1 | 0 |
| `cargo test --locked --test f1b3b_storage -- --test-threads=1` | PASS | 33 | 0 | 3 | 0 |
| `cargo test --locked --lib -- --test-threads=1 --skip backend::wakeword_ml::tests::download_and_extract_onnx_runtime --skip backend::tool_executor::tests::show_image_huge_url_is_rejected_before_download` | PASS | 156 | 0 | 1 | 2 |
| `cargo test --locked --test app_paths` | PASS | 2 | 0 | 0 | 0 |
| `cargo test --locked --test f1b1_text_startup qml_offscreen -- --ignored --nocapture` | PASS | 1 | 0 | 0 | 13 |
| `cargo test --locked --test f1b2_conversation qml_ -- --ignored --nocapture --test-threads=1` | PASS | 3 | 0 | 0 | 20 |
| `cargo test --locked --test f1b3b_storage qml_ -- --ignored --nocapture --test-threads=1` | PASS | 2 | 0 | 0 | 34 |

Total de ejecuciones de tests padre que pasaron: **269 Rust + 6 Qt/QML = 275**, cero FAIL finales. No significa haber ejecutado `cargo test` sin filtros ni toda posible prueba del repositorio.

IGNORED conserva su significado: F1A mount namespace requiere unshare/mount y no se ejecutó; los Qt opt-in de F1B1/F1B2/F1B3B sí se ejecutaron en los últimos tres gates. Los workers crash/cache no son pruebas independientes ignoradas que puedan invocarse indiscriminadamente: sus padres los ejecutan en fixtures privadas. Lo mismo aplica al crash worker unitario. Los dos unitarios externos listados permanecen FILTERED y su funcionalidad remota es NOT_TESTED. Los 13/20/34 FILTERED en selecciones Qt son pruebas fuera del filtro, ya cubiertas por su suite normal cuando corresponde.

## Cobertura F1B-3B solicitada

| Requisito | Evidencia ejecutada / resultado |
| --- | --- |
| 1. Historial corto/largo | `short_history_preserves_multi_call_causality_and_failures`, `long_history_selects_complete_suffix_without_summary`: PASS. Sufijo causal completo sin inyectar resumen. |
| 2. Múltiples calls/results | Batch de dos calls, resultados en orden causal inverso a sus IDs, output fallido conservado; `orphan_duplicate_and_missing_results_fail`: PASS. |
| 3. Corte assistant/tool/result | `cutoff_never_splits_assistant_and_results`: PASS sobre varios presupuestos; nunca quedan pares separados. |
| 4. Tokens/tamaño/respuesta | `oversized_latest_input_and_response_fail_explicitly`, `essential_instructions_cannot_be_cropped`, `regeneration_input_is_bounded`, `live_request_growth_is_checked_each_iteration`: PASS. Contabilidad conservadora por bytes; JSON legacy inválido de 4 MiB rechazado antes de parsear; no tokenizer remoto certificado. |
| 5. Cancelación/fallo | `cancelled_failed_and_previous_retry_unknown_are_visible_in_context`, `interrupted_outputs_and_post_deletion_writes_are_rejected`: PASS; conserva input y no acepta outputs no completados. |
| 6. UnknownOutcome previo | Retry enlazado a input original; aviso esencial y regenerate restringido: PASS. Cobertura de no replay/SIGKILL heredada sigue PASS. |
| 7. Legacy sin Turn ID | `legacy_null_turn_ids_remain_causal`, `incomplete_legacy_groups_are_errors_never_fabricated`: PASS; no reasignación de UUID ni resultados inventados. |
| 8. Borrar sesión con varias operaciones | `uncertain_deletion_requires_confirmation_and_leaves_no_orphans`: PASS, dos UnknownOutcome y snapshot turn_recovery. |
| 9. Fallo a mitad de borrado | `deletion_failure_mid_transaction_rolls_back_every_table`: PASS con trigger RAISE(ABORT); journal/eventos restaurados y conexión reutilizable. |
| 10. Sesiones independientes | Mismo test de eliminación y `exact_session_matching_preserves_independent_metadata`: PASS; igualdad exacta, contenido/metadata ajenos conservados. |
| 11. Borrado activo | `deletion_rejects_persisted_and_reserved_active_turns`, `dispatched_operation_blocks_deletion_even_with_terminal_turn`: PASS. Router real devuelve 409 para reserva activa. |
| 12. Confirmar evidencia incierta | Primer DELETE real 409 y conservación de filas; segundo explícito 200. Harness Main no borra antes de pulsar confirmación: PASS. |
| 13. Symlinks maliciosos | `cache_symlinks_are_not_followed_and_cleanup_can_recover`, `inactive_symlink_cleanup_failure_retries_without_touching_target`, `cache_ownerless_and_symlink_roots_are_never_removed`: PASS. Target externo de fixture intacto. |
| 14. Recursos abiertos | `active_open_and_persistent_resources_are_retained`, `referenced_and_young_cache_survive_repeated_sweeps`: PASS, leases/referencias/gracia conservados. |
| 15. Reinicio/migración | `migrations_repeat_and_preserve_pre_f1b2_uuid_history`: PASS, tres aperturas y marcador 004 único. Suites anteriores conservan restart/recovery. |
| 16. UI incierta | `qml_recovery_states_and_stale_responses`, `qml_session_banner_and_explicit_evidence_confirmation`: PASS Qt offscreen con componentes reales. |
| 17. API cero/error | Router real: lista vacía tras eliminación, 401 sin auth, 404 sesión ausente, 409 actividad/consentimiento, 500 fallo de tabla, 502 presupuesto. QML mock: empty/unavailable/respuesta tardía: PASS. |
| 18. Privacidad | `diagnostics_and_paged_recovery_are_private_and_non_destructive` y router: PASS; metadata sin argumentos privados, API audit redacta mientras SQLite conserva evidencia raw histórica. |
| 19. Integridad mantenimiento | integrity_check=ok, foreign_key_check vacío y diagnóstico sin enlaces huérfanos tras borrado/rollback/migración: PASS. No VACUUM nuevo. |
| 20. Seguridad/cancelación | F1A/F1B1/F1B2/F1B3A y selección segura unitaria: PASS con exclusiones explícitas arriba. Autorización/IPC/no replay conservados. |

Además: límites de cuotas/admisión concurrente, guard de recursos no publicados, preview de imagen que conserva su snapshot y fallo de render que recoge temporales, orden causal de system históricos y eliminación explícita de metadatos huérfanos legacy: PASS. El caller heredado de voz tiene dos unitarios de selección íntegra/error conservador; no se certifica captura o síntesis de voz real.

## Demostración de borrado

La fixture crea una sesión con dos operaciones inciertas, sus eventos y un snapshot de recuperación. Otra sesión contiene un turno Completed con input + assistant/tool/tool/final. Sin flag de consentimiento no se retira ninguna operación. Con consentimiento, cero messages/conversation_turns/tool_audit/operation_journal pertenecen a la sesión eliminada, cero operation_events y turn_recovery quedan de ella, y la otra sesión conserva sus cinco mensajes y su turn_inputs. foreign_key_check no devuelve filas y integrity_check devuelve `ok`.

El trigger de fallo se ejecuta al borrar messages, después de eliminar eventos, journal, recovery, inputs y audit dentro de la misma transacción. Rollback conserva la operación, sus cuatro eventos y el input; al retirar el trigger el borrado se completa en la misma conexión y vuelve a pasar integridad. Guards adicionales impiden borrar padres fuera del procedimiento con dependientes vivos.

La fixture de corrupción legacy desactiva FK **sólo durante la siembra sintética**, retira dos guards para reproducir el borrado anterior y reactiva FK inmediatamente. Antes de abrir producción afirma que foreign_key_check detecta la orfandad. Migración conserva UnknownOutcome, diagnóstico lo informa, borrado sin consentimiento falla y borrado confirmado por ID exacto repara la orfandad. No se desactiva enforcement durante la migración ni durante el procedimiento probado.

## QML y aislamiento

Componente RecoveryHistory: frase exacta de incertidumbre, semántica Failed/Abandoned, placeholders de fila antigua, ausencia de registros, indisponibilidad HTTP 500, cancelación de confirmación sin eliminación y generación de requests que descarta una respuesta tardía. Main real: aviso de sesión, 409 de primer intento, pausa sin borrar antes de consentimiento, acción «Eliminar evidencia», segunda petición explícita y sesión eliminada como vacía. El mock verifica el contador de eliminaciones confirmadas.

Los harnesses se ejecutan con Qt offscreen en XDG privados. F1B1 prueba también fallo de carga QML visible y liberación de puerto sin matar un QML ajeno; F1B2 mantiene sus tres escenarios de cancelación/regenerate/cambio de sesión. Los mocks de UI y las pruebas del router real son evidencias separadas; no se presenta un mock como prueba de SQLite ni offscreen como certificación Wayland.

## Iteraciones previas y correcciones

| Ejecución | Resultado observado | Corrección y revalidación |
| --- | --- | --- |
| `/tmp/kda-f1b3b-check-102aycs9` | FAIL F1B3A: 25 PASS, 1 FAIL, 1 IGNORED. Un test suponía que sólo existía el marcador de migración 003. Gates posteriores NOT_TESTED en esa ejecución. | Se comprueba versión 3 por separado, sin eliminar el marcador nuevo 4; nueva suite prueba su unicidad. |
| `/tmp/kda-f1b3b-check-v6vbqd64` | FAIL F1B1: 12 PASS, 1 FAIL, 1 IGNORED. Reapertura compitió con último owner SQLite del servidor detenido. Gates posteriores NOT_TESTED allí. | Teardown espera a que desaparezca el Weak de SessionManager antes de reabrir; no se debilita flock ni la exclusión de instancia de producción. |
| `/tmp/kda-f1b3b-check-rwn4owjw` | FAIL F1B3B: 30 PASS, 1 FAIL, 3 IGNORED. FK impedía sembrar deliberadamente una base huérfana legacy. Gates posteriores NOT_TESTED allí. | La conexión de siembra reproduce explícitamente el enforcement antiguo, lo reactiva y afirma el error real antes del ejercicio de producción. |
| `/tmp/kda-f1b3b-check-ghouod_0` | PASS de los 13 gates, cero FAIL. | Después, la revisión de memoria sustituyó length(CAST(...)) por octet_length y amplió el caso de payload legacy. |
| `/tmp/kda-f1b3b-check-fedrbire` | PASS previo a la auditoría de consolidación, cero FAIL. | Código de implementación con guards de tamaño optimizados validado; manifiesto inalterado. |

Durante desarrollo también se corrigieron un import faltante de anyhow::Context y un import de test no usado señalado por Clippy. No quedan warnings/fallos en la ejecución final. El fallo de fixture y los cambios de compatibilidad de tests no se contabilizan como pruebas ignoradas que hubieran pasado.

## NOT_TESTED y límites de la recomendación

Proveedores externos/tokenizadores/templates privados, modelos descargables, voz/micrófono/TTS reales, Plasma Wayland/tray real, namespace mount de F1A, fallos físicos de disco/dispositivo, destrucción forense de datos y uso de recursos por procesos QML huérfanos fuera del lifecycle cooperante: **NOT_TESTED**. No se implementaron exportación, ACL multiusuario, exactamente una ejecución externa ni F2.

La ventana del modelo debe configurarse; la cota conservadora local no descubre sus capacidades. La carga legacy de mensajes para UI sigue completa. Quota de caché puede denegar recursos nuevos si todos están protegidos; un renderer puede usar espacio transitorio antes de validar su salida. Eliminar filas no destruye WAL/backups/copias cacheadas inmediatamente. UnknownOutcome nunca se limpia automáticamente y la auditoría histórica no se presenta como redactada en disco. Véanse [contrato](F1B3B-STORAGE-CONTRACT.md) e [informe](F1B3B-REPORT.md).

## Auditoría de consolidación: antes / después

Evidencia de revisión `/tmp/kda-f1b3b-consolidation-f24ll7kk/`: manifests, archives, diffs, before.json/after.json y logs por reproducción. Ambos tests se invocaron individualmente con `cargo test --locked --test f1b3b_storage <nombre> -- --exact --nocapture`, XDG/TMPDIR privados y sin claves/proxies.

| Test | Antes | Después | Invariante |
| --- | --- | --- | --- |
| replaced_workspace_cannot_publish_or_delete_another_live_resource | FAIL (exit 101): publish=Ok y cleanup=Ok sobre workspace sustituido. | PASS (exit 0) | Rechazo por identidad de lease; recurso ajeno activo intacto; restaurar la ruta original permite cleanup legítimo. |
| image_download_does_not_follow_planted_output_symlinks | FAIL (exit 101): bytes del target cambiaron a PNG. | PASS (exit 0) | Archivo destino de fixture intacto, operación rechazada sin seguir enlace. |

Se añadió a la prueba de preview una página PDF sintética, ejecutada con pdftoppm local y firma PNG comprobada; sin documentos personales. Los cambios compartidos de escritura abarcan también SVGs. Dos unitarios heredados de render conservan sus assertions y usan ahora allocate_at en una fixture, porque una ruta sin lease debe seguir siendo rechazada.

Iteraciones de esta revisión: `/tmp/kda-f1b3b-check-jxng6pnz` FAIL Clippy por import tokio::fs ya no usado; `/tmp/kda-f1b3b-check-idmdtx9c` FAIL unit-safe (154 PASS, 2 FAIL por fixtures no registradas, 1 IGNORED, 2 FILTERED); gates posteriores NOT_TESTED en esas ejecuciones. Se corrigieron el import y las fixtures. `/tmp/kda-f1b3b-check-4js24vaa`: **13 gates PASS**, 33 PASS/3 IGNORED nuevos, 156 PASS/1 IGNORED/2 FILTERED unitarios, 6 Qt PASS; 275 ejecuciones padre totales, cero FAIL finales.

Las comprobaciones sobre cada árbol staged y su comparación con los commits se registran en F1B3B-COMMIT-VALIDATION. El árbol A sólo dispone de suites heredadas; suite/harness/runner F1B3B nuevos se incorporan en B. Esta disponibilidad no se confunde con un test omitido que hubiera pasado.
