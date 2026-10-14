# GATE-W1R — Incidencias y límites pendientes

No se introdujeron cambios de backend, commits ni fases nuevas. Reproducciones y trazas están en `/tmp/kda-gate-w1r-7pmmhybe`; sólo contienen SQLite y configuración sintéticos. El estado general es **PARTIAL** por rendimiento visible en reposo y observación humana pendiente, aunque los dos fallos funcionales principales de W1 quedaron resueltos en el retest automático Wayland.

## W1R-01 — Estado minimizado no reflejado por `Window.visibility`

**Severidad residual: baja. Estado: mitigado y PASS para transiciones probadas.** Al minimizar la ventana desde KWin, el compositor informó `minimized=true`, mientras QML mantenía `visible=true` y `visibility=Windowed` (`2`). La primera corrección que sólo comprobaba `Window.Minimized` no restauró la ventana; se reprodujo y se corrigió antes de cerrar la prueba. El respaldo actual usa `root.active`: si la ventana está visible pero inactiva, la bandeja intenta mostrarla con `showNormal()`, `raise()` y `requestActivate()`.

Una ventana visible pero desenfocada se trata también como candidata a mostrar. Por ello, «Mostrar ventana» puede aparecer en el menú mientras la ventana está mapeada detrás de otra. El retest sintético de bandeja pasó visible activa, oculta, minimizada, cerrada y tres ciclos. **NOT_TESTED:** clic físico del usuario cuando otra aplicación conserva foco, y comportamiento de una ventana maximizada y totalmente tapada. Si esa ambigüedad afecta el uso diario, la corrección futura debe obtener el estado minimizado real desde una fuente Qt/KWin fiable; no inferirlo sólo de `visible`. El archivo afectado es `qml/Main.qml`, bloque `TrayMenu`; `qml/components/TrayMenu.qml` consume el estado compartido. No se observó denegación de foco por KWin en los casos medidos.

## W1R-02 — CPU visible en reposo sigue alrededor del 11 %

**Severidad: media para autonomía/temperatura en este equipo. Estado: pendiente de diagnóstico adicional.** Dos muestras de 5 s del proceso real posterior dieron 10,79 % y 11,19 % de un núcleo (W1: 11,0 % y 10,6 %). Oculto dio 2,60 % y 2,40 % (W1: 2,2 %). La CPU durante streaming **sí** bajó en el fixture comparable de 41,25/67,41/95,96 % a 15,44/16,34/15,40 % en tres turnos; no volvió a crecer con el historial corto.

Se investigaron el pulso del indicador de conexión en `qml/Main.qml` y `hotkeyTimer` de 300 ms. Copias privadas con cada uno desactivado dieron 10,34 % y 10,75 % frente a 11,53 % de referencia. Al ser muestras únicas y próximas, no prueban causalidad ni justifican quitar una operación de supervisión. El refresco del tray cada 15 s y otros timers/animaciones tampoco se atribuyeron sin evidencia. Próximo paso acotado para F5: traza de scenegraph/render de reposo con varias ventanas idénticas y repeticiones suficientes; comprobar si la carga viene del render, composición o actividad de QML. No introducir un cambio ciego en el polling de red o del turno.

**F2:** no se identificó bloqueo funcional por este consumo. **Uso cotidiano:** podría elevar temperatura y gasto energético mientras la ventana permanece abierta; no se midió batería ni sesión prolongada.

## W1R-03 — Evidencia visual y duración limitadas

**Estado: NOT_TESTED, no fallo afirmado.** La prueba automatizada de 80 mensajes verificó que 20 cambios del último mensaje no desplazan al lector situado al principio; no midió fluidez perceptible de scroll ni accesibilidad con un usuario. El primer token del harness se detectó con sondeo de 20 ms, no con cámara ni medida de fotón en pantalla. No se tomó una nueva captura: las evidencias son KWin, DBusMenu, Qt journal, profiler y pruebas QML. El usuario aún debe observar clic físico en bandeja, texto, foco y scroll durante uso real. Tema claro, atajos globales y varias horas de actividad permanecen sin ensayar para no cambiar preferencias KDE ni invadir atajos.

RSS/PSS se midieron después de varios turnos y ciclos; no apareció crecimiento evidente en esa ventana corta, pero eso no certifica ausencia de fugas a largo plazo. El PSS de la app real (~123 MiB) no se compara directamente con los ~98 MiB de W1 como si fueran páginas privadas: depende del reparto de librerías Qt con otros procesos. Se requiere una serie prolongada bajo condiciones controladas para atribuir cambios de memoria.

## W1R-04 — Voz diferida y fases futuras

**Estado: corregido para la configuración ensayada.** `capabilities.voice=deferred` oculta/deshabilita el micrófono del InputBar y deja «Dictar» oculto/deshabilitado en DBusMenu; el popup propio omite esa fila. `GetLayout` aún contiene la entrada nativa con `visible=false` y `enabled=false`, que es la forma de exportación de Qt/Plasma. No se ejercieron micrófono, Whisper, Piper, ONNX, wake word, avatar ni atajos globales. Sus pruebas e implementación pertenecen a F3/F4 y no se cuentan como regresión F1B.

## Resultado para decisiones

No hay incidencia reproducida de orden de tokens, UTF‑8, identidad de turno/sesión, cancelación, error explícito, persistencia o seguridad que bloquee técnicamente F2. W1R permanece **PARTIAL** como aceptación de escritorio por W1R-01 en el estado visible desenfocado, W1R-02 y la observación humana pendiente. La autorización de F2 es independiente de este informe; no se inició F2, F3 ni F4.
