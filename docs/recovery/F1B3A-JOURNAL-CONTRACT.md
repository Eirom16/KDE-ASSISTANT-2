# F1B-3A — Contrato del journal y recuperación

Fecha: 2026-10-10. Baseline obligatorio: `27f2db9430bcbc7c074472383fc8acbc142b5f89`. Rama: `recovery/f1b3a-persistence`.

## Alcance y autoridad

El journal cubre las invocaciones de herramientas del backend con `ToolExecutor::with_audit`: chat escrito supervisado F1B-2, complete/regenerate y las invocaciones heredadas manuales/de voz que pasan por ese executor. Estas últimas reciben un Turn ID independiente generado por el backend; no se integran sus coordinadores con TurnRegistry ni se reconstruye voz. El constructor sin almacenamiento `ToolExecutor::new` sigue sirviendo a fixtures y al smoke opt-in, no al Backend de producción.

No se journalizan tokens, peticiones LLM, captura/reproducción de audio, auto-speak, acciones fuera del executor ni cada entrega del scheduler de recordatorios. No se ofrece un lifecycle nuevo para procesos externos o voz. La capacidad de recuperación especificada aquí es la de las herramientas, no una garantía global sobre todo el escritorio.

`ToolCtx.decided` sigue siendo metadato declarado por el caller. **No es una capacidad ni una autorización confiable**. `Authorized` significa que se registró la decisión de consentimiento de la política existente (`auto`/`approved`); no certifica que todas las comprobaciones de configuración, SafePath o del sistema externo hayan pasado. El executor conserva el snapshot y las comprobaciones SEC-01/02 en su ruta de ejecución. Un rechazo dentro de esa ruta produce un resultado `Failed`; una denegación del consentimiento produce `Denied` sin despacho. La reconstrucción central de permisos queda en F2.

Las identidades del journal se generan en Rust: Operation ID UUID v4 nuevo; Turn ID es el del supervisor escrito o el UUID independiente de la invocación heredada; Session ID se obtiene del turno persistido. Antes de crear un turno nuevo, una sesión proporcionada debe existir en SQLite; se rechaza un identificador arbitrario antes de guardar metadatos o despachar. Las filas legacy no se reescriben. El journal no acepta un ID del proveedor como Operation ID. En chat, la autorización comprueba que la operación pertenece al Turn ID y al nombre de herramienta que se van a ejecutar. Este enlace verifica identidad, no convierte ToolCtx en autoridad.

## Esquema y migración

`migrations/003_operation_journal.sql` es aditiva e idempotente. Se ejecuta junto con las migraciones históricas de summary/turn_id dentro de una transacción de arranque propia. `recovery_migrations(version=3)` identifica su instalación; no se reutiliza ni modifica `PRAGMA user_version`. La base previa no tenía un registro numerado de migraciones.

| Tabla | Función |
| --- | --- |
| `operation_journal` | Estado actual; id/turn/session, kind=tool, nombre de herramienta, prepared_at/updated_at, permission/consent, audit_id y tamaño del resultado opcional. |
| `operation_events` | Timeline de estados confirmados: id creciente SQLite, operation_id, state, timestamp, audit_id. APIs sólo añaden eventos. Sus IDs no son las secuencias SSE. |
| `turn_recovery` | Snapshot del último estado persistido del turno abandonado y fecha de recuperación, una vez por turno. |
| `turn_inputs` | Enlace de turno/intentos de regenerate al mismo mensaje de usuario, sin cambiar el esquema de messages. |
| `recovery_migrations` | Instalación idempotente del esquema nuevo. |

