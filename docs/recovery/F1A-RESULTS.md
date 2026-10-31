# F1A — Resultados verificables

Fecha: 2026-10-10. Implementación **IMPLEMENTED** para SEC-01/02; validación global con fallos heredados. Rama `recovery/f1a-containment`, HEAD/base `12da960ed4722401f1cb42b7a96d9cef912cf90a`. Cambios sin commit/push. Decisiones: [REPORT](F1A-REPORT.md). Archivos/riesgos: [DIFF-REVIEW](F1A-DIFF-REVIEW.md).

## Aislamiento

Tests nuevos: directorios UUID creados con modo 0700, subdirectorios `allowed` y `outside` hermanos, archivos y texto sintéticos. `outside` significa exclusivamente el hermano temporal del fixture. Cada fixture se elimina al terminar; no se ejecutaron pruebas destructivas en HOME/Documentos/Descargas/Escritorio. La reproducción SEC-01 usa argumentos incompletos de notify para probar que el flag se comprueba antes del parsing, sin enviar una notificación real.

Validación agrupada bajo `/tmp/kda-f1a-validation-d28f496n/` (0700), `umask 077`, `XDG_CONFIG_HOME`, `XDG_CACHE_HOME`, `XDG_DATA_HOME` y `TMPDIR` privados. Se retiraron variables terminadas en `_API_KEY` del entorno de los tests y se usó `CARGO_NET_OFFLINE=true`. HOME no se cambió. Offline sólo impide red de Cargo; los tests de red/descarga fueron excluidos explícitamente. No se ejecutaron main, snap_ui, pkill, sidecar, micrófono, inferencia/modelos ni proveedores reales. No se instaló ni simuló Piper.

## Reproducciones antes y después

Se agregaron los primeros cuatro tests **antes de modificar producción** y se ejecutó `cargo test --test f1a_security -- --nocapture` sobre la base con la corrección previa del usuario. Exit **101**: `0 passed; 4 failed; 0 ignored; 0 measured; 0 filtered out`. Tras la primera corrección se repitieron los mismos cuatro: exit **0**, `4 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out`.

| Reproducción | Antes observado | Después observado |
| --- | --- | --- |
| Flag notify=false sin lock y con write guard | Sin lock: deshabilitado. Con guard: `Argumento 'title' requerido y debe ser string`; el flag fue omitido por fallback. No se ejecutó notify real. | Sin lock: deshabilitado. Con guard: configuración no disponible; no despacho. |
| create_file con symlink colgante | `success=true`; archivo exterior creado; assertion falla. | `success=false`; archivo exterior inexistente; link no seguido. |
| copy_file hacia directorio con hijo symlink, overwrite=true | `success=true`; exterior cambió de `original` a `copy fixture`. | `success=false`; exterior sigue `original`. |
| edit_file con backup `.bak` symlink | `success=true`; exterior cambió de `outside original` a `main original`. | `success=false`; exterior y principal mantienen sus contenidos originales. |

Las tres reproducciones SEC-02 ejecutaron realmente escrituras sólo sobre fixtures temporales antes del fix. La reproducción SEC-01 es una prueba sin efecto de omisión del flag; la ejecución efectiva de system_info bajo contención ya estaba registrada en F0, no se repitió sobre el host.

La ausencia/corrupción de config se comparó con la evidencia F0 `INVALID_CONFIG returns_ok=true default_tools_enabled=true wake_enabled=true`. Ahora los tests con archivos privados ausentes, JSON corrupto, esquema inválido y config válida comprueban error sin recuperación/sobrescritura de archivo. No se hizo una nueva reproducción sobre la configuración real.

### Fallo intermedio de un fixture, corregido

Al ampliar de 12 a 14 tests, una ejecución produjo `13 passed; 1 failed`. El atacante de `concurrent_parent_directory_symlinks_do_not_escape` usaba rename -> symlink -> unlink -> rename. El executor podía crear el directorio permitido durante el hueco sin entrada, y el rename de restauración del atacante falló con `DirectoryNotEmpty` (error 39). El thread de fixture paniqueó; no fue un fallo de assertion sobre escritura exterior.

Se corrigió el atacante usando `renameat2(EXCHANGE)` entre directorio y symlink, manteniendo ambas entradas presentes. Se repitió la batería: **14 passed; 0 failed**. Tras agregar la prueba de primitivas indisponibles, la batería final dio **15 passed; 0 failed**. El fallo intermedio queda registrado, no contado como regresión abierta ni como PASS anterior.

## Comandos y resultados finales

Duración de comando incluye Cargo; tiempo de tests se muestra por separado cuando ayuda. Se usó el lock/cache local existente. La última modificación de producción precede todos estos checks; la prueba de kernel añadida después fue incluida en el último check/Clippy/integración.

