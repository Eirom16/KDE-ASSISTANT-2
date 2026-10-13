# F1B-3B — Contrato de historial, almacenamiento y recuperación visible

Baseline obligatorio `d8a6972b5bc3977408c8d0956fce3196623d5458`; rama `recovery/f1b3b-history-retention`. Complementa F1B2-CONTRACT y F1B3A-JOURNAL-CONTRACT; no sustituye el motor, la autorización SEC-01/02 ni la política de no replay.

## Historial e identidades

SQLite sigue siendo la fuente del historial original. `messages.id` define el orden causal de inserción dentro de cada sesión; no se ordena por fechas que puedan empatar o cambiar. `messages.turn_id` identifica el intento que produjo la fila. Los UUID originales y los NULL legacy se conservan. `conversation_turns` contiene el estado de cada intento; `turn_inputs` vincula todos los intentos de regenerate con el mismo `messages.id` de usuario. Operación, intento, input y sesión son identidades distintas.

Una unidad de contexto abarca un input de usuario y todas las filas hasta el siguiente input, incluyendo batches assistant/tool y el assistant final. Los outputs de retries permanecen en la unidad de su input; los enlaces de `turn_inputs` permiten detectar resultados inciertos de intentos anteriores sin reasignar el Turn ID original. El segmento legacy previo al primer usuario también puede constituir una unidad. Las instrucciones históricas `system` son esenciales: se conservan todas; dentro de las unidades seleccionadas mantienen su posición causal. Las anteriores a la ventana se anteponen en su orden original. El prompt configurado y los facts habilitados también son esenciales.

No se transforma un estado Failed/Cancelled en una respuesta completa. Sólo se aceptan outputs con Turn ID conocido si ese intento está Completed y pertenece a la sesión; los outputs legacy sin Turn ID se validan estructuralmente. Los inputs de intentos fallidos/cancelados siguen conservados. Se añade un aviso explícito de intentos interrumpidos/operaciones fallidas, abandonadas o inciertas. Si hay operaciones UnknownOutcome/Dispatched en la sesión, un aviso esencial de incertidumbre permanece incluso cuando el input original queda fuera de la ventana o la operación no tiene input de chat. Estos avisos son metadatos de recuperación, nunca tool results ni afirmaciones de éxito o rollback.

## Selección y presupuestos

Configuración JSON `ai.context_window_tokens` (default 32768), `ai.max_context_bytes` (default 1048576), `ai.max_tokens` (reserva de respuesta existente, default 2048). No se adivina la ventana real del modelo remoto: el mantenedor debe configurar una ventana que el modelo admita. Un rechazo del proveedor conserva la semántica de error F1B2, sin retry automático.

La contabilidad local usa una envolvente conservadora: dos veces los bytes JSON serializados más framing. Contar se hace mediante un writer sin construir otra copia JSON del historial. La segunda contabilización cubre también el escaping de argumentos al convertirse en strings del formato OpenAI. Se carga contra el menor presupuesto de bytes y de ventana menos respuesta; los esquemas completos de herramientas se descuentan antes de seleccionar historial. No se usa la heurística optimista de cuatro caracteres por token. Es una cota local conservadora para tokenizadores que representan el texto mediante bytes; no se certifica un conteo exacto ni el template privado de cualquier proveedor/modelo custom. Los límites reales del proveedor y su tokenizador no se consultan ni certifican en pruebas externas.

Se reserva primero el contenido esencial y el input actual. Después se recorre el historial desde el input más reciente, seleccionando un sufijo de unidades completas. Antes de leer contenido se consultan tamaños y cantidad de filas con SQL `octet_length(column)`. La SQLite bundled del lockfile optimiza esa función para no cargar payloads de páginas overflow, a diferencia de `length(CAST(... AS BLOB))`. No se exige actualizar el formato de bases antiguas. Una unidad demasiado grande no se materializa ni se parsea su JSON. Se detiene ante la primera unidad antigua que no cabe, sin saltar unidades intermedias ni alterar su orden. La unidad más reciente es obligatoria: si no cabe, se devuelve un error. Facts, instrucciones históricas y input de regenerate tienen comprobaciones de tamaño antes de reconstruirse. La memoria de esta reconstrucción depende del presupuesto configurado, no de todos los mensajes de la sesión. El endpoint/UI legacy de carga de mensajes conserva su comportamiento; esta cota corresponde a la reconstrucción para el proveedor.