Se conservan `sessions`, `messages`, `conversation_turns` y `tool_audit`; no se añade otra columna de mensajes. Las filas antiguas mantienen su Turn ID nullable. Índices por turno, sesión/fecha, estados pendientes, eventos por operación y entrada de usuario. CHECKs restringen estados, consentimiento, permiso y tamaño; triggers comprueban identidad, pertenencia y referencias nuevas, e impiden transiciones inválidas o cambio de identidad. Las REFERENCES son declarativas y pueden comprobarse con `foreign_key_check`. No se activa globalmente foreign_keys sobre el esquema legacy para cambiar silenciosamente su comportamiento de borrado; los enlaces nuevos de operaciones/eventos se verifican mediante triggers y APIs. Borrar una sesión no purga el journal; retención y poda quedan en F1B-3B.

## Máquina persistente

| Estado | Lo que se sabe |
| --- | --- |
| Prepared | Intento/anchor de auditoría confirmados; no hay despacho registrado; consentimiento pendiente. |
| Authorized | Decisión `auto`/`approved` confirmada; aún no hay despacho registrado. |
| Dispatched | Marcador confirmado antes de permitir sondear el executor. El efecto puede no haber empezado, haber terminado o ser parcial. |
| Succeeded | El executor devolvió success y su evidencia mínima/auditoría se confirmó en SQLite. |
| Failed | El executor devolvió error y ese resultado se confirmó. No implica ausencia de efectos parciales ni rollback. |
| Denied | Consentimiento denegado, registrado junto con su auditoría; cero despacho. |
| UnknownOutcome | Hubo despacho registrado y falta un resultado confirmado; exige inspección externa. |
| Abandoned | Prepared/Authorized cerrado sin despacho: no se ejecutará esa operación. Distingue abandono de denegación. |

Transiciones legales:

```text
Prepared -> Authorized -> Dispatched -> Succeeded | Failed | UnknownOutcome
Prepared -> Denied
Prepared | Authorized -> Abandoned
```

Succeeded, Failed, Denied, UnknownOutcome y Abandoned son terminales; no se reaniman. Estado y evento se actualizan en una misma transacción. Cada timestamp de fase está en `operation_events`; `updated_at` es el último cambio. El consentimiento permanece inmutable después de Prepared. `Dispatched` es deliberadamente conservador: un crash entre su COMMIT y el primer sondeo también termina en UnknownOutcome, aunque el fixture externo no observe ningún efecto.

`Succeeded` conserva la semántica real del executor existente: en open_app, por ejemplo, puede significar que se lanzó un proceso, no que éste terminó su tarea. No se inventa una verificación independiente del mundo externo.

## Orden, transacciones y publicación

1. El supervisor reserva el Turn UUID y conserva el contrato F1B-2 de exclusión por sesión. Antes del proveedor, turno + usuario + turn_inputs se confirman juntos. Se construye el contexto antes de ese INSERT para no duplicar el usuario en el prompt.
2. Tras validar el batch completo de herramientas, se confirma Prepared y su anchor/evento antes de esperar aprobación. Los duplicados omitidos por F1B-2 no son nuevas ejecuciones y no crean una operación ejecutada.
3. Denegación: actualización Denied + audit + evento juntos; el endpoint manual de herramienta roja registra un intento/Denied antes de devolver 403, o error 500 si no puede confirmarlo; un error de escritura falla el turno. Aprobación/auto: confirmar Authorized. No existe una transacción retenida durante la espera de consentimiento.
4. Bajo el mutex del turno escrito, comprobar Running y confirmar Dispatched. Sólo después se libera el lock y se sondea el futuro. Cancelar antes de esta sección impide ejecución; cancelar después exige drenar el futuro, incluyendo su registro de resultado.
5. El resultado del executor se registra en una transacción breve con estado final, tamaño del resultado, actualización del anchor de auditoría, duración y evento. Sólo entonces puede devolverse al agente/publicarse `tool_result` o avanzar al siguiente request LLM. No se almacena el contenido del resultado en el journal.
6. Si falla ese registro, el executor retorna `operation_result_not_persisted`; no devuelve éxito. La finalización del supervisor intenta convertir Dispatched en UnknownOutcome y Prepared/Authorized en Abandoned junto con el terminal del turno. Si también falla, quedan pendientes para recuperación en el próximo inicio. No se reejecuta para intentar averiguar el resultado.
7. La publicación final sigue usando Turn.complete y el SAVEPOINT F1B-2: mensajes assistant/tools + Completed atómicos, arbitraje con cancelación y manejo de RELEASE fallido. Se rechaza publicar con operaciones pendientes o UnknownOutcome. Un efecto Succeeded puede coexistir con un turno Cancelled/Failed, sin respuesta final inventada.

