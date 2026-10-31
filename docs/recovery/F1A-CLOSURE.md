# F1A — Consolidación local

Fecha: 2026-10-10. Estado: **IMPLEMENTED, consolidado en tres commits locales validados individualmente**. SEC-01 y SEC-02 aceptados técnicamente por dirección; la regresión puntual de búsqueda también está corregida. La dependencia del hotkey está consolidada antes del commit de seguridad. No hubo push, merge ni release y no se inició F1B.

Rama: `recovery/f1a-containment`. Baseline publicado: `12da960ed4722401f1cb42b7a96d9cef912cf90a`. HEAD: `ca82e722935e8076f526a484655f2f95f87b5d4c`.

## Commits y archivos

| Orden | SHA | Responsabilidad y archivos |
| --- | --- | --- |
| 1 | `6a088bba20b4f4c00c103c0673bb430e44416a01` | Corrección previa del usuario y formato heredado: `src/backend/hotkey_listener.rs`, `src/lib.rs`. |
| 2 | `7dc6938ca73b8fc2994fda0cf29ca8a95b6ca7c7` | Contención y búsqueda: `Cargo.toml`, `src/backend/ai_service.rs`, `src/backend/mod.rs`, `src/backend/safe_fs.rs`, `src/backend/tool_executor.rs`, `src/models/config.rs`, `tests/f1a_security.rs`. |
| 3 | `ca82e722935e8076f526a484655f2f95f87b5d4c` | Reproducibilidad y validación básica: `.gitignore`, `Cargo.lock`, `rust-toolchain.toml`, `scripts/validate_f1a.py`, `docs/recovery/F1A-TOOLCHAIN.md`. |

Son **14 archivos distintos versionados** en esta consolidación. Cada commit tiene como padre el anterior; el primero parte del baseline indicado. Los mensajes explican el alcance y las comprobaciones. Al no existir identidad Git configurada, se utilizó `Codex <codex@localhost>` mediante opciones locales al comando, sin cambiar configuración Git persistente. El mensaje del primero acredita expresamente que la corrección funcional proviene del usuario y precede F1A.

Los dos informes de cierre se generan después de obtener los tres SHAs y se entregan sin commit. Los nueve documentos/parche anteriores que ya estaban sin versionar se conservaron byte por byte y siguen fuera de esos commits. [F1A-CHANGES.patch](F1A-CHANGES.patch) conserva su SHA-256 histórico `a10d25ebdb14d2a317df1ef58f33a9b22c3161e8c0643f4a20f96fe8b090661c`; no representa la corrección adicional de búsqueda. El commit 2 es la referencia de implementación consolidada.

## Corrección de búsqueda

`SafePath::search_root` resuelve con las mismas restricciones de F1A y fija el directorio completo por descriptor. También cubre raíces solapadas, donde la resolución inicial puede haber abierto una raíz padre y dejado componentes relativos.

Sólo un error tipado `rustix::io::Errno::NOENT` devuelve ausencia. En búsqueda **implícita**, `search_authorized` omite esa raíz y continúa con las demás. Si ninguna resulta utilizable devuelve `No hay ninguna raíz utilizable para buscar en allowed_paths`. No se recupera HOME, `/` ni ninguna raíz alternativa fuera de la lista.

Symlinks, incluso colgantes, raíces relativas, archivos que no son directorios y errores de permisos/montajes/primitivas permanecen como errores. Su contexto es `Raíz de búsqueda insegura o inaccesible`; no se confunden con ausencia ni se ignoran para obtener resultados parciales de otras raíces. Un `dir` explícito conserva la resolución estricta: una ruta ausente, externa o insegura se rechaza, sin iniciar una búsqueda implícita de sustitución.

La política de montajes no cambió: se rechazan cruces bajo una raíz; una raíz montada autorizada directamente puede utilizarse. Sigue vigente la selección por primera coincidencia de allowlist, incluida la restricción que puede provocar una raíz amplia anterior sobre una raíz montada más específica posterior.

Se añadieron cinco tests, todos usando fixtures privados y ambos despachos `find_file`/`find_document`:

| Test | Evidencia |
| --- | --- |
| `search_existing_roots_survive_missing_roots` | Dos raíces válidas más raíces ausentes, incluida una hija solapada; encuentra los dos documentos. |
| `search_security_errors_are_not_missing_roots` | Symlink exterior, symlink colgante, archivo como raíz y raíz relativa causan rechazo aun con otra raíz válida. |
| `search_explicit_invalid_directory_is_never_skipped` | `dir` ausente, symlink, exterior, con `..` o relativo no dispara fallback. |
| `search_without_usable_roots_returns_clear_error` | Allowlist vacía o sólo ausente produce el error claro. |
| `search_mount_boundaries_remain_closed` | En namespace hijo: búsqueda del padre omite el bind mount; `dir` que lo atraviesa se rechaza; raíz montada autorizada directamente funciona pese a otra raíz ausente. |

