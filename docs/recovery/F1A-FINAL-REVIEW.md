# F1A — Revisión final del parche real

Fecha: 2026-10-10. Recomendación: **NEEDS_FIXES para consolidar un commit independiente sobre el baseline**. La contención SEC-01/02 revisada mantiene sus resultados; no se encontró un defecto crítico nuevo que justifique modificar la implementación. El impedimento demostrado es una dependencia de compilación del trabajo previo del usuario, que debe consolidarse por separado antes de F1A. Este informe no autoriza integración definitiva.

## Estado de Git y separación del trabajo

- Rama: `recovery/f1a-containment`.
- HEAD y baseline: `12da960ed4722401f1cb42b7a96d9cef912cf90a`.
- Cinco archivos de implementación F1A modificados y dos nuevos, enumerados abajo. El árbol también contiene el hotkey del usuario y documentación sin versionar.
- `src/backend/hotkey_listener.rs` conserva SHA-256 `2622363c9d4323d81822d529edc95a449b75b387a379d831ae9e3b37988f40ab`. Su diff queda fuera del parche F1A.
- Los cuatro documentos F0 y los tres informes F1A existentes se preservaron. En esta revisión sólo se añadieron este informe y `F1A-CHANGES.patch`; ningún archivo de implementación cambió. Los hashes de los siete archivos de implementación coinciden antes y después de la revisión.
- El índice real conserva SHA-256 `2bbcae2b90be2cc208fa0846ca94b62677e0af4a7b666136c7eba30e28f1ff9d`; no quedaron archivos staged. No hubo reset, clean, stash, checkout, commit, push ni merge. No se comenzó F1B.

## Correspondencia entre código e informes

| Archivo | Revisión del código real |
| --- | --- |
| `src/backend/safe_fs.rs` | Selección léxica de raíz seguida de apertura con `openat2`; lectura desde FD, resolución restringida de componentes, temporales exclusivos y publicación con `renameat2`. Origen, hijo de copia y `.bak` pasan por este límite. No hay fallback por canonicalización/pathname. |
| `src/backend/tool_executor.rs` | `execute` obtiene una sola copia de configuración y la pasa a los 23 despachos privados. `try_read` fallido produce `success=false` antes del despacho. Flags individuales y allowlist usan la misma copia. |
| `src/backend/ai_service.rs` | Constructor/snapshot rechazan contención y proveedor inválido. Completion y turno de agente fijan configuración; `call_once` la recibe sin volver a leerla. |
| `src/models/config.rs` | Carga ausente/corrupta/inválida devuelve error. Campos omitidos de capacidades quedan apagados, allowlist vacía, confirmación activada; proveedor obligatorio. `Config::default()` permanece para construcción explícita. |
| `tests/f1a_security.rs` | 15 tests con fixtures privados: flags, actualizaciones concurrentes, las tres explotaciones originales, acceso legítimo, carreras, denegaciones sin daño y primitivas indisponibles. Cinco unitarios adicionales están en los módulos afectados. |
| `Cargo.toml`, `src/backend/mod.rs` | Dependencia directa `rustix` con feature `fs` y registro privado del módulo; sin cambios de voz/QML. |

La revisión concuerda con [F1A-REPORT](F1A-REPORT.md), [F1A-RESULTS](F1A-RESULTS.md) y [F1A-DIFF-REVIEW](F1A-DIFF-REVIEW.md), con las precisiones de compatibilidad que siguen. Las cuatro reproducciones iniciales permanecen registradas como **0 passed / 4 failed antes** y **4 passed / 0 failed después**; esta revisión no volvió a ejecutar código vulnerable.

