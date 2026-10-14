# F2 — Contrato de autorización verificable

Baseline `991f5c78dddf0b4f208bf5683604db2cb5c51112`. Complementa F1B-2, F1B-3A y F1B-3B. El ámbito es **despacho de herramientas del agente** desde chat, voz o endpoint manual. Las acciones directas de configuración y UI siguen el modelo de aplicación local autenticada y no son entradas del catálogo LLM.

## Autoridad y modelo de confianza

`permission_engine::plan` decide el riesgo desde el identificador, argumentos validados, efecto y recurso. Sólo las herramientas incluidas en su `match` pueden llegar al ejecutor productivo. `filtered_tools` anuncia cuatro herramientas activas: `system_info`, `read_file`, `create_file`, `edit_file`. El punto obligatorio de despacho es `ToolExecutor::execute_for_turn` para turnos supervisados y `ToolExecutor::execute` para la ruta manual. Ambos usan `SessionManager::dispatch_permission`. `execute_raw_authorized` sólo acepta las cuatro herramientas activas; el resto de adaptadores heredados no son llamados por el backend productivo.

El proveedor, documentos, resultados y `ToolCtx.decided` son datos no confiables. Los IDs del proveedor se sustituyen por UUID locales. El principal `local_authenticated` es la aplicación local bajo su bearer existente; no se afirma aislamiento entre usuarios del mismo UID ni se introduce un permiso root. `origin` es `chat`, `voice` o `manual`, asignado por el caller Rust; no se lee del JSON del modelo. Session ID y Turn ID se comprueban contra `conversation_turns` y, en el endpoint de aprobación, también contra el supervisor vivo.

## Plan, capacidad y política

El plan F2 contiene riesgo, descripción del efecto, destino seguro para mostrar, identidad de recurso y SHA-256 de herramienta + JSON de argumentos ordenado + identidad. SQLite conserva sólo el digest, no el contenido ni la ruta. `permission_requests` contiene Operation ID, Consent ID separado, Tool Call ID local, riesgo, principal, origen, estado y tiempos. No es un permiso de categoría: se crea una fila por operación. La duración es 120 segundos; Green se marca `auto`, Yellow/Red empiezan `pending` y sólo aceptan respuesta específica. No hay persistencia de grants ni reutilización por defecto. La aprobación se consume una vez. El Consent ID no es una autoridad por sí solo: debe coincidir con turno, sesión, tool call, estado pendiente y vigencia.

| Riesgo | Semántica F2 | Ejemplos activos |
| --- | --- | --- |
| Green | Auto sólo tras plan válido, flag individual y journal persistido | `system_info` acotado |
| Yellow | Confirmación de efecto exacto, sin modo potencia automático | `read_file`, `create_file` |
| Red | Confirmación de efecto exacto y consumo único; sin aprobación genérica | `edit_file` |

Las rutas de red, procesos y otras herramientas sin adaptador F2 permanecen deshabilitadas aunque un flag legado esté activo o el usuario pulse una confirmación genérica. Una lectura no se clasifica Green por ser sólo lectura. Las rutas que parecen secretos (`.ssh`, `.gnupg`, `.aws`, `.config`, `.local`, `.mozilla`, `.password-store`, `.pem`, `.key`) se bloquean incluso dentro de `allowed_paths`. La clasificación de rutas no promete detectar todos los secretos; el consentimiento Yellow todavía debe tratar cualquier lectura como privada.

## Consentimiento y QML

Preparar la operación inserta intención, anchor de auditoría y solicitud F2 antes de emitir `approval_needed`. La UI muestra herramienta, acción, recurso, riesgo y alcance de una sola operación durante 120 segundos; las escrituras muestran tamaño y SHA-256 del contenido sin revelar el texto automáticamente. Tiene botones Aprobar/Denegar. La acción de aprobación del badge abre el mismo diálogo; cerrar el diálogo o cambiar de sesión envía Denegar y descarta el diálogo anterior. Los eventos terminales eliminan consentimientos visibles obsoletos. El endpoint valida el bearer heredado, el supervisor vivo en `WaitingForApproval`, Session/Turn/Call/Consent IDs y la fila SQLite `pending`. Doble clic y respuestas repetidas pierden la carrera por `state='pending'` y fallan. Si la UI desaparece, el `PendingApproval` tiene timeout y su guard elimina la espera; no hay futuro sin límite. Una aprobación tardía no reabre un turno cancelado o terminal.

