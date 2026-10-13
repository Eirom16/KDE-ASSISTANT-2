# F1B-3B — Historial íntegro, retención y recuperación visible

Fecha: 2026-10-12. Estado: **IMPLEMENTED**. Recomendación: **READY_FOR_REVIEW** para F1B-3B, con los límites descritos abajo. No implica aprobación de merge ni certificación de proveedores, voz o Plasma reales.

Los trece gates de `python3 scripts/validate_f1b3b.py --qml` finalizaron con exit 0: formato, check, Clippy, F1A/F1B1/F1B2/F1B3A/F1B3B, unitarios seguros, app-paths y tres selecciones Qt. Resultados finales: 269 PASS de tests padre Rust y 6 PASS de harnesses QML; cero FAIL. IGNORED, FILTERED, fallos de iteraciones anteriores y NOT_TESTED se distinguen en [F1B3B-RESULTS.md](F1B3B-RESULTS.md). El manifiesto SHA256 de 127 fuentes no cambió durante la validación y coincide con el árbol final de código. Contrato: [F1B3B-STORAGE-CONTRACT.md](F1B3B-STORAGE-CONTRACT.md).

## Baseline y preparación

Repositorio local `Eirom16/KDE-ASSISTANT-2`. HEAD inicial y base obligatoria: **`d8a6972b5bc3977408c8d0956fce3196623d5458`**, rama anterior `recovery/f1b3a-persistence`. Tracked/staged estaban limpios. Se conservaron los veinte documentos/parches previos sin versionar; no se añadieron al índice ni se modificaron. Rama nueva: **`recovery/f1b3b-history-retention`**. Durante la implementación HEAD y rama anterior permanecieron en el baseline. La consolidación local posterior se registra en F1B3B-CLOSURE y F1B3B-COMMIT-VALIDATION; sin push, merge, release, reset, clean ni stash.

Se leyeron AGENTS, los tres documentos de arquitectura/UI y los contratos, informes, resultados, cierre, validación y plan de recuperación solicitados. Se inspeccionaron las ocho tablas, los callers de contexto de texto/voz, el bucle ReAct, APIs, QML y los workspaces de SafePath/previews/imágenes. No hubo delegación ni trabajo de F2. Cargo.toml/Cargo.lock y las migraciones 002/003 se conservan.

## Implementación

El defecto principal de contexto era una ventana de 40 mensajes después de cargar toda la sesión. El caller heredado de voz recortaba a 12 y quitaba tools de los bordes. Ambas rutas usan ahora un selector compartido de unidades de input completas con presupuesto, validación causal y metadatos honestos sobre intentos cancelados/fallidos/inciertos. El resto de la arquitectura de voz no se modifica. Antes de cada request ReAct se vuelve a comprobar el presupuesto, incluyendo esquemas y reserva de respuesta. Las consultas `octet_length` aprovechan el tamaño en el registro SQLite sin cargar payloads overflow antes de comprobarlo; una fixture rechaza un JSON legacy inválido de 4 MiB por presupuesto antes de parsearlo. Historial original y UUID no se reescriben; un grupo legacy incompleto dentro de la ventana produce error explícito. Los resúmenes no reemplazan el original; la generación legacy opcional usa un excerpt acotado.

El borrado anterior eliminaba `messages` y `sessions` sin transacción ni journal. Ahora arbitra con reservas/supervisores, rechaza turnos/operaciones activos, comprueba ownership y elimina los dependientes de las ocho tablas dentro de BEGIN IMMEDIATE/COMMIT. UnknownOutcome requiere consentimiento adicional en QML y API. La política automática destructiva de DB permanece desactivada. También se permite investigar y eliminar explícitamente los metadatos exactos de una sesión que el borrado legacy ya hubiera eliminado; no se purgan durante migración.

Los recursos nuevos tienen raíz propia, UUID/UID/marker verificables, leases y cuotas de admisión. Las copias usadas sólo como fuente de render se recogen al terminar; imágenes publicadas y recursos referenciados/activos se conservan. Guards recogen recursos fallidos no publicados. Sweep sólo durante arranque sobre recursos verificables, inactivos, no referenciados y con periodo de gracia de caché; nunca sobre `/tmp` legacy. El límite de download se aplica durante lectura, y la publicación verifica el tamaño del recurso.

