# F2 — Cierre de auditoría independiente

**A. Motor F2: CONSOLIDATED para el catálogo reducido.** **B. Capacidades: cuatro habilitadas, 19 deshabilitadas.** Rama `recovery/f2-permission-engine`; baseline `991f5c78dddf0b4f208bf5683604db2cb5c51112`; HEAD `8b21c7fd505c8c6b5d0e9bcb5c7ecbd7455c9835`. Dos commits locales, sin push, merge ni release. Los documentos históricos sin seguimiento permanecieron fuera de ambos commits.

## Alcance aceptado

| Riesgo | Herramientas habilitadas | Decisión efectiva |
| --- | --- | --- |
| Green | `system_info` | Automática sólo con flag individual, plan y journal válidos; rechaza argumentos inesperados. Devuelve `PRETTY_NAME`, release de kernel, uptime, modelo/conteo de CPU, RAM total/disponible y nombre/porcentaje/estado de batería si existe. No incluye usuario, home, tokens ni redes. La salida queda acotada a 32 000 caracteres. |
| Yellow | `read_file`, `create_file` | Consentimiento exacto, de una operación y con expiración; `read_file` advierte que el texto puede llegar al proveedor configurado. |
| Red | `edit_file` | Consentimiento exacto, consumo único; copia `.bak` exclusiva antes de editar. Un fallo tras iniciar escritura queda `UnknownOutcome`, no se promete rollback. |

Deshabilitadas en catálogo, plan y despacho manual/voz/chat incluso con flags legados: `open_app`, `web_search`, `show_image`, `find_file`, `find_document`, `preview_document`, `open_file`, `copy_file`, `open_url`, `list_open_apps`, `focus_app`, `close_app`, `notify`, `media`, `volume`, `brightness`, `network_status`, `remind_in` y `kdeconnect`. El scheduler no dispara recordatorios legados al arrancar; sus filas se conservan. Esta pérdida temporal de funcionalidad es una restricción de seguridad, no una certificación de sus adaptadores.

## Defectos corregidos y autoridad

- `ToolExecutor::new` exponía un ejecutor sin journal. Se eliminó del build predeterminado; `legacy_fixture` sólo existe bajo `cfg(test)` o feature explícita `legacy-fixtures`. La ruta `execute` normal rechaza ausencia de journal. El backend productivo instancia `with_audit`. Un embebedor que habilite deliberadamente la feature de fixtures asume el riesgo de ese arnés; no se ofrece en el binario normal.
- `create_file` podía dejar el destino parcial ante un fallo de escritura. Ahora escribe y sincroniza un temporal exclusivo dentro del padre aprobado, publica con `renameat2(NOREPLACE)` y limpia el temporal si falla antes de publicar.
- `edit_file` podía devolver un fallo ordinario después de modificar parcialmente el inode. Ahora distingue resultado incierto, registra `UnknownOutcome`, conserva `.bak` y no permite replay. También limita el total tras append. Un `.bak` preexistente impide editar; un fallo al crear el respaldo puede dejar respaldo parcial sin cambiar el original.
- QML descarta y deniega consentimientos visibles al cerrar diálogo, separar sesión o finalizar turno. El diálogo muestra herramienta, efecto, recurso, riesgo, alcance y aviso de envío al proveedor en lecturas; conserva acceso por teclado. Las respuestas tardías y dobles no despachan.
- El plan rechaza claves inesperadas también en Green y respeta el flag individual. Un log de denegación dejó de imprimir el path privado del error.
- Un fixture F1B-1 liberaba implícitamente un backend antes de abrir otro sobre la misma SQLite y fallaba de forma intermitente por el lock exclusivo correcto. Ahora suelta cliente y backend explícitamente; cinco repeticiones dirigidas pasaron.

La autoridad es `permission_engine::plan` y `SessionManager::dispatch_permission`. El modelo y los resultados de herramientas no pueden producir grants; `ToolCtx.decided` no habilita despacho. IDs de tool call del proveedor se sustituyen por UUID locales. El plan valida argumentos y enlaza fingerprint a herramienta, efecto, recurso e identidad de archivo. SQLite guarda `permission_requests` con Operation/Consent/Tool Call, Session/Turn mediante journal, principal local, origen, riesgo, estado y expiración. `pending→approved→consumed` se comprueba dentro de transacciones; consumo y `Authorized→Dispatched` se persisten en una sola transacción antes del I/O. Un fallo SQLite previo bloquea el efecto. Reiniciar revoca pendientes, y un marcador sin resultado recupera `UnknownOutcome` sin replay. No se sostiene transacción durante espera humana ni ejecución. No se detectó inversión de locks entre supervisor y SessionManager en los callers revisados.

F1A conserva `openat2` por descriptor y rechazo de symlink, hardlink y escapes. Lectura y edición revalidan el inode usado; creación verifica ausencia del destino y padre aprobado. La lectura limita 1 MiB y las rutas que parecen secretos se bloquean; ese filtro no detecta todos los secretos. Un proceso hostil del mismo UID con capacidad de mover directorios abiertos queda fuera del modelo de aislamiento F2.

## Validación y límites

En el árbol staged final y antes del commit B: `python3 scripts/validate_f2.py` **PASS**, `python3 scripts/validate_f1b3b.py --qml` **PASS**, `python3 scripts/validate_gate_w1r.py` **PASS**. F2: 17 pruebas Rust, 1 HTTP, 4 QML; GATE-W1R: 11 QML. Los conteos PASS/FAIL/IGNORED/FILTERED y el historial de un intento fallido de fixture figuran en [F2-RESULTS.md](F2-RESULTS.md). El ensayo QML con Wayland y tema genérico pasó 4/4; el tema KDE abortó al inicializar el runner, así que integración real Plasma/KWin es **NOT_TESTED**. Proveedor real, voz, micrófono, modelos, redirecciones/DNS/SSRF de adaptadores deshabilitados, ownership/timeout de procesos y cierre real del proceso QML siguen **NOT_TESTED**.

Para ampliar el catálogo se requieren controles verificables de destino efectivo HTTP y redirecciones, cuotas de bytes/tiempo, ownership y timeout de procesos, y binding por descriptor para operaciones adicionales de archivos. No activar por flags legados. Las acciones directas HTTP autenticadas de configuración/recursos y el ciclo de vida de la app mantienen su modelo local anterior: F2 cubre tool calls, no constituye un sandbox multiusuario ni privilegios root. Una escritura externa ya despachada puede quedar parcial o completarse tras cancelación; tampoco hay exactly-once frente a servicios externos.

El parche baseline→HEAD está en [F2-CHANGES.patch](F2-CHANGES.patch), SHA-256 `6d6d928d1bd68955e716e930b5997005c641c4cc9e29a04cf29397f94c7c915c`. `git apply --check` y aplicación real sobre archivo Git limpio del baseline pasaron; árbol reconstruido y HEAD: `91ff63ae4e8ec7df31cb5117974e9be6b00e95a4`. Detalle en [F2-COMMIT-VALIDATION.md](F2-COMMIT-VALIDATION.md). **Recomendación: READY_FOR_REVIEW del catálogo reducido.** Esperar revisión técnica antes de consolidación externa o fases F3/F4.
