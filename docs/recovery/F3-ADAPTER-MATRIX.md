# F3 — Matriz de adaptadores y capacidades

Los riesgos de capacidades apagadas orientan un contrato futuro; **no conceden
permisos**. F2 conserva exactamente cuatro identificadores admitidos por plan y
executor, condicionados también a sus flags. Las otras 19 se rechazan aunque
la configuración legacy las marque true. No se modificó el catálogo activo.

## Integración interna

| Adaptador | Efecto/API e identidad | Origen / riesgo / permiso | Seguridad y estado | Evidencia / límites |
|---|---|---|---|---|
| Instancia | flock del inode propio + DBus Desktop1 sin argumentos | Launcher directo / bajo / usuario local | UID, modo, nlink, NOFOLLOW, exclusión y nombre por perfil; sin PID killing | UNIT + arranque/duplicado con XDG privado |
| UI/tray | QtQuick Window, KWindowSystem público opcional, StatusNotifierItem y com.canonical.dbusmenu | Usuario directo / bajo / autenticación local para HTTP | Sólo ventana/sesiones propias; conservación maximizado y F2 detach; no dispatcher LLM | AUTOMATED_QML; resultado KWin en F3-RESULTS |
| GlobalShortcuts | Portal CreateSession/ListShortcuts/BindShortcuts, Activated/Deactivated, Session.Close | Opt-in usuario / bajo / consentimiento del sistema | Una acción toggle, sesión/conexión propias, timeout y sin fallback de captura | UNIT con portal privado; usuario eligió no registrar; tecla real NOT_TESTED |
| Tema | org.freedesktop.portal.Settings.Read, appearance/color-scheme | Interno Rust / bajo / lectura | DBus async, tres segundos, respuesta u32; constructor sin I/O | Sonda real; no se implementa sincronizado automático QML |
| Notificación interna | org.freedesktop.Notifications.Notify, ID u32 | Sólo código interno de app / Yellow si se expusiera / no expuesto | App fija, argumentos/markup limitados, timeout, sin retry ni caller LLM | Servicio sintético privado; ninguna notificación real de prueba |
| Blur | No hay API implementada | Interno / bajo / no disponible | NOT_SUPPORTED real; no éxito por spawn/log | UNIT; transparencia Qt no prueba blur |
| Pin superior | Hint Qt no garantiza keepAbove en Wayland | Usuario directo / bajo | Deshabilitado en Wayland, NOT_SUPPORTED en esta integración | KWin informó keepAbove=false; QML comprueba que el control no actúa |
| Overlay/avatar heredado | qml6 layer-shell opt-in y ventana standalone | Config directa / no agente | Child propio bajo supervisor; no ampliado | No certificado nuevamente, F5 |

## Catálogo F2 y adaptadores heredados