Antes de **cada** request del bucle ReAct, `AiService` vuelve a comprobar presupuesto y grupos, incluyendo el crecimiento de la operación actual y la reserva de respuesta. Si los resultados de un batch hacen exceder el presupuesto, no se envía la siguiente petición. Los efectos ya registrados permanecen con su estado real en el journal; un fallo del turno no los deshace. No se recortan instrucciones ni resultados dentro de un grupo para conseguir que quepa.

Una llamada assistant con tool_calls debe preceder a exactamente un resultado por ID. Se rechazan IDs ausentes/duplicados, resultados huérfanos, resultados repetidos, JSON almacenado inválido y grupos interrumpidos por otro mensaje. Varios resultados pueden llegar en el orden causal guardado; no se ordenan artificialmente por ID. Nunca se sintetizan resultados faltantes.

Política legacy conservadora: si una unidad que entra en la ventana tiene un grupo incompleto o corrupto, la petición falla con `context_invalid`; SQLite queda intacto. Una unidad fuera del presupuesto no se lee ni se repara. Si no cabe el contenido imprescindible, falla con `context_budget_exceeded`. SSE informa error terminal; complete devuelve 502 y código explícito. Los resúmenes históricos siguen almacenados y la opción heredada de generar resúmenes no se convierte en otra autoridad: **ningún resumen se inyecta como sustituto silencioso del historial** en los contextos de chat escrito y del caller heredado de voz. Este último usa el mismo selector; no se cambia la arquitectura de audio ni su supervisión.

## Retención y eliminación

La retención automática destructiva de conversaciones/metadatos está **desactivada**. No existe caducidad obligatoria, scheduler de poda, VACUUM ni checkpoint forzado nuevo. El diagnóstico informa `automatic_destructive_retention=false` y `retention_period_days=null`; un periodo futuro necesitará una política explícita y no podrá incluir UnknownOutcome de forma automática.

`DELETE /api/session?session_id=...` usa igualdad parametrizada exacta. Con UnknownOutcome devuelve 409 `uncertain_evidence_confirmation_required`; no elimina ninguna fila. La UI explica que la operación pudo realizarse y que eliminar la evidencia impedirá investigar después. Sólo una segunda acción explícita añade `erase_uncertain_evidence=true`, vinculada a esa sesión. Este flag es consentimiento de eliminación para la aplicación local autenticada; no es una capacidad nueva de F2.

Se rechaza con 409 si el registry todavía posee un supervisor de esa sesión, incluso antes de crear su turno SQLite o mientras recoge tareas después del terminal. El bloqueo de registry se mantiene durante el borrado para arbitrar con reservas nuevas. Dentro de BEGIN IMMEDIATE también se rechazan turnos persistidos no terminales y operaciones Prepared/Authorized/Dispatched. El orden de locks es registry -> DB; el callback nunca adquiere un lock Turn ni hace await. Los escritores manuales/de voz que preparan un turno pasan por la misma conexión y su validación transaccional de sesión. Nuevos mensajes no pueden insertarse en una sesión eliminada.

Orden transaccional:

1. Comprobar existencia o evidencia legacy todavía asociada, actividad, incertidumbre y enlaces de ownership cruzados.
2. Eliminar `operation_events` de operaciones de la sesión.
3. Eliminar `operation_journal` de esa sesión.
4. Eliminar `turn_recovery` y `turn_inputs` de sus turnos.
5. Eliminar `tool_audit` de esa sesión, incluidas filas históricas con argumentos.
6. Eliminar `messages`, `conversation_turns` y finalmente `sessions`.
7. Confirmar COMMIT. Cualquier error de statement/COMMIT usa el rollback propio F1B3A; no existe éxito parcial.

Nunca se usa LIKE, prefijos de Session ID ni coincidencia por nombres de herramienta para borrar. No se desactivan constraints. Triggers aditivos impiden borrar padres con dependientes, incluso con el modo foreign_keys heredado; no se cambia globalmente ese modo ni se reescribe el esquema antiguo. Un enlace cruzado inconsistente provoca rechazo, en vez de borrar datos de otra sesión.

