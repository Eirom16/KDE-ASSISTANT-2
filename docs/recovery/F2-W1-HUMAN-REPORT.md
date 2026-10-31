# GATE-F2-W1H — informe de aceptación interactiva

**Dictamen final: ACCEPTED (17 de octubre de 2026).** H1–H6 tuvieron interacción humana y resultados coherentes con el journal. La continuación H7/H8 del 17 de octubre se documenta al final; el cierre anterior del 15 de octubre había sido `PARTIAL`.

## Baseline y aislamiento

- Rama `recovery/f2-permission-engine`; HEAD `8b21c7fd505c8c6b5d0e9bcb5c7ecbd7455c9835`.
- No se modificó código de producción ni el índice Git. Los documentos históricos sin seguimiento se conservaron.
- Se leyeron `AGENTS.md`, `F2-CLOSURE.md`, `F2-PERMISSION-CONTRACT.md` y los tres informes F2-W1.
- Plasma/KWin 6.7.5, Qt/QML 6.12.0, sesión `wayland-0` con KWin real.
- Perfil sintético privado `/var/tmp/kda-f2-w1h-GvGz81` (0700) con `XDG_CONFIG_HOME`, `XDG_DATA_HOME`, `XDG_CACHE_HOME` y `TMPDIR` privados. SQLite y archivos `read.txt`, `edit.txt` son nuevos; `create.txt` permanece ausente. La raíz autorizada es exclusivamente `synthetic/`.
- Backend real `--ui-off --no-shortcuts`, interfaz `Main.qml` auténtica mediante `qml6 -apptype widget` y wrapper temporal que emite cada turno de prueba. Proveedor OpenAI compatible simulado en `127.0.0.1:8877`; backend en `127.0.0.1:8765`. Sin credenciales comerciales, voz, micrófono ni modelos.
- `/api/tools` mostró 23 entradas, sólo `system_info`, `read_file`, `create_file` y `edit_file` habilitadas; las otras 19 permanecieron restringidas.

## Control gráfico y KWin

La API Computer Use disponible devolvió `apps: []` y no ofreció control de aplicaciones Linux en esta sesión. No se utilizó XTest ni `xdotool`. El portal `org.freedesktop.portal.RemoteDesktop` de KDE concedió sesiones de puntero tras autorización física del usuario. Su método `NotifyPointerMotion` rechazó el movimiento con `Session is not allowed to call NotifyPointer methods`; `ConnectToEIS` abrió el canal `libei`, pero no entregó un dispositivo de puntero reanudado al cliente temporal. El movimiento seguro de demostración fue rechazado y **no hubo clic automatizado**. Se cerraron las sesiones del portal utilizadas. La clasificación de automatización es `AUTOMATION_UNAVAILABLE`.

Un script KWin temporal, filtrado por PID y título, constató la ventana propia `KDE Assistant v2` con geometría `(395,10) 575×708` en una muestra; se descargó el script inmediatamente. QML registró la recepción de consentimientos y sus cierres. Esto es evidencia `AUTOMATED_KWIN` y `AUTOMATED_QML`, no `HUMAN_VERIFIED`.

## Interacción y seguridad observadas

H1 tuvo un primer intento en que el usuario pulsó físicamente **Aprobar**, según su aclaración posterior. El backend registró correctamente `approved → consumed`, lectura lograda y envío del contenido sintético al mock. Ese intento no cuenta como denegación.

H1 se repitió con un consentimiento nuevo. El usuario confirmó haber pulsado físicamente **Denegar**. La solicitud y la operación quedaron `denied`; el journal sólo registró `prepared → denied`; el mock no recibió el contenido. SQLite pasó `integrity_check` y `foreign_key_check`, sin permisos pendientes reutilizables.

H2: el usuario confirmó aprobación física tras comprobar el recurso sintético. La solicitud quedó `consumed`; journal `prepared → authorized → dispatched → succeeded`; lectura de 47 bytes sintéticos remitida al mock. Reenviar el mismo Consent ID devolvió HTTP 409 y no produjo otro despacho.

H3: se abrió un consentimiento nuevo. El usuario confirmó después de la interrupción que pulsó físicamente la **X del popup**. El backend registró `denied` a los 47 segundos, antes del vencimiento de 120 segundos, sin contenido enviado al mock. La confirmación humana y el journal son coherentes.