| Comando | Exit | Resultado exacto | Tiempo comando |
| --- | --- | --- | --- |
| `cargo fmt --check` | 1 | Sólo diff heredado en hotkey_listener.rs:419 y src/lib.rs:6. | 0,451 s |
| `cargo check --all-targets` | 0 | PASS. | 0,653 s |
| `cargo clippy --all-targets -- -D warnings` | 0 | PASS, sin warnings. | 0,824 s |
| `cargo test --test f1a_security -- --nocapture` | 0 | **15 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out**, tests 0,12 s. | 0,524 s |
| `cargo test --lib f1a_ -- --nocapture` | 0 | **5 passed; 0 failed; 0 ignored; 0 measured; 134 filtered out**, tests 0,02 s. | 25,438 s (build) |
| `cargo test --lib -- --test-threads=1` con los 14 `--skip` de abajo | 0 | **125 passed; 0 failed; 0 ignored; 0 measured; 14 filtered out**, tests 0,34 s. | 0,671 s |
| `cargo test --lib -- --exact --test-threads=1` con los 12 nombres heredados de abajo | 101 | **0 passed; 12 failed; 0 ignored; 0 measured; 127 filtered out**, tests 0,03 s; todos por Piper ausente. | 0,361 s |
| `cargo test --test app_paths` | 0 | **2 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out**. | 1,672 s |
| `rustfmt --check --edition 2021 --config skip_children=true src/backend/mod.rs src/backend/ai_service.rs src/backend/tool_executor.rs src/backend/safe_fs.rs src/models/config.rs tests/f1a_security.rs` | 0 | Formato de los Rust tocados aprobado, sin alterar hijos/heredados. | — |
| `git diff --check` | 0 | PASS. | — |
| `cargo test --test f1a_security --no-run --message-format=json` | 0 | Identificó/compiló el binario de tests para seccomp; no ejecuta main. | — |
| Hijo de tests bajo seccomp, openat2 -> ENOSYS | 0 | **1 passed; 0 failed; 0 ignored; 0 measured; 14 filtered out**. | — |
| Hijo de tests bajo seccomp, renameat2 -> ENOSYS | 0 | **1 passed; 0 failed; 0 ignored; 0 measured; 14 filtered out**. | — |

**20 tests nuevos únicos aprobados**: los 15 de integración y los 5 unitarios. Los 5 unitarios también forman parte de los 125 aprobados; no se deben sumar como si fueran tests distintos. Las dos ejecuciones ENOSYS reejecutan el mismo test de integración en condiciones adicionales, no agregan dos tests únicos. La selección offline incluye **120 unitarios existentes + 5 nuevos**; app_paths aporta otros 2 existentes.

No se ejecutó `cargo test --all-targets`: no se cuenta agent_smoke como prueba de proveedor sin key, ni se ejecutan wrappers de UI/voz ni pruebas que puedan descargar. Los smokes ignored de TTS/wake permanecen NOT_TESTED.

### Tests nuevos: nombres y alcance

Todos los siguientes tienen resultado final **PASS**.

| Test en `tests/f1a_security.rs` | Evidencia |
| --- | --- |
| `disabled_tool_with_and_without_write_lock` | Deshabilitado siempre, contención denegada antes del parsing/efecto. |
| `dangling_symlink_creation_has_no_outside_effect` | Creación rechaza link colgante; exterior no creado. |
| `copy_derived_destination_symlink_has_no_outside_effect` | Destino derivado también protegido; exterior intacto. |
| `malicious_backup_symlink_leaves_main_and_outside_unchanged` | Backup denegado antes de modificar principal/exterior. |
| `all_public_tool_dispatches_respect_disabled_flags_and_contention` | Los 23 despachos rechazan flags apagados y write lock, con args vacíos, sin subprocess/efectos. |
| `concurrent_updates_never_mix_tool_flag_and_custom_allowlist` | 500 llamadas durante 500 alternancias de políticas; una habilita flag pero prohíbe ruta y la otra habilita ruta pero apaga flag; todas rechazadas. |
| `legitimate_create_read_edit_and_copy` | Subdirectorios nuevos, read, append, overwrite/backup, copia a archivo/directorio, exclusividad y reemplazo legítimo. |
| `outside_operations_and_invalid_edit_have_no_effects` | Origen/destino fuera de raíz, `..` y modo inválido denegados, sin modificar principales ni crear directorios/backup exteriores. |
| `intermediate_symlinks_empty_allowlist_and_hardlinks_are_denied` | Links en ancestros, alias hardlink y allowlist vacía no otorgan acceso. |
| `disabled_llm_catalog_preserves_explicit_manual_file_flag` | Catálogo apagado no redefine flags manuales; flag individual apagado sí deniega. |
| `pathname_delegation_is_contained_until_f2` | open_file y KDE Connect share_file bloqueados, sin lanzar apps. |
| `concurrent_symlink_replacement_never_writes_outside` | Atacante reemplaza hojas de creación/copia/principal/backup; 150 rondas (450 operaciones), exterior intacto/inexistente. |
| `concurrent_parent_directory_symlinks_do_not_escape` | Atacante intercambia directorio/link atómicamente; 150 creaciones + 150 lecturas, sin bytes/escrituras exteriores; assert de actividad del atacante. |
| `failed_append_decode_and_backup_collision_preserve_main` | UTF-8 inválido y backup igual al principal no destruyen contenido. |
| `unavailable_kernel_primitives_fail_closed` | Camino normal legítimo y reejecución con cada syscall denegada; archivos originales intactos y temporales limpiados. |

