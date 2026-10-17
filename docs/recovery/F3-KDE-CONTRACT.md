# F3 — Contrato KDE y procesos

Complementa los contratos F1B/F2; no cambia `UnknownOutcome`, el catálogo ni
la autoridad de `dispatch_permission`. Linux, Rust 1.99 fijado por el proyecto,
Qt Quick en `qml6 -apptype widget`, Tokio/Axum y SQLite siguen siendo la arquitectura.

## Instancia y perfil

`desktop.lock` vive junto a `config.json`. Se abre NOFOLLOW/NONBLOCK/CLOEXEC,
se comprueban archivo regular, UID, nlink=1 y permisos privados. El descriptor
conserva flock exclusivo no bloqueante; se verifica device/inode tras adquirirlo.
Nunca se interpreta un PID guardado ni se borra el lock para reclamarlo. El kernel
libera el lock al morir el controlador. El archivo vacío puede permanecer.
Como F2, esto no aísla procesos hostiles con control de la misma cuenta.

La identidad es el directorio de configuración canonicalizado. El nombre DBus
es `org.kde.assistant.p<SHA256 de ese directorio>`; no contiene bearer, configuración,
contenido de chat ni paths en claro. Sólo se exportan `Activate()` y `Toggle()` en
`/org/kde/assistant/Desktop`, interfaz `org.kde.assistant.Desktop1`, sin argumentos.
Se solicita DoNotQueue, sin ReplaceExisting ni AllowReplacement. Un duplicado
envía Activate y termina; si la UI no está conectada o aún arranca, recibe error
y no crea otra instancia. `--toggle` requiere una instancia existente del mismo
perfil. `--send-msg` se rechaza: no hay un transporte DBus de prompts o tools.

Sin bus, el lock todavía impide duplicados; bandeja/activación/portal pueden no
estar disponibles. Nunca se mata un servicio que ocupa un puerto. El puerto por
defecto sigue siendo 8765; `--http-port N` y `--http-port 0` permiten perfiles de
prueba concurrentes. La UI recibe su URL en el módulo privado generado, separado
por identidad de perfil. Host/Origin sólo admiten las autoridades locales del
puerto real; no se aceptan sufijos, paths ni puertos alternativos. Se conserva
el bearer F2 y su modelo local de confianza. Dos perfiles deben separar también
data/cache; la DB mantiene su propio lock si se reutiliza el mismo almacenamiento.

## Ciclo de vida y errores

El listener se reserva antes de inicializar/publicar el backend. SO_REUSEADDR
permite reinicio tras TIME_WAIT; no se utiliza SO_REUSEPORT ni se comparte un
listener vivo. Errores críticos de configuración, token o recursos son fatales.

OwnedChild posee el `Child` obtenido de spawn; kill/wait y Drop sólo actúan sobre
ese handle, sin búsquedas por nombres/títulos/PID recibido del modelo. Antes de
exec, Linux PR_SET_PDEATHSIG=SIGKILL y comprobación de PPID vinculan los procesos
QML propios al thread controlador que los creó. SIGINT y SIGTERM solicitan cierre.
Fallo de QML termina la aplicación con error; no hay reinicio automático de Qt,
turnos, solicitudes LLM ni efectos. Relanzar explícitamente ejecuta la recuperación
F1B/F2: revocación y UnknownOutcome, sin replay.

En cierre se bloquea nueva admisión del registry, se solicita cancelación y se
espera hasta cinco segundos a los terminales. Un despacho iniciado conserva su
resultado real; vencer el plazo no afirma rollback. Se cierra el portal propio,
se detiene HTTP (hasta tres segundos de drenaje antes de abortarlo) y se detiene
el pipeline opcional existente. El runtime tiene cierre acotado. El journal sigue
siendo la autoridad tras interrupción; un efecto sin resultado persistido se
recupera como UnknownOutcome. SIGKILL no permite limpieza ejecutada por Rust:
Qt recibe la señal del kernel y el reparenting/reaping corresponde al supervisor
del sistema; el harness usa un subreaper sólo para recoger sus propios hijos.
No se certifica aquí supervisión completa de audio/TTS (F4).

## Canal UI y consentimiento

`GET /api/desktop/events` es SSE autenticado, de salida, con broadcast de 16
eventos. No hay endpoint para despachar acciones de escritorio arbitrarias.
Sólo Show/Toggle y aviso de estado de atajos. Sin receptor se informa error;
desbordamiento cierra el stream, sin replay de toggles obsoletos. QML reintenta
la conexión después de dos segundos ante desconexión. No se consulta el archivo
`hotkey.state` ni se escucha el teclado global con rdev. Los recents del tray
provienen del modelo de sesiones, sin sondeo de 15 segundos.

