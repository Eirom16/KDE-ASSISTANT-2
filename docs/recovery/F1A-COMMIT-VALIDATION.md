# F1A — Validación de commits

Fecha: 2026-10-10. Rama `recovery/f1a-containment`, baseline `12da960ed4722401f1cb42b7a96d9cef912cf90a`, HEAD `ca82e722935e8076f526a484655f2f95f87b5d4c`. **Los tres commits locales pasaron validación independiente de su árbol exacto.** No se hizo push, merge ni release; no se inició F1B. Alcance/decisiones: [F1A-CLOSURE](F1A-CLOSURE.md).

## Método y aislamiento

Se añadió al índice real únicamente la lista de archivos de cada responsabilidad. Antes de crear cada commit:

1. Se comprobó la lista exacta con `git diff --cached --name-only`, el diff completo y `git diff --cached --check`.
2. Se obtuvo el árbol con `git write-tree` y se extrajo con `git archive <tree>` en un directorio privado; los cambios sin stage y documentos anteriores quedaron fuera.
3. Se ejecutaron los comandos indicados abajo sobre esa copia, con XDG/TMPDIR privados, umask 077, claves `*_API_KEY` retiradas y Cargo offline. Se compartió sólo el target/cache local de compilación.
4. Se exigieron exits cero y se volvió a comprobar que padre e índice no habían cambiado.
5. Se creó el commit y se verificó que `git rev-parse <SHA>^{tree}` coincidía con el árbol validado y que `git diff-tree` enumeraba exactamente los mismos archivos.

Los commits 1/2 aún no versionan lock: se copió el lock local a sus copias privadas y Cargo pudo ajustar allí el lock para sus respectivos manifests, offline. No se afirma resolución independiente de caché/lock para esos dos commits. El commit 3 contiene el lock y toda su validación utiliza `--locked`. El primer árbol no contiene la implementación F1A ni la toolchain/script posteriores; el segundo no contiene el commit de reproducibilidad. Ningún check se basó exclusivamente en el árbol local que mezclaba cambios futuros.

No hubo reset, clean, stash, checkout destructivo ni reescritura de commits. Se utilizaron tres commits normales. Los informes finales se generan sin commit para registrar sus SHAs definitivos; los nueve documentos/parche preexistentes permanecen sin versionar y con hashes intactos.

## Árboles y manifests comprobados

| Commit | Padre | Árbol validado | Archivos |
| --- | --- | --- | --- |
| `6a088bba20b4f4c00c103c0673bb430e44416a01` | `12da960ed4722401f1cb42b7a96d9cef912cf90a` | `2abca9490b93b87bd9504fbdfef662467a2ae082` | hotkey_listener.rs, src/lib.rs (2). |
| `7dc6938ca73b8fc2994fda0cf29ca8a95b6ca7c7` | `6a088bba20b4f4c00c103c0673bb430e44416a01` | `49afbcfc02985b0a58b59b06e71712554b0f0ad4` | Los siete archivos de seguridad de F1A-CLOSURE. |
| `ca82e722935e8076f526a484655f2f95f87b5d4c` | `7dc6938ca73b8fc2994fda0cf29ca8a95b6ca7c7` | `e93523e698d0f8f507aff507a3b71bb71bcc518c` | .gitignore, Cargo.lock, rust-toolchain.toml, scripts/validate_f1a.py, F1A-TOOLCHAIN.md (5). |

`git show --stat`, diff completo por padre y comparación de manifests/árboles confirmaron esa separación. No hay archivos de configuración personal, `.env`, claves, cachés, logs, modelos ni binarios en esos commits.

## Commit 1: corrección previa, sin F1A

| Comando en su copia aislada | Exit | Resultado |
| --- | --- | --- |
| `cargo fmt --check` | 0 | PASS |
| `cargo check --all-targets` | 0 | PASS |
| `cargo test --lib backend::hotkey_listener -- --test-threads=1` | 0 | **5 passed; 0 failed; 0 ignored; 0 measured; 129 filtered out** |

Los cinco tests son parsing/aliases/modificadores y contrato de archivo UI; no arrancan listeners ni necesitan hardware. Los cambios funcionales proceden del usuario y son anteriores a F1A. El resto es exclusivamente el formato solicitado, comprobado al formatear una copia del hotkey previo y compararla byte por byte con el nuevo archivo.

