# F1B-2 — Contrato del motor de conversación

Fecha: 2026-10-10. Baseline: `600a363ee3a9d64d9ea2adbdbda5448efacc7e8a`. Rama: `recovery/f1b2-conversation-engine`.

## Identidad y ownership

- **Session ID** identifica el hilo SQLite existente. Sin sesión, se reserva un slot independiente con clave `None`; nunca se confunde con una sesión llamada `default`.
- **Turn ID** es un UUID v4 generado por el backend al reservar la operación. No procede del modelo ni del body de chat. Se conserva durante preparación, peticiones del bucle ReAct, aprobaciones, herramientas y publicación.
- **Propietario**: un supervisor Tokio por turno; su `JoinHandle` permanece en `TurnRegistry`. El request HTTP/SSE posee un `ClientLease`, no el derecho a abortar un coordinador con hijos. El registro sólo recoge handles que terminaron; no reemplaza un propietario activo.
- La petición al proveedor y el forward de deltas son futuros inline unidos con `try_join!`. No hay tarea HTTP hija creada mediante `spawn`. Cancelación o timeout descartan ambos futuros y sus buffers/response. El completion de resúmenes también usa futuros inline.
- Cada evento público lleva `turn_id`, `session_id`, `sequence` (entero creciente, desde 1) y `state`. SSE `id` reproduce la secuencia; es local al turno. No hay replay/reconexión automática.
- Los IDs de tools del proveedor sólo sirven para ensamblar sus fragmentos; se sustituyen por UUID locales antes de publicar, aprobar, ejecutar o persistir. Assistant/tool del historial usan esos mismos IDs locales. Una aprobación se registra antes de emitir `approval_needed` y su guard borra la espera al terminar o descartarse.

Una segunda solicitud activa en la misma sesión recibe **409 `session_busy`**, antes de cargar/modificar historial o contactar al proveedor. El slot no se libera durante `Cancelling`. Sesiones diferentes tienen supervisores distintos. Este contrato cubre chat escrito, complete y regenerate; no incorpora la arquitectura/identidad propia del pipeline de voz a un registro común (fuera de alcance).

## Estados y punto de publicación

```text
Created -> Running -> WaitingForApproval -> Running
Created | Running | WaitingForApproval -> Cancelling -> Cancelled
Created | Running | WaitingForApproval | Cancelling -> Failed
Running -> Completed
```

`Completed`, `Cancelled` y `Failed` son terminales inmutables. `check`/`dispatch` rechazan también terminales, y sus esperas no pueden quedarse esperando otra transición. El trabajo del supervisor captura unwinds y produce error `supervisor_panic`; como defensa del registry, observar un handle terminado sin terminal registra `supervisor_ended_without_terminal` y marca Failed (incluso si estaba Cancelling), sin afirmar rollback o cancelación efectiva. El siguiente reserve recoge sólo handles terminados. No se abordan panic=abort, SIGKILL ni efectos bloqueados que impidan terminar la tarea. El mutex del turno arbitra cancelación y publicación: sólo `Running` puede publicar una respuesta y pasar a `Completed`. Publicación SQLite y decisión Completed ocurren dentro de esa sección crítica; una cancelación que pierde frente a ella informa que el turno ya terminó. Si gana la cancelación, no hay respuesta final persistida ni evento `done`. Una prueba de carrera comprueba los dos resultados legales y la imposibilidad de ejecutar el callback de persistencia cuando ganó cancelar.

El punto de despacho de una herramienta es el check bajo el mutex de estado realizado por `Turn::dispatch`. Una cancelación anterior impide sondear el futuro de ejecución. Una vez ganado el despacho, la operación se considera iniciada y se recoge su finalización incluso si otra CPU solicita cancelar inmediatamente después. No se despacha la siguiente herramienta cuando se observa `Cancelling`.

## HTTP y eventos

`POST /api/chat` mantiene SSE. `POST /api/chat/complete` mantiene JSON en éxito y añade identidad/estado. `GET /api/chat/turn?session_id=...` devuelve la identidad/estado del slot más reciente aún retenido, útil para cancelar un complete que espera respuesta. Todas estas rutas usan el middleware F1A/F1B1 existente.

| Evento | Significado |
| --- | --- |
| `turn` | Identidad backend; primer evento. |
| `token` | Delta de texto provisional; puede terminar en error/cancelación. |
| `tool_call` | Call ensamblado; identidad local; aún no representa ejecución. |
| `approval_needed` | Solicitud registrada; estado de espera. |
| `tool_result` | Resultado asociado; incluye `success`, contenido e imagen opcional. |
| `done` | Sólo después de validar terminación, recoger tareas y publicar resultado SQLite. |
| `cancelled` | Cancelación terminal; recursos supervisados recogidos; no promete rollback de efectos. |
| `error` | Terminal Failed, con `code`, `message`, `retryable: false` e identidad/secuencia. |

