# F2 — Validación de commits y parche

Rama `recovery/f2-permission-engine`. Baseline y padre de A: `991f5c78dddf0b4f208bf5683604db2cb5c51112` (árbol `23f33be11391d3b726f879de6b1462acad19974a`). Índice inicial vacío; historial sin seguimiento excluido. `Cargo.lock` sin cambio; ninguna dependencia nueva ni trabajo F3/F4.

| Commit local | Padre | Árbol staged validado = árbol commit | Contenido |
| --- | --- | --- | --- |
| A `92ea1674ae2f9822ae423a2e03df2f719f2d05f2` | baseline | `5b41a0faa1388a2ae05540ce3b3b0c4c34b16e18` | Rust, QML, migración 005 y adaptaciones mínimas F1 para compilar/regresar |
| B `8b21c7fd505c8c6b5d0e9bcb5c7ecbd7455c9835` | A | `91ff63ae4e8ec7df31cb5117974e9be6b00e95a4` | suites, runner y cinco informes F2; teardown de fixture F1B-1 |

Antes de cada commit se revisó el diff staged completo, nombres, modos, `git diff --cached --check` y líneas añadidas con patrones de credenciales. Sólo aparecen nombres de variables, tokens de fixture y descripciones, sin claves reales. Se produjo `git write-tree`, se extrajo un archivo Git de ese árbol y se validó allí. No hubo modificaciones sin stage al crear los commits. Los hashes del árbol final coinciden con los hashes staged. La identidad de autor del historial, `Codex <codex@openai.com>`, se pasó sólo a cada comando de commit porque el repositorio no tenía identidad Git local configurada.

**Árbol A:** `cargo fmt --check`, `cargo check`, `cargo clippy`, F1A, F1B-1/2/3A/3B, unitarios seguros y QML del runner F1B3B: todos exit 0. `validate_gate_w1r.py`: 11 PASS, 0 FAIL. El primer archivo A se dejó de ejecutar durante una recompilación redundante; la validación completa se repitió usando el target Cargo compartido en el mismo árbol staged.

**Árbol B final:** `python3 scripts/validate_f1b3b.py --qml`: todos los pasos exit 0. `python3 scripts/validate_f2.py`: fmt/check/Clippy/default y fixtures PASS; F1A 19 PASS/1 IGNORED; F1B-1 13/1; F1B-2 20/3; F1B-3A 26/1; F1B-3B 33/3; F2 17/0; HTTP F2 1 PASS/160 FILTERED; QML F2 4 PASS; GATE-W1R 11 PASS. `python3 scripts/validate_gate_w1r.py` independiente: 11 PASS. Todos con Cargo offline, XDG/SQLite privados y sin proveedor externo. Un archivo B previo falló por el fixture F1B-1 que reabría SQLite demasiado pronto; tras corregirlo se construyó y validó un árbol staged nuevo. No se comprometió el árbol fallido.

## Reproducción del parche

- Comando conceptual: `git diff --binary 991f5c78dddf0b4f208bf5683604db2cb5c51112 8b21c7fd505c8c6b5d0e9bcb5c7ecbd7455c9835 > docs/recovery/F2-CHANGES.patch`.
- Parche: 171 269 bytes; SHA-256 `6d6d928d1bd68955e716e930b5997005c641c4cc9e29a04cf29397f94c7c915c`.
- Se extrajo `git archive` del baseline a un directorio temporal limpio. Su `git write-tree` dio `23f33be11391d3b726f879de6b1462acad19974a`.
- `git apply --check` pasó. `git apply` y `git add -A` produjeron árbol `91ff63ae4e8ec7df31cb5117974e9be6b00e95a4`, igual a `HEAD^{tree}`.
- Se calcularon SHA-256 de los contenidos de los 251 archivos rastreados del árbol reconstruido y de un archivo Git independiente de HEAD. Ambos manifiestos ordenados por path dieron `ffb188dfc4a83358bdc6791ef71c7e0aa05a192c69aa51e8c098b8934ab9da14`.

`F2-CLOSURE.md`, este recibo y `F2-CHANGES.patch` permanecen sin versionar deliberadamente para evitar autorreferencia. No se hizo push, merge ni release.