Las bases que sufrieron un borrado legacy pueden tener sesión/input ausentes pero turnos/journal todavía existentes. La migración **no borra esa evidencia** ni inventa contenido perdido. El diagnóstico cuenta enlaces huérfanos. La consulta global de recuperación permite inspeccionarlos y solicitar la eliminación explícita de todos los metadatos de su Session ID exacto, con la misma confirmación de UnknownOutcome. Si no hay sesión ni registros asociados se responde 404. Los registros sin Session ID no se asignan por conjetura a otra conversación ni se incluyen en un borrado de sesión.

Facts globales y recordatorios del scheduler no tienen ownership de sesión en el esquema heredado y quedan fuera del borrado de una conversación. No se inventa esa relación. Los archivos que una herramienta haya creado/editado fuera de la caché tampoco son contenido de conversación que pueda borrarse por esta API.

## Cachés y copias privadas

Los nuevos workspaces usan `$XDG_CACHE_HOME/kde-assistant/resources-v1/<UUID>`: directorios 0700, marker `.lease` con formato fijo, UID comprobado y flock exclusivo. Las rutas se abren por descriptor con openat2; no se siguen symlinks, magic links ni submontajes dentro de las raíces. Los leases fijan raíz, directorio y marker por descriptor y comparan dispositivo/inodo antes de publicar o recoger un recurso; una ruta sustituida no transmite ownership. Las salidas se crean con openat2 y CREATE|EXCL (0600), incluidas imágenes descargadas y SVGs; pdftoppm entrega el PNG por stdout a ese descriptor. La limpieza reclama un nombre UUID nuevo con renameat2 NOREPLACE y comprueba identidad antes de unlink; una sustitución se conserva y se intenta restaurar sin sobrescribir. La limpieza sólo acepta archivos regulares sin hardlinks en workspaces planos verificables. Nombres/markers desconocidos, directorios anidados o enlaces causan conservación/fallo diagnosticable. No se recorre ni borra HOME, `/tmp` compartido, cachés ajenas o workspaces legacy `kda-view-*`, cuya propiedad no se puede reconstruir de forma confiable.

Los recursos publicados conservan un lease durante la vida del backend para proteger su uso por QML. Los snapshots privados usados únicamente como entrada de render se eliminan después del render, también cuando éste falla; una imagen ya preparada como snapshot se conserva si es el propio recurso mostrado. Guards recogen recursos **no publicados** en errores/drop. Un fallo de limpieza conserva ownership para un intento posterior; si el último rmdir falla, se intenta restaurar el marker. No se afirma recuperación ante cualquier fallo físico.

Sweep automático únicamente al arrancar el Backend, antes de lanzar su QML: candidatos sin lease activo, sin referencia `messages.image_url` y con al menos siete días. Siete días es un periodo de gracia de caché regenerable, **no** caducidad de conversaciones ni del journal. Recursos todavía referenciados, abiertos en el lifecycle activo o poseídos por otro backend cooperante permanecen. No se limpia en medio de una operación ni se invalidan imágenes de una sesión viva. Este lifecycle no certifica procesos QML ajenos u huérfanos que ignoren el ownership de la aplicación; la supervisión global de procesos sigue perteneciendo a F3.

Admisión: máximo 1024 workspaces y 256 MiB; cada productor reserva 32 MiB mientras genera. Se valida el tamaño antes de publicar y se libera esa reserva conservando el lease. Copias fuente y recursos publicados: máximo 32 MiB por workspace; descargas de imágenes: lectura incremental limitada a 10 MiB. Quota llena devuelve error en vez de borrar un recurso activo/referenciado. Son límites de almacenamiento gestionado/admisión, no una cuota del filesystem contra escritores del mismo UID ni una cota del espacio transitorio que un renderer externo podría consumir antes de terminar. Una imagen demasiado grande no se publica y su guard intenta limpiarla.

Eliminar una conversación retira sus referencias SQLite; las imágenes que su QML aún use conservan el lease. Los derivados no referenciados se recogen en un arranque posterior elegible; no se promete eliminación inmediata de todas las copias cacheadas. Caché regenerable y snapshots son temporales; los mensajes, journal y evidencias de resultado son persistentes y jamás se sustituyen por esa caché.