El proceso supervisor cayó durante la interrupción de infraestructura. Se verificó que 8765/8877 estaban libres, el permiso H3 estaba denegado y SQLite íntegra. Se reinició sólo el backend/QML/mock propios con el mismo perfil y el control de turnos en secuencia cero, sin reemitir H3. La sesión de escritorio del usuario no se alteró.

## Historial de pruebas

H4: el usuario pulsó Escape físicamente y observó el cierre. La solicitud quedó `denied`, el journal `prepared → denied`, sin contenido enviado al mock; SQLite íntegra y sin permisos pendientes.

H5: el usuario recorrió con Tab sin pulsar Enter y describió literalmente: “El foco iniciar es x, luego cerrar y luego aceptar. Si los focos son visibles.” Después el backend registró `prepared → denied`, sin contenido en el mock. No hubo aprobación accidental. La descripción “cerrar” no se reinterpreta como una etiqueta concreta del botón.

H6 primer intento: el usuario afirmó entender la acción Red antes de aprobar. Aclaró que pulsó físicamente **Enter** y no ocurrió nada; no pulsó el botón Aprobar con el mouse. La solicitud expiró exactamente a los 120 segundos y terminó `denied`, sin despacho. Se informó antes de reemitir Red. El segundo consentimiento Red también expiró exactamente a los 120 segundos, sin aprobación ni despacho. El usuario dijo literalmente “No veo el recurso, esta vez no aparece para pulsarlo” y “NO LO VI” sobre el popup. Tras esos dos intentos `edit.txt` mantuvo su SHA-256 inicial `81824e58b19d84c09171d0ae8c65671652b6300466a1e136da919ddbeda1a1d3` y no había respaldo. Una muestra KWin posterior vio la ventana propia mapeada y no minimizada, pero sin foco; no demuestra su estado durante el popup. El usuario aclaró: “Antes de la prueba de aceptar veía la ventana claramente.”

Se informó de un **tercer consentimiento Red de diagnóstico** y se pidió no pulsar ningún botón hasta comprobar la visibilidad. Con la solicitud pendiente, KWin observó la ventana propia activa y mapeada; una [captura privada de la ventana propia](/var/tmp/kda-f2-w1h-GvGz81/evidence/h6-diagnostic.png) mostró completo el popup `edit_file`, su recurso, el riesgo Red y los botones. El usuario confirmó que lo veía y aclaró posteriormente que **pulsó Aprobar con el mouse físico** antes de responder. El backend registró `approved → consumed` y `prepared → authorized → dispatched → succeeded`. El fixture quedó editado y `edit.bak` contiene exactamente el original: el código usa `with_extension("bak")`, por lo que el respaldo de `edit.txt` es `edit.bak`. Reenviar el Consent ID consumido devolvió HTTP 409. La secuencia se desvió de la indicación de esperar, pero la aprobación tuvo una acción física identificada y el backend no despachó sin consentimiento.

H7 se intentó dos veces el 15 de octubre. En la primera, el usuario informó «Cambió de sesión», y SQLite confirmó la creación de una nueva conversación **13 segundos después** de que el consentimiento venciera a los 120 segundos. La solicitud quedó `denied`, sin contenido del fixture en el mock. En el segundo intento, también venció a los 120 segundos sin despacho ni una nueva sesión observada hasta aquel cierre. Se solicitó aclaración al usuario sobre ese intento, sin respuesta antes de cerrar la corrida. **En ese momento** el cambio de sesión durante un consentimiento pendiente no estaba demostrado. H8 tampoco había recibido respuesta entonces. Ambos se completaron en la continuación del 17 de octubre.

## Dictamen por área al cierre del 15 de octubre

