# F1B-1 — Arranque seguro y núcleo de texto

Fecha: 2026-10-10. Estado: **IMPLEMENTED** para el alcance reproducible de texto. Recomendación: **READY_FOR_REVIEW**. KDE Plasma Wayland real: **NOT_TESTED**; no se certifica distribución ni voz real.

Rama creada sin alterar trabajo pendiente: `recovery/f1b1-text-startup`. Base obligatoria comprobada: `ca82e722935e8076f526a484655f2f95f87b5d4c`, desde `recovery/f1a-containment`. **Durante la implementación no se hicieron commits, push, merge ni release. La consolidación local posterior autorizada se registra en F1B1-CLOSURE.md y F1B1-COMMIT-VALIDATION.md. No se comenzó F1B-2.** Los once documentos/parche preexistentes sin versionar se conservaron; no hubo reset, clean ni stash. No se modificaron datos/configuración/modelos personales.

## Resultado y arquitectura

El ejecutable y Main.qml pueden arrancar y completar dos turnos escritos con mock local sin Piper ni modelos. Backend::new dejó de descargar recursos y de exigir motor TTS/cargar ONNX; se conserva el pipeline de voz con controladores diferidos. Config, cliente IA, sesiones, SQLite, token/IPC y UI son el núcleo obligatorio. El listener se reserva una sola vez, y los fallos críticos terminan de forma visible sin dejar API escuchando.

Antes: config → descarga de modelos/runtime → AI/SQLite → Piper obligatorio/ML → HTTP → UI, más prewarm y limpieza por nombre. Ahora: reserva de instancia → configuración validada o primer arranque restrictivo → AI/SQLite → bootstrap privado/token válido → HTTP/QML; voz y recursos permanecen diferidos y requieren activación explícita. Detalle: [F1B1-ARCHITECTURE](F1B1-ARCHITECTURE.md).

## Archivos modificados y nuevos

| Archivo | Responsabilidad |
| --- | --- |
| `Cargo.toml` | Feature process de rustix para comprobar ownership de archivos; lock sin cambios. |
| `src/lib.rs` | Exporta el guard de hijos propios. |
| `src/main.rs` | Reserva única de listener; errores críticos; quita prewarm/downloads y cull global; voz explícita; cierre/guard de UI. |
| `src/backend/mod.rs` | Núcleo sin descargas, http_state fallible; activación opcional exige dependencias antes de captura. |
| `src/backend/speech_service.rs` | Piper real diferido mediante OnceCell; constructor sin dependencia de TTS. |
| `src/backend/hotword.rs` | ML no se carga desde constructor. |
| `src/backend/chime_player.rs` | Generación de recursos diferida; test de generación la solicita explícitamente. |
| `src/backend/auth.rs` | Token/bootstrap fail-closed, QML privado, autoridades exactas, rechazo de bearer vacío. |
| `src/backend/private_files.rs` (nuevo) | Archivos privados, lectura segura y publicación atómica. |
| `src/backend/http_server.rs` | Contrato config redactada/conservación de claves, publicación de RAM después de guardar, capacidades diferidas/503 y validación del token. |
| `src/models/config.rs` | Primer arranque restrictivo, validación, permisos y escritura segura; conserva selección antigua sin migración automática de modelo. |
| `src/owned_process.rs` (nuevo) | Guard kill/wait sólo para Child propio. |
| `src/bin/snap_ui.rs` | Elimina limpieza global y borrado de locks; display libre y guards propios. |
| `qml/Main.qml` | Estado de credenciales desde has_api_key, elimina instrucción pkill en error. |
| `qml/components/SettingsDialog.qml` | Conserva claves redactadas, elimina consultas automáticas, muestra disponibilidad/error de guardado. |
| `tests/f1b1_text_startup.rs` (nuevo) | Trece casos por defecto y un caso Qt opt-in, mocks y fixtures aislados. |
| `tests/qml/F1B1Text.qml` (nuevo) | Harness del flujo real de Main y Settings; asserts de dos turnos visibles. |
| `scripts/validate_f1b1.py` (nuevo) | Validación reproducible con XDG privado, Cargo offline y logs. |
| `docs/recovery/F1B1-REPORT.md` (nuevo) | Informe y criterio de revisión. |
| `docs/recovery/F1B1-RESULTS.md` (nuevo) | Comandos, resultados exactos y límites. |
| `docs/recovery/F1B1-ARCHITECTURE.md` (nuevo) | Inicialización, contratos y compatibilidad. |

## Gates demostrados