Antes del cambio de producción, `cargo test --test f1a_security search_ -- --nocapture` dio exit 101: **1 passed; 3 failed; 1 ignored; 15 filtered out**. La raíz ausente bloqueó realmente la búsqueda válida; los otros dos fallos verificaban la distinción/contexto y el mensaje exigidos. Después: exit 0, **4 passed; 0 failed; 1 ignored; 15 filtered out**. El quinto test se ejecutó por separado y aprobó; permanece ignored por defecto porque necesita namespaces y utilidades de montaje.

No se cambió la semántica del catálogo LLM ni de flags manuales, confirmaciones, bloqueo temporal de delegación, acceso por descriptor, symlinks/hardlinks o publicación de archivos. Fuera de esa corrección, ai_service, config, mod y Cargo.toml conservan el contenido F1A aceptado. ToolExecutor recibió la corrección de búsqueda y normalización de formato en el archivo tocado.

## Trabajo previo preservado

El cambio funcional previo del hotkey importa `anyhow::Context` y elimina la segunda conversión `.as_millis()` sobre un valor ya convertido. Se añadió únicamente el formato del test existente. `src/lib.rs` sólo cambió el orden formateado de sus declaraciones de módulos.

SHA-256 del hotkey previo: `2622363c9d4323d81822d529edc95a449b75b387a379d831ae9e3b37988f40ab`. Tras formato: `e66af0109beee54ecbcea5611679b2dbda7fec2be12a7382dd5b066ef5b1241e`. Formatear una copia privada del archivo previo produce exactamente el archivo del commit 1; no se alteró su comportamiento. Sus cinco tests pasan en el árbol aislado del primer commit, sin incorporar F1A.

## Reproducibilidad autorizada

`Cargo.lock` está versionado: formato 4, 343 paquetes, rustix 1.1.5; SHA-256 `4ba3f1a3c81bfa0d3e608a5d6d02aca9ecd4aa531a4f5a0eea1894074724c15e`. La excepción `!/Cargo.lock` conserva el resto de la política de `.gitignore`.

Se fija la toolchain **1.99.0** realmente comprobada, con rustfmt y Clippy. No se certifica MSRV 1.75, otras arquitecturas ni construcción bit a bit reproducible. El lock no fija kernel, paquetes nativos, Qt o modelos. Se usó la caché local; no se descargaron dependencias ni modelos. Detalles en [F1A-TOOLCHAIN](F1A-TOOLCHAIN.md).

Validación básica automatizada: `python3 scripts/validate_f1a.py`. `--mounts` añade el test opt-in de montaje; `--piper-inherited` reproduce aparte los 12 tests heredados y falla si fallan. El script anuncia sus exclusiones, usa lock y fixtures/XDG privados, conserva logs, limpia datos sintéticos y no arranca la aplicación. Esta preparación puntual está autorizada en esta consolidación; no abre la refactorización de F1B.

## Resultado final y límites

- `cargo fmt --check`: exit **0**; los dos fallos heredados de formato quedan resueltos por el commit 1.
- `cargo check --locked --all-targets`: exit **0**.
- `cargo clippy --locked --all-targets -- -D warnings`: exit **0**.
- `cargo test --locked --test f1a_security`: exit **0**, **19 passed; 0 failed; 1 ignored**.
- Test de montajes opt-in: exit **0**, **1 passed; 0 failed; 19 filtered out**; padre e hijo reejecutan el mismo test, no son dos tests distintos.
- Unitarios seguros: exit **0**, **125 passed; 0 failed; 14 filtered out**. Incluyen los cinco unitarios F1A, que también aprobaron su selección dedicada en el commit 2.
- `app_paths`: exit **0**, **2 passed; 0 failed**.
- ENOSYS aislado para openat2 y renameat2: exit **0**, **1 passed por ejecución**; sin fallback ni efectos exteriores.
- Piper heredado, selección separada: exit **101**, **0 passed; 12 failed; 127 filtered out**, todos por `piper-tts no encontrado`. No se instaló ni simuló el motor.

Se comprobaron **25 tests F1A únicos**: 20 de integración, contando el montaje ejecutado explícitamente, más cinco unitarios. Las repeticiones, los cinco tests de hotkey y los unitarios incluidos en la selección segura no se suman otra vez. No hay una regresión nueva abierta observada en la selección ejecutada; la suite completa sigue sin estar aprobada.

Persisten los límites aceptados/documentados: reemplazos con inode nuevo y pérdida de atributos/permisos; edición posible de un archivo readonly si su entrada puede sustituirse; backup/principal no transaccionales y append susceptible a pérdida de actualizaciones; retención y límites de vistas; configuración antigua/inexistente que requiere intervención explícita; delegaciones bloqueadas hasta F2; ausencia de sandbox contra traslado de directorios por el mismo UID. SEC-03–SEC-10, voz, UI, proveedores reales, descarga de modelos y release quedan fuera de alcance.

La evidencia por árbol, comandos, secretos, filtros y regresiones está en [F1A-COMMIT-VALIDATION](F1A-COMMIT-VALIDATION.md). Los tres commits son compilables sobre sus padres; no se mantiene el bloqueo de compilación independiente descrito en la revisión anterior. Esto valida la consolidación local de F1A, sin afirmar que el producto completo esté listo para publicación.