- **Seguridad del backend:** PASS en H1/H2, cierres seguros H3–H5 y H6 Red aprobado físicamente; los consentimientos consumidos rechazaron replay con HTTP 409. H7 confirmó expiración segura dos veces, pero no invalidez por cambio de sesión mientras pendiente.
- **Interfaz:** PARTIAL; apertura, aprobación, denegación Yellow, cierre mediante X o Escape y navegación Tab sin aprobación accidental tuvieron interacción física. El popup Red se capturó y KWin lo vio activo en el tercer intento; el usuario lo aprobó con mouse. La visibilidad fue inconsistente entre el segundo y tercer intento. El cambio de sesión durante un popup pendiente sigue sin demostrarse.
- **Integración KWin:** PASS acotado al mapeo y geometría de una ventana propia; no se validó automatización del cursor ni todos los estados de foco.
- **Observación humana:** H1–H6 verificadas en el alcance descrito; H7 sólo tuvo cambio de sesión después del vencimiento en el primer intento; H8 `NOT_TESTED` por falta de respuesta.
- **Herramientas restringidas:** `/api/tools` mantuvo exactamente 19 deshabilitadas; no se activaron adaptadores adicionales.

Esta sección describe el estado `PARTIAL` del 15 de octubre. La continuación posterior abrió un perfil nuevo y repitió sólo H7, con una prueba humana por turno. No se inició F3.

## Cierre

En el cierre anterior a la continuación: `PRAGMA integrity_check=ok`, `PRAGMA foreign_key_check` sin filas y cero solicitudes `pending`/`approved`; se detuvieron backend, QML y mock propios, se cerró el portal, se descargó el script KWin propio y quedaron libres 8765/8877. La continuación reabrió únicamente procesos propios del mismo perfil. Al cierre definitivo: `edit.txt` contiene la edición sintética y `edit.bak` el original; SQLite sigue íntegra, con 3 solicitudes `consumed`, 8 `denied` y ninguna `pending` o `approved`. Backend, QML y mock propios se detuvieron; PID 19705/19715 ya no existen, puertos 8765/8877 libres y script KWin propio descargado (`isScriptLoaded=false`). El portal de control remoto se había cerrado y no hubo cursor automatizado. HEAD e índice Git no cambiaron. El perfil de evidencia permanece 0700 sin mostrar el bearer.

## Límites de seguridad

El mock y los fixtures están confinados al perfil privado; los logs pueden contener identificadores de consentimientos y resultados sintéticos, pero no muestran el bearer. No se han abierto documentos personales ni habilitado herramientas adicionales. Ningún resultado de este informe autoriza iniciar F3.

## Continuación del 17 de octubre de 2026 — H7

Se mantuvieron la rama y HEAD indicados. Se leyó el estado anterior y se creó un **perfil nuevo** `/var/tmp/kda-f2-w1h-close-daQqpJ1C` con permisos 0700, XDG y SQLite privados, fixture de 47 bytes y proveedor simulado en loopback. Los puertos 8765/8877 estaban libres antes de arrancar. La sesión es Wayland `wayland-1`, con KWin y Plasma activos; el backend real y `qml6 -apptype widget` arrancaron con procesos propios. `/api/tools` confirmó cuatro herramientas habilitadas y 19 deshabilitadas. No se repitieron H1–H6 ni la automatización de cursor.

Antes de emitir el permiso, el usuario localizó físicamente «Nueva sesión» en el menú de bandeja y respondió **LISTO**. Se creó una única solicitud Yellow `read_file` sobre el fixture privado, operación `02bfd7b8-62a6-4b42-933f-f52705409842`, Consent ID `46c015aa-7876-4e6a-9f65-49c9b6eab58e`, Session ID anterior `b066d686-f375-45b1-99af-55ee841031be` y Turn ID `a2c0637c-f013-4c92-ae25-c98c900addc0`. SQLite confirmó `pending` a las **13:00:36** hora local, con vencimiento a las 13:02:36. QML registró `W1H_PENDING 1`; KWin observó la ventana propia mapeada y activa a las 13:00:46, mientras la solicitud seguía pendiente.

El usuario confirmó haber seleccionado físicamente «Nueva sesión». El journal cambió la operación a `abandoned` a las **13:00:48.910**; `permission_requests.state` pasó a `revoked`. La nueva sesión `fa594d33-25dd-44a9-a881-45c15fcd60b7` se creó a las **13:00:48.927**, aproximadamente 12 segundos después de emitir el permiso y 107 segundos antes de caducar. La secuencia `prepared → abandoned` conservó el Session ID y Turn ID antiguos. El mock recibió sólo la solicitud inicial, sin contenido del fixture ni seguimiento de herramienta. Una aprobación tardía con los IDs originales devolvió HTTP 409 (`turn_not_waiting_for_approval`); los estados siguieron `revoked`/`abandoned`. SQLite: `integrity_check=ok`, cero violaciones de claves foráneas y cero permisos `pending` o `approved`.

