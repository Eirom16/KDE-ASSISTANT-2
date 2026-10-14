# GATE-W1R — Bandeja y rendimiento QML en Plasma Wayland

**Estado: PARTIAL.** El defecto de restauración y el coste creciente del streaming quedaron corregidos y comprobados en KWin Wayland real. La CPU visible en reposo continúa alrededor del 11 % de un núcleo y no hubo observación humana de clic físico, legibilidad o fluidez prolongada. No se inventó un umbral de aceptación para convertir esa medición en PASS.

## Alcance y baseline

- 13 de octubre de 2026, `America/Santo_Domingo` (14 de octubre UTC). Rama `recovery/gate-w1r-ux-performance`, HEAD sin mover `e00fc1f77322b8fd1dace4c9c4858327a141f82a`.
- Se leyeron `AGENTS.md`, los tres documentos de referencia del proyecto, `GATE-W1-{REPORT,RESULTS,ISSUES}.md`, `F1B3B-CLOSURE.md` y `F1B2-CONTRACT.md`. Los informes GATE-W1 existentes y todos los demás documentos sin seguimiento se conservaron.
- Archivos de producción modificados: `qml/Main.qml`, `qml/components/{ChatView,InputBar,TrayMenu}.qml`. Regresiones nuevas: `tests/qml/tst_gate_w1r.qml`, `scripts/validate_gate_w1r.py`. Este informe y los dos documentos W1R son nuevos. No hubo commit, push, merge, release ni trabajo F2/F3/F4.

La sesión era `XDG_SESSION_TYPE=wayland`, `WAYLAND_DISPLAY=wayland-0`, Plasma/KWin 6.7.5 y Qt/QML 6.12.0, con cuatro CPU, 7,6 GiB de RAM e Intel UHD Graphics 605. `qml6` y `/usr/lib/qt6/bin/qmlprofiler` estaban disponibles. La ejecución usó `target/debug/kde-assistant --no-shortcuts`, QML real bajo Wayland, y en los harnesses el mismo `Main.qml` con backend `--ui-off --no-shortcuts`. No se usó `QT_QPA_PLATFORM=offscreen` como evidencia de KWin; sólo en la prueba de componentes aparte.

Todo el estado sintético está en `/tmp/kda-gate-w1r-7pmmhybe` (subperfiles privados `0700` con `XDG_CONFIG_HOME`, `XDG_DATA_HOME`, `XDG_CACHE_HOME` y `TMPDIR`). El proveedor fue `127.0.0.1:8877`; backend `127.0.0.1:8765`. Se quitaron claves API y proxies del entorno. La configuración privada mantuvo `enable_tool_calling=false`, herramientas de acción desactivadas, voz y personaje desactivados. No se pasó `--enable-voice`; no hubo micrófono, descarga de modelos ni proveedor comercial. Cada proceso lanzado se identificó por PID y se recogió con su propio padre; los puertos quedaron libres.

## Defectos reproducidos antes de modificar

**W1-01.** KWin informó ventana `minimized=true`, pero QML registró `visible=true`, `visibility=2` (`Windowed`, frente a `Window.Minimized=3`). El primer `StatusNotifierItem.Activate` emitió `hideRequested` y KWin dejó de listar la ventana; el segundo clic la restauró activa. Por tanto, la hipótesis inicial de `root.visible` era correcta pero comprobar sólo `Window.Minimized` no era suficiente en esta sesión. La corrección usa también `root.active` para decidir si la bandeja debe traer la ventana al frente y llama `showNormal()` al restaurar una ventana visible e inactiva.

**W1-02.** El Qt QML Profiler registró, para 50 deltas en ~2,5 s, 56 reevaluaciones del binding `ChatView.model`, 105 construcciones de `MessageBubble` y 970 imágenes `Octicon`; `refreshUI` apareció 53 veces con ~603 ms inclusivos y el binding del modelo ~615 ms inclusivos. Esos tiempos del profiler se solapan y no se suman. La causa demostrada fue la reasignación del array completo a `ListView.model` por cada delta, que destruía y recreaba delegados visibles. En tres turnos idénticos sin profiler, la CPU de QML subió de 41,25 % a 67,41 % y 95,96 % de un núcleo al crecer el historial.

**W1-03.** `/api/config` comunicó `capabilities.voice=deferred`, pero el InputBar mostraba micrófono y el menú anunciaba «Dictar». No se pulsó el control ni se inicializó audio.

## Correcciones y alcance

