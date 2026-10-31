# F1B-1 — Cierre y consolidación local

Fecha: 2026-10-10. Estado: **CONSOLIDATED**. Recomendación: **READY_FOR_REVIEW**. Revisión satisfactoria del alcance de texto tras corregir defectos acotados; sin regresiones nuevas observadas en la selección ejecutada. **Plasma Wayland real: NOT_TESTED**. No se inició F1B-2.

## Cadena de Git

- Rama: `recovery/f1b1-text-startup`.
- Base obligatoria: `ca82e722935e8076f526a484655f2f95f87b5d4c`. Era HEAD al comenzar; no había commits inesperados ni index previo.
- Commit A: `ddb3bce7610c1e770ab1d90c21e1050b3e37c40e` — `feat: consolidar núcleo de texto y arranque seguro F1B-1`. Padre: base.
- Commit B: `600a363ee3a9d64d9ea2adbdbda5448efacc7e8a` — `test: consolidar pruebas y validación de F1B-1`. Padre: A; HEAD final.
- Exactamente dos commits locales en base..HEAD. Sin push, merge, release, reset, clean, stash ni checkout destructivo.

Git carecía de identidad y el primer intento de A devolvió 128 sin crear commit. Se reutilizó `Codex <codex@localhost>`, presente en los tres commits F1A, mediante opciones `git -c` sólo para estos dos commits. No se cambió configuración global/local de Git.

## Archivos exactos

### Commit A — 15 archivos de producción

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

Incluye adaptaciones mínimas de fixtures unitarios ya embebidos en archivos Rust de producción para mantener compilación y contratos; las pruebas F1B1 nuevas están exclusivamente en B. Cargo.lock, AiService, tool_executor, safe_fs, SessionManager y tests F1A permanecen idénticos a la base.

### Commit B — seis archivos de pruebas e informes

- `docs/recovery/F1B1-ARCHITECTURE.md`
- `docs/recovery/F1B1-REPORT.md`
- `docs/recovery/F1B1-RESULTS.md`
- `scripts/validate_f1b1.py`
- `tests/f1b1_text_startup.rs`
- `tests/qml/F1B1Text.qml`

## Revisión del diff y correcciones

Se revisaron todos los cambios de producción y el diff staged completo de ambos commits, incluidos archivos nuevos. Los manifests se compararon exactamente con el plan, `git diff --cached --check` devolvió 0 antes de cada commit y se comprobaron secretos por lectura y patrones de claves/tokens/private keys. Ningún secreto real: sólo nombres de variables y credenciales ficticias de fixtures. Los árboles publicados coinciden con los árboles staged revisados y validados.

| Riesgo | Resultado de revisión y evidencia |
| --- | --- |
| Reserva, HTTP y UI | Listener reservado una sola vez antes de config; se transfiere a Axum sin intervalo probe/rebind. Config/SQLite/token/módulo QML críticos deben funcionar antes de publicar. HTTP puede preceder al primer frame de UI después de ese bootstrap; health 200 y flujo QML lo prueban. |
| Procesos y qml6 | OwnedChild termina/recoge sólo el Child recibido de spawn. Sin cull/pkill global en main/snap_ui. Errores de spawn, exit no cero y try_wait devuelven error. Pruebas de qml6 ausente/carga QML fallida/SIGINT dejan puerto libre; un qml6 independiente del fixture sobrevive y se limpia al terminar su propio test. |
| Primer arranque y config existente | Sólo NotFound crea configuración restrictiva, por publicación exclusiva sin reemplazo. Config válida conserva bytes, datos y credenciales; corrupción se conserva y bloquea con error recuperable. No hay migración general ni cambio de esquema SQLite. |
| Guardado redactado | Corregido caso de mapa activo vacío/diferente de legacy: preserva el valor legacy literal y todas las claves; sólo valores no vacíos enviados explícitamente reemplazan. Prueba de cambio de proveedor y relectura de disco. |
| Catálogo manual | Corregido uso de URL anterior con clave de otro proveedor: selección conjunta de URL/clave, campos legacy descartados sólo en la copia de consulta. Dos mocks locales verifican Authorization y ausencia de peticiones al destino anterior. |
| Token, archivos y autoridades | Token 64 hex no vacío; error de bootstrap bloquea API. Archivos config/backup/token/QML 0600, directorios privados 0700. GET config redacta claves. Host/Origin exactos y bearer vacío rechazado. |
| Escritura/sync/init | Corregido bootstrap que ignoraba cualquier error si el archivo ya existía: únicamente AlreadyExists permite releer al ganador; otros errores se propagan. Prueba EIO inyecta el resultado al helper real, no una syscall. Backup symlink produce 500 sin modificar RAM/principal/archivo exterior. Límite de fsync posterior a rename descrito abajo. |
| Voz opcional | No downloads ni prewarm desde startup; Piper OnceCell, Whisper/ONNX/ML diferidos, captura None. Texto inicia con Piper ausente y XDG sin modelos; wake antiguo true no activa audio. Dependencias de compilación de voz se conservan. |
| SEC-01/SEC-02 | Código de mitigación F1A sin cambios y 19 pruebas de seguridad aprobadas; cinco unitarios de contención incluidos en los 137. No regresiones nuevas observadas. El test de mounts es opt-in, no ejecutado en esta repetición final. |
| Calidad de las pruebas | Health requiere 200 y PID QML se identifica por argv. Dos turnos ASCII con cuatro mensajes SQLite y reinicio real de los objetos Backend/servidor. Guard estático de downloads complementado por filesystem vacío, sin afirmar captura total de tráfico. Qt prueba bindings/estado del chat y lifecycle offscreen; no certifica compositor KDE. |