La captura de H7 se tomó a las 13:00:50, **después** del cambio de sesión, y muestra la ventana sin popup; no se usa como prueba visual de un consentimiento pendiente. La evidencia del intervalo pendiente es QML, KWin y SQLite. El resultado H7 es **PASS** para invalidación por cambio de sesión real y rechazo de aprobación tardía, con acción humana confirmada.

La incidencia del popup Red anterior no tiene causa demostrada: QML registró consentimiento pendiente en el intento no visto, pero la muestra KWin de ese intento fue posterior al vencimiento. El tercer intento Red anterior sí tuvo captura del popup y aprobación humana. No se emitió otra operación Red en esta continuación.

## Continuación del 17 de octubre de 2026 — H8

Observación **literal** del usuario, recibida tras H7:

> Esta todo perfecto. Lo entendi bastante bien. Es bastente legible, el orden es el que mencione antes, y el foco se ve claramente, no quedo ningun pupup. Solo adaptaria el diseño del popup para que sea el mismo de la app.

El usuario describió comprensión general, legibilidad y foco visible, y propuso que el diseño del popup coincida con el de la aplicación. Respondió además literalmente «No, todo fue perfecto», «De ambos, no quedo nigun pupup mal colocado» (sobre Yellow y Red) y «Sí, claramente» al preguntarle si distinguió los riesgos y pudo leer archivo y consecuencia antes de decidir. Estas observaciones no sustituyen el registro previo de un intento Red que no vio: ese episodio no tuvo diagnóstico KWin durante el permiso pendiente y su causa sigue sin demostrarse.

**Dictamen final: ACCEPTED para GATE-F2-W1H.** H1–H6 conservaron sus resultados humanos documentados; H7 demostró cambio físico de sesión durante un permiso pendiente, revocación antes de caducidad y rechazo de aprobación tardía; H8 confirmó comprensión, legibilidad, foco y distinción Yellow/Red. El backend no despachó la lectura revocada, SQLite permaneció íntegra y las otras 19 herramientas siguieron deshabilitadas. La preferencia de igualar el diseño del popup al de la aplicación queda registrada sin modificar F2. La aparición intermitente Red del intento anterior queda como incidencia visual no reproducida ni explicada, sin evidencia de bypass.

### Estado final por área

- **Seguridad del backend:** PASS para los escenarios humanos H1–H7 documentados; revocación H7 previa al vencimiento, sin lectura ni replay.
- **Interfaz:** PASS en los controles humanos ensayados; H8 encontró comprensión, legibilidad y foco visibles. Sigue abierta una observación de aparición Red intermitente sin causa demostrada ni reproducción en esta continuación.
- **Integración KWin:** ventana propia activa y mapeada durante H7 pendiente; la captura H7 posterior al cambio no se presenta como evidencia del popup. El intento Red no visto carece de muestra KWin durante su vigencia.
- **Observación humana:** confirmaciones físicas H1–H7 y evaluación literal H8; ningún clic automatizado se clasificó como humano.
- **Herramientas restringidas:** cuatro habilitadas, 19 deshabilitadas; sin cambio de flags.

### Cierre de la continuación

Tras H8 se detuvieron sólo backend, QML y mock propios. Los PID 46265 y 46276 dejaron de existir; 8765 y 8877 quedaron libres. Ambos scripts KWin temporales constan descargados. No se abrió sesión nueva de control remoto ni se automatizó el cursor. El perfil nuevo permanece 0700 y la captura privada 0600. SQLite terminó con `integrity_check=ok`, `foreign_key_check` vacío, una solicitud `revoked`, una operación `abandoned`, eventos `prepared → abandoned` y cero autorizaciones activas. Rama, HEAD, índice y archivos rastreados permanecieron intactos; sólo se actualizaron estos tres informes sin seguimiento. No se inició F3.
