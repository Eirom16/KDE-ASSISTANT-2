# F1B-2 — Streaming, cancelación e identidad de turnos

Fecha: 2026-10-10. Estado: **IMPLEMENTED** para el motor de conversación escrito y los proveedores Chat Completions compatibles. Recomendación: **READY_FOR_REVIEW**. No se certifican voz real, Plasma Wayland ni APIs remotas.

## Baseline y working tree

Repositorio local `/home/eirom/KDE-ASSISTANT-2`, proyecto `Eirom16/KDE-ASSISTANT-2`. HEAD al finalizar la implementación sin consolidar: `600a363ee3a9d64d9ea2adbdbda5448efacc7e8a`; padre `ddb3bce7610c1e770ab1d90c21e1050b3e37c40e`, padre de éste `ca82e722935e8076f526a484655f2f95f87b5d4c`. Rama inicial `recovery/f1b1-text-startup`; rama de trabajo creada **`recovery/f1b2-conversation-engine`**. La rama anterior sigue apuntando al baseline.

Se comprobaron HEAD, padres, estado y documentación antes de modificar código. No había diferencias tracked ni staged: había 14 documentos/parches históricos sin versionar, que se conservaron. Se leyeron AGENTS y referencias de arquitectura/arranque/recuperación, manteniendo Rust/Tokio, Axum REST/SSE, SQLite, autorización F1A y voz diferida F1B1. Durante implementación no hubo commits. La revisión final autoriza únicamente los dos commits locales posteriores a validación de árboles exactos; véanse CLOSURE y COMMIT-VALIDATION. No hubo push, merge, borrado de documentos ni modificación de configuración personal. No se inició F1B-3.

## Cambios concretos

| Archivo | Cambio |
| --- | --- |
| `src/backend/sse_parser.rs` (nuevo) | Framing byte incremental y decoder de deltas/herramientas, límites y tests exhaustivos. |
| `src/backend/conversation_turn.rs` (nuevo) | UUID/estados/secuencia, watch de cancelación, arbitraje de publicación, despacho drenado, registry y lease de cliente. |
| `src/backend/ai_service.rs` | Elimina HTTP spawn hijos, usa futuros inline; parser estricto; valida batch de tools; IDs locales; elimina retry automático 429; deadline externo; máximo 8 iteraciones; telemetría distingue cancelación/error. |
| `src/backend/approvals.rs` | Registro síncrono antes del evento y guard que libera esperas al cancelar. |
| `src/backend/http_server.rs` | Supervisor único de chat/complete/regenerate; 409 concurrente; cancelación dirigida; consulta de identidad; terminal tras publicación; permiso reservado; summary/TTS opcionales supervisados. |
| `src/backend/mod.rs` | Registro de turnos único del Backend, compartido por todos sus AppState. |
| `src/backend/session_manager.rs` | Migración aditiva, Turn ID en INSERT y publicación mínima con SAVEPOINT; abandono tras reinicio marcado Failed. |
| `src/backend/tool_executor.rs` | Timeout web_search; barrera sólo cfg(test) posterior a SafePath para simular una operación iniciada. No modifica permisos ni SafePath. |
| `src/models/message.rs` | `success` en ToolResult de streaming. |
| `qml/Main.qml` | Generación/Session/Turn/secuencia; invalida callbacks antes de abortar; EOF/error parcial y estado real de herramientas. |
| `tests/agent_smoke.rs` | Ajusta patrón al campo adicional; sólo compilado, no ejecutado contra proveedor. |
| `tests/f1b2_conversation.rs` (nuevo) | Casos aislados con mocks TCP y SQLite privado, conexión/cancelación/concurrencia/migración/errores/summary/complete/regenerate. |
| `tests/fixtures/f1b2/{openai,groq,openrouter}.sse` (nuevos) | Fixtures sintéticos específicos sin depender de una API real. |
| `scripts/validate_f1b2.py` (nuevo) | Validación reproducible, XDG privado, secretos/proxies retirados, Cargo offline, logs y exclusiones explícitas. |
| `docs/recovery/F1B2-{REPORT,RESULTS,CONTRACT}.md` (nuevos) | Entrega, evidencia y semántica de integración. |

