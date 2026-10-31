# GATE-F2-W1H — resultados individuales

**Dictamen final: ACCEPTED.** Perfil privado inicial: `/var/tmp/kda-f2-w1h-GvGz81`. Continuación H7/H8 del 17 de octubre: `/var/tmp/kda-f2-w1h-close-daQqpJ1C`.

| Prueba | Acción física del usuario | Resultado backend/journal | Evidencia y clasificación |
|---|---|---|---|
| Cursor | Autorizó el portal KDE | No se movió: `NotifyPointerMotion` denegado; EIS sin dispositivo listo | `AUTOMATION_UNAVAILABLE`; ningún clic automatizado |
| H1 intento inicial | Aclaró que pulsó Aprobar | `consumed`, `succeeded`; contenido sintético remitido al mock | `HUMAN_VERIFIED` para aprobación, no para denegación |
| H1 repetida | Pulsó Denegar y confirmó `DENEGADO` | `denied`, `denied`; eventos `prepared → denied`; sin texto del fixture en follow-up del mock | `HUMAN_VERIFIED` + SQLite/mock |
| H2 | Pulsó Aprobar y confirmó `LISTO` | `consumed`, `succeeded`; eventos `prepared → authorized → dispatched → succeeded`; replay HTTP 409 | `HUMAN_VERIFIED` + SQLite/mock |
| H3 | Confirmó que pulsó la X del popup | `denied`, `denied` antes de caducidad; sin texto del fixture en mock | `HUMAN_VERIFIED` + SQLite/mock |
| H4 Escape | Pulsó Escape y confirmó cierre | `denied`, `denied`; eventos `prepared → denied`; sin texto del fixture en mock | `HUMAN_VERIFIED` + SQLite/mock |
| H5 teclado | “El foco iniciar es x, luego cerrar y luego aceptar. Si los focos son visibles.”; no pulsó Enter | `denied`, `denied`; eventos `prepared → denied`; ningún contenido al mock | `HUMAN_VERIFIED` para foco observado; SQLite/mock para no despacho |
| H6 primer intento Red | Confirmó que entendía; pulsó Enter físicamente y no ocurrió nada | Caducó a los 120 s: `denied`, `denied`; `edit.txt` intacto, sin `.bak` | Seguridad de expiración PASS; Enter no activó Aprobar; edición humana pendiente |
| H6 segundo intento Red | “No veo el recurso, esta vez no aparece para pulsarlo”; “NO LO VI” sobre el popup | Caducó a los 120 s: `denied`, `denied`; `edit.txt` intacto, sin `.bak` | Seguridad de expiración PASS; visibilidad del popup sin resolver; edición `NOT_TESTED` |
| H6 tercer intento Red, diagnóstico | Confirmó que veía el popup y que pulsó Aprobar con el mouse físico | `consumed`, `succeeded`; `edit.txt` editado, `edit.bak` con original; replay HTTP 409; captura privada y KWin activo | `HUMAN_VERIFIED` + backend/journal/filesystem |
| H7, primer intento | Usuario indicó «Cambió de sesión» | El permiso caducó a los 120 s y quedó `denied`; la nueva sesión se creó 13 s después del vencimiento; mock sin contenido del fixture | Cambio humano observado, pero no ocurrió mientras el permiso seguía pendiente; invalidez por cambio de sesión `NOT_TESTED` |
| H7, segundo intento | Se solicitó de nuevo «Nueva sesión» desde la bandeja; no llegó respuesta humana antes del cierre | El permiso volvió a caducar a los 120 s, `denied`; no se creó otra sesión antes del cierre; mock sin contenido | Expiración segura PASS; cambio durante consentimiento `NOT_TESTED` |
| H7, continuación | Usuario localizó previamente «Nueva sesión», respondió LISTO y confirmó que cambió de conversación físicamente desde la bandeja | Solicitud `pending` a las 13:00:36; `revoked`/operación `abandoned` a las 13:00:48.910; nueva sesión a las 13:00:48.927, antes del vencimiento; late approval HTTP 409; mock sin contenido | **HUMAN_VERIFIED + AUTOMATED_QML + AUTOMATED_KWIN + SQLite/journal: PASS** |
| H8 usabilidad del 15 de octubre | Solicitada; no llegó respuesta antes de aquel cierre | — | `NOT_TESTED` en esa corrida |
| H8 continuación del 17 de octubre | «Esta todo perfecto. Lo entendi bastante bien. Es bastente legible, el orden es el que mencione antes, y el foco se ve claramente, no quedo ningun pupup. Solo adaptaria el diseño del popup para que sea el mismo de la app.» Aclaró «De ambos, no quedo nigun pupup mal colocado» y «Sí, claramente» sobre distinguir riesgo, recurso y consecuencia. | No requiere despacho de herramienta | `HUMAN_VERIFIED` para comprensión, legibilidad, foco y distinción Yellow/Red; sugerencia de diseño literal |

Los cuatro consentimientos emitidos hasta H3 tuvieron IDs de operación, consentimiento, llamada y turno distintos, todos en la misma sesión de conversación. Los dos intentos H7 quedaron asociados a sesiones diferentes y terminaron denegados por vencimiento. SQLite pasó `PRAGMA integrity_check=ok` y `PRAGMA foreign_key_check` sin filas; tras la interrupción siguió íntegra y sin solicitudes `pending` o `approved`.

Los dos primeros intentos Red no editaron el fixture. El tercero lo hizo tras un clic físico de aprobación confirmado por el usuario. `create.txt` sigue ausente. El catálogo del mock anunció sólo las cuatro herramientas habilitadas.

El usuario confirmó posteriormente que cerró H3 mediante la X. Los procesos de aquella corrida se cerraron y los puertos quedaron libres. Se reabrió el mismo perfil sin reutilizar consentimientos; H4–H6 pasaron en el alcance indicado. **Al cierre del 15 de octubre**, H7 aún no demostraba el cambio mientras el permiso estaba pendiente y H8 no había recibido valoración. SQLite entonces: 3 permisos `consumed`, 8 `denied`, 0 activos; integridad `ok`, 0 errores de claves foráneas. Se detuvieron los procesos propios y 8765/8877 quedaron libres.

La última frase anterior describe el **cierre del 15 de octubre**. En la continuación del 17 de octubre, H7 sí demostró el cambio durante un permiso pendiente. El nuevo perfil tiene un único `permission_request` en estado `revoked`, una operación `abandoned`, eventos `prepared → abandoned`, dos sesiones y ningún permiso activo. El mock registró sólo la solicitud inicial H7, sin texto del fixture. La captura privada de las 13:00:50 ocurrió después del cambio y no se presenta como imagen del popup pendiente.

H8 recibió además las respuestas literales «No, todo fue perfecto», «De ambos, no quedo nigun pupup mal colocado» y «Sí, claramente» sobre la distinción de riesgos y lectura de recurso/consecuencia. Al cierre de la continuación, SQLite seguía íntegra, ambos procesos propios habían terminado, 8765/8877 estaban libres y los scripts KWin temporales descargados. `git diff` del árbol rastreado e índice vacío; HEAD conservado.
