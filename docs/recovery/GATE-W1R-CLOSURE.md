# GATE-W1R — Cierre de revisión y consolidación local

Fecha: 2026-10-13. Rama `recovery/gate-w1r-ux-performance`.

**Código: CONSOLIDATED. Escritorio: PARTIAL.** La consolidación local quedó autorizada y se hizo en dos commits normales. La aceptación global sigue parcial por CPU visible en reposo, accesibilidad/fluidez observadas por una persona y el clic físico de bandeja. No se inició F2, F3, F4 ni F5; no hubo push, merge ni release.

## Baseline y commits

Baseline `e00fc1f77322b8fd1dace4c9c4858327a141f82a`, árbol `2ee7dd18e778b10a8344aa42bef7eb61f11fd82b`. Al iniciar, HEAD coincidía con ese SHA y no había commits inesperados. Los 26 documentos históricos sin seguimiento conservaron su contenido; los dos documentos de cierre y el parche se dejan sin seguimiento porque documentan los hashes finales.

| Commit | Padre | Árbol | Archivos |
| --- | --- | --- | --- |
| A `28e09b98cd6baade2830617f929d68867a917926` | baseline | `230418119d463b5248f90ccef22c0d93130f282e` | Sólo `qml/Main.qml`, `qml/components/ChatView.qml`, `InputBar.qml`, `TrayMenu.qml`. |
| B `991f5c78dddf0b4f208bf5683604db2cb5c51112` | A | `23f33be11391d3b726f879de6b1462acad19974a` | Sólo `tests/qml/tst_gate_w1r.qml`, `scripts/validate_gate_w1r.py` y los informes `GATE-W1R-{REPORT,RESULTS,ISSUES}.md`. |

El manifiesto, las pruebas sobre cada archive staged y los controles del parche están en [GATE-W1R-COMMIT-VALIDATION.md](GATE-W1R-COMMIT-VALIDATION.md). El parche [GATE-W1R-CHANGES.patch](GATE-W1R-CHANGES.patch) contiene sólo el diff baseline → HEAD.

## Auditoría del código y defectos

**Bandeja W1-01.** Antes del arreglo, KWin indicó `minimized=true` mientras Qt seguía con `visible=true` y `visibility=Windowed`; el primer `StatusNotifierItem.Activate` ocultó la ventana. El handler consolidado considera `active` y `Window.Minimized`, usa `showNormal()` para ventana visible/inactiva y después `raise()`/`requestActivate()`. En el retest actual, visible → oculta, oculta → visible, minimizada → activa con un clic, cierre → restauración y tres ciclos adicionales pasaron. Una ventana auxiliar propia puso KDE Assistant visible pero inactiva; el menú cambió a «Mostrar» y un solo Activate la volvió activa sin ocultarla. KWin puede aplicar otras políticas de foco en contextos no ensayados; no se implementó integración KWin/DBus nueva.

**Streaming W1-02.** El profiler previo demostró 105 creaciones de `MessageBubble` por 50 deltas y 56 reevaluaciones del modelo anterior. La reasignación del array completo a `ListView.model` recreaba delegates. El `ListModel` estable conserva las filas existentes y actualiza la fila cuyo objeto cambió. La auditoría confirmó que `refreshUI`, cancelación y aprobación de herramientas reemplazan el objeto afectado; la aprobación copia también `toolCalls`, evitando mutación in-place invisible a los bindings. `loadMessages` prepara el historial antes de asignar el array. Los guards existentes de Session ID, Turn ID, generación y secuencia no se tocaron. Inserción, eliminación y recarga de igual longitud tienen una regresión QML adicional. No hay conexiones ni timers nuevos en el diff. El autoscroll mantiene la posición del lector en la prueba de 80 filas/20 cambios.