## Journal, concurrencia y recuperación

El orden es `Prepared → Authorized → Dispatched → Succeeded / Failed / UnknownOutcome`, o `Prepared → Denied / Abandoned`. `dispatch_permission` usa una transacción `BEGIN IMMEDIATE`: comprueba owner, estado del turno, fingerprint, expiración y estado de consentimiento; marca `consumed` y escribe `Authorized` y `Dispatched` con sus eventos antes de invocar I/O. El mutex del `Turn` rodea esa transacción mediante `dispatch_recorded`, arbitrando con cancelación. No se mantiene una transacción mientras el usuario decide o el ejecutor trabaja. Un fallo de cualquier escritura previa impide el efecto. Un fallo al registrar resultado no informa éxito; la recuperación trata un marcador despachado sin resultado como UnknownOutcome. Nunca se reejecuta automáticamente.

Cancelar antes de despacho deja Prepared/Authorized como Abandoned y revoca un consentimiento pendiente o aprobado. Tras despacho se drena la operación, se registra el resultado si es posible y la cancelación no afirma rollback. Dos sesiones pueden esperar por separado; un turno procesa las llamadas de su batch secuencialmente. La recuperación de arranque revoca solicitudes pendientes/aprobadas ligadas a Abandoned. Las filas históricas sin solicitud F2 nunca pueden satisfacer `dispatch_permission`; no se reinterpretan como autorizadas. El borrado de sesión elimina filas F2 dentro de la transacción F1B-3B después de rechazar turnos/operaciones activos y evidencia incierta no confirmada.

## Filesystem y cuotas

F1A sigue seleccionando raíces por descriptor con `openat2` y `BENEATH | NO_SYMLINKS | NO_MAGICLINKS | NO_XDEV`. Para una operación de archivo, el digest incluye identidad de raíz, directorio padre e inode existente, o ausencia de destino para creación. Al despachar se reevalúa la identidad. `read_bound` y `edit_bound` comprueban el inode en el descriptor efectivamente usado; `create_bound` usa el descriptor padre aprobado, temporal `CREATE|EXCL` y publicación `renameat2(NOREPLACE)`. Edit crea copia `.bak` exclusiva y escribe en el inode aprobado. Si falla tras iniciar la modificación, devuelve un resultado incierto y el journal lo deja en `UnknownOutcome`; el original puede estar parcial, con `.bak` para revisión manual. Se rechazan hardlinks y archivos especiales. Faltan garantías frente a un proceso hostil del mismo UID que mueva un directorio ya abierto; éste es un límite explícito F1A, no un sandbox multiusuario.

Máximos F2: argumentos JSON 256 KiB y 16 claves, cadena de recurso 4096 bytes, lectura/escritura individual 1 MiB, texto entregado 32 000 caracteres, consentimiento 120 segundos, cuatro despachos concurrentes por executor y espera de cuota hasta 10 segundos. `system_info` tiene plazo de 2 segundos. El supervisor conserva límites F1B-2 de turnos, eventos y ReAct. Las herramientas de procesos/red permanecen apagadas hasta tener límites de destino, bytes, timeout, redirección y ownership verificables. No se promete exactly-once sobre archivos, SQLite ajeno, DBus, procesos o servicios remotos.

## Límite pendiente de API

`ToolExecutor::new` fue eliminado. `legacy_fixture` sólo existe bajo `cfg(test)` o la feature opt-in `legacy-fixtures`, seleccionada por los runners sintéticos F1A/F1B-3B. Un build predeterminado no expone ese constructor y `execute` rechaza un executor sin journal. Un embebedor que habilite explícitamente la feature de fixtures asume el riesgo de ese arnés; no se compila en el binario predeterminado. Los auxiliares de ciclo de vida y las acciones directas de la UI autenticada no pasan por F2 porque no se originan como tool calls; un futuro agente con acceso HTTP local necesitará entrar por esta autoridad, no por esos endpoints.