`TrayMenu.windowVisible` considera visibilidad, activación y `Window.Minimized`. El handler de mostrar usa `showNormal()` para la ventana visible que KWin dejó inactiva, luego `raise()` y `requestActivate()`. El menú nativo y el popup usan el mismo estado. El criterio `active` es un respaldo para esta combinación Qt/KWin; una ventana visible pero desenfocada también se tratará como candidata a mostrar. Esta ambigüedad residual está registrada en ISSUES.

`ChatView` mantiene un `ListModel` estable. Al reemplazar `Main.messages`, sólo cambia la fila cuyo objeto cambió; añadir, borrar o cargar historial reconcilia el resto. Se mantuvieron las rutas de SSE, Turn ID/Session ID, cancelación, errores, tool calls, recuperación y autoscroll. `approveTool` ahora copia el mensaje y su lista de herramientas afectados para que el badge se actualice con el modelo incremental sin mutar el objeto anterior. No se cambió el backend.

`Main` toma la capacidad efectiva de `/api/config`; el micrófono queda oculto y deshabilitado por defecto y cuando la voz está diferida. «Dictar» se omite del popup y el item de DBusMenu queda `visible=false`, `enabled=false`. La exportación DBusMenu conserva el item oculto en `GetLayout`; Plasma recibe explícitamente esos dos atributos.

## Retest y rendimiento

El retest automático sobre KWin Wayland real pasó visible → oculta, oculta → visible, minimizada → visible y activa en **un clic**, cierre → bandeja → restauración, y tres ciclos adicionales sin perder foco. KWin sólo recibió scripts temporales filtrados por el PID y título de esta instancia; no se modificaron otras ventanas. `DBusMenu` mostró «Ocultar ventana» al estar activa y «Mostrar ventana» al estar oculta o minimizada. No se observó rechazo de foco de KWin en esta ejecución.

Con el mismo mock de 50 deltas/2,5 s, sin profiler, tres turnos dieron **15,44 %, 16,34 % y 15,40 %** de CPU QML de un núcleo; el backend se mantuvo entre 1,76 % y 2,20 %. Los 50 caracteres llegaron en orden en los tres turnos. El primer token se detectó a 50, 31 y 33 ms desde el envío del harness (resolución de sondeo 20 ms), frente a 95, 79 y 94 ms antes. El profiler posterior registró 2 construcciones de `MessageBubble`, frente a 105 antes; sus tiempos sólo se usan para atribuir causa, no como benchmark de CPU.

En reposo visible, dos muestras de 5 s del proceso real dieron 10,79 % y 11,19 %; oculto, 2,60 % y 2,40 %. La ablación privada del pulso del indicador online dio 10,34 % frente a 11,53 % de referencia; la del temporizador de hotkeys dio 10,75 %. Son muestras únicas y próximas entre sí: no se desactivó una animación ni una supervisión necesaria por esa evidencia. RSS visible fue ~187 MiB; el PSS del proceso real quedó en ~123 MiB en esa ejecución, sensible a qué páginas Qt estaban compartidas. Tras tres turnos de streaming el PSS del harness pasó de ~101,1 a ~101,9 MiB; no prueba ausencia de fugas a largo plazo. Véase RESULTS para condiciones y valores completos.

## Regresiones y dictamen

`python3 scripts/validate_gate_w1r.py` pasó 11 pruebas QML aisladas: modelo estable, 50 repintados, UTF‑8, cancelación, sesión nueva, error, recuperación de texto, inserción/eliminación y recarga de igual longitud, badge, voz diferida, cambios rápidos de bandeja, scroll y modelo de 120 mensajes. El harness Wayland real de conversación volvió a pasar dos turnos con `¡Hola, señor! ñandú 😀`, cancelación antes y durante tokens, cambio de sesión sin evento obsoleto y error 503 explícito; SQLite privado conservó dos respuestas confirmadas y pasó `integrity_check=ok`. La batería F1B3B completa se ejecutó con `--qml`; sus resultados finales están en RESULTS. Los tests offscreen no certifican KWin.

**Aceptación de escritorio:** PARTIAL. El chat escrito, la bandeja y el streaming frecuente son utilizables según automatización real de Wayland; faltan observación humana, prueba de ventana visible pero desenfocada mediante clic físico y análisis suficiente de la CPU visible en reposo antes de cerrar F5. La respuesta del backend, cancelación, persistencia y seguridad no mostraron un bloqueo funcional nuevo.

**Autorización de F2:** decisión separada. Esta evidencia no identifica un bloqueo técnico de F1B para F2; tampoco concede autorización ni inicia F2. El estado de escritorio parcial y el consumo en reposo deben seguir visibles para esa decisión.