Mostrar consentimiento usa la ventana propia, Qt y la API pública opcional
KWindowSystem.activateWindow; el compositor conserva su política de foco. Los logs F3_CONSENT_WINDOW se
emiten al presentar/abrir y cambiar visibilidad/foco durante un pending: tiempo
de observación, tiempo desde presentación, visible/active/visibility, geometría,
popup visible/foco/z. No registran argumentos, rutas, prompts, bearer ni contenido.
`popup_z` no representa stacking de KWin; se declara no observable desde QML.
Una muestra posterior no prueba el foco anterior. No hay timers de autoaprobación.
Cierre de diálogo, cambio/nueva sesión y pérdida de cliente conservan el cierre
seguro F2. El diagnóstico no explica retrospectivamente W1H-05.

## Ventanas, tray y overlay

Se conserva SystemTrayIcon/StatusNotifierItem y DBusMenu nativo con
`Menu.visible: false`; no se llama Menu.popup(). Click izquierdo usa estado
visible **y activo**. Visible/activa oculta; oculta o inactiva presenta; minimizada
restaura en una activación aunque Qt aún informe Windowed. En Wayland, ese
estado inconsistente requiere ocultar y remapear en el siguiente turno del event
loop Qt, sin recrear el árbol QML. Una generación invalida presentaciones
pendientes al ocultar/minimizar. El rescate de arranque no deshace una ocultación
solicitada por el usuario. Se conserva el último
estado maximizado al ocultar/minimizar o recuperar foco. El compositor decide
foco y posición final; no hay XTest, xdotool ni scripts KWin en producción F3.
El remapeo también cubre una ventana inactiva maximizada; una petición sintética
de SNI sin token de entrada no garantiza foco. La prueba establece sus
precondiciones de actividad sólo sobre ventanas propias y registra por separado
el resultado del intento de foco, sin forzar su resultado para certificarlo.

El menú expone mostrar/ocultar, restaurar, minimizar, nueva sesión, recientes,
estado, ajustes y salir. Nueva sesión sigue pasando por cancel/detach F2.
`ui.close_to_tray` es aditivo, default true, editable en ajustes. Sin tray
disponible cerrar sale, evitando una UI inaccesible. Salir deniega/descarta los
consentimientos visibles y termina Qt; Rust recoge sus hijos y recursos.

La ventana normal Qt Quick satisface la apertura rápida; no se añade overlay
privilegiado ni dependencia de KWin privada. Tamaños y mínimos usan coordenadas
lógicas Qt y se ajustan al área disponible al iniciar. Colocación absoluta en
Wayland queda a KWin. El avatar/overlay opt-in heredado se conserva y supervisa,
sin cambios de comportamiento o certificación nuevos (F5). Blur nativo es
NOT_SUPPORTED: transparencia no equivale a desenfoque.
El pin “Siempre visible” queda deshabilitado en Wayland: el hint Qt no demostró
keepAbove efectivo en KWin. No se instala una regla personal para suplirlo.

## Atajos

Sólo `--enable-shortcuts`, sin `--no-shortcuts`, inicia GlobalShortcuts.
`--no-shortcuts` siempre prevalece. El atajo inicial es `shortcuts.toggle`;
la sintaxis aceptada exige modificadores explícitos y una letra/dígito. Valores
inválidos producen error, nunca sustitución silenciosa por un default.

Secuencia: detectar versión → CreateSession → ListShortcuts → BindShortcuts de
una acción `toggle`. El portal arbitra colisiones/consentimiento. No se editan
kglobalshortcutsrc ni acciones personales por DBus. Se escucha Response antes
de llamar al método para cubrir respuesta rápida; error/denegación/timeout no
registra éxito. Métodos DBus: hasta tres segundos; Response: hasta 120 segundos,
Close del request al vencer. Una conexión dedicada posee la sesión; cerrar
la app cancela solicitudes propias pendientes, nunca las de otra aplicación.
Session.Close permite desregistro de la sesión; una futura configuración
persistida por el portal pertenece al sistema/usuario. Reiniciar requiere otra
sesión; no se guarda un grant universal en la aplicación.

Activated sólo produce Toggle de la ventana propia. Deactivated se consume sin
inventar PTT, audio u otros efectos. Se filtran session_handle e ID de acción.
Session.Closed deja el estado no disponible. No hay fallback rdev, XTest, root,
/dev/input o escucha global. Si no hay API/consentimiento, queda deshabilitado.
Las respuestas del portal se limitan a 64 KiB antes de deserializar y los
eventos de activación a 8192 bytes; respuestas excesivas fallan sin binding.
El activation_token opcional del portal se valida (4096 bytes, sin controles)
y se reenvía únicamente por SSE autenticado: no se exporta en la interfaz DBus
de instancia ni se escribe en logs o SQLite. Su recepción original es la señal
DBus del portal. El módulo
opcional org.kde.kwindowsystem usa las APIs públicas setCurrentXdgActivationToken
y activateWindow sólo sobre el QWindow propio. Su ausencia se comunica en la UI
y conserva el fallback Qt, sin certificar foco nativo. La tecla y concesión real
siguen pendientes por decisión del usuario. Registro y gesto humanos se clasifican por separado.

