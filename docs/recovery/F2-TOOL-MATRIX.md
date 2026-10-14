# F2 — Inventario real de herramientas y rutas

Baseline `991f5c78dddf0b4f208bf5683604db2cb5c51112`; rama `recovery/f2-permission-engine`. Inspección de `tool_registry`, `tool_executor`, `ai_service`, `voice_pipeline`, HTTP, QML y callers auxiliares. C = chat SSE/complete/regenerate; V = pipeline de voz; M = `/api/tools/execute` autenticado. Las 23 herramientas figuran en `all_tools`; sólo las cuatro activas aparecen en `filtered_tools`. En F2, `ToolCtx.decided` es ignorado para despachar.

| ID | Operación y efecto | Origen | Antes de F2: permiso / evasión | F2: riesgo y política | Dependencia; cancelación; efectos parciales; datos |
| --- | --- | --- | --- | --- | --- |
| `open_app` | Resolver `.desktop`, lanzar proceso | C/V/M | Yellow; modo potencia y M auto | **Deshabilitada** | `gtk-launch`/launcher; hijo no poseído; proceso puede seguir; nombre de app |
| `create_file` | Crear archivo dentro de raíz configurada | C/V/M | Yellow; M auto; `decided` declarativo | **Yellow**, consentimiento exacto | `openat2`, temporal exclusivo y `NOREPLACE`; destino no queda parcial ante fallo previo a publicación; contenido nunca se guarda en permiso |
| `edit_file` | Sobrescribir/añadir a inode y crear copia `.bak` exclusiva | C/V/M | Red en chat; `decided` falsificable en executor | **Red**, consentimiento exacto; M denegada | `openat2`, inode fijado; edición parcial posible, registrada como `UnknownOutcome`; contenido local privado |
| `read_file` | Leer hasta 1 MiB y devolver texto al modelo | C/V/M | Green automática, incluso archivos personales permitidos por path | **Yellow**, consentimiento exacto; M denegada | `openat2`; cancelable antes de despacho; texto se transmite al proveedor en siguiente vuelta ReAct |
| `web_search` | Buscar en DuckDuckGo Lite; consulta/salida HTTP | C/V/M | Green, sin control DNS/redirección suficiente | **Deshabilitada** | `reqwest`; SSRF y destino efectivo no certificados; consulta externa |
| `show_image` | Copia local o descarga URL a caché | C/V/M, adjunto manual | Green; URL arbitraria y redirecciones | **Deshabilitada** | `reqwest`/caché; SSRF remoto y binding local incompletos; imagen privada |
| `find_file` | Enumerar nombres en raíces/dir | C/V/M | Green | **Deshabilitada** | `openat2`; listado puede revelar nombres personales; falta descriptor de consentimiento para búsqueda |
| `find_document` | Enumerar documentos en raíces/dir | C/V/M | Green | **Deshabilitada** | `openat2`; metadatos privados; mismo límite |
| `preview_document` | Render de documento a recurso privado | C/V/M | Green | **Deshabilitada** | parser PDF/Office, `pdftoppm`; proceso y lectura privada sin límite F2 completo |
| `open_file` | Delegar ruta a app predeterminada | C/V/M | Yellow; adaptador ya bloqueado | **Deshabilitada** | `xdg-open`/pathname; no se cede recurso por descriptor |
| `copy_file` | Leer origen y crear/sobrescribir destino | C/V/M | Yellow; M auto; overwrite variable | **Deshabilitada** | `openat2`; falta fijar simultáneamente identidades de origen/destino; parcial |
| `open_url` | Abrir URL en navegador | C/V/M | Yellow; sólo validación textual http(s) | **Deshabilitada** | `xdg-open`; destino final/redirección y proceso no poseídos |
| `list_open_apps` | Enumerar ventanas/títulos | C/V/M | Green | **Deshabilitada** | KWin/ventanas; títulos potencialmente privados; comando externo |
| `focus_app` | Cambiar ventana enfocada | C/V/M | Yellow; M auto | **Deshabilitada** | KWin/ventanas; target por nombre ambiguo |
| `close_app` | Cerrar ventana/proceso | C/V/M | Yellow; M auto | **Deshabilitada**, Red si se rediseña | Puede perder trabajo ajeno; ownership insuficiente |
| `system_info` | Leer OS, kernel, CPU, RAM, batería, uptime | C/V/M | Green, incluía `df` child | **Green** con flag individual; sin `df` child | Sólo `/etc`, `/proc`, `/sys` acotados; metadatos generales enviados al proveedor si C/V |
| `notify` | Publicar notificación KDE | C/V/M | Yellow; M auto | **Deshabilitada** | `dbus-send` bloqueante; puede duplicar efectos |
| `media` | Play/pause/next/status | C/V/M | Yellow; M auto | **Deshabilitada** | `playerctl`; salida de proceso sin cuota firme |
| `volume` | Get/set/mute | C/V/M | Yellow; M auto | **Deshabilitada** | `wpctl`/`pactl`; proceso/resultado sin ownership firme |
| `brightness` | Get/set | C/V/M | Yellow; M auto | **Deshabilitada** | `brightnessctl`; mismo límite |
| `network_status` | Leer red/Bluetooth | C/V/M | Green | **Deshabilitada** | `nmcli`/comandos; nombres de red privados y salida sin cuota |
| `remind_in` | Insertar recordatorio y programar notificación | C/V/M | Yellow; M auto | **Deshabilitada** | SQLite + tarea Tokio; scheduler legacy al arranque desactivado, filas preservadas |
| `kdeconnect` | Devices/ping/ring/share/SMS | C/V/M | Yellow incluso SMS/share; M auto | **Deshabilitada**, Red para share/SMS | `kdeconnect-cli`; envío externo, destino y resultado no verificables |

## Rutas adicionales inspeccionadas

- `ai_service` valida el batch del proveedor y sustituye IDs del proveedor por UUID locales; llama al executor para cada herramienta. Voz llega por el mismo bucle ReAct; sin UI interactiva, Yellow/Red se deniegan. El texto y los resultados de herramientas no son fuentes de autoridad.
- `/api/tools/execute` es una ruta heredada manual autenticada. En producción sólo puede despachar Green. Yellow/Red producen denegación auditada; una carga de imagen manual queda deshabilitada hasta tener un flujo de consentimiento apropiado.
- `/api/tools/approve` exige bearer local, Turn ID vivo y en espera, Session ID, Tool Call ID y Consent ID coincidentes. No ejecuta herramientas directamente.
- `/api/config`, `/api/facts`, borrado de sesión, voz/dictado/TTS y selección de dispositivos son acciones directas de la aplicación local autenticada, no herramientas ofrecidas al LLM. Conservan sus controles propios; el modelo no tiene una herramienta HTTP local. La ruta de listado de modelos contacta al proveedor configurado por la aplicación, fuera del despacho de herramientas.
- `main.rs`, `model_downloader`, `wakeword_ml`, `tts/piper`, `stt_api`, `hotkey_listener`, `kde_integration` y el caché hacen I/O de ciclo de vida o UI que no procede de tool calls. No se les asignan capacidades F2. La reproducción de recordatorios antiguos al arrancar se suspende porque sus consentimientos no son reconstruibles.
- `ToolExecutor::new` fue eliminado. `legacy_fixture` sólo se compila en tests de unidad o con feature opt-in `legacy-fixtures`; los tests históricos seleccionan esa feature. El backend productivo construye exclusivamente `with_audit`, y su `execute` rechaza ausencia de journal.
