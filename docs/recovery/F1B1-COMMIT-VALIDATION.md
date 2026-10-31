# F1B-1 — Validación de commits locales

Fecha: 2026-10-10. Estado **CONSOLIDATED / READY_FOR_REVIEW**. Rama `recovery/f1b1-text-startup`. Base `ca82e722935e8076f526a484655f2f95f87b5d4c`.

## Identidad exacta de los árboles

| Unidad | Padre | Commit | Árbol |
| --- | --- | --- | --- |
| A: núcleo/arranque | `ca82e722935e8076f526a484655f2f95f87b5d4c` | `ddb3bce7610c1e770ab1d90c21e1050b3e37c40e` | `3d3206896d225caaac565c895ccb68d4ae9e8748` |
| B: tests/informes | `ddb3bce7610c1e770ab1d90c21e1050b3e37c40e` | `600a363ee3a9d64d9ea2adbdbda5448efacc7e8a` | `916e4c1f5930a5f363f8cbdb2ee02b758749d497` |

Se creó cada árbol mediante git write-tree; se exportó con git archive antes del commit y se compiló/probó en un directorio temporal independiente. Cargo.lock del padre permanece intacto. XDG_CONFIG_HOME, XDG_DATA_HOME, XDG_CACHE_HOME y TMPDIR privados; umask 077; claves *_API_KEY retiradas; Cargo offline; CARGO_TARGET_DIR compartido sólo como cache de compilación. Las pruebas ejecutables usan config/SQLite temporales y proveedores loopback.

Comprobaciones antes de cada commit: manifest staged exacto, diff completo revisado, `git diff --cached --check` exit 0, lectura de credenciales ficticias y búsqueda de patrones de claves reales/private keys/tokens hex literales con cero coincidencias. Después: padre, árbol publicado y archivos de diff-tree iguales a lo revisado. Base..HEAD contiene exactamente A y B. Históricos: once hashes SHA256 iguales al inicio. Sin cambios tracked ajenos.

Git no tenía identidad: primer intento A exit 128 sin commit; se reutilizó identidad `Codex <codex@localhost>` de F1A mediante `-c` para ambos commits, sin configurar global ni localmente. Creación final de ambos commits exit 0.

## Compilación y gates sobre cada padre

Los tiempos incluyen compilación; no son benchmarks. Logs y JSON de cada árbol están en `/tmp/kda-f1b1-closure-3i6zho0b/`.

| Árbol | Comando | Exit | Segundos | Log |
| --- | --- | --- | --- | --- |
| base | `cargo check --locked --all-targets` | 0 | 10.201 | `base-0.log` |
| A | `cargo fmt --check` | 0 | 0.575 | `A-0.log` |
| A | `cargo check --locked --all-targets` | 0 | 9.743 | `A-1.log` |
| A | `cargo clippy --locked --all-targets -- -D warnings` | 0 | 10.232 | `A-2.log` |
| A | `cargo test --locked --test f1a_security` | 0 | 41.736 | `A-3.log` |
| B | `cargo fmt --check` | 0 | 0.618 | `B-0.log` |
| B | `cargo check --locked --all-targets` | 0 | 9.61 | `B-1.log` |
| B | `cargo clippy --locked --all-targets -- -D warnings` | 0 | 10.467 | `B-2.log` |
| B | `cargo test --locked --test f1a_security` | 0 | 39.697 | `B-3.log` |
| B | `cargo test --locked --test f1b1_text_startup -- --test-threads=1` | 0 | 8.123 | `B-4.log` |
| B | `python3 scripts/validate_f1b1.py --qml` | 0 | 37.362 | `B-5.log` |

F1A en A y B: **19 passed, 0 failed, 1 ignored**. F1B1 en B: **13 passed, 0 failed, 1 ignored** (Qt opt-in se ejecuta explícitamente dentro del script). A no depende de las pruebas/informes nuevos de B para compilar. El padre de B es exactamente A ya validado, no un árbol de trabajo diferente.

## Resultado interno de validate_f1b1.py --qml sobre el árbol B

Evidencia: `/tmp/kda-f1b1-closure-3i6zho0b/B-tmp/kda-f1b1-check-q_0j0fv9/`, nueve logs y results.json. Los directorios de datos de los fixtures se eliminaron; los logs se conservan privados.

| Selección | Exit | Resultado | Segundos |
| --- | --- | --- | --- |
| `fmt` | 0 | PASS | 0.585 |
| `check` | 0 | PASS | 0.39 |
| `clippy` | 0 | PASS, sin warnings | 0.498 |
| `f1a-security` | 0 | 19 passed; 0 failed; 1 ignored | 0.567 |
| `f1b1` | 0 | 13 passed; 0 failed; 1 ignored | 1.113 |
| `unit-safe` | 0 | 137 passed; 0 failed; 2 filtered out | 27.83 |
| `piper-inherited-now-lazy` | 0 | 12 passed; 0 failed; 127 filtered out | 1.566 |
| `app-paths` | 0 | 2 passed; 0 failed | 1.692 |
| `qml-offscreen` | 0 | 1 passed; 0 failed; 13 filtered out | 2.968 |