**Voz W1-03.** `/api/config` devolvió `capabilities.voice=deferred`. `voiceAvailable` arranca en falso; InputBar oculta y deshabilita dictado, el menú propio omite «Dictar» y DBusMenu exporta ese item con `visible=false` y `enabled=false`. La prueba QML confirma que cambiar legítimamente `voiceEnabled` a verdadero vuelve a mostrar el control. No se inicializó audio, Whisper, Piper ni ONNX.

## Validación funcional y Wayland real

El árbol de trabajo pasó `python3 scripts/validate_gate_w1r.py`: **11 casos QML offscreen**, y `python3 scripts/validate_f1b3b.py --qml`: **13 gates**. Ambos archives staged compilaron y pasaron todos sus gates disponibles; A sólo tenía la suite F1B, B tenía además W1R. Una primera ejecución del gate F1B en el árbol de trabajo coincidió por error con el retest Wayland en el puerto 8765 y falló al reservarlo. Se esperó el cierre de todos los procesos propios y se repitió en exclusiva: PASS. No se cuenta esa colisión como fallo de producto.

En Plasma/KWin Wayland real (`XDG_SESSION_TYPE=wayland`, `WAYLAND_DISPLAY=wayland-0`), con XDG/TMPDIR privados, `--no-shortcuts`, voz y acciones desactivadas y proveedor simulado en `127.0.0.1:8877`, el retest automático de tray pasó para PID QML 37341 y el caso visible/desenfocado para PID QML 39885. Los scripts KWin filtraron por PID y título propios; la ventana auxiliar también era un proceso propio. Los puertos 8765/8877 quedaron libres y los procesos se recogieron.

El harness de chat real de PID QML 39526 observó dos respuestas confirmadas con `¡Hola, señor! ñandú 😀`, streaming parcial, `Cancelando` → `Cancelado` antes y durante tokens, cambio de sesión sin eventos obsoletos y fallo 503 explícito sin éxito falso. Un segundo harness, PID 47761, emitió 50 deltas distintos `00,`…`49,`; cada muestra del texto fue prefijo del esperado y el final coincidió exactamente (150 caracteres). Qt offscreen, automatización KWin y observación humana se registran como evidencias distintas. No hubo clic físico ni evaluación humana de legibilidad, fluidez o accesibilidad.

## Rendimiento y riesgos transferidos

En el fixture original de 50 deltas cada 50 ms, CPU QML de un núcleo: **antes 41,25 / 67,41 / 95,96 %**, **después 15,44 / 16,34 / 15,40 %**. El profiler pasó de 105 a 2 creaciones de `MessageBubble`. La repetición sin profiler tras el cierre dio **14,44 / 16,43 / 16,08 %**, primer token **55 / 38 / 28 ms** por sondeo de 20 ms, y backend **1,70 / 1,78 / 1,27 %** durante esos turnos. RSS/PSS QML al final de cada turno: 186132/108643, 186928/109439 y 187928/110439 KiB. Son muestras del equipo objetivo, no límites universales ni garantía de ausencia de fugas largas.

El reposo visible del proceso real después del arreglo fue **10,79 / 11,19 %** en dos intervalos de 5 s; permanece como deuda de F5. La muestra de reposo posterior del harness actual fue 12,22 % tras tres turnos; la previa de 22,45 % coincidió con calentamiento de arranque y no se usa para comparar. No se eliminaron timers ni animaciones por una ablación inconclusa. F3 puede estudiar foco, interacción física con bandeja y accesibilidad; F5 puede perfilar el reposo. Ninguno se inició aquí.

**Uso cotidiano:** el chat escrito, recuperación, cancelación, errores y restauración desde bandeja funcionan en la automatización real de esta sesión Wayland. La CPU visible en reposo puede afectar batería/temperatura; falta la validación humana del gesto y la lectura visual. No se observó un bloqueo funcional nuevo del núcleo F1B.

**F2:** esta consolidación no identifica un bloqueo técnico nuevo de F1B para F2. La autorización de F2 corresponde a una decisión posterior de dirección técnica; no se concede ni se inicia por este cierre.