Se reserva un permiso del canal para el único evento terminal: tokens pendientes no pueden desplazarlo y entregarlo no requiere mantener una petición de proveedor activa. La desconexión puede impedir físicamente su recepción; el estado lógico sigue terminal. El cliente debe exigir un evento terminal, no interpretar EOF/HTTP 200 como éxito.

Errores de inicio usan 409 para sesión ocupada o 400 para sesión inválida. Errores durante complete usan **502**, cancelación **409**; no se convierten en una respuesta aparentemente correcta con HTTP 200. Clasificación: `invalid_provider_stream`, `provider_transport`, `provider_timeout`, `provider_error`, `turn_failed`. El detalle conserva la causa y se recorta. Una aprobación desconocida/expirada devuelve el rechazo existente; no reanima el supervisor.

QML captura la generación local del request y su sesión; admite sólo Turn ID coincidente y secuencias crecientes. Stop conserva SSE, identidad y estado ocupado, y muestra «Cancelando» hasta el terminal; 202 y 200 del endpoint son acknowledgments, nunca sustitutos del evento terminal. Si falta el primer evento turn, la solicitud espera esa identidad. Abandonar la sesión invalida callbacks antes de abortar el body, cuyo lease solicita cancelación. Callbacks antiguos de carga de historial/lista y creación de sesión tampoco reemplazan una sesión/turno más reciente. Errores después de tokens se muestran como error, y EOF sin terminal como interrupción.

## Semántica de cancelación

```json
POST /api/chat/cancel
{"session_id":"...","turn_id":"UUID generado por backend"}
```

- Session/Turn deben coincidir con el propietario registrado. Turn desconocido/recogido o de otra sesión: **404**. Falta de Turn ID: rechazo del extractor JSON; no existe fallback que cancele el slot actual sólo por Session ID.
- La operación señala `Cancelling` y espera hasta 2 segundos. **200** si el turno ya es terminal; **202** si sigue recogiendo una operación despachada. Body: `cancellation_requested`, `cancellation_effective`, `turn_id`, `session_id`, `state`. `cancellation_effective` sólo es true para `Cancelled`, nunca para un Completed anterior.
- Red/espera de configuración/aprobación: cancelación despierta el `select!`, descarta futuro/response y libera la espera. No depende de recibir otro token.
- Herramienta iniciada: no se aborta un futuro que espera un `spawn_blocking`; Tokio no puede matar ese trabajo. Se drena, se conserva auditoría de efectos y se suprimen resultados tardíos/nuevos despachos. Mientras tanto sigue `Cancelling`, no `Cancelled`. Una operación externa que queda bloqueada puede mantener 202; no se afirma un límite general para syscalls/procesos heredados.
- Desconexión SSE o abandono de complete: el lease y `Sender::closed()` solicitan la misma cancelación, incluso antes de headers del proveedor. Un cliente vivo y lento conserva backpressure; el plazo externo del stream evita retener la petición indefinidamente.
- Timeout de cada petición LLM: 120 s por defecto, incluyendo forwarding/backpressure; tests inyectan 150 ms mediante `AiService::with_timeout`. Sin retries implícitos, tampoco en 429. Aprobaciones conservan su timeout 120 s y guard de limpieza. Web search tiene timeout HTTP explícito 120 s; show_image conserva el suyo existente.

No se promete deshacer escrituras, apps abiertas, notificaciones, audio ya reproducido o recordatorios ya programados. Los recordatorios persistentes despachados son efectos del scheduler de aplicación existente, no peticiones LLM pendientes; su lifecycle heredado no se reconstruye aquí. La cancelación tampoco detiene globalmente TTS de otra sesión. Auto-speak, si la capacidad de voz está explícitamente activa, se espera dentro del supervisor como operación iniciada. Auto-summary de texto conserva su opción y se espera como HTTP cancelable, sin lanzar mantenimiento detached desde estos endpoints. Puede retrasar `done`; voz real no fue ejercitada.

## SSE del proveedor

`SseParser` conserva bytes entre chunks; decodifica UTF-8 estricto sólo al terminar líneas. Reconoce LF, CRLF y CR, BOM inicial, comentarios, campos data/event/id/retry, líneas vacías y data multilínea con newline. Conserva Last Event ID, pero nunca lo usa como autoridad del turno. UTF-8 inválido produce error, no caracteres de sustitución.

`ChatStreamParser` procesa JSON por evento, texto por delta y hasta 32 tool calls por índice, ordenadas por índice al terminar. Ensambla nombre/argumentos, comprueba IDs ausentes/duplicados/cambiantes y argumentos JSON objeto; nunca reemplaza argumentos inválidos por `{}`. Antes de cualquier despacho se valida todo el batch contra las herramientas ofrecidas: requeridos, tipos, enum, arrays y argumentos desconocidos. El executor mantiene la política y SafePath F1A como autoridad de permisos.