| Test unitario | Evidencia |
| --- | --- |
| `backend::tool_executor::tests::f1a_snapshot_stays_coherent_after_config_update` | Snapshot A ejecuta bajo guard con política B, sin releer/mix; una operación nueva respeta B. |
| `backend::ai_service::tests::f1a_ai_contention_fails_before_any_request` | Constructor/snapshot/completion denegados bajo guard; copia previa conserva proveedor y la siguiente refleja actualización, sin HTTP. |
| `backend::ai_service::tests::f1a_unknown_provider_never_falls_back_to_openrouter` | Proveedor desconocido no produce fallback. |
| `models::config::tests::f1a_missing_security_fields_do_not_enable_capabilities` | Omisiones/typo no habilitan tools/allowlist; confirm_sensitive conserva true. |
| `models::config::tests::f1a_absent_corrupt_and_invalid_config_are_errors` | Archivos privados ausentes/corruptos/inválidos fallan sin reescritura; configuración válida restrictiva se conserva. |

Las carreras son fixtures adversariales acotados, no una prueba exhaustiva de todos los interleavings. El argumento de contención se basa además en resolución del kernel, descriptor fijado y publicación sin seguimiento de symlinks.

## Fallos heredados / selección reproducible

Formato global: los dos diffs coinciden con F0; se preservó íntegro el archivo del usuario. No se amplió alcance para corregir lib.rs o formatear hotkey_listener. SHA-256 antes/después `2622363c9d4323d81822d529edc95a449b75b387a379d831ae9e3b37988f40ab`.

Los **12 tests heredados** siguen fallando por `inicializando Piper TTS` -> `piper-tts no encontrado`. No fallan por un resultado de seguridad nuevo. Selección exacta usada para reproducirlos:

```bash
cargo test --lib -- --exact --test-threads=1 \
  backend::http_server::tests::dictate_roundtrip_empty_transcript_releases_flag \
  backend::http_server::tests::dictate_start_conflicts_with_voice_pipeline \
  backend::http_server::tests::dictate_stop_without_start_conflicts \
  backend::speech_service::tests::piper_engine_initializes \
  backend::voice_pipeline::tests::auto_barge_ignora_mic_alto_si_tts_aun_no_suena \
  backend::voice_pipeline::tests::barge_in_transita_interrupted_a_listening \
  backend::voice_pipeline::tests::dictado_activo_aborta_wait_for_speech_rapido \
  backend::voice_pipeline::tests::dictado_activo_bloquea_start_listening \
  backend::voice_pipeline::tests::emit_state_mapea_a_strings_legados \
  backend::voice_pipeline::tests::signal_broadcast_state \
  backend::voice_pipeline::tests::signal_level_throttled \
  backend::voice_pipeline::tests::wait_for_speech_times_out_quiet
```

Para la selección independiente de hardware/red/modelos se usaron exactamente esos 12 nombres con `--skip`, más dos exclusiones preventivas. Comando completo:

```bash
cargo test --lib -- --test-threads=1 \
  --skip backend::http_server::tests::dictate_roundtrip_empty_transcript_releases_flag \
  --skip backend::http_server::tests::dictate_start_conflicts_with_voice_pipeline \
  --skip backend::http_server::tests::dictate_stop_without_start_conflicts \
  --skip backend::speech_service::tests::piper_engine_initializes \
  --skip backend::voice_pipeline::tests::auto_barge_ignora_mic_alto_si_tts_aun_no_suena \
  --skip backend::voice_pipeline::tests::barge_in_transita_interrupted_a_listening \
  --skip backend::voice_pipeline::tests::dictado_activo_aborta_wait_for_speech_rapido \
  --skip backend::voice_pipeline::tests::dictado_activo_bloquea_start_listening \
  --skip backend::voice_pipeline::tests::emit_state_mapea_a_strings_legados \
  --skip backend::voice_pipeline::tests::signal_broadcast_state \
  --skip backend::voice_pipeline::tests::signal_level_throttled \
  --skip backend::voice_pipeline::tests::wait_for_speech_times_out_quiet \
  --skip backend::wakeword_ml::tests::download_and_extract_onnx_runtime \
  --skip backend::tool_executor::tests::show_image_huge_url_is_rejected_before_download
```