El baseline solo tenía los errores de `Context` registrados en la revisión anterior. Este árbol compila sin incorporar el executor seguro: la dependencia que impedía un commit independiente de F1A queda resuelta en el padre.

## Commit 2: seguridad y búsqueda, sobre commit 1

| Comando en su copia aislada | Exit | Resultado |
| --- | --- | --- |
| `cargo fmt --check` | 0 | PASS |
| `cargo check --all-targets` | 0 | PASS |
| `cargo clippy --all-targets -- -D warnings` | 0 | PASS, sin warnings |
| `cargo test --test f1a_security` | 0 | **19 passed; 0 failed; 1 ignored; 0 measured; 0 filtered out** |
| `cargo test --lib f1a_` | 0 | **5 passed; 0 failed; 0 ignored; 0 measured; 134 filtered out** |
| `cargo test --lib -- --test-threads=1` con 14 `--skip` enumerados abajo | 0 | **125 passed; 0 failed; 0 ignored; 0 measured; 14 filtered out** |
| `cargo test --test f1a_security -- --exact search_mount_boundaries_remain_closed --ignored --nocapture` | 0 | **1 passed; 0 failed; 0 ignored; 0 measured; 19 filtered out**, tanto hijo como padre |

Los cinco tests de búsqueda nuevos fallaron antes según el registro de reproducción: **1 passed / 3 failed / 1 ignored**, con 15 anteriores filtrados. Tras el cambio: **4 passed / 0 failed / 1 ignored** y montaje explícito aprobado. Los nombres y el alcance están en F1A-CLOSURE. El ignored no se contabiliza como aprobado en la ejecución predeterminada.

## Commit 3: lock y automatización, sobre commit 2

En su copia aislada se ejecutó `python3 scripts/validate_f1a.py --mounts`, exit **0**. El script conservó resultados de cada subcomando:

| Subcomando | Exit | Resultado |
| --- | --- | --- |
| `cargo fmt --check` | 0 | PASS |
| `cargo check --locked --all-targets` | 0 | PASS |
| `cargo clippy --locked --all-targets -- -D warnings` | 0 | PASS |
| `cargo test --locked --test f1a_security` | 0 | **19 passed; 0 failed; 1 ignored; 0 measured; 0 filtered out** |
| `cargo test --locked --lib -- --test-threads=1` más los 14 `--skip` | 0 | **125 passed; 0 failed; 0 ignored; 0 measured; 14 filtered out** |
| `cargo test --locked --test app_paths` | 0 | **2 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out** |
| `cargo test --locked --test f1a_security -- --exact search_mount_boundaries_remain_closed --ignored --nocapture` | 0 | **1 passed; 0 failed; 0 ignored; 0 measured; 19 filtered out**, hijo y padre |

El comando del script duró 96,167 s, incluida compilación. No ejecuta main, modelos, audio ni proveedores. La comprobación del lock encontró 343 paquetes, formato 4 y cero fuentes autenticadas; su hash coincide con el archivo finalmente versionado. El archivo de toolchain selecciona 1.99.0, cuyos componentes rustfmt/Clippy ya estaban instalados; no hubo descargas.

## Repetición final sobre HEAD

Después de crear el tercer commit se volvió a ejecutar `python3 scripts/validate_f1a.py --mounts` en el repositorio real: exit **0**, con los mismos siete subcomandos y resultados de la tabla anterior. Los cuatro comandos exigidos se ejecutaron exactamente como aparecen allí, con `--locked` donde corresponde.

Tiempos de tests finales: integración 0,12 s; unitarios seguros 0,33 s; app_paths 0,00 s; montaje hijo 0,01 s/padre 0,02 s. No son medidas de rendimiento de la aplicación.

También se compiló/identificó el artifact con `cargo test --locked --test f1a_security --no-run --message-format=json` (exit **0**) y se reejecutó el test `unavailable_kernel_primitives_fail_closed` en hijos seccomp del diagnóstico existente:

- openat2 → ENOSYS: exit **0**, **1 passed; 0 failed; 0 ignored; 0 measured; 19 filtered out**.
- renameat2 → ENOSYS: exit **0**, mismo resultado. Sólo read permanece disponible en ese caso; no se publican destinos/backups ni quedan temporales en el fixture.

