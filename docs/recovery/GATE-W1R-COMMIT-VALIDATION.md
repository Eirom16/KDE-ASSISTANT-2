# GATE-W1R — Validación de commits y parche

Fecha: 2026-10-13. Rama `recovery/gate-w1r-ux-performance`. Baseline `e00fc1f77322b8fd1dace4c9c4858327a141f82a` (árbol `2ee7dd18e778b10a8344aa42bef7eb61f11fd82b`). Al comienzo, HEAD era exactamente el baseline y no había commits inesperados. Los 26 documentos históricos sin seguimiento se registraron por SHA-256 antes de preparar el índice y se conservaron. Se usaron `git add` con rutas explícitas, sin `reset`, `clean` ni stash.

## Commit A: cuatro QML de producción

- SHA: `28e09b98cd6baade2830617f929d68867a917926`.
- Padre: `e00fc1f77322b8fd1dace4c9c4858327a141f82a`.
- Árbol staged archivado y árbol del commit: `230418119d463b5248f90ccef22c0d93130f282e`.
- Únicos archivos: `qml/Main.qml`, `qml/components/ChatView.qml`, `qml/components/InputBar.qml`, `qml/components/TrayMenu.qml`.
- `git diff --cached --check`: PASS antes del commit. Revisión de credenciales: sólo referencias a API key/token y header Bearer existentes en código; ningún valor secreto o archivo personal agregado.
- `git archive` del árbol staged extraído en `/home/eirom/.cache/kda-w1r-stage-A-j8ufsh3z/source`. `cargo build --locked --offline`: PASS, 46,33 s. `python3 scripts/validate_f1b3b.py --qml`: PASS, 13 gates, 127,5 s; evidencia del runner `/tmp/kda-f1b3b-check-k898zw9o`. Las pruebas W1R nuevas todavía no existían en A y no se les atribuye PASS.

## Commit B: regresiones y tres informes

- SHA: `991f5c78dddf0b4f208bf5683604db2cb5c51112`.
- Padre: `28e09b98cd6baade2830617f929d68867a917926`.
- Árbol staged archivado y árbol del commit: `23f33be11391d3b726f879de6b1462acad19974a`.
- Únicos archivos: `tests/qml/tst_gate_w1r.qml`, `scripts/validate_gate_w1r.py`, `docs/recovery/GATE-W1R-REPORT.md`, `GATE-W1R-RESULTS.md`, `GATE-W1R-ISSUES.md`.
- `git diff --cached --check`: PASS antes del commit. Ninguna clave, credencial ni dato personal en los cinco archivos; el runner elimina variables de claves/proxy y usa XDG privado.
- `git archive` del árbol staged extraído en `/home/eirom/.cache/kda-w1r-stage-B-hil3ajnj/source`. `cargo build --locked --offline`: PASS, 42,22 s. `python3 scripts/validate_f1b3b.py --qml`: PASS, 13 gates, 125,4 s; evidencia `/tmp/kda-f1b3b-check-_p2dc333`. `python3 scripts/validate_gate_w1r.py`: PASS, 11 casos QML offscreen, 1,78 s.

Los builds usaron `CARGO_TARGET_DIR` del repositorio para reutilizar dependencias compiladas, pero `cwd`, manifest y fuentes eran los archives staged. Los runners usaron sus propios XDG/SQLite sintéticos. El archivo `receipt.json` y logs de cada archive están junto a sus directorios de extracción. En el árbol de trabajo final, `python3 scripts/validate_f1b3b.py --qml` y `python3 scripts/validate_gate_w1r.py` también pasaron. La primera ejecución F1B concurrente con Wayland falló exclusivamente por puerto 8765 ocupado por el proceso propio; su repetición exclusiva pasó antes de los commits.

## Parche baseline → HEAD

Parche: [GATE-W1R-CHANGES.patch](GATE-W1R-CHANGES.patch). Tamaño **42088 bytes**; SHA-256 **`d6a57df4a2503debf059304ff9e7bbbf705011099ae5972980fd7cd2246d9c56`**. Cubre nueve archivos: los cuatro QML de A y los cinco tests/documentos de B. Se generó con `git diff --binary baseline HEAD`, por lo que no incluye los dos documentos de cierre ni los documentos históricos sin seguimiento.

Se extrajo un `git archive` limpio del baseline en `/home/eirom/.cache/kda-w1r-patch-nv5awppy/reconstructed`. `git apply --check` pasó allí; tras `git apply`, un índice Git privado produjo el árbol **`23f33be11391d3b726f879de6b1462acad19974a`**, idéntico a `HEAD^{tree}`. No se alteró el índice del repositorio para esta reconstrucción. Recibo privado: `/home/eirom/.cache/kda-w1r-patch-nv5awppy/receipt.json`.

## Evidencia Wayland adicional

Automatización real de KWin/DBus y conversación en `/tmp/kda-gate-w1r-7pmmhybe/`:

| Evidencia | Resultado |
| --- | --- |
| `review-tray.log` | PASS: ocultar/mostrar, minimizar/restaurar en un clic, cierre/restauración, tres ciclos. |
| `review-focus.log` | PASS: ventana propia auxiliar produjo KDE Assistant visible/inactiva; un Activate la activó sin ocultarla. |
| `chat-qml-journal.log` | PASS: dos turnos UTF-8, cancelación antes/durante, cambio de sesión sin eventos obsoletos, 503 visible. |
| `unique-qml-journal.log` | PASS: 50 deltas distintos, prefijos correctos, final exacto de 150 caracteres. |
| `review-metrics.json` | Tres turnos de 50 deltas sin profiler: CPU QML 14,44/16,43/16,08 %; primer token 55/38/28 ms. |

La automatización de KWin no equivale a clic físico ni observación visual humana. Qt offscreen se usó sólo para los tests de regresión. Los procesos propios se recogieron y los puertos 8765/8877 quedaron libres. Al cierre, índice y archivos versionados están limpios; el parche y estos dos recibos de cierre quedan sin seguimiento junto a los 26 documentos históricos preservados.