Las 12 pruebas heredadas Piper están incluidas en los 137 unitarios y se repitieron por nombre exacto: no son 149 pruebas diferentes. Las ejecuciones padre/hijo de fixtures tampoco se suman dos veces. Excluidos de unit-safe exactamente:

- `backend::wakeword_ml::tests::download_and_extract_onnx_runtime`
- `backend::tool_executor::tests::show_image_huge_url_is_rejected_before_download`

No se ejecutó la suite heredada completa ni tests de proveedores reales/voz/hardware. El test de límites de mounts de F1A sigue ignored en esta selección final; su ejecución opt-in previa está documentada en F1B1-RESULTS.md. SEC-01/SEC-02 se conservaron sin cambios; sin nuevas regresiones observadas en las pruebas ejecutadas.

## Qué acreditan las pruebas F1B1

- Primer arranque restrictivo, config existente intacta y config corrupta preservada con error.
- Piper ausente en PATH y modelos/runtime ausentes de fixtures; texto inicia sin captura/ML/descargas. Un guard estático se complementa con comprobación dinámica de models/lib; no es captura general de tráfico.
- Clave ausente: error explícito y cero peticiones a mock; token vacío inválido; token malformado en bootstrap bloquea binario; autoridades maliciosas 403.
- Config/backup/token/QML privados, GET redactado, guardado vacío conserva secretos incluidos legacy/mapa activo vacío o diferente/cambio de proveedor/relectura de disco; reemplazo explícito permitido.
- Catálogo de dos mocks: URL/Authorization corresponden al seleccionado y el anterior recibe cero requests.
- Dos turnos escritos ASCII, cuatro mensajes SQLite y misma sesión/token tras recrear Backend/servidor; no es reinicio completo de una sesión KDE ni prueba Unicode fragmentado.
- Puerto ocupado, config/token inválidos, falta de qml6 y QML con fallo real de carga: errores visibles, exit no cero y puerto liberado. SIGINT cierra hijo propio identificado por argv; qml6 ajeno del fixture permanece vivo, luego lo limpia su propio guard.
- Publicación exclusiva conserva un ganador existente; helper de bootstrap rechaza un resultado EIO inyectado. No se inyectó una syscall fsync real.

Marcadores Qt verificados:

```text
F1B1_UI_PASS; QML terminado y recogido; cuatro mensajes persistidos
F1B1_BINARY_UI_PASS; hijo QML propio recogido; QML ajeno sigue vivo; puerto liberado
F1B1_QML_FAILURE_PASS; fallo real de carga QML visible; puerto liberado; QML ajeno vivo
```

Qt offscreen: **PASS** para el flujo y lifecycle probado. **KDE Plasma Wayland real: NOT_TESTED**. Unicode/cancelación profunda: conocidos y no resueltos; F1B-2 no iniciado. Durabilidad post-rename/backup conjunto, SIGKILL, ataques del mismo UID y supervisor avanzado no se certifican.

## Manifests de commits

### A

- `Cargo.toml`
- `qml/Main.qml`
- `qml/components/SettingsDialog.qml`
- `src/backend/auth.rs`
- `src/backend/chime_player.rs`
- `src/backend/hotword.rs`
- `src/backend/http_server.rs`
- `src/backend/mod.rs`
- `src/backend/private_files.rs`
- `src/backend/speech_service.rs`
- `src/bin/snap_ui.rs`
- `src/lib.rs`
- `src/main.rs`
- `src/models/config.rs`
- `src/owned_process.rs`

### B

- `docs/recovery/F1B1-ARCHITECTURE.md`
- `docs/recovery/F1B1-REPORT.md`
- `docs/recovery/F1B1-RESULTS.md`
- `scripts/validate_f1b1.py`
- `tests/f1b1_text_startup.rs`
- `tests/qml/F1B1Text.qml`

## Validación del parche y comprobación final

`docs/recovery/F1B1-CHANGES.patch`: 140791 bytes; SHA256 `b78122212e4f60acca7dced31b00beeb7c7160df353c8400066db8bc4fcda06f`. Generado exclusivamente desde `git diff --binary --full-index BASE HEAD -- <21 archivos A+B>`. `git apply --check` exit 0; aplicado a copia temporal del base y los 21 contenidos resultantes coinciden byte por byte con HEAD. No incluye documentos históricos sin versionar ni artefactos/datos sensibles ni los tres entregables posteriores al commit.

`git diff HEAD --` vacío; índice vacío; cadena de dos commits y manifests exactos verificados. Puerto 8765 libre al final; ningún proceso qml6/backend con argv de fixtures permaneció. Los guards de prueba recogen sus propios hijos.

Los tres entregables de cierre/parche se dejan sin versionar, junto a los once históricos preservados, para no crear un tercer commit ni alterar los dos SHA validados. Inventario completo y conclusiones en [F1B1-CLOSURE.md](F1B1-CLOSURE.md).