RecoveryHistory integra el diseño actual con tokens/Octicons, aviso por sesión y acceso desde el historial del drawer. Muestra metadatos seguros, estados honestos, páginas de 50 y confirmación antes de borrar evidencia. No ofrece replay/retry. Errores, registros incompletos, sesión eliminada y callbacks obsoletos tienen estados explícitos. `/api/storage` añade diagnóstico de tamaño, conteos, orfandad y política; `/api/tools/audit` redacta argumentos **en la respuesta**, preservando las filas históricas originales.

## Archivos del alcance

| Archivos | Cambio |
| --- | --- |
| `src/backend/history_context.rs` | Selector acotado de unidades, invariantes tool/result, presupuesto por request, inputs/facts/excerpt acotados. |
| `src/backend/storage_retention.rs` | Borrado transaccional/confirmado, ownership, diagnóstico y referencias de recursos. |
| `src/backend/resource_cache.rs` | Ownership, leases, cuotas, publicación, guards y sweep seguro. |
| `migrations/004_storage_retention.sql` | Índices, guards de borrado de padres y de inserción de mensajes, marcador aditivo 004. |
| `src/backend/session_manager.rs`, `operation_journal.rs` | Wiring del borrado/migración, tamaño de WAL, cursor/filtro de recuperación. |
| `src/backend/ai_service.rs`, `http_server.rs`, `conversation_turn.rs` | Presupuesto antes de cada request, selector, errores explícitos, redacción de API, diagnóstico/borrado y arbitraje de registry. |
| `src/backend/safe_fs.rs`, `tool_executor.rs`, `mod.rs` | Cache propia, fuentes privadas acotadas, guards, snapshots/previews e inicio de mantenimiento. |
| `src/backend/voice_pipeline.rs` | Caller de historial compartido; dos unitarios legacy de recorte sustituidos por selección íntegra/error conservador. Audio/estado/STT/TTS sin cambio. |
| `src/models/config.rs` | Presupuestos de contexto con defaults compatibles. |
| `qml/Main.qml`, `qml/qmldir`, `qml/components/RecoveryHistory.qml` | Aviso, consulta por sesión/global, estados/paginación y confirmación de eliminación. |
| `tests/f1b3b_storage.rs`, `tests/qml/F1B3BRecovery.qml`, `tests/qml/F1B3BMain.qml` | Bases/directorios privados, router real, mocks, guards/quotas, integridad y harnesses Qt. |
| `tests/f1b1_text_startup.rs` | Esperar al último owner DB en un teardown de reinicio que podía competir con el flock. No se debilita exclusión de instancia. |
| `tests/f1b3a_persistence.rs` | Comprobar idempotencia del marcador 003 por versión; el marcador nuevo 004 se comprueba aparte. |
| `scripts/validate_f1b3b.py` | Gates exactos con XDG/TMPDIR privados, exclusiones seguras, Qt opt-in y manifiesto de fuentes. |
| `docs/recovery/F1B3B-{REPORT,RESULTS,STORAGE-CONTRACT}.md` | Entregables y evidencia. |

## Demostración de eliminación e integridad

`uncertain_deletion_requires_confirmation_and_leaves_no_orphans` prepara dos operaciones UnknownOutcome y un snapshot turn_recovery en una sesión, además de otra sesión con un turno completo. El primer borrado sin confirmación falla conservando ambas operaciones. El borrado confirmado deja cero mensajes, turnos, auditorías y operaciones de la sesión eliminada; cero eventos de esas operaciones y cero snapshots; conserva el input, turno y cinco mensajes de la sesión independiente. integrity_check=ok y foreign_key_check sin filas.

`deletion_failure_mid_transaction_rolls_back_every_table` inyecta RAISE(ABORT) cuando se borran mensajes, después de retirar eventos/journal/inputs/audit dentro de la transacción. El retorno es error; rollback conserva el usuario, la operación y sus cuatro eventos. Al retirar el trigger el mismo manager completa el borrado y vuelve a pasar integridad. Otros casos comprueban matches exactos, supervisor reservado antes de persistencia, turno activo y Dispatched con turno ya terminal.

Una fixture de borrado legacy retira sesión/input dejando metadatos inciertos. Sólo para sembrar esa corrupción sintética se desactiva el enforcement de FK en su conexión de fixture y se retiran dos guards; inmediatamente se reactiva y se comprueba que foreign_key_check detecta el defecto. Producción no desactiva constraints ni oculta errores. Reabrir no elimina ni reejecuta la evidencia; diagnóstico informa orfandad, borrar sin consentimiento falla y la eliminación confirmada por Session ID exacto deja cero enlaces huérfanos y SQLite íntegro. Bases previas a F1B2 conservan contenido/UUID/NULL turn_id en tres aperturas. Se conserva la cobertura de SIGKILL/no replay F1B3A.

