# F3 — Validación de commits y reconstrucción

Evidencia privada: `/var/tmp/kda-f3-audit-577vs_tr`. Este documento y cierre/parche se crearon después
de ambos commits y permanecen sin versionar para evitar autorreferencia.

## Integridad Git

Baseline `8b21c7fd505c8c6b5d0e9bcb5c7ecbd7455c9835`; rama recovery/f3-kde-wayland-integration; dos commits exactos.
A `e4bb565e6961c1262a93121ab88057713157c6ac` / tree `7c1b5d9db9f61e98f0508f109595f10a779d9229` / padre baseline.
B `1d2186e5dae78ae9ddc1dbed01c44c9b7d67f920` / tree `b71420c56ced8874e3889fb31e8b6f29ea43cd80` / padre A.
Cada tree del commit equivale al tree staged que se archivó y probó antes.
No reset, clean, stash, push, merge ni modificación de otros worktrees.

El primer commit falló antes de crearse por falta de identidad configurada.
Se reutilizó por comando (-c) la identidad Codex del baseline, sin modificar
configuración Git global/local. No se atribuye autoría a otra persona.

## Manifests staged exactos

A incluye tres ajustes mínimos previos: app_paths y startup para recursos/argv;
F1B2 para esperar flock real tras Drop. Se separan de las suites nuevas de B
para que las regresiones del árbol A sigan siendo válidas y estables.

### A — 29 archivos

```text
Cargo.lock
Cargo.toml
qml/Main.qml
qml/agent/AgentOverlay.qml
qml/auth/AuthToken.qml
qml/components/AgentDashboard.qml
qml/components/AuditDialog.qml
qml/components/DesktopWindowController.qml
qml/components/KdeWindowActivation.qml
qml/components/SettingsDialog.qml
qml/components/TrayMenu.qml
qml/components/qmldir
src/app_paths.rs
src/backend/auth.rs
src/backend/conversation_turn.rs
src/backend/global_shortcuts.rs
src/backend/hotkey_listener.rs
src/backend/http_server.rs
src/backend/kde_integration.rs
src/backend/mod.rs
src/backend/tool_executor.rs
src/desktop.rs
src/lib.rs
src/main.rs
src/models/config.rs
src/owned_process.rs
tests/app_paths.rs
tests/f1b1_text_startup.rs
tests/f1b2_conversation.rs
```

Tree `7c1b5d9db9f61e98f0508f109595f10a779d9229`; 255 blobs antes de compilar.
Archive SHA-256 `cc5374298ef060f80a0402af58fa112aa500b36e06fe579b73db16d080a20d56`.
Diff staged SHA-256 `e94513d15252374c770bb2052f38a8205f54573c1bf1a4743d5d9201426691db`.

### B — 15 archivos

```text
docs/recovery/F3-ADAPTER-MATRIX.md
docs/recovery/F3-ISSUES.md
docs/recovery/F3-KDE-CONTRACT.md
docs/recovery/F3-REPORT.md
docs/recovery/F3-RESULTS.md
scripts/f3_runtime.py
scripts/qml_test_harness.py
scripts/validate_f2.py
scripts/validate_f3.py
scripts/validate_gate_w1r.py
tests/f3_desktop.rs
tests/f3_process_owner.rs
tests/qml/tst_f3_desktop.qml
tests/qml/tst_gate_w1r.qml
tests/qt/quicktest_main.cpp
```

Tree `b71420c56ced8874e3889fb31e8b6f29ea43cd80`; 267 blobs antes de compilar.
Archive SHA-256 `8f73bba424ec02bc57206cacd1a8076e9a749295f1b01dbc03acbf2b835e770c`.
Diff staged SHA-256 `2c66eb9376e719c0476167781c0e766f120089c047c22aa18238d67f3b57ce00`.


Se revisaron los diffs completos guardados como a-staged.patch y b-staged.patch,
comparación estructural Cargo.lock y fuentes por responsabilidad. Cero versiones
previas eliminadas/actualizadas; nuevas dependencias zbus y unificación de features.
Escaneo de credenciales más revisión de valores/paths: sin secretos detectados;
único /home/eirom añadido en B es un comando de reproducción con ruta del repo.
No se incluyeron server.token, módulos privados generados, SQLite ni logs personales.
Los históricos se comprobaron por SHA-256 y no aparecen en ningún manifest.

Cada git archive se extrajo fuera de worktrees; antes de ejecutar se verificaron
contenido Git-blob, modos y lista exacta de archivos. Después se revalidaron las
fuentes: único extra permitido fue target, enlace al caché de compilación compartido.
A usa shim externo QApplication para los scripts baseline; su fuente equivale al
harness de 10 líneas incluido en B. Se evita el fallo QWidget sin desactivar tray.
No se construyó ni cambió un paquete global.

## Comandos y gates exactos

Cada fila tiene JSON con cwd, argv, exit, duración y conteos en la raíz de evidencia.
A/B indican ejecución desde el árbol archivado, no desde el workspace modificado.