| Herramienta | Acción/API actual y API oficial a evaluar | Riesgo / identidad / permiso requerido | Estado F3 / origen permitido | Controles, pruebas y límites |
|---|---|---|---|---|
| system_info | Lectura Rust de metadatos locales acotados | Green; recurso sistema; plan+journal | AUTORIZADA F2, chat/manual/voz supervisada según contrato | Dos segundos y salida acotada; regresión F2 |
| read_file | SafePath/openat2 por descriptor | Yellow; raíz/padre/inode; consentimiento exacto | AUTORIZADA F2 | Binding/revalidación, límites, timeout de consentimiento; regresiones F1/F2 |
| create_file | Temporal exclusivo y renameat2 NOREPLACE | Yellow; padre y ausencia; consentimiento exacto | AUTORIZADA F2 | Publicación atómica, no overwrite; regresiones F1/F2 |
| edit_file | Descriptor fijado y copia .bak exclusiva | Red; inode y contenido; consentimiento único | AUTORIZADA F2 | Fallo parcial UnknownOutcome, sin replay; regresiones F2 |
| open_app | Legacy gtk-launch/desktop executable; evaluar GIO/portal OpenURI/desktop IDs | Yellow; desktop ID/ejecutable y Child, no nombre libre | DESHABILITADA; ningún origen LLM/manual | Spawn no verifica efecto; ownership de apps delegadas insuficiente; sin activación real |
| web_search | Legacy HTTP de buscador; API del proveedor | Yellow; destino IP efectivo/saltos | DESHABILITADA | Faltan binding DNS/redirect/SSRF y contrato; sin red externa |
| show_image | Legacy archivo/HTTP/render | Yellow lectura privada/red; inode o URL efectiva | DESHABILITADA | Contención/cache F1 conservadas; no basta para autorización completa |
| find_file | Legacy recorrido filesystem | Yellow; raíces por descriptor y coste | DESHABILITADA | No identidad de todos los resultados autorizada; sin búsqueda personal |
| find_document | Legacy búsqueda de documentos | Yellow; árboles y archivos efectivos | DESHABILITADA | Contrato de búsquedas por descriptor pendiente |
| preview_document | Legacy render/pdftoppm y caché privada | Yellow; documento fijado, renderer propio | DESHABILITADA | Fixtures F1B3B de render/caché; no permiso implícito de lectura |
| open_file | Legacy xdg-open; evaluar OpenURI/OpenFile portal | Yellow; objeto fijado y aplicación destino | DESHABILITADA | Delegación por pathname/terceros pendiente; no abrir archivos personales |
| copy_file | Legacy filesystem | Yellow; origen y destino ligados | DESHABILITADA | Falta plan específico y binding completo de ambos recursos |
| open_url | Legacy xdg-open; OpenURI portal como API futura | Yellow; URL validada y destinatario | DESHABILITADA | Navegación externa no autorizada por conexión DBus |
| list_open_apps | Legacy kdotool/xdotool, X11 o script externo | Yellow privacidad; ventanas/servicios | APLAZADA por garantías | No endpoint público genérico KWin de enumeración autorizado al LLM; sólo script temporal de test filtrado por PID propio |
| focus_app | Legacy coincidencia título/clase + xdotool/kdotool | Yellow; identidad estable de ventana y consentimiento | APLAZADA por garantías | No se llama en producción F3; no focus stealing de terceros |
| close_app | Legacy coincidencia/título/ventana activa | Red; identidad/ownership no demostrables | APLAZADA por garantías | pkill por nombre retirado incluso de legacy; no se restaura control de terceros |
| notify | Legacy proceso dbus-send; nuevo adaptador Rust interno separado | Yellow; app/emisor/texto/destino | DESHABILITADA para agente | Motor F2 no reconoce notify; mock interno prueba firma/errores/timeout, sin grant |
| media | Legacy playerctl; MPRIS2 org.mpris.MediaPlayer2.Player es API mantenida | Yellow; bus name y owner del reproductor | DESHABILITADA | Sin nuevo adaptador MPRIS F2, selección de player/timeout/resultados pendientes; no tocar playback real |
| volume | Legacy wpctl/pactl; evaluar PipeWire/WirePlumber | Yellow; node ID/serial/owner estable | DESHABILITADA | No se invoca ni cambia volumen; falta contrato por dispositivo/salida y lectura acotada |
| brightness | Legacy brightnessctl; logind/sysfs según política del sistema | Yellow; dispositivo backlight y autorización sistema | DESHABILITADA | Sin root ni cambio de permisos; acceso seguro no certificado, no se escribe brillo |
| network_status | Legacy nmcli/bluetoothctl; NetworkManager y BlueZ DBus públicos | Yellow privacidad; adaptador/conexión concreta | DESHABILITADA | No divulgar SSID/MAC/IP sin plan/consentimiento; sin sondeo personal |
| remind_in | SQLite/scheduler/notify legacy | Yellow persistente; recordatorio y alcance temporal | DESHABILITADA | Scheduler no reactivado; filas antiguas preservadas y no disparadas |
| kdeconnect | Legacy kdeconnect-cli; org.kde.kdeconnect.device/plugins DBus | Yellow/Red según acción; dispositivo/owner/destinatario/archivo | DESHABILITADA | Compartir/SMS/envíos externos no habilitados; argumentos/timeout CLI no constituyen consentimiento |

## Frontera de despacho

Balance: cuatro herramientas autorizadas F2; dieciséis deshabilitadas y tres
aplazadas por identidad/garantías (las diecinueve están bloqueadas para el
agente). Tema, instancia y tray son integración interna operativa; notificación
es adaptador endurecido no expuesto al LLM. Atajo nativo tiene protocolo
implementado, aceptación humana pendiente. Ningún envío externo reactivado.

Evidencia final: F3-RESULTS.md; unitarias en /tmp/kda-f3-final-m25yawoq,
QML en /tmp/kda-f3-auth-qml-final-qx427p3_ y Plasma en
/tmp/kda-f3-validation-5sjtuawb/runtime. El foco observado en la llamada
sintética no certifica un gesto humano ni disponibilidad universal del token.

Para cualquier futura exposición al modelo será obligatorio:
`permission_engine::plan → consentimiento válido → dispatch_permission → journal → executor`.
Los nuevos métodos de instancia no aceptan tool names, argumentos, rutas, URLs,
texto de notificación ni código. SSE de escritorio sólo entrega eventos a la UI.
La bandeja usa funciones de cliente/sesión existentes y no transforma respuestas
del proveedor en acciones internas. Los contratos de red, recursos de terceros,
procesos delegados y envíos externos necesitan una decisión técnica separada.

APIs primarias para futuras adaptaciones:
[Notifications](https://specifications.freedesktop.org/notification-spec/latest/protocol.html),
[MPRIS2](https://specifications.freedesktop.org/mpris-spec/latest/Player_Interface.html),
[WirePlumber](https://pipewire.pages.freedesktop.org/wireplumber/),
[OpenURI portal](https://github.com/flatpak/xdg-desktop-portal/blob/main/data/org.freedesktop.portal.OpenURI.xml).


### Revisión final del protocolo

GlobalShortcuts valida el handle de sesión por conexión/token y binding exacto,
suscribe antes de crear/bindear y termina ante Closed o cambio de owner.
Drop no deja listener separado; close solicita cierre de la sesión propia.
Las nuevas regresiones son exclusivamente sintéticas, incluidas respuestas
adelantadas, ownership y cancelación de arranque. Prueba de tecla real sigue
NOT_TESTED por elección del usuario; no impide consolidar el alcance del código.
Sin cambios en la clasificación de las 23 herramientas ni en su autoridad F2.