No se amplían dependencias. Cargo.toml y Cargo.lock permanecen iguales al baseline.

## Diseño del parser

Dos capas independientes: SseParser conserva bytes hasta completar líneas; ChatStreamParser interpreta eventos JSON y acumula tools por índice. Ninguna conversión lossy de chunks. CR/LF/CRLF, comentarios, BOM, multiline data, id/retry y eventos múltiples/partidos se procesan sin ligar framing a chunks HTTP. Texto se publica por delta; no se espera al final para mostrarlo.

Se exige DONE completo y argumentos JSON objeto válidos. EOF temprano, JSON/UTF-8 inválidos, motivos de finalización incompleta y errores del proveedor no devuelven éxito. Varias tools intercaladas conservan sus argumentos y orden por índice. El batch entero se valida antes de ejecutar cualquier herramienta: requeridos, tipos, enum/arrays y nombres/argumentos ofrecidos. No se convierte una tool desconocida o malformada en una acción por defecto. DONE duplicado es idempotente; el parser lo cuenta sin repetir efectos, y el cliente deja de leer después del primer marcador válido. No espera un EOF que podría no llegar.

Límites explícitos: evento/linea y argumentos por call 256 KiB, respuesta/fragmentos de tools 4 MiB por petición, 32 calls, canales 256 y máximo 8 iteraciones. OpenAI usage, metadata Groq y comentarios OpenRouter se prueban con fixtures. Sólo Chat Completions n=1; Responses API y APIs nativas distintas están fuera de alcance. Detalle y referencias primarias en [CONTRACT](F1B2-CONTRACT.md).

## Cancelación y propiedad

Se eligió `watch` para señalar estados/cancelación, un mutex breve para su arbitraje, futuros inline con `try_join!` para red/forward y un supervisor retenido en el registry. Evita añadir dependencias y elimina la falsa suposición de que abortar un padre termina sus hijos. El supervisor nunca se aborta desde cancel: el lease y el endpoint señalan Cancelling.

La red, buffers y aprobaciones se descartan al cancelar, incluso antes del primer token/headers. El deadline incluye forwarding para liberar HTTP aunque haya backpressure. Una herramienta despachada se drena, incluyendo su spawn_blocking, antes de declarar Cancelled; las herramientas siguientes y resultados tardíos quedan suprimidos. Cancel responde 202 mientras drena y 200 al observar terminal, con flags separados de solicitud/efectividad. Un permiso de canal garantiza espacio para el evento terminal sin retener red para entregarlo.

Auto-summary de texto también se espera/cancela bajo ese supervisor; se prueba cierre de su HTTP. Auto-speak conserva la condición voice_enabled/auto_speak y se drena como efecto iniciado, sin detener globalmente otra sesión. No se ejecutó voz real. Los recordatorios despachados y procesos externos son efectos del sistema heredado: no se promete rollback ni cancelación del efecto ya programado. Una syscall/herramienta heredada que no retorna puede mantener Cancelling/202; no se anuncia una cancelación efectiva ficticia.

## Turn ID, eventos y persistencia

Session ID y Turn ID son distintos; todos los eventos llevan secuencia e identidad. El Backend comparte una única reserva por sesión entre sus estados HTTP. Otra sesión no comparte el control de cancelación. Una petición tardía para A no cancela B; session-only cancel deja de ser suficiente. El endpoint de identidad permite obtener el UUID de complete antes de su respuesta final.

La publicación y cancelación compiten bajo el mismo lock. El ganador determina Completed o Cancelled; ningún error/partial se guarda como una respuesta completa. Nuevo usuario y assistant/tools válidos llevan la misma identidad, insertada junto con el mensaje. Migración nullable conserva bases antiguas. SAVEPOINT sólo cubre publicación de outcome/estado; no pretende ser un journal de efectos. Reinicio marca ejecuciones abandonadas Failed y nunca las repite.