| Registro | CWD / selección | Comando | Exit | Segundos |
|---|---|---|---:|---:|
| gate-f3 | `/home/eirom/KDE-ASSISTANT-2` | `python3 scripts/validate_f3.py` | 0 | 137.106 |
| gate-plasma | `/home/eirom/KDE-ASSISTANT-2` | `python3 scripts/validate_f3.py --specific-only --plasma` | 0 | 55.985 |
| gate-f2 | `/home/eirom/KDE-ASSISTANT-2` | `python3 scripts/validate_f2.py` | 0 | 62.02 |
| gate-f1b3b | `/home/eirom/KDE-ASSISTANT-2` | `python3 scripts/validate_f1b3b.py --qml` | 0 | 58.68 |
| gate-w1r | `/home/eirom/KDE-ASSISTANT-2` | `python3 scripts/validate_gate_w1r.py` | 0 | 17.048 |
| fmt | `/home/eirom/KDE-ASSISTANT-2` | `cargo fmt --check` | 0 | 2.427 |
| clippy | `/home/eirom/KDE-ASSISTANT-2` | `cargo clippy --locked --all-targets -- -D warnings` | 0 | 0.947 |
| clippy-fixtures | `/home/eirom/KDE-ASSISTANT-2` | `cargo clippy --locked --features legacy-fixtures --all-targets -- -D warnings` | 0 | 0.596 |
| production-build | `/home/eirom/KDE-ASSISTANT-2` | `cargo build --locked --bin kde-assistant` | 0 | 0.64 |
| archive-a-build | `/var/tmp/kda-f3-audit-577vs_tr/archive-a` | `cargo build --locked --bin kde-assistant` | 0 | 59.33 |
| archive-a-f2 | `/var/tmp/kda-f3-audit-577vs_tr/archive-a` | `python3 scripts/validate_f2.py` | 0 | 268.883 |
| archive-a-f1b3b | `/var/tmp/kda-f3-audit-577vs_tr/archive-a` | `python3 scripts/validate_f1b3b.py --qml` | 0 | 52.052 |
| archive-a-w1r | `/var/tmp/kda-f3-audit-577vs_tr/archive-a` | `python3 scripts/validate_gate_w1r.py` | 0 | 1.848 |
| archive-b-build | `/var/tmp/kda-f3-audit-577vs_tr/archive-b` | `cargo build --locked --bin kde-assistant` | 0 | 48.998 |
| archive-b-f3 | `/var/tmp/kda-f3-audit-577vs_tr/archive-b` | `python3 scripts/validate_f3.py` | 0 | 355.701 |
| archive-b-plasma | `/var/tmp/kda-f3-audit-577vs_tr/archive-b` | `python3 scripts/validate_f3.py --specific-only --plasma` | 0 | 50.832 |
| restored-build | `/home/eirom/KDE-ASSISTANT-2` | `cargo build --locked --bin kde-assistant` | 0 | 47.623 |
| restored-binary | `/home/eirom/KDE-ASSISTANT-2` | `python3 /var/tmp/kda-f3-audit-577vs_tr/restore_check.py` | 0 | 0.402 |


F3 completo invoca validate_f2.py, validate_f1b3b.py --qml y validate_gate_w1r.py;
además se ejecutaron individualmente en workspace. Sus repeticiones no se suman.
A W1R: 11 PASS; B W1R: 12 PASS (nuevo pin deshabilitado). F3 QtTest: 12 PASS,
incluye init/cleanup. F2 consentimiento: 4 PASS. F3: 21 PASS, ownership hilo 1,
HTTP/SSE 1 (157 FILTERED), recursos 3, tema 3 (155 FILTERED). F2: 17 PASS.
F1A 19 PASS/1 IGNORED; F1B1 13/1; F1B2 20/3; F1B3A 26/1; F1B3B 33/3.
Las selecciones Qt opt-in F1B1/F1B2/F1B3B pasan; EXTERNAL F1A queda excluido.
HUMAN_VERIFIED NOT_TESTED; shortcut real NOT_TESTED; blur/pin NOT_SUPPORTED.
Los resultados AUTOMATED_KWIN sólo son los del flag --plasma, con PID propio;
si el compositor deniega foco el runner lo registra BLOCKED, no PASS.

El binario de desarrollo se recompiló/verificó al volver al workspace, porque
los archives usan otro CARGO_MANIFEST_DIR y comparten caché target. El chequeo
final observa el argumento Main.qml del hijo propio y su cierre normal.
La primera restauración detectó todavía la raíz archive-b: un build Fresh y
recompilar sólo main no renovaban la librería cacheada. Se forzó el rebuild de
librería/binario actualizando únicamente mtimes de main.rs y app_paths.rs;
restore-metadata.json prueba hashes de contenido idénticos, sin tercer commit.
El chequeo final PASS observa /home/eirom/KDE-ASSISTANT-2/qml/Main.qml.
Los FAIL de artefacto previos se conservan como restored-binary-*-fail.log/json.

## Parche reproducible

Comandos: git diff --binary --full-index 8b21c7fd505c8c6b5d0e9bcb5c7ecbd7455c9835 HEAD > F3-CHANGES.patch;
git archive del baseline; git apply --check y git apply sobre esa copia limpia.
SHA-256 `5ce8ae62f8dd16064b24e63dfb0d386820147eb7ff2ece2627be38082732f774`; 277747 bytes; 44 rutas del diff.
Tras aplicar: 267 archivos, modos y blobs iguales a HEAD, sin extras.
Se reconstruyó un índice temporal independiente con esos blobs/modos y write-tree
produjo `b71420c56ced8874e3889fb31e8b6f29ea43cd80`, exactamente el tree del commit B. El índice real no se tocó.

La validación del parche está en patch-validation.json; manifiestos y resultados
de cada archive en a/b-staged.json, a/b-archive.json y archive-*-*.json/log.
Riesgos/limitaciones humanas permanecen en F3-ISSUES.md y F3-CLOSURE.md.