Es el mismo test reejecutado, no dos tests nuevos. El filtro seccomp sólo afecta al hijo de diagnóstico. El montaje también ocurre exclusivamente en un namespace hijo; se comprueba que difiera del namespace padre antes de montar y se desmonta antes de eliminar fixtures.

## Selección segura y fallos heredados

Se excluyeron de la selección unitaria segura exactamente estos 12 tests de Piper, usando `--skip <nombre>`; después se ejecutaron **por separado**, usando esos mismos nombres como filtros exactos:

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

Comando de reproducción final: `cargo test --locked --lib -- --exact --test-threads=1` seguido de esos 12 nombres. Exit **101**: **0 passed; 12 failed; 0 ignored; 0 measured; 127 filtered out**. Los 12 muestran `inicializando Piper TTS` / `piper-tts no encontrado`. No se instaló ni simuló Piper. No se ocultan como tests aprobados; la suite completa permanece fallida por esta dependencia heredada.

Otras dos exclusiones preventivas:

```text
backend::wakeword_ml::tests::download_and_extract_onnx_runtime
backend::tool_executor::tests::show_image_huge_url_is_rejected_before_download
```

La primera puede descargar ONNX y la segunda necesita loopback. No se ejecutaron ni se consideran aprobadas. El script versionado contiene la selección exacta, anuncia los 14 filtros y ofrece la reproducción Piper opt-in con salida de fallo.

Formato heredado: antes de consolidar fallaba en hotkey y src/lib.rs; el commit 1 lo corrigió con autorización expresa. Ahora pasa en los tres árboles y HEAD. No hay un fallo nuevo observado en check, Clippy, integración o selección segura. No se certifican smokes de UI/voz/proveedor, audio real, modelos, instalación limpia ni toda la suite.

## Verificación de secretos y conservación

En cada diff se revisaron los paths y las líneas añadidas. Búsqueda de credenciales OpenAI/OpenRouter/Groq, claves privadas y asignaciones de secretos largos: **0 coincidencias en cada commit**. Las fuentes de Cargo.lock no contienen usuario/password. Los cambios usan fixtures sintéticos o metadata pública de paquetes; no se incorporaron logs privados ni datos de ejecución. Esta comprobación complementa la revisión, no acredita ausencia de todas las clases de secretos posibles en toda la historia del repositorio.

Los hashes de los nueve documentos/parche existentes son idénticos a los capturados al inicio. Hotkey conserva su corrección funcional previa y sólo cambia por rustfmt. `git diff --check` pasa y no hay modificaciones de archivos versionados ni contenido staged al terminar. Los documentos de cierre nuevos y los documentos históricos conservados permanecen untracked; el commit 3 sí incluye la documentación de toolchain.

No se tocaron QML, arquitectura de voz, sidecar Python ni otros módulos. No se ejecutaron snap_ui, pkill global, pruebas destructivas sobre archivos reales ni pruebas que descarguen modelos. Todos los fixtures son privados/sintéticos; sus datos se limpiaron tras conservar logs. Los límites de compatibilidad y seguridad pendientes permanecen en F1A-CLOSURE y F1A-FINAL-REVIEW.

## Entorno y evidencia

Comprobado: rustc `1.99.0 (b940084d7 2026-09-28)`, Cargo `1.99.0 (5f94df478 2026-08-27)`, Linux `7.2.7-1-cachyos`, x86_64; Python 3.14.8, Git 2.56.0, CMake 4.4.4, Clang 23.1.1. pkg-config: ALSA 1.2.16.1, X11 1.8.13, Xi 1.8.3, Xtst 1.2.5. No se afirma MSRV distinto ni validación de otros entornos.

Evidencia local 0700: `/tmp/kda-f1a-closure-d0h0mszm/`. Contiene `c1-validation.json`, `c2-validation.json`, `c3-validation.json`, `commits.json`, diffs exactos, comandos y logs por árbol, `initial-hashes.json`, `search-before.log`, `search-after.log`, `search-mount.log`, `final-validation.log`, `piper-inherited.log`, resultados/artifacts/diagnóstico seccomp. Logs detallados del script aislado: `tmp/kda-f1a-check-uz8euoul/`; ejecución final: `tmp/kda-f1a-check-04_enisd/`. Sus fixtures/XDG ya fueron eliminados por el script; permanecen logs y results.json. Estas rutas pueden desaparecer; los SHAs, filtros y cifras relevantes quedan registrados aquí.