La primera exclusión adicional puede descargar ONNX; la segunda necesita loopback. **No se ejecutaron** ni se consideran aprobadas. Repetir con XDG y TMPDIR bajo un mktemp 0700 nuevo y claves retiradas; no iniciar Backend::new ni main para validar esto.

## Primitivas indisponibles: reproducción aislada

En Linux x86_64, un script de diagnóstico **fuera del producto** instala seccomp sólo en el hijo de tests, con NO_NEW_PRIVS, y hace que una syscall devuelva ENOSYS. No altera kernel/config de la sesión ni introduce un hook de fallos en producción. ABI observado: x86_64; números openat2=437 y renameat2=316. El filtro se hereda por los threads del hijo.

Primero obtener la ruta del `executable` del artifact `f1a_security` con `cargo test --test f1a_security --no-run --message-format=json`. La fuente esencial del script ejecutado está abajo; recibe esa ruta como argumento. Ejecutarlo dentro de un directorio temporal privado, con TMPDIR privado. El test sólo usa la variable de fixture `KDA_F1A_TEST_DENY_SYSCALL` para seleccionar assertions; no cambia la implementación ni sus primitivas.

```python
import ctypes, errno, os, pathlib, platform, subprocess, sys
assert platform.machine() == "x86_64"
binary = sys.argv[1]
class Filter(ctypes.Structure):
    _fields_ = [("code", ctypes.c_ushort), ("jt", ctypes.c_ubyte),
                ("jf", ctypes.c_ubyte), ("k", ctypes.c_uint)]
class Program(ctypes.Structure):
    _fields_ = [("len", ctypes.c_ushort), ("filter", ctypes.POINTER(Filter))]
def deny(number):
    libc = ctypes.CDLL(None, use_errno=True)
    filters = (Filter * 4)(
        Filter(0x20, 0, 0, 0),  # syscall number
        Filter(0x15, 0, 1, number),
        Filter(0x06, 0, 0, 0x00050000 | errno.ENOSYS),
        Filter(0x06, 0, 0, 0x7fff0000))
    program = Program(len(filters), filters)
    if libc.prctl(38, 1, 0, 0, 0) != 0 or \
       libc.prctl(22, 2, ctypes.byref(program), 0, 0) != 0:
        os._exit(90)
os.umask(0o077)
for syscall, number in [("openat2", 437), ("renameat2", 316)]:
    env = os.environ.copy()
    for key in list(env):
        if key.endswith("_API_KEY"):
            env.pop(key)
    env["KDA_F1A_TEST_DENY_SYSCALL"] = syscall
    command = [binary, "--exact", "unavailable_kernel_primitives_fail_closed",
               "--nocapture"]
    result = subprocess.run(command, env=env, preexec_fn=lambda: deny(number))
    assert result.returncode == 0, (syscall, result.returncode)
```

Resultados reales: **openat2 ENOSYS: exit 0 / 1 passed**; **renameat2 ENOSYS: exit 0 / 1 passed**. En el primer caso ninguna de create/read/copy/edit obtiene acceso; en el segundo read sigue legítimamente disponible, pero create/copy/edit fallan sin modificar principal, crear backup/destinos o dejar temporales. No se probó un kernel antiguo físicamente; sí la respuesta de indisponibilidad de las primitivas, sin fallback.

## Evidencia local y límites

Logs temporales en `/tmp/kda-f1a-validation-d28f496n/`: `before.log`, `after-initial.log`, `security.log` (fallo intermedio), `final-security.log`, `new-unit.log`, `unit-offline.log`, `piper-inherited.log`, `app-paths.log`, `final-fmt.log`, `final-check.log`, `final-clippy.log`, `deny-openat2.log`, `deny-renameat2.log`; metadatos `validation-results.json`, `validation-final-results.json`, `kernel-denial-results.json`, `excluded-tests.json`; scripts `validate.py`, `revalidate.py`, `kernel-denial.py`. Pueden desaparecer; cifras y fuentes relevantes quedan preservadas aquí.

**Regresiones nuevas abiertas observadas: ninguna en la selección ejecutada.** Restricciones nuevas intencionales se describen en REPORT/DIFF-REVIEW. No se certifica la suite completa, primer arranque, providers reales, consumo/privacidad de caches, Plasma/QML, voz ni la totalidad SEC-03–SEC-10. No se hizo commit ni push y no se inició Fase 1B.