Regenerate ahora rechaza historial ya respondido, no borra mensajes antes de fallar ni repite tools previas. Puede reintentar explícitamente un último usuario sin respuesta. Completar errores usa 502 y cancelar complete 409; se retiró el HTTP 200 con un mensaje de error disfrazado de respuesta. QML discrimina sesiones/turnos/secuencias y muestra errores aun tras tokens. Stop conserva identidad/SSE y estado ocupado hasta terminal; 202 se muestra Cancelling. El botón Regenerar/retry explica la repetición con mensaje nuevo y conserva historial, sin reejecutar tools. Listas/historial y creación de sesiones tienen guards de callbacks obsoletos. Contrato completo: [F1B2-CONTRACT](F1B2-CONTRACT.md).

## Pruebas y recursos observables

Validación final `python3 scripts/validate_f1b2.py --qml`, exit **0**; evidencia `/tmp/kda-f1b2-check-3z4avo14/`.

| Gate | Resultado exacto final |
| --- | --- |
| cargo fmt --check | exit 0 |
| cargo check --locked --all-targets | exit 0 |
| cargo clippy --locked --all-targets -- -D warnings | exit 0 |
| cargo test --locked --test f1a_security | 19 passed, 0 failed, 1 ignored |
| cargo test --locked --test f1b1_text_startup -- --test-threads=1 | 13 passed, 0 failed, 1 ignored |
| cargo test --locked --test f1b2_conversation -- --test-threads=1 | 20 passed, 0 failed, 3 ignored QML |
| Unitarios seguros | 150 passed, 0 failed, 2 filtered out |
| Heredados Piper diferidos (incluidos en los unitarios) | 12 passed, 0 failed |
| App_paths | 2 passed, 0 failed |
| QML offscreen opt-in | 1 heredado + 3 nuevos passed, 0 failed |

Resultados exactos de cada caso, comandos, exclusiones y logs en [F1B2-RESULTS](F1B2-RESULTS.md). El mock usa sockets HTTP/1.1 reales con chunked encoding, fragmentación de un byte, conexiones abiertas, headers retrasados, EOF abrupto y escrituras tardías. Lee del socket de proveedor para observar EOF/cierre del cliente; no toma HTTP 200 como evidencia de liberación. Se comprueban además handles activos, actividad AI, aprobaciones pendientes, ausencia de mensajes contaminados y efectos/auditoría privados.

La prueba de herramienta usa HTTP simulado y el executor real autorizado por SafePath; su barrera cfg(test) retiene una operación después del despacho. Cancel devuelve 202/false mientras está activa, rechaza otro turno y sólo pasa a Cancelled después de recogerla. El primer archivo privado se crea, el segundo no; queda una auditoría exitosa y ningún assistant completado. La barrera no existe en builds de producción ni evita un límite de seguridad.

Los tests no ejecutan proveedores remotos, smokes de agente reales, herramientas destructivas del escritorio, captura de micrófono ni descargas. La fixture de creación sólo escribe dentro de un directorio temporal propio y se recoge.

## Regresiones, límites y revisión

Los gates de seguridad, arranque/texto y QML offscreen pasan en la validación final. Los fallos intermedios de compilación se registran en RESULTS, incluidos el préstamo de un canal, helpers sin uso y el patrón antiguo de agent_smoke sin `success`; se corrigieron. No se ocultan ni se cuentan como PASS las pruebas ignoradas/excluidas.

Cambios de compatibilidad deliberados: cancel exige Turn ID; complete devuelve error HTTP real; regenerate rechaza un trailing assistant/tools ya publicado; DONE/JSON/UTF-8/args incompletos ya no se toleran como éxito; no hay retry automático 429; done espera el mantenimiento opcional supervisado. No se rediseña QML, IPC, permisos ni voz. Los callers Rust que destructuran ToolResult exhaustivamente deben admitir `success`; agent_smoke se adaptó.

