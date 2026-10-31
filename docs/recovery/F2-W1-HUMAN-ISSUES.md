# GATE-F2-W1H — incidencias y límites

## W1H-01 — automatización Wayland no disponible

El portal KDE concedió acceso de puntero, pero el método legado `NotifyPointerMotion` respondió `Session is not allowed to call NotifyPointer methods`. El cliente `libei` compilado localmente abrió EIS y recibió `CONNECT`/`SEAT_ADDED`, sin `DEVICE_RESUMED`; rechazó el movimiento solicitado. No hubo cursor movido ni clic automatizado. No se instalaron paquetes ni se pidió root/uinput. El gate humano puede continuar en modo manual.

## W1H-02 — H3, atribución humana resuelta

La solicitud H3 terminó `denied` antes del vencimiento y sin lectura. Se interrumpió la conexión antes de recibir la atribución, pero el usuario confirmó posteriormente que pulsó físicamente la X del popup. El resultado backend y la observación humana son coherentes; no queda incidencia de autorización para H3.

La atribución se registró sólo al recibir la respuesta del usuario; no se infirió del estado SQLite.

## W1H-03 — interrupción de infraestructura durante H3

La conexión de Codex y los procesos propios terminaron mientras H3 estaba abierto. La base persistió íntegra y no quedó consentimiento reutilizable. Se reinició el perfil propio sin volver a emitir el caso. No se tocaron procesos ajenos.

## Observaciones heredadas para opinión humana

F2-W1 observó etiquetas `Riesgo: yellow` y `Riesgo: red` en minúsculas inglesas y mensajes técnicos para herramientas no disponibles. Se solicitará opinión en H8. No se cambiará la implementación durante este gate.

## W1H-04 — cobertura humana incompleta en el cierre del 15 de octubre; resuelta el 17

H7 se intentó dos veces el 15 de octubre. El primer cambio de sesión confirmado ocurrió 13 segundos después de expirar el permiso, y el segundo permiso caducó sin nueva sesión observada antes de aquel cierre. La invalidez por cambio de sesión **mientras el consentimiento estaba pendiente** seguía `NOT_TESTED`; ambos intentos comprobaron denegación al vencimiento. Se pidió al usuario aclarar el segundo intento, sin respuesta antes de cerrar la corrida. H8 tampoco recibió respuesta entonces. H5 observó el orden de foco como “x, luego cerrar y luego aceptar”; la referencia “cerrar” es ambigua. La continuación del 17 de octubre resolvió la cobertura H7/H8 sin iniciar F3.

## W1H-05 — primer consentimiento Red caducó

H6 llegó al plazo natural de 120 segundos sin aprobación registrada. SQLite muestra `permission_requests.denied`, `operation_journal.denied`, eventos `prepared → denied`; el fixture permaneció intacto y sin respaldo. El usuario aclaró que pulsó Enter físicamente y no pasó nada. Es una observación de teclado/foco a investigar; no demuestra defecto de autorización. No se amplió el plazo. Se informó al usuario antes de emitir un nuevo consentimiento Red para probar el botón con mouse.

El segundo consentimiento Red también caducó a los 120 segundos sin clic registrado ni efecto. El usuario declaró que no vio el popup ni el recurso para pulsar. Una comprobación KWin posterior mostró la ventana propia mapeada, no minimizada y sin foco; no permite concluir si estaba detrás durante la solicitud. El usuario indicó que veía la ventana principal antes de la prueba. Se informó antes de emitir un tercer consentimiento Red de diagnóstico.

Posteriormente se informó de un tercer consentimiento Red, limitado al mismo fixture, para diagnóstico visual. La captura privada de la ventana propia y KWin mostraron el popup completo y la ventana activa; el usuario confirmó verlo. Esto no explica por qué no vio el segundo popup.

## W1H-06 — aprobación Red en diagnóstico, atribución resuelta

Durante el tercer consentimiento Red se pidió al usuario no pulsar aún. El backend registró una aprobación y despachó `edit_file` a las 21:19:11 hora local. El usuario confirmó después que pulsó **Aprobar con el mouse físico** antes de responder. `edit.txt` contiene el texto nuevo y `edit.bak` el original, como especifica `with_extension("bak")`; SQLite es íntegra, el permiso se consumió y replay devolvió HTTP 409. La instrucción de espera no se siguió, pero no se observó bypass ni aprobación espontánea.

## Continuación del 17 de octubre — H7 resuelto

W1H-04 quedó resuelta: el usuario cambió físicamente de sesión desde la bandeja mientras `read_file` seguía `pending`. El permiso quedó `revoked` y la operación `abandoned` unos 107 segundos antes de su expiración. La nueva sesión se creó 17 ms después de la marca `abandoned`; una aprobación tardía obtuvo HTTP 409 y el mock no recibió contenido. H8 también recibió valoración humana, registrada en W1H-07.

W1H-05 sigue como observación de visibilidad intermitente sin causa identificada. El intento Red no visto no tiene muestra KWin durante el intervalo pendiente; el QML sí registró una solicitud pendiente. La muestra KWin anterior fue posterior al vencimiento. No se reemitió Red en esta continuación ni se afirma que el fallo haya reaparecido.

## W1H-07 — preferencia visual del usuario

En H8, el usuario escribió literalmente: «Esta todo perfecto. Lo entendi bastante bien. Es bastente legible, el orden es el que mencione antes, y el foco se ve claramente, no quedo ningun pupup. Solo adaptaria el diseño del popup para que sea el mismo de la app.» Respondió además: «No, todo fue perfecto», «De ambos, no quedo nigun pupup mal colocado» y «Sí, claramente» al confirmar que distinguió Yellow/Red y pudo leer recurso y consecuencia. Se registra como sugerencia de coherencia visual entre popup y aplicación, sin cambio de implementación en este gate. Su observación actual no elimina el episodio Red anterior que declaró no haber visto; éste permanece sin causa demostrada y no se reprodujo en esta continuación.