`BEGIN IMMEDIATE`/COMMIT propios para journal, arranque y recuperación. Fallos de statement o COMMIT recogen la transacción, respetando rollback automático de SQLite; fallo de limpieza conserva las dos causas. Un guard también intenta rollback al unwinding. Si el almacenamiento impide incluso rollback, no se certifica una conexión utilizable: las operaciones posteriores rechazan una transacción ajena abierta y el error se registra. No se promete curar hardware averiado.

Orden de locks: turno -> DB sólo para despacho/publicación síncronos; registry -> turno sigue F1B-2. Preparación/autorización/resultados usan DB y la liberan antes de tocar el turno. Nunca hay DB -> turno anidados, ni await/red/LLM/aprobación/executor bajo una transacción DB. SQLite sigue síncrono: una espera SQLITE_BUSY puede bloquear un worker Tokio aproximadamente 5 s y retrasar solicitudes de cancelación antes de su espera asíncrona de 2 s. busy_timeout limita contención SQLite, no espera de mutex ni latencia de fsync/hardware; no es una cota general de respuesta HTTP. La prueba busy verifica fallo cerrado sin efectos y reutilización posterior; no se introduce un actor de DB ni se toca el thread Qt.

## Recuperación al iniciar

La apertura obtiene un `flock` exclusivo no bloqueante sobre el inode de la base y lo conserva durante la vida de SessionManager. Un segundo backend cooperante falla antes de migrar/recuperar una ejecución viva. SIGKILL libera ese lock. No es un límite de seguridad frente a escritores del mismo UID que ignoren flock, renombren/reemplacen la base o manipulen SQLite por fuera del backend.

Después de migrar, antes de servir requests, una transacción:

- convierte Prepared/Authorized en Abandoned;
- convierte Dispatched en UnknownOutcome, incluso si su turno ya era terminal;
- conserva todos los resultados terminales y los mensajes confirmados;
- guarda en turn_recovery el último estado SQLite de turnos created/running/waiting_for_approval/cancelling y los marca Failed;
- no contacta proveedores, abre archivos objetivo, ejecuta herramientas ni emite una respuesta de éxito.

El snapshot refleja el último estado **persistido**, no todas las transiciones RAM del supervisor. No se recuperan parciales del stream, aprobaciones activas ni leases. TurnRegistry arranca vacío y admite turnos nuevos: no hay slots perpetuos por operaciones abandonadas. Un fallo de recuperación impide abrir el backend; rollback conserva el estado anterior para otro arranque. Repetir recuperación no crea más eventos UnknownOutcome ni snapshots duplicados.

Consulta autenticada: `GET /api/tools/operations?session_id=...&limit=...`, bajo el middleware existente de host/origin/bearer. Devuelve hasta 500 operaciones recientes con metadatos y errores HTTP explícitos. El bearer local autoriza la aplicación completa, no una sesión particular: omitir session_id consulta el journal global. El filtro es igualdad parametrizada; sesión inexistente o texto de inyección devuelve una lista vacía, sin ampliar la consulta. El timeline completo y snapshots están en SQLite; no hay una nueva pantalla ni replay SSE. Consulta de inspección local:

```sql
SELECT j.id, j.turn_id, j.state, e.state, e.timestamp, e.audit_id
FROM operation_journal j JOIN operation_events e ON e.operation_id=j.id
ORDER BY e.id;
```

## Política de reintento manual

Nunca reanudar/repetir automáticamente una operación de este journal. No existe endpoint de replay de Operation ID y no se cambia un terminal para reutilizarlo.