## Privacidad, exportación y límites

| Datos | Persistencia / eliminación / exposición |
| --- | --- |
| Mensajes completos y tool_calls/results publicados | SQLite; pueden contener prompts, rutas, argumentos o contenido privado. Eliminables por sesión. No se redactan retrospectivamente ni se envían automáticamente fuera de las peticiones de chat autorizadas. |
| Journal, eventos y recuperación | Metadatos F1B3A, sin argumentos/destinos/resultados privados; conservados hasta eliminación explícita. UnknownOutcome nunca se poda automáticamente. |
| Auditoría histórica | Puede contener hasta 2000 caracteres de argumentos privados u otros valores heredados. **No** se afirma redacción histórica de SQLite. El endpoint `/api/tools/audit` redacta el campo args en su respuesta; no modifica las filas. |
| Caché y snapshots | Copias privadas temporales, potencialmente con contenido sensible; lifecycle y límites anteriores. No incluidos en listados de recuperación. |
| Facts, recordatorios, config | Persistentes con sus APIs/políticas existentes; sin enlace confiable de sesión, no se borran por coincidencias. |

No se implementa exportación nueva ni se generan dumps automáticos. El contenido original y los metadatos son conceptualmente exportables mediante una futura acción explícita a un destino privado; nunca se trataría un dump crudo de toda SQLite como exportación redactada. APIs de recuperación y diagnóstico retornan sólo allowlists de metadatos/conteos, sin rutas de DB ni argumentos/resultados. Auth/host/origin siguen siendo los de F1A/F1B1: el bearer local autoriza la aplicación completa, no implementa ACL multiusuario.

Borrar filas SQLite **no garantiza destrucción forense inmediata** de WAL, backups, snapshots, copias cacheadas, espacio libre o bloques del dispositivo. No se implementa secure-delete ficticio ni se promete deshacer efectos de herramientas. Failed puede tener efectos parciales; Succeeded conserva la semántica del executor, no verificación externa; UnknownOutcome no demuestra ni ausencia ni éxito del efecto.

## Compatibilidad y UI

Migración `004_storage_retention.sql`: índices y triggers nuevos, marcador `recovery_migrations(version=4)`, dentro de la transacción de arranque F1B3A. Aditiva, idempotente y sin UPDATE/DELETE de datos históricos. `user_version`, UUID, mensajes, resumen, estados terminales y evidencias inciertas quedan intactos. El recovery previo conserva no replay y funciona antes de servir HTTP. La capacidad de texto no depende de voz.

`GET /api/storage` autenticado: tamaño lógico DB, WAL, páginas libres, conteos de las ocho tablas, enlaces huérfanos, operaciones activas/inciertas y política de retención; sólo lectura, sin mantenimiento pesado. `GET /api/tools/operations` añade `recovery_only=true` y cursor `before=<audit_id>`, con orden por anchor de auditoría descendente y limit 1..500. El cursor estable permite acceder a evidencia antigua aunque existan operaciones recientes exitosas. Se conservan formato y filtro de sesión parametrizado.

QML añade un aviso en la sesión y acceso desde el botón de historial del drawer. RecoveryHistory muestra herramienta, fecha, estado, Session ID y Operation ID. Nunca renderiza args, destinos ni resultados; metadatos variables se dibujan como PlainText. Texto UnknownOutcome: «La operación pudo haberse realizado, pero no se pudo confirmar su resultado.» Failed advierte efectos parciales; Abandoned explica abandono previo al marcador de despacho. No hay botón de retry/replay.

Estados: loading, ready, empty, unavailable. Empty no implica éxito de herramientas ni demuestra que existiera una sesión; cubre también sesión eliminada. Un error de API muestra indisponibilidad explícita y no afirma ausencia de evidencia. Filas antiguas incompletas muestran placeholders. Generaciones de request invalidan respuestas tardías al cambiar/cerrar la vista o sesión. La lista usa páginas de 50, reemplazadas al consultar anteriores; no acumula todas las páginas en QML. Confirmación de eliminación y cancelación son acciones separadas; cerrar la confirmación no borra nada.