Garantías alcanzadas: contenido válido fragmentado no se corrompe; peticiones supervisadas se liberan al cancelar/expirar; el turno tiene un terminal lógico; espera de aprobación se recoge; no hay nuevo despacho tras cancelar antes de su punto de despacho; otra sesión/B no recibe contenido de A; no se persiste éxito falso; la base anterior migra conservando filas. Los efectos ya iniciados se distinguen de cancelación efectiva.

Pendiente F1B-3: transacción/journal completo de intentos/efectos/auditoría/usuario, reconstrucción de intermedios tras crash, durabilidad, retención/poda, contexto truncado que conserve parejas de tools. Sin exactly-once externo tras crash. Identidad de voz/sidecar, supervisión global de procesos/SIGKILL y scheduler de recordatorios siguen con sus contratos previos, sin certificación nueva. Sin pruebas de Plasma Wayland, audio/modelos reales ni todos los proveedores remotos.

Con esos límites explícitos, la revisión real resulta satisfactoria y mantiene **READY_FOR_REVIEW** para F1B-2. La consolidación local autorizada se registra en CLOSURE; no autoriza F1B-3, push, merge ni publicación.

## Revisión final del código real

Se inspeccionaron todos los archivos del diff y callers ejecutables de chat/complete/regenerate/cancel. Se reprodujeron y corrigieron siete defectos acotados (dos UI, dos SQLite, dos de turnos y callback de lista) antes de consolidar; evidencia previa y regresiones exactas en RESULTS. No se amplió política SEC-01/SEC-02 ni voz.

SQLite: RELEASE se incluye en el resultado de publicación; cualquier fallo recoge la transacción propia, respeta rollback ya ejecutado por SQLite y conserva la causa original. El mutex del turno precede al mutex DB sólo durante publicación síncrona. Reserve/diagnóstico usan registry -> turno; la validación inicial de sesión libera DB antes de reserve. No hay camino que mantenga DB y espere registry/turno, ni await bajo DB o lock de publicación. Sesiones comparten breves operaciones SQLite serializadas, pero mantienen red/aprobaciones/supervisores independientes. FK diferida y trigger RAISE(ROLLBACK) comprueban fallos reales de SQLite; no representan fault injection de hardware/fsync.

Turnos: dispatch/check rechazan todos los terminales. El work del supervisor captura panic; si un handle termina fuera del camino terminal, el registry registra diagnóstico y lo marca Failed al observarlo, sin afirmar cancelación efectiva ni rollback. El slot se recoge sólo cuando el handle terminó. La prueba de despacho drenado conserva 202/409 hasta finalizar el efecto y suprime la segunda herramienta.

Parser: límites por byte durante recepción, UTF-8 estricto, ensamblado por índice, batch validado antes de efectos, ID local, DONE obligatorio y ausencia de retries; fixtures deterministas prueban las variantes soportadas. Serializar HashMap<String, serde_json::Value> para mensajes salientes tiene fallback heredado inalcanzable para valores JSON representables; no es parsing ni recuperación de argumentos inválidos recibidos, que se rechazan antes del executor.

Compatibilidad: los callers ejecutables hallados son Main.qml y tests; no se halló cancelación legacy activa en otro cliente. README/planes históricos describen el API antiguo y no se modifican aquí; CONTRACT documenta la ruptura deliberada. AgentActivity cuenta invocaciones ReAct y no sustituye la autoridad del turno/publicación. Voz y su summary heredado mantienen lifecycle previo, fuera de esta revisión.

Partición de commits: A contiene producción y adaptaciones mínimas de fixtures/caller heredados para compilar all-targets; B incorpora tests nuevos, hooks cfg(test), fixtures, script y los tres informes. Los nuevos unitarios embebidos en fuentes se agregan en B, por lo que A no depende de archivos B sin versionar. Manifiestos exactos y hashes se entregan en COMMIT-VALIDATION.
