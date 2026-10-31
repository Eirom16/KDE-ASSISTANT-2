# F1A — Contención de SEC-01 y SEC-02

Fecha: 2026-10-10. Estado: **IMPLEMENTED**, limitado a la implementación de esta contención. **SEC-01 y SEC-02 corregidos** en las rutas activas del executor. No certifica una entrega del producto ni cierra F1/F2: persisten los fallos heredados de formato/Piper y los límites de autorización de F0. No se inició Fase 1B.

Rama: `recovery/f1a-containment`. HEAD/base: `12da960ed4722401f1cb42b7a96d9cef912cf90a`. Rama creada sobre el árbol local sin reset, stash, clean ni sustitución de archivos. Sin commit ni push. Los cuatro documentos F0 preexistentes no se modificaron. El cambio previo del usuario en `src/backend/hotkey_listener.rs` conserva exactamente SHA-256 `2622363c9d4323d81822d529edc95a449b75b387a379d831ae9e3b37988f40ab`; no se atribuye a F1A.

Decisión B aplicada sólo al límite de configuración y acceso a archivos. Referencias operativas: AGENTS.md, los cuatro documentos F0, PLAN, PROMPT y apple-DESIGN. Cambios por archivo: [F1A-DIFF-REVIEW.md](F1A-DIFF-REVIEW.md). Reproducciones, comandos y resultados: [F1A-RESULTS.md](F1A-RESULTS.md).

## SEC-01: snapshot y fallo cerrado

`ToolExecutor::execute` toma **una** copia de `Config` mediante `try_read`. Contención devuelve un error explícito, convertido al contrato existente `ToolResult { success: false }` y auditado como fallo. No espera indefinidamente ni obtiene defaults. El snapshot se pasa por referencia a los 23 despachos y a todas las autorizaciones de rutas. Los métodos de ejecución con snapshot son privados; ninguna entrada pública que despacha ToolCall evita esta adquisición. `record_denial` sólo audita.

Se eligió rechazo inmediato ante contención en lugar de mantener el lock durante I/O o esperar al escritor. Puede requerir repetir una acción cuando se guarda configuración; es un coste explícito de disponibilidad. Una operación ya autorizada conserva su snapshot hasta terminar. Una actualización posterior afecta a operaciones nuevas; revocación/cancelación de operaciones en curso corresponde al contrato de F2.

En `AiService`, constructor y snapshot ya no sustituyen contención por OpenRouter/defaults. `simple_completion` fija configuración antes de crear la tarea HTTP; `run_agent` mantiene el mismo snapshot para todas sus iteraciones y reintentos. `call_once` recibe ese snapshot, sin releer el lock. Se rechazan proveedores desconocidos o vacíos antes de cualquier petición. No se alteró parser SSE, cancelación ni arquitectura de voz.

**Semántica conservada:** `tools.<nombre>` autoriza esa herramienta tanto para el agente como para acciones manuales. `ai.enable_tool_calling` controla el catálogo LLM; no se convirtió en un interruptor general de acciones manuales. `tools.confirm_sensitive` y el flujo rojo/amarillo mantienen su significado; `ToolCtx` continúa siendo metadato de auditoría, no una capacidad comprobada. Hay un test que conserva explícitamente la acción manual con catálogo LLM apagado y la rechaza cuando su flag individual está apagado. La autorización central y el catálogo por origen/turno siguen pendientes de F2 (SEC-03/08).

## SEC-02: archivos relativos a descriptores

Se agregó `backend::safe_fs`, módulo privado, con `rustix` como dependencia directa (resolución local 1.1.5, ya presente transitivamente). Se prefirió su interfaz segura a syscalls FFI manuales o una biblioteca con emulación por pathname.

- La selección léxica de raíz sólo identifica qué directorio abrir: **no autoriza un acceso posterior por string**. Cada raíz existente se abre con `openat2`, sin symlinks en sus ancestros. Las resoluciones internas usan `BENEATH | NO_SYMLINKS | NO_MAGICLINKS | NO_XDEV`.
- No se admiten `..`, rutas relativas, roots vacías/inaccesibles, symlinks internos/externos, cruces de montajes bajo la raíz ni archivos especiales. Se rechazan archivos con más de un hardlink para evitar alias externos.
- Leer usa el descriptor validado; `O_NONBLOCK` evita bloquear antes de detectar FIFO/archivo especial. No se vuelve a abrir el pathname para leer sus bytes.
- Crear directorios usa `mkdirat` con un basename y descriptores de directorio; cada componente se abre con resolución restringida. Se permiten subdirectorios nuevos bajo una raíz existente.
- Crear, editar y copiar escriben en temporales UUID, exclusivos y 0600 dentro del directorio fijado; publican con `renameat2` relativo a descriptores. Crear/copia sin overwrite usa `NOREPLACE`; overwrite reemplaza la entrada sin seguir el destino.
- Copia autoriza el origen, el destino y el hijo derivado cuando el destino es un directorio. Backup autoriza el `.bak` derivado. Un backup inseguro o fallido detiene la edición. Backup y archivo principal coincidentes se deniegan.
- Append propaga fallo de lectura/UTF-8: ya no transforma una lectura fallida en cadena vacía y pérdida de contenido.
- `ENOSYS`, `EINVAL`, `EPERM` y otros fallos de primitivas se propagan. **No hay fallback inseguro.** Pruebas en procesos hijos con seccomp devolviendo ENOSYS para `openat2` y para `renameat2` verifican rechazo y ausencia de escrituras.