## Recursos y DBus

Prioridad de recursos: KDE_ASSISTANT_DATA_DIR absoluto y válido → share junto
al prefijo del ejecutable → raíz absoluta de desarrollo compilada. El CWD nunca
aporta código QML. Override inválido falla, sin caer silenciosamente en otra raíz.
Main/Theme/Octicon/tray/iconos críticos se verifican antes de lanzar Qt. Rutas
QML/import absolutas; assets relativos al archivo QML; sonidos embebidos y
config/data/cache XDG no dependen del CWD. No se construye un instalador F6.

zbus con Tokio y cola limitada sustituye dbus-send interno bloqueante.
Tema: portal Settings.Read; 1 dark, 2 light, 0 sin preferencia. El sincronizado
automático del tema visual sigue fuera de esta recuperación.
Notificaciones: firma oficial Notify, app fija, sin acciones, texto validado
(título 160 bytes, cuerpo 2048), markup escapado, tres segundos, ID real del
servidor o error. Un timeout tiene efecto incierto, sin retry. No hay caller
LLM/HTTP de este adaptador y no se envían notificaciones al escritorio en tests.

Fuentes primarias: [GlobalShortcuts](https://github.com/flatpak/xdg-desktop-portal/blob/main/data/org.freedesktop.portal.GlobalShortcuts.xml),
[KGlobalAccel](https://api.kde.org/kglobalaccel.html),
[KWindowSystem QML](https://api.kde.org/qml-org-kde-kwindowsystem-kwindowsystem.html),
[Qt Quick Test](https://doc.qt.io/qt-6.12/qtquicktest-index.html),
[KWin scripting para el harness de pruebas](https://develop.kde.org/docs/plasma/kwin/api/).


## Precisiones de la auditoría final

La revisión final mantiene las fronteras F2. `src/backend/global_shortcuts.rs`
es la ubicación real del módulo (no `src/global_shortcuts.rs`). Se corrigieron
casos reproducidos en bus sintético; no se registró un atajo en Plasma.

- Se calcula el handle de sesión exacto con el nombre único de la conexión y
  session_handle_token. No se acepta ni se cierra un handle diferente devuelto
  por el portal. El binding debe devolver exactamente la única acción toggle.
- Activated, Deactivated y Session.Closed se suscriben antes de CreateSession.
  Closed tiene prioridad durante el handshake y durante escucha. Una activación
  adelantada sólo se entrega después de confirmar el binding válido.
- Un cambio de owner del servicio invalida la sesión y se comunica como cerrada;
  no se intenta registrar de nuevo sin intervención explícita.
- ShortcutSession tiene Drop que aborta su listener y actualiza estado. El cierre
  explícito espera la cancelación y solicita Session.Close. Main siempre recoge
  un owner devuelto por la tarea, incluso si completó al competir con abort.
  Producción usa una conexión dedicada: al desaparecer el último owner se cierran
  sus sesiones según el contrato del portal. Un embebedor con conexión compartida
  debe invocar close para pedir Session.Close mientras conserva esa conexión.

PR_SET_PDEATHSIG pertenece al hilo que ejecuta spawn, no al runtime Tokio entero.
Las dos llamadas de UI ocurren sincrónicamente en main, fuera de tareas Tokio;
el harness comprueba el hilo propietario en /proc/PID/task/TID/children. La
prueba f3_process_owner mantiene vivo el proceso y un hijo hermano, termina
solamente el hilo creador y comprueba SIGKILL del hijo vinculado y su reaping.
El chequeo PPID cubre muerte del proceso antes de prctl. No detecta un hilo
creador ya muerto si el hijo se reparenta dentro del mismo grupo aún vivo;
por eso los launch sites deben permanecer en el líder de proceso. No se afirma
supervisión genérica de descendientes ni de ejecutables setuid/con capabilities.
Referencia: [Linux PR_SET_PDEATHSIG](https://man7.org/linux/man-pages/man2/PR_SET_PDEATHSIG.2const.html).

El módulo AuthToken persiste como caché privada 0700/0600 del perfil, igual que
el token local persistente: no se promete borrado seguro al salir. Los temporales
de publicación se eliminan y cada escritura es atómica. No se introduce un token
en entorno/DBus/logs. Un proceso hostil con control del mismo UID o del bearer
permanece fuera del aislamiento prometido por F2. La raíz de desarrollo compilada
es un fallback explícito después de share; el empaquetado y un modo sin fallback
se reservan para F6, sin afirmar una instalación probada ahora.


La garantía de no transportar prompts/bearer/comandos se refiere a Desktop1.
DBusMenu conserva las etiquetas de UI y títulos de sesiones recientes que ya
publicaba la bandeja; no transporta argumentos de herramientas ni grants F2.
Los object paths del portal y su activation_token son protocolo del sistema,
no el bearer HTTP de la aplicación.