`ai.enable_tool_calling` sigue controlando el catálogo LLM; no sustituye los flags de acciones manuales. Cada ejecución del executor es una operación con snapshot propio, distinto del snapshot del turno HTTP. Una revocación posterior no cancela operaciones ya autorizadas. `ToolCtx` no comprueba consentimiento. `open_file` y KDE Connect `share_file`/`compartir_archivo` permanecen bloqueados explícitamente hasta F2. Notificaciones programadas y persistencia de configuración no fueron reconstruidas ni deben interpretarse como cubiertas por las pruebas de los 23 despachos.

## Btrfs y directorios montados

La apertura inicial de una raíz configurada usa `NO_SYMLINKS | NO_MAGICLINKS`, sin `NO_XDEV`: permite que esa raíz sea un montaje autorizado explícitamente. Dentro de ella se usan `BENEATH | NO_SYMLINKS | NO_MAGICLINKS | NO_XDEV`. Un montaje descendiente, incluido bind mount, se rechaza; una búsqueda puede omitir esa entrada y devolver resultados parciales. La elección de raíz usa la primera coincidencia en la allowlist: una raíz amplia anterior puede impedir acceder a una raíz montada más específica posterior. No se prueba automáticamente otra raíz tras un fallo.

`NO_XDEV` restringe cruces de montajes VFS; no compara `st_dev`. La documentación de Btrfs distingue un subvolumen accesible como directorio de uno montado por separado. Referencias: [openat2(2)](https://man7.org/linux/man-pages/man2/openat2.2.html) y [Btrfs: subvolúmenes](https://btrfs.readthedocs.io/en/latest/Subvolumes.html).

Evidencia nueva, exclusivamente sintética:

- `findmnt -T .`: repositorio sobre `/home`, Btrfs, subvolumen `@home`; observación de sólo lectura. `findmnt -T /tmp`: tmpfs. `findmnt -T /var/tmp`: Btrfs, subvolumen `@tmp`.
- Se repitió el binario de los 15 tests con `TMPDIR=/var/tmp/kda-f1a-final-w89xsuy8` (0700): **15 passed; 0 failed**. No se usó HOME como fixture.
- Se creó un subvolumen privado anidado, sin montaje separado: lectura y creación mediante el executor dieron `success=true`, aun con `st_dev` del padre 59 y del hijo 72. El temporal y el archivo publicado comparten directorio de destino; el rename no requiere cruzar subvolúmenes.
- En un namespace de usuario/montaje privado se montó con bind un directorio sintético sobre un descendiente de la raíz. Con la raíz padre autorizada, lectura y creación dieron `success=false`; autorizando directamente el directorio montado ambas dieron `success=true`. El hijo desmontó el bind antes de terminar; no modificó el namespace de la sesión.

No se certificaron NFS/FUSE, subvolúmenes readonly, todas las variantes de ACL ni otros kernels. El FD representa el directorio autorizado originalmente: un proceso con el mismo UID que pueda trasladar ese objeto fuera de la raíz textual puede conservar acceso a través del FD. Sigue siendo un límite declarado del adversario, no un sandbox contra procesos del mismo UID ni administradores. Las pruebas de symlinks no demuestran confinamiento frente a traslado arbitrario de directorios.

## Archivos preexistentes, permisos y atributos

Crear y copiar sin `overwrite` usan `NOREPLACE`, por lo que una entrada aparecida concurrentemente no se sobrescribe. Un symlink preexistente se rechaza; si aparece durante una publicación con overwrite, el rename sustituye su entrada y no escribe en su referente. Se rechazan directorios como destino final de archivo, objetos especiales y archivos con `nlink != 1`. Copiar a directorio autoriza además el basename derivado. Las comprobaciones de tipo/nlink son observaciones del objeto, no una garantía contra cualquier mutación posterior por otro proceso del mismo UID.

Editar y copiar con overwrite publican un inode nuevo con modo solicitado 0600, sujeto a umask y atributos heredados del directorio. No preservan permisos ejecutables, propietario/grupo anteriores, ACL, xattrs, timestamps ni flags específicos del inode. Watchers/FD abiertos pueden seguir viendo el inode anterior. No hay reflink/copias de atributos Btrfs. ACL heredadas no se probaron; no se presume que toda ACL final quede vacía.

Un probe externo contra la biblioteca real confirmó, en un temporal privado:

- Append sobre `readonly.txt` 0444: `success=true`, modo final 0600, inode 36046 → 36055, xattr sintético `user.f1a_fixture` eliminado.
- Copia de origen 0755 sobre destino 0640: `success=true`, destino 0600 e inode nuevo, xattr del destino eliminado. El origen conservó modo, inode y xattr.

La edición de un archivo readonly es posible cuando se puede leerlo y reemplazar su entrada en el directorio; no se conserva el rechazo que habría producido una escritura directa al inode 0444. Es una incompatibilidad concreta que debe aceptarse antes de publicar. El `.bak` regular preexistente se puede reemplazar: no es un historial versionado. `with_extension("bak")` también hace coincidir backups de archivos con igual stem y distinta extensión; ese diseño ya existía. Un principal que ya termina en `.bak` se rechaza para evitar colisión consigo mismo.

## Efectos parciales y limpieza

El rename publica cada archivo individualmente; backup y principal no forman una transacción. Se sincronizan bytes del temporal, no el directorio. No hay garantía completa ante crash. Append puede perder actualizaciones de otros escritores; el executor no serializa procesos externos. Un `spawn_blocking` ya iniciado no se cancela al abandonar su await.

Para comprobar efectos parciales se limitó **sólo un hijo de diagnóstico** con `RLIMIT_FSIZE=64` y `SIGXFSZ` ignorada, manteniendo los fixtures dentro de su raíz privada. El original medía menos de 64 bytes y la actualización 128:

- Overwrite devolvió `success=false` (`File too large`, errno 27); principal intacto y backup publicado con el original.
- Crear un archivo de 128 bytes en `nested/deep` devolvió el mismo error; quedaron los directorios nuevos, pero ningún archivo final.
- No quedaron `.kda-*` en ese fixture. No hubo escrituras exteriores.

Esto confirma el límite descrito en los informes: denegar autorización inicialmente no destruye datos; un error posterior de I/O puede dejar efectos autorizados parciales. La limpieza del temporal intenta `unlinkat` e ignora su error: no garantiza limpieza si también falla unlink, el proceso termina o el directorio cambia de permisos.

Las vistas locales usan copias privadas; renderers y descargas todavía escriben por pathname dentro de workspaces UUID/0700. No es un fallback de acceso al origen autorizado, pero depende de que otros procesos del mismo UID no manipulen esos workspaces. No hay recolección automática ni límite global de almacenamiento. Un probe de imagen local y preview de extensión no soportada dejó **tres directorios 0700**: copia de imagen, copia del documento rechazado y workspace de render vacío. Permanecían al terminar el proceso. Los snapshots exitosos necesitan una vida útil suficiente para QML; los rechazados también se retienen actualmente. Tamaño total, retención, SSRF y límites de lectura siguen pendientes.

Los probes no forman parte del producto/parche. Se limpiaron sus fixtures y cachés sintéticos tras registrar resultados; los logs privados se conservan temporalmente. Dos incidencias del harness quedaron documentadas: `setxattr` después de chmod 0444 falló antes de ejecutar producción y se corrigió el orden; `btrfs subvolume delete` dio EPERM con el montaje actual, pero vaciar los dos fixtures y usar `rmdir` eliminó completamente el subvolumen. No se usó sudo.

## Compatibilidad de configuración y dependencias

Una configuración válida y completa conserva sus valores explícitos. Las antiguas sin proveedor dejan de cargar; las que omiten flags/allowlist no recuperan capacidades permisivas. Instalaciones nuevas sin configuración dejan de arrancar automáticamente. Un guard de escritura provoca rechazo inmediato de herramientas/AI, que puede requerir repetir la acción. No se añadieron onboarding, migración general ni política de reintentos.

`Config::save` sigue copiando backup y escribiendo por pathname, ignorando algunos errores de backup/permisos; la migración de modelo puede usar ese código. SEC-07 no está completo. La validación del proveedor en carga/AI no reconstruye todas las entradas de actualización del servidor.

`Cargo.lock` existe localmente, pero `git ls-files Cargo.lock` no devuelve entradas y `git check-ignore Cargo.lock` confirma la regla `*.lock`. Las validaciones usaron ese lock/cache; `rustix` resolvió a 1.1.5, cuyo manifest declara Rust 1.65. Eso no certifica Rust 1.75 para el proyecto completo. Sin lock versionado, una instalación limpia puede resolver versiones diferentes; no hay garantía de repetibilidad ni se declara una entrega reproducible. No se cambió `.gitignore`, no se añadió el lock al parche y no se inició F1B. La decisión de versionarlo sigue pendiente.

## Validación repetida

Entorno: Linux `7.2.7-1-cachyos`, x86_64, rustc/Cargo 1.99.0. Directorio de validación 0700 `/tmp/kda-f1a-final-dfs64_he`, XDG y TMPDIR privados, umask 077, claves `*_API_KEY` retiradas, Cargo offline. No hubo tests de proveedor, hardware, UI, descarga de modelos ni instalación/simulación de Piper. Offline de Cargo no se considera una prohibición de red de los tests: los dos tests de red/descarga se excluyeron explícitamente.

| Comando | Exit | Resultado |
| --- | --- | --- |
| `cargo check` | 0 | PASS |
| `cargo check --all-targets` | 0 | PASS |
| `cargo clippy --all-targets -- -D warnings` | 0 | PASS, sin warnings |
| `cargo test --test f1a_security -- --nocapture` | 0 | **15 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out**, 0,13 s |
| `cargo test --lib f1a_ -- --nocapture` | 0 | **5 passed; 0 failed; 0 ignored; 0 measured; 134 filtered out** |
| `cargo test --lib -- --test-threads=1` con los mismos 14 `--skip` de F1A-RESULTS | 0 | **125 passed; 0 failed; 0 ignored; 0 measured; 14 filtered out** |
| `cargo test --test app_paths` | 0 | **2 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out** |
| Selección `cargo test --lib -- --exact --test-threads=1` de los 12 nombres Piper de F1A-RESULTS | 101 | **0 passed; 12 failed; 0 ignored; 0 measured; 127 filtered out**; todos por `piper-tts no encontrado` |
| `cargo fmt --check` | 1 | Diffs heredados en `hotkey_listener.rs:419` y orden de módulos de `src/lib.rs` (hunk desde línea 3); no se modificaron |
| `rustfmt --check --edition 2021 --config skip_children=true` sobre los seis Rust F1A | 0 | PASS |
| `git diff --check` | 0 | PASS |
| `cargo test --test f1a_security --no-run --message-format=json` | 0 | Binario de integración identificado sin ejecutar la aplicación |
| Mismo binario, TMPDIR privado sobre Btrfs | 0 | **15 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out**, 2,37 s |
| Hijo seccomp: openat2 → ENOSYS | 0 | **1 passed; 0 failed; 0 ignored; 0 measured; 14 filtered out** |
| Hijo seccomp: renameat2 → ENOSYS | 0 | **1 passed; 0 failed; 0 ignored; 0 measured; 14 filtered out** |

Son **20 tests nuevos únicos**, no 20 más otros cinco: los cinco unitarios también están entre los 125. Btrfs/seccomp reejecutan tests existentes. Los probes externos verifican características concretas y no se contabilizan como nuevos tests de repositorio. Los nombres exactos de los 20 tests y de las exclusiones están en [F1A-RESULTS](F1A-RESULTS.md); esta revisión repitió esa selección sin alterar los filtros. No se declara aprobada la suite completa.

## Parche y aplicación al baseline

[F1A-CHANGES.patch](F1A-CHANGES.patch) contiene **7 archivos, 1299 inserciones y 438 eliminaciones**, incluidos los dos archivos nuevos:

1. `Cargo.toml`
2. `src/backend/ai_service.rs`
3. `src/backend/mod.rs`
4. `src/backend/safe_fs.rs` (nuevo)
5. `src/backend/tool_executor.rs`
6. `src/models/config.rs`
7. `tests/f1a_security.rs` (nuevo)

Tamaño: **86871 bytes**. SHA-256: `a10d25ebdb14d2a317df1ef58f33a9b22c3161e8c0643f4a20f96fe8b090661c`.

Se generó con un índice alternativo privado: `GIT_INDEX_FILE=<privado>/f1a.index git read-tree <baseline>`, `git add -- <los siete archivos>` con ese mismo índice y `git diff --cached --binary --full-index <baseline> -- <los siete archivos>`. El índice real no cambió. No se incluyeron hotkey, documentación preexistente, informes, este informe, el propio parche, Cargo.lock, caches, logs, configuración personal ni archivos generados. Revisión de las líneas añadidas y detección de patrones de credenciales: **0 coincidencias** de claves OpenAI/OpenRouter/Groq, claves privadas o asignaciones de secretos largos; el contenido añadido usa fixtures sintéticos.

Se extrajo `git archive <baseline>` en un directorio privado. Allí:

| Comprobación | Exit | Evidencia |
| --- | --- | --- |
| `git apply --check <F1A-CHANGES.patch>` | 0 | Aplica sin conflictos sobre el baseline exacto |
| `git apply <F1A-CHANGES.patch>` | 0 | Los siete archivos resultantes coinciden byte por byte con el árbol revisado |
| `cargo check --locked --all-targets --manifest-path <copia>/Cargo.toml`, baseline + sólo F1A | 101 | Cuatro E0599 heredados: falta importar `anyhow::Context` en hotkey, líneas 320/326/331/334; repetidos al construir lib y lib test |
| Mismo check tras aplicar **por separado, sólo en la copia** el diff previo del usuario en hotkey | 0 | PASS |

Para esas dos compilaciones se copió el lock local a la copia privada y se reutilizó el target/cache local. No se incluyó ese lock ni el hotkey en F1A. La corrección previa también elimina el segundo `.as_millis()` sobre un valor ya convertido a milisegundos. La dependencia es de compilación, no de aplicación textual del parche.

## Decisión de consolidación

**NEEDS_FIXES**: F1A se puede revisar/aplicar aisladamente, pero no debe convertirse en un commit independiente que se presente como compilable sobre `12da960...` sin consolidar primero la corrección previa del hotkey. Esa corrección debe conservar su atribución y responsabilidad separada. El árbol local actual compila porque contiene ambos cambios; eso no valida automáticamente el árbol que produciría un commit de sólo F1A.

No hay un nuevo defecto crítico demostrado en SEC-01/02 que requiera editar el parche. Los fallos de formato/Piper son heredados y siguen abiertos. Antes de integración definitiva también debe quedar aceptado el contrato de primer arranque/config antigua, pérdida de atributos y readonly, efectos parciales, retención de vistas y límite frente a traslado de directorios por el mismo UID. La ausencia de Cargo.lock versionado impide certificar reproducibilidad. Este informe identifica esos requisitos sin iniciar su implementación ni autorizar commit/integración.

Logs y metadatos privados: `validation-results.json`, `initial-hashes.json`, `patch-results.json`, `baseline-build-results.json`, `review-probes.json`, `partial-probe.json`, `mount-probe.log`, `btrfs-security.log`, logs individuales de check/Clippy/tests/formato/Piper/seccomp y fuentes de los probes en `/tmp/kda-f1a-final-dfs64_he`. Los resultados relevantes quedan preservados aquí aunque desaparezcan los temporales.