- Formato, `cargo check --locked --all-targets` y `cargo clippy --locked --all-targets -- -D warnings`: exit **0**.
- Seguridad F1A: **19 passed, 0 failed, 1 ignored**; montaje opt-in también aprobado por separado. Executor/safe_fs/ai_service y tests F1A no cambiaron; sin regresiones SEC-01/SEC-02 observadas en su selección.
- F1B1: **13 passed, 0 failed, 1 ignored** por defecto; Qt opt-in ejecutado y aprobado (**1 passed**).
- Unitarios seguros ampliados: **137 passed, 0 failed, 2 filtered out**. Los doce antes bloqueados por Piper pasan y se verificaron aparte: **12 passed**. App_paths: **2 passed**.
- Instalación nueva con Piper ausente en el entorno, XDG sin modelos y ORT_DYLIB_PATH inexistente: texto inicializa, no abre micrófono ni crea models/lib ni descarga; modelos/voz no se fingen disponibles.
- Bearer vacío/token malformado rechazados; bootstrap inválido no publica API; Hosts/Origins preparados reciben 403. Bootstrap/config/backup son privados; GET config no devuelve las claves.
- Mock local: dos peticiones, historial correcto en la segunda, cuatro mensajes en SQLite. Recrear backend/servidor conserva sesión, mensajes y token. QML muestra ambos intercambios y permite configurar proveedor/modelo desde Settings.
- Ejecutable real offscreen: lanza su UI, SIGINT recoge su QML y libera puerto, un QML independiente del fixture sigue vivo. Puerto ocupado, config corrupta, token inválido, qml6 ausente y QML que falla al cargar muestran errores y terminan sin listener superviviente.

Resultados y nombres completos: [F1B1-RESULTS](F1B1-RESULTS.md). Evidencia final: `/tmp/kda-f1b1-check-gqxb7pa7/`. Reproducción: `python3 scripts/validate_f1b1.py --qml` con qml6 y puerto 8765 libre. La selección sólo usa proveedores loopback; no prueba una API externa ni impone un sandbox general de red.

## Compatibilidad y asuntos pendientes

Se conserva esquema SQLite y datos; no hay migración general. Config antigua válida conserva bytes/credenciales/selección, con endurecimiento explícito de permisos. El guardado desde la vista redactada integra claves legacy en el mapa por proveedor si faltan, sin borrado implícito. Se retiró la sustitución automática de modelo; el usuario puede elegir otro en Settings. No se persisten valores permissivos ante corrupción.

Cambio deliberado: el arranque ordinario difiere voz incluso si una configuración antigua guarda wake=true/auto_speak=true. La ruta heredada sólo se intenta con `--enable-voice` y recursos ya instalados; no se reconstruye ni valida hardware. Catálogo de modelos pasa a consulta explícita en Settings. Clientes HTTP deben usar la autoridad exacta 8765 y metadata de clave presente, en vez de recuperar secretos de GET config. No se habilitan tools nuevas, delegación ni capacidades de F2.

Permanecen conocidos y **sin resolver**: corrupción UTF-8 al fragmentar bytes SSE, tolerancia de fin/error/argumentos y cancelación que no termina peticiones hijas. La persistencia del happy path pasa, pero el journal/transacciones de turnos parciales siguen pendientes. No se certifican fallos de fsync posteriores a rename, backup/principal transaccionales, aislamiento frente al mismo UID, limpieza tras SIGKILL, modelos/audio/voz reales ni proveedores remotos. Las dependencias de audio/Whisper siguen en la compilación; se retiró su obligatoriedad de runtime, no se creó una build feature de texto.

La integración real con **KDE Plasma Wayland, compositor, tray, atajos y overlay es NOT_TESTED**. La prueba gráfica aprobada es offscreen. Los smokes de TTS/wake y el test de descarga ONNX continúan identificados como externos, no aprobados. No se ejecutó snap_ui; su limpieza se corrigió y compiló. El executor conserva su control de aplicaciones existente fuera del arranque y no se usó para terminar procesos reales.

El alcance F1B-1 está listo para revisión con esos límites explícitos; el inicio de F1B-2 requiere una instrucción posterior.

## Revisión previa a consolidación

La revisión del diff completo detectó y corrigió tres defectos acotados: bootstrap ignoraba cualquier fallo de publicación si el destino existía; el guardado redactado podía sustituir una clave legacy diferente del mapa activo; el catálogo manual podía usar la clave de otro proveedor contra la URL anterior. Ahora sólo AlreadyExists permite releer un ganador concurrente; otros errores se propagan, las credenciales legacy y por proveedor se conservan salvo reemplazo explícito, y catálogo selecciona conjuntamente URL/clave. Un error de try_wait de la UI devuelve error de inicio.

Se añadieron tres regresiones: publicación exclusiva y clasificación EIO, combinaciones legacy/mapa vacío o diferente con cambio de proveedor, y catálogo contra dos mocks que registran Authorization. La prueba EIO inyecta el resultado de error al helper real, no un fallo real de syscall fsync. El lifecycle exige health 200, identifica el hijo qml6 por argv y comprueba falta de ejecutable y error real de carga QML, conservando vivo un proceso QML ajeno del fixture. Validación repetida: todos los comandos del script --qml exit 0; 13 predeterminados más 1 opt-in, 137 unitarios y 12 heredados Piper aprobados. Sin rediseño ni trabajo F1B-2.