## Recuperación QML

Harness de componente: texto exacto UnknownOutcome, Failed con efectos parciales, Abandoned previo al despacho, fila antigua incompleta, lista vacía, HTTP 500, cierre de confirmación sin borrado y respuesta tardía descartada.

Harness de Main: aviso de sesión, primer DELETE rechazado con 409, historial conservado mientras se muestra confirmación, activación explícita de «Eliminar evidencia», segundo DELETE confirmado y sesión borrada representada como vacía. El mock cuenta las eliminaciones confirmadas. Las pruebas de Auth/HTTP/SQLite son separadas y usan el router real; Qt offscreen no certifica Plasma.

## Límites

La cota de tokens es conservadora por bytes, sin tokenizer específico ni descubrimiento de capacidades del modelo. La ventana configurada debe corresponder al modelo; templates privados y proveedores reales no se certifican. La memoria acotada corresponde a contextos para proveedor; la carga legacy de mensajes para UI sigue siendo completa. Un historial seleccionado corrupto se conserva y falla; no se repara silenciosamente.

DB automática no caduca; imágenes todavía referenciadas pueden conservarse hasta eliminar su conversación. La cuota puede impedir generar más recursos antes de poder recoger los activos. El periodo de gracia de caché no equivale a eliminación inmediata ni forense, y el renderer externo puede consumir espacio transitorio antes de que se valide su salida. Leases protegen el lifecycle cooperante de la aplicación, no procesos huérfanos/ajenos fuera de esa autoridad. Ownership desconocido o inconsistente se conserva/rechaza.

No exactly-once externo, rollback de efectos ni secure delete ficticio. Auditoría antigua y mensajes pueden contener información privada; redacción de API no significa redacción histórica de SQLite. Facts, recordatorios, archivos creados por tools y backups no tienen ownership de conversación que se pueda inventar. Auth/ToolCtx conservan los límites F1A–F1B3A y no se convierten en permisos definitivos F2. No proveedores remotos, micrófono/modelos, Plasma ni fallos físicos durante las pruebas.

## Revisión final antes de consolidación

Se reprodujeron dos defectos adicionales con fixtures privadas. `replaced_workspace_cannot_publish_or_delete_another_live_resource` fallaba: publish y cleanup devolvían Ok para una ruta cuyo directorio pertenecía a otro lease activo. `image_download_does_not_follow_planted_output_symlinks` fallaba: la descarga sobrescribía el target sintético antes de que la publicación rechazara el enlace. Ambos pasan después del arreglo.

Los leases guardan descriptores de raíz/directorio/marker y comparan device/inode. La limpieza reclama un nombre nuevo y valida identidad antes del borrado; una identidad inesperada no se elimina. Las escrituras de descargas/SVGs usan CREATE|EXCL por descriptor; el PDF se recibe por stdout en un archivo creado de esa forma. Se comprobó también un PDF sintético con pdftoppm local y firma PNG. Los dos unitarios de render heredados se adaptaron a una caché registrada de fixture, manteniendo sus assertions de contenido; no se añadió fallback inseguro para rutas no gestionadas.

Fuentes finales: trece gates PASS en `/tmp/kda-f1b3b-check-4js24vaa`, 269 Rust + 6 Qt. Se conservan las evidencias de FAIL antes del arreglo y de las iteraciones de validación. El usuario autorizó expresamente incluir en A las dos adaptaciones mínimas de tests heredados (marcador 003 y teardown SQLite); todas las pruebas nuevas quedan en B.

El orden de locks revisado es Registry -> Turn -> SQLite, y Registry -> SQLite en borrado. Ningún caller inspeccionado toma Registry mientras conserva SQLite; start_turn suelta SQLite antes de reserve, complete toma Turn antes del callback SQLite, manual/voz preparan sus turnos mediante transacción y comprobación de sesión. El scheduler sólo toma SQLite para recordatorios globales y no tiene enlace de sesión que deba borrarse. Una inserción posterior al borrado es rechazada por el guard de sesión. No se afirma una certificación general contra procesos hostiles con el mismo UID o acceso directo a la DB fuera del backend.
