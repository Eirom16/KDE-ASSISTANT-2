# F2 — Threat model

## Activos y fronteras

Activos: archivos dentro de raíces permitidas, sesiones y journal SQLite, consentimiento humano, token IPC, información devuelta al proveedor y estado del escritorio. Fronteras: proveedor/modelo → parser SSE → call estructurado; archivos/resultados externos → contexto del modelo; QML autenticado → endpoint de consentimiento; herramienta → filesystem/proceso/red; memoria del turno → SQLite. El adversario principal puede controlar texto y argumentos de un `tool_call`, contenido de archivos/resultados y timing de respuestas. No se afirma defensa frente a un atacante con control del proceso Rust, del bearer local, de la cuenta Linux o del kernel.

| Amenaza | Ruta anterior o ataque | Control F2 | Riesgo residual |
| --- | --- | --- | --- |
| LLM malicioso/confundido | «usuario ya aprobó», `approved=true`, ToolCtx falso | Sólo `permission_requests` y estado SQLite permiten despacho; prueba de falsificación | Un modelo puede solicitar muchas operaciones hasta cuotas/timeout |
| Prompt injection en documento/web/resultado | Texto que imita system/admin o niega necesidad de confirmar | Datos de tool no alteran política; nueva call repite plan y consentimiento | El modelo puede contaminar su respuesta verbal, fuera de autoridad de efectos |
| Caller alternativo | `/api/tools/execute` antes ejecutaba Yellow con `auto` | En producción sólo Green, resto denegado y auditado; `ToolExecutor::new` eliminado | `legacy_fixture` requiere feature opt-in para regresiones sintéticas |
| Aprobación falsa/replay | ID viejo, doble clic, otra sesión/turno/call | Consent ID UUID separado, owner y call comprobados; `pending→approved→consumed` con CAS SQLite | Otro cliente con el bearer del mismo principal puede actuar como UI: confianza local existente |
| Carrera de cancelación | Aprobar justo antes de Stop | Mutex de turno + transacción de despacho; cancelación anterior bloquea; efecto iniciado se drena | El efecto externo puede completarse tras solicitar Stop |
| SIGKILL/fallo SQLite | Crash tras marcador o durante resultado | Intento y decisión persistidos; UnknownOutcome y nunca replay; rollback propio | Efecto parcial/duplicado por servicios externos no puede deducirse sólo del journal |
| Sustitución de path/inode | Symlink, hardlink, swap tras consentimiento | F1A `openat2`, inode/padre en digest, comprobación en descriptor de uso, temporal CREATE EXCL y `NOREPLACE` para crear | Otro proceso del mismo UID puede mover un directorio ya abierto; no hay sandbox de UID |
| Fallo de escritura | Crear directamente podía dejar destino parcial; editar inode puede fallar tras truncar | Creación publica temporal sólo tras `fsync`; edición conserva `.bak` y registra `UnknownOutcome` si falla después de iniciar cambio | La restauración de un edit parcial exige revisión humana; un backup fallido puede dejar un `.bak` parcial sin alterar el original |
| Lectura/exfiltración | Read-only tratado Green y reenviado al LLM | `read_file` Yellow, límite de bytes, aviso de envío, rutas de secretos comunes bloqueadas | Un archivo autorizado puede contener secretos no reconocidos; el usuario debe evaluar el alcance |
| SSRF/redirección/DNS rebinding | `web_search`/`show_image` URL y `open_url` | Herramientas remotas deshabilitadas en catálogo y plan | Clientes HTTP de proveedor/modelos siguen su propia configuración directa de la aplicación |
| Proceso ajeno/huérfano | `open_app`, `close_app`, `run_cmd`, `kdeconnect` | Herramientas de procesos deshabilitadas; `system_info` ya no invoca `df` | TTS/sidecar y acciones directas de UI conservan su ciclo de vida previo |
| Scheduler persistente | Recordatorio legado sin consentimiento F2 verificable | `remind_in` deshabilitado y auto disparo al arranque suspendido; filas preservadas | Recordatorios anteriores no suenan hasta una migración autorizada |
| Borrado de sesión | Consentimiento pendiente pierde owner/evidencia | F1B-3B rechaza actividad y UnknownOutcome; elimina filas F2 en misma transacción | Borrar con confirmación explícita destruye evidencia lógica; WAL/backups externos no se borran |

## Supuestos de seguridad

- El bearer IPC existente sólo está disponible al principal local autenticado. F2 no separa dos aplicaciones del mismo UID que compartan ese bearer.
- El código productivo usa `ToolExecutor::with_audit`. Los fixtures que habilitan explícitamente `legacy-fixtures` ejercitan adaptadores F1A fuera de la ruta de agente; el constructor no existe en builds predeterminados.
- Linux soporta `openat2` con los flags requeridos. Su ausencia produce rechazo, nunca fallback de pathname.
- El reloj de pared para expiración puede cambiar si el usuario local o root cambia el tiempo del sistema; los estados persistidos y el consumo único siguen evitando replay tras terminal/reinicio.
- Red y procesos no reciben permisos por clasificación declarativa mientras no haya controles del destino/child comprobables.

## Próximos controles antes de reactivar herramientas

Red: resolver y fijar destino efectivo por salto, bloquear rangos locales/metadatos salvo allowlist explícita, desactivar redirección automática o validarla, acotar bytes/tiempo y probar con loopback. Procesos: catálogo de ejecutables por ruta/argumentos, child ownership, kill-on-drop/timeout, stdout/stderr acotados y prohibición de `pkill` por nombre. Filesystem adicional: usar descriptores de consentimiento para búsquedas, vistas y copias simultáneas. Ninguna de estas capacidades se habilita por un flag legado o confirmación genérica.