La elección aprovecha la resolución restringida del kernel y la publicación relativa a descriptores; agregar `canonicalize()` no resolvería la carrera. Referencias técnicas: [openat2](https://man7.org/linux/man-pages/man2/openat2.2.html), [renameat2](https://man7.org/linux/man-pages/man2/renameat2.2.html) y el wrapper `rustix::fs::openat2` inspeccionado en la versión local.

## Contención directamente relacionada

Para retirar completamente el validador por pathname de las rutas activas, búsquedas recorren descriptores y previews/imágenes locales leen primero una copia autorizada. Renderers y QML reciben una copia en un workspace privado UUID/0700, en lugar de volver a abrir el origen cambiante. Se retiró el fallback compartido/predecible de cache de previews; las descargas de imágenes también usan un workspace privado. Los helpers antiguos de búsqueda quedan compilados sólo para sus tests existentes y no son una ruta alternativa de producción.

**Bloqueados explícitamente hasta F2:** `open_file` (incluido reveal) y `kdeconnect share_file`/`compartir_archivo`. Delegar rutas mutables a `xdg-open`, Dolphin o KDE Connect no preserva la autorización por descriptor; requieren un contrato de capacidad/delegación. Devuelven `success=false` con mensaje F2 y no lanzan procesos. Los schemas/flags siguen presentes para conservar el contrato; un catálogo efectivo que represente disponibilidad corresponde a F2.

`Config::load` ahora devuelve error por ausencia, lectura fallida o JSON inválido y no crea/recupera defaults habilitados. El arranque existente propaga ese error antes de inicializar capacidades. Un proveedor omitido tampoco se presume OpenRouter. Al deserializar, las capacidades `tools.*` y `ai.enable_tool_calling` omitidas quedan en false, `allowed_paths` queda vacío y `confirm_sensitive` sigue en true. Los valores explícitos se conservan. `Config::default()` sigue disponible para construcción explícita/fixtures; no se utiliza como recuperación de fallos en estas rutas.

Esto contiene la reactivación de capacidades identificada en SEC-07, **sin declarar SEC-07 completo**: escritura atómica de config, backups/secretos, publicación de RAM tras persistir y migraciones transaccionales siguen pendientes.

## Riesgos y decisiones pendientes

1. Instalación nueva sin configuración y configs antiguas sin proveedor ya no arrancan automáticamente. Hace falta crear/migrar configuración explícita; no se cambió UI ni se agregó onboarding. Campos de tools omitidos en configs antiguas necesitan habilitación explícita.
2. Linux debe soportar `openat2` y `renameat2`/flags necesarios, y permitirlos en su sandbox/filesystem. Se rechaza una operación si no están disponibles. Se probó en Linux x86_64 local; no se certificó el MSRV 1.75 ni otras plataformas.
3. Las raíces autorizadas son objetos de directorio fijados por descriptor. Sustituir sus nombres por symlinks no redirige los efectos. Esto no es un sandbox frente a un proceso con el mismo UID capaz de mover arbitrariamente los directorios/alterar sus objetos, ni frente a un administrador capaz de modificar namespaces/montajes. Las carreras probadas intercambian archivos/links y un subdirectorio/link, manteniendo los directorios autorizados dentro de su raíz.
4. Reemplazar archivos cambia inode y usa 0600; no preserva modos ejecutables, ACL, xattrs ni watchers ligados al inode. Symlinks legítimos, hardlinks, submontajes y roots aún inexistentes se rechazan. Son restricciones explícitas, revisables.
5. Append sigue siendo lectura más publicación; escritores concurrentes pueden perder actualizaciones. No se implementó serialización/transacción entre usuarios externos. Backup/principal no constituyen una transacción conjunta ni se garantiza durabilidad del directorio tras crash. Fallos de I/O pueden dejar un backup válido o directorios nuevos dentro de la zona permitida; un rechazo inicial de autorización no altera el principal ni el exterior.
6. Copias privadas de vistas se retienen para que la UI pueda leerlas. Recolección, límites de tamaño y retención de contenido son pendientes; no se certifica privacidad de largo plazo ni resistencia a agotamiento de disco. Los límites de red/SSRF de show_image siguen pendientes (SEC-08).
7. SEC-03–SEC-10 restantes, dependencias obligatorias de voz, fallos heredados de formato y contratos completos de autorización/cancelación siguen fuera de esta entrega. No se arrancó la aplicación completa, UI, sidecar, audio ni modelos.

Evidencia final: **20 tests nuevos únicos aprobados** (15 integración + 5 unitarios), dos ejecuciones adicionales de ENOSYS aprobadas; **125 unitarios aislados aprobados**, 2 tests de rutas aprobados. Check y Clippy estrictos pasan. Formato global y 12 tests de Piper mantienen fallos heredados, registrados por separado. Esta contención no autoriza publicar herramientas antes del límite completo de F2.