Antes de repetir una operación incierta, inspeccionar el efecto fuera del journal (archivo/destino/app/servicio). El usuario debe enviar una solicitud **nueva y explícita**, aceptar las confirmaciones existentes y asumir que puede duplicarse un efecto anterior. Se genera otro Turn ID/Operation ID; los originales conservan su incertidumbre. No hay comprobación automática de existencia que se use para afirmar éxito retrospectivo.

Regenerate no puede reutilizar un usuario con alguna invocación Dispatched/Succeeded/Failed/UnknownOutcome en cualquiera de sus intentos vinculados. Se exige mensaje nuevo. Prepared/Authorized abandonados y Denied no representan despacho y permiten el retry explícito ya existente. El enlace turn_inputs evita que un regenerate fallido o un usuario legacy sin Turn ID oculte operaciones de otro intento. La transacción de begin_retry_turn vuelve a exigir que la última fila sea user y utiliza su ID exacto, además de comprobar todos sus intentos: un precheck HTTP anterior a un await no basta. Esta política no reconstruye los posibles efectos históricos anteriores a la instalación del journal ni convierte los mensajes nuevos en capacidades de F2.

## Redacción, evidencia y auditoría

Allowlist de metadatos: IDs backend/session, kind/tool del catálogo, estados, timestamps, permission/consent, audit_id, duración y número de bytes del resultado. **No** argumentos, paths, URLs, documentos, prompts, claves, transcripciones, errores completos ni contenido de resultados; tampoco hashes de contenido privado. El tamaño/timing y los nombres de herramientas todavía son metadatos observables, no anonimato.

La auditoría nueva usa `[redacted]`. Un anchor se crea junto con Prepared con decided=prepared/success=false; la transición final actualiza ese mismo anchor y queda referenciada por el evento. La timeline diferencia intento, consentimiento, despacho y resultado; `success=false` de un anchor pendiente **no** demuestra un fallo del efecto. Las métricas heredadas de tool_usage excluyen anchors todavía prepared. Un fallo del anchor inicial impide despacho; un fallo del audit final revierte también el estado/evento final. Las filas privadas de auditoría histórica no se borran ni se redactan retrospectivamente. UnknownOutcome conserva Operation/Turn/Session ID, herramienta, timeline y auditoría. Permite localizar el intento, pero no reconstruir su destino ni sus argumentos exactos: éstos se omiten por privacidad. La inspección externa es indispensable; el tamaño del resultado no demuestra un efecto. El contenido de mensajes completados sigue bajo la política SQLite de F1B-2, fuera del journal.

## Durabilidad y límites

Se comprueban WAL y synchronous=FULL (valor 2); busy_timeout=5000 ms. Errores de configuración ya no se ignoran. No hay retry de efectos ni de la petición LLM; busy_timeout sólo espera una oportunidad de escritura SQLite. WAL requiere almacenamiento local compatible; no se certifican shares de red. FULL sincroniza el WAL al confirmar la transacción, sujeto al contrato del VFS/OS/dispositivo, no a una garantía universal de hardware. Referencias primarias: [WAL](https://www.sqlite.org/wal.html), [synchronous](https://www.sqlite.org/pragma.html#pragma_synchronous).

No existe una transacción atómica compartida entre SQLite y filesystem/DBus/red/procesos. Después de confirmar Dispatched y antes de confirmar resultado hay una ventana inevitable: el efecto puede haber ocurrido sin evidencia persistida. Eso es UnknownOutcome, **no exactly-once**, rollback ni éxito deducido. SIGKILL prueba consistencia ante pérdida del proceso; no prueba pérdida de alimentación, controladoras que mienten sobre fsync, VFS averiado o corrupción física. Los tests simulan fallos de escritura, prueban SQLITE_FULL real mediante cuota de páginas privada y fallos de COMMIT; no simulan todos los SQLITE_IOERR de hardware.

No incluye retención/poda/contexto nuevo, F1B-3B, permisos definitivos, diseño/voz nuevos, publicación, release o sincronización multiequipo.