Límites: 256 KiB por línea/evento SSE acumulado y por argumentos de cada herramienta; 4 MiB por texto de una petición y por fragmentos acumulados de herramientas; 32 calls; canales de 256 eventos; bucle máximo 8 iteraciones. No se bufferiza la respuesta completa antes de mostrar tokens.

Éxito requiere **`[DONE]`** como evento completo, sin línea/evento pendiente. EOF sin ese marcador es `sse_premature_eof`, incluso con finish_reason o texto previo. Finish reasons aceptados: `stop`, `tool_calls`; `length`, `content_filter`, motivos desconocidos y evento/objeto `error` son fallo, no respuesta completa. No se admiten varias choices (`n=1`); sí varias tools en choice 0. `choices: []` de usage es válida. Custom legacy sin finish_reason es compatible si termina en `[DONE]` y hay respuesta/calls válidos.

`[DONE]` repetido es **idempotente**, contado por el parser cuando se le alimenta el duplicado, sin repetir tools/terminal. El cliente cierra el response tras el primer marcador válido, sin esperar que el proveedor cierre un socket que puede quedar abierto. Por eso no certifica ni espera bytes enviados posteriormente a esa terminación. Datos distintos de DONE posteriores que sí se alimenten al parser producen `sse_after_done`. Fixtures sintéticos específicos cubren metadata/usage OpenAI, `x_groq` y comentarios/metadata OpenRouter, sin proveedores reales.

Referencias primarias: [framing SSE de WHATWG](https://html.spec.whatwg.org/multipage/server-sent-events.html#event-stream-interpretation) y [streaming OpenRouter](https://openrouter.ai/docs/api/reference/streaming). Los fixtures no certifican todos los proveedores/modelos remotos ni implementan Responses API/Anthropic nativa.

## Persistencia mínima y compatibilidad

Migración aditiva: `messages.turn_id TEXT` nullable, índice `idx_messages_turn` y tabla `conversation_turns(id TEXT PRIMARY KEY, session_id TEXT, state TEXT NOT NULL)`. Se conserva el resto del esquema. Filas antiguas mantienen NULL; las escrituras antiguas sin la nueva columna siguen siendo válidas. Voz y endpoints no conversacionales conservan escrituras legacy sin Turn ID en esta fase.

El mensaje de usuario nuevo incluye Turn ID en el mismo INSERT. Sólo un outcome válido añade assistant/tools/final, con el mismo Turn ID y un SAVEPOINT de publicación mínimo que también actualiza Completed. Se exige autocommit al entrar para poseer la transacción exterior. INSERT/UPDATE y RELEASE son parte del mismo resultado: cualquier fallo, incluido RELEASE, intenta ROLLBACK de la transacción propia; si SQLite ya hizo rollback se conserva la causa original sin buscar un savepoint inexistente. Un error de limpieza conserva ambas causas y se informa como fallo; averías de almacenamiento que impidan incluso rollback/durabilidad no se certifican aquí. Una respuesta parcial/cancelada/fallida no se publica como assistant completado; el usuario queda asociado a su turno. Sin sesión se conserva el registro de estado, sin inventar mensajes en otro hilo. Al abrir backend, estados no terminales de ejecuciones previas se marcan Failed; no hay reanudación ni replay de acciones.

Regenerate ya no borra historial antes de obtener respuesta: sólo admite un último usuario sin respuesta persistida y obtiene un turno nuevo. Rechaza regenerar un trailing assistant/tools ya publicado, para no repetir acciones anteriores silenciosamente; una repetición de esas acciones requiere un mensaje nuevo explícito. No hay reintentos automáticos. El botón Regenerar/retry de QML explica que debe enviarse un mensaje nuevo explícito y conserva el historial local, también en errores de tools; no llama al endpoint. El endpoint conserva su compatibilidad de reintento explícito del último usuario sin respuesta persistida para clientes que entiendan los posibles efectos previos. No ofrece exactly-once de acciones de un intento anterior.

Pendiente de F1B-3: journal de intentos/efectos, recuperación de herramientas intermedias tras fallo/crash, semántica transaccional de usuario+turno+auditoría+efectos, durabilidad/fsync, poda de turnos, retención de contenido parcial y contexto con parejas tool_call/tool_result en ventanas recortadas. No se afirma exactly-once de efectos externos tras crash ni rollback. Supervisión global de procesos/voz/SIGKILL sigue fuera de F1B-2.

La telemetría AgentActivity describe la invocación ReAct, no la publicación del turno; su métrica completed no sustituye conversation_turns/SSE terminal. Voz, sidecar y documentación histórica README mantienen sus contratos anteriores; ningún caller ejecutable hallado envía cancelación legacy por sesión sola. Clientes externos deben adoptar el Turn ID o reciben rechazo, nunca cancelan otro turno por fallback.
