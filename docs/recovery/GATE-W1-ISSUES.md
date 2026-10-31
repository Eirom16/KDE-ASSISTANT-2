# GATE-W1 — Incidencias y límites

No se aplicaron correcciones de producción. Todas las reproducciones usaron la configuración XDG sintética de `/tmp/kda-gate-w1-jehX1Fa7` y el HEAD `e00fc1f77322b8fd1dace4c9c4858327a141f82a`.

## W1-01 — Bandeja requiere dos clics tras minimizar

**Severidad: media. Estado: FAIL reproducible.**

1. Mostrar la ventana real bajo KWin Wayland. KWin informó `minimized=false`.
2. Minimizar **sólo** la ventana PID 19276 mediante KWin. KWin informó `minimized=true`, `active=false`.
3. Invocar `org.kde.StatusNotifierItem.Activate(0,0)` del indicador propio. KWin pasó a **cero ventanas mapeadas** para ese PID.
4. Invocar `Activate` otra vez. La ventana volvió, `minimized=false`, `active=true`.

Evidencia: `evidence-journal.log` (`GATE_W1_MINIMIZE`, `GATE_W1_KWIN_COUNT`) y DBusMenu `Menu=/MenuBar`. La ruta sospechosa está en `qml/Main.qml`, `TrayMenu { windowVisible: root.visible }`, y `qml/components/TrayMenu.qml`, `onActivated`: la ventana minimizada sigue siendo `visible=true`, por lo que el primer clic emite `hideRequested`. Corrección mínima propuesta: tratar `minimized` como estado que debe restaurarse al activar la bandeja, y sincronizar el texto Mostrar/Ocultar del menú con ese criterio. Probar después visible, minimizada y oculta bajo Wayland, sin modificar otros QML.

Impacto: el usuario cree que el primer clic de bandeja no respondió al intentar recuperar una ventana minimizada. **No bloquea la corrección funcional F2**, pero sí una aceptación pulida de KDE/F3 o uso diario sin fricción.

## W1-02 — CPU de QML alta durante streaming y reposo visible

**Severidad: media. Estado: observación reproducida, causa exacta pendiente de perfilado.**

Con 50 deltas sintéticos en ~2,5 s, la ventana Wayland QML gastó 102,93 % y 96,46 % de un núcleo en dos muestras de 1,7 s; el backend consumió 2,35 % y 1,76 %. Visible y sin conversación activa consumió 11,0 % y 10,6 % en dos muestras de 5 s; oculta quedó en 2,2 %. No se deduce un umbral de calidad ni se extrapola a otros equipos o tasas de tokens.

Ruta sospechosa para investigar: `qml/Main.qml` `streamAgent`/`refreshUI` copia y reasigna `messages` en cada `readyState` y delta; `qml/components/ChatView.qml` usa ese array como modelo de `ListView` y reubica el scroll al cambiar altura. En reposo, `qml/Main.qml` mantiene animado el indicador online y un `hotkeyTimer` de 300 ms. Es una **hipótesis**, no un perfil de llama confirmado. Corrección mínima propuesta tras medir con Qt/QML Profiler: actualizar sólo el delegado de la respuesta en curso o limitar repintados al frame, y suspender animaciones/polling prescindibles cuando la ventana esté oculta o los atajos deshabilitados. Repetir el mismo fixture y comparar CPU y latencia visible antes/después.

Impacto: puede elevar consumo y temperatura en este equipo de cuatro núcleos durante respuestas largas. No se observó pérdida de tokens ni bloqueo de UI en estos turnos; por ello **no se clasifica como bloqueo de corrección F2**, pero merece atención antes de recomendar uso cotidiano prolongado o cerrar F5.

## W1-03 — Controles de dictado visibles con voz diferida

**Severidad: baja. Estado: observación visual; alcance F3.** La captura inicial muestra el icono de micrófono y el DBusMenu anuncia «Dictar», mientras `/api/config` comunica `voice=deferred` y el proceso fue iniciado sin `--enable-voice`. Rutas a revisar: `qml/components/InputBar.qml` (`voiceEnabled` por defecto) y `qml/components/TrayMenu.qml` (fila Dictar). Corrección propuesta para F3: reflejar la capacidad efectiva en el estado y texto de esos controles. No se pulsó el micrófono para respetar el límite de esta prueba; su comportamiento al pulsarlo queda NOT_TESTED. No es una regresión de chat F1B ni bloquea F2.

## Cobertura pendiente

La sesión KDE del usuario estaba en tema oscuro; cambiar su tema habría alterado configuración personal, así que claro es NOT_TESTED. Atajos globales quedaron deshabilitados por `--no-shortcuts`; el registro y conflictos son NOT_TESTED. El diálogo de recuperación tuvo comprobación de propiedades y señales del QML real en Wayland, pero no captura fiable ni revisión humana de sus controles. Tampoco se hizo ensayo humano de navegación, ratón, accesibilidad o varias horas de uso. Estas ausencias no se cuentan como PASS ni como defectos F1B.

La captura directa por `org.kde.KWin.ScreenShot2` fue denegada con `NoAuthorized`; Spectacle produjo capturas de la ventana real. Es una restricción de autorización de captura, no un fallo de KDE Assistant. Un intento posterior de Spectacle apuntó a otra ventana activa del escritorio: ese archivo temporal se eliminó inmediatamente y no se incorporó como evidencia. Los informes sólo usan capturas confirmadas de KDE Assistant.