## Validación final del árbol que quedó publicado en B

Los seis comandos pedidos se ejecutaron en una copia `git archive` del árbol exacto staged de B, con XDG/TMPDIR privados, claves de entorno retiradas, Cargo offline y mocks loopback. El hash del árbol de B se comprobó después de crear el commit. Base y A se compilaron por separado antes de publicar. No se dependió de archivos sin versionar para estas validaciones.

| Comando | Exit | Resultado |
| --- | --- | --- |
| `cargo fmt --check` | 0 | PASS |
| `cargo check --locked --all-targets` | 0 | PASS |
| `cargo clippy --locked --all-targets -- -D warnings` | 0 | PASS, sin warnings |
| `cargo test --locked --test f1a_security` | 0 | 19 passed, 0 failed, 1 ignored |
| `cargo test --locked --test f1b1_text_startup -- --test-threads=1` | 0 | 13 passed, 0 failed, 1 ignored Qt |
| `python3 scripts/validate_f1b1.py --qml` | 0 | Nueve comandos internos aprobados: repite los cinco anteriores; 137 unitarios seguros, 12 heredados Piper, 2 app_paths y 1 Qt offscreen aprobados |

Piper/piper-tts ausentes en PATH. XDG de fixtures sin Whisper/ONNX/wake ni librería descargada; la prueba de arranque no crea models/lib. Dos peticiones de chat a mock local, historial correcto en el segundo turno, cuatro mensajes y mismo token después de recrear Backend/servidor. API sin clave devuelve error y el mock registra cero peticiones. No se contactaron proveedores reales ni se instalaron modelos; no se modificaron configuraciones/datos personales. Tras las pruebas el puerto 8765 está libre y los guards recogieron los hijos propios.

Resultados por comando, tiempos, hashes de árbol y evidencia: [F1B1-COMMIT-VALIDATION.md](F1B1-COMMIT-VALIDATION.md). Reproducción y doce nombres heredados exactos: [F1B1-RESULTS.md](F1B1-RESULTS.md). Repetición de revisión previa: `/tmp/kda-f1b1-check-gqxb7pa7/`; árboles exactos y logs finales: `/tmp/kda-f1b1-closure-3i6zho0b/`.

## Compatibilidad y límites pendientes

Se conserva QML/REST/SSE y SQLite existente. Cambios deliberados: GET config sin secretos; Host/Origin exactos; modelos bajo consulta explícita; arranque de texto difiere voz incluso en config antigua hasta `--enable-voice` con recursos locales. Texto validado con mock y UI offscreen; voz/KDE anteriores no se certifican.

Para F1B-2 siguen abiertos Unicode fragmentado en SSE, fin/errores/argumentos incompletos, cancelación profunda y persistencia de turnos parciales. No se presentan como corregidos. Fuera de esta consolidación: reconstrucción de voz/modelos, captura/hardware, proveedores remotos, Plasma Wayland/tray/atajos/overlay reales, supervisión F3/SIGKILL. Los dos tests externos de la selección F1A siguen excluidos; los smokes de voz/ONNX y proveedor remoto no se ejecutaron.

Durabilidad: backup y principal no son una transacción conjunta. Un fsync de directorio posterior a rename puede fallar con el archivo ya publicado: se informa error y no se publica RAM nueva, pero no se garantiza rollback de disco ni durabilidad ante pérdida de energía. El arranque ya no ignora ese error. No hubo fault injection de syscall fsync real, prueba de carreras con proceso adversario del mismo UID ni sandbox completo de egress.

## Parche y working tree final

[F1B1-CHANGES.patch](F1B1-CHANGES.patch): diff binario con índices completos de base a B, exactamente los 21 archivos A+B, incluidos nuevos. Excluye historia sin versionar, estos documentos de cierre, el parche mismo, configs/tokens/logs/bases/modelos y artefactos generados. SHA256: `b78122212e4f60acca7dced31b00beeb7c7160df353c8400066db8bc4fcda06f`. Tamaño: 140791 bytes. `git apply --check` aprobado sobre copia temporal del base y aplicación real comparada byte por byte con los 21 archivos de HEAD.

No quedan diferencias tracked ni staged. Se conservan sin versionar los once documentos históricos del proyecto con los mismos SHA256 de inicio:

- `docs/recovery/ARCHITECTURE.md`
- `docs/recovery/F1A-CHANGES.patch`
- `docs/recovery/F1A-CLOSURE.md`
- `docs/recovery/F1A-COMMIT-VALIDATION.md`
- `docs/recovery/F1A-DIFF-REVIEW.md`
- `docs/recovery/F1A-FINAL-REVIEW.md`
- `docs/recovery/F1A-REPORT.md`
- `docs/recovery/F1A-RESULTS.md`
- `docs/recovery/RECOVERY-PLAN.md`
- `docs/recovery/REPORT.md`
- `docs/recovery/RESULTS.md`

También quedan sin versionar estos tres entregables del proyecto, creados después de obtener los SHA finales, para mantener la consolidación en los dos commits autorizados:

- `docs/recovery/F1B1-CLOSURE.md`
- `docs/recovery/F1B1-COMMIT-VALIDATION.md`
- `docs/recovery/F1B1-CHANGES.patch`

Son 14 archivos sin versionar, todos documentación/parches del proyecto; ninguno es un dato personal o un artefacto de ejecución. No se añadieron documentos F0/F1A a los commits.
