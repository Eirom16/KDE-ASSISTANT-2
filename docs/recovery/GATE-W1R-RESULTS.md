# GATE-W1R — Resultados medidos

**Leyenda:** PASS comprobado; PARTIAL comprobación incompleta; NOT_TESTED sin ensayo; NOT_SUPPORTED fuera de la configuración segura. La automatización de QML sobre Wayland, la API, el estado KWin y los tests offscreen se identifican por separado. No hubo observación humana del usuario.

## Entorno, aislamiento y retest KWin

| Caso | Estado | Evidencia |
| --- | --- | --- |
| Baseline Git | PASS | HEAD `e00fc1f77322b8fd1dace4c9c4858327a141f82a`, nueva rama `recovery/gate-w1r-ux-performance`; informes W1 originales y otros archivos sin seguimiento conservados. |
| Wayland real | PASS | `XDG_SESSION_TYPE=wayland`, `WAYLAND_DISPLAY=wayland-0`, KWin/Plasma 6.7.5, Qt/QML 6.12.0; app `target/debug/kde-assistant --no-shortcuts` e hijo `qml6` nativo. |
| Aislamiento | PASS | Raíz privada `/tmp/kda-gate-w1r-7pmmhybe` modo 0700; XDG y TMPDIR propios; mock sólo `127.0.0.1:8877`; backend sólo `127.0.0.1:8765`. Sin claves comerciales, atajos, audio ni herramientas de acción. |
| Previo: minimizada → primer Activate | FAIL reproducido | KWin `minimized=true`; Qt `visible=true`, `visibility=2` (`Windowed`); primer clic ocultó y segundo restauró. PID QML inicial 23392; journal de KWin y trazas en perfil privado. |
| Corregido: visible → bandeja → oculta | PASS | PID QML 30240, KWin pasó de una ventana activa a cero ventanas listadas; menú cambió de «Ocultar» a «Mostrar». |
| Oculta → bandeja → visible | PASS | Una ventana mapeada y `active=true` en un clic. |
| Minimizada → bandeja → visible | PASS | KWin `minimized=true`, `active=false`; menú «Mostrar»; un `Activate` dejó `minimized=false`, `active=true` con el mismo `internalId`. |
| Cierre → bandeja → visible | PASS | `closeWindow()` sólo al PID propio dejó cero ventanas; un `Activate` restauró y activó. |
| Ciclos y foco | PASS | Tres pares adicionales ocultar/mostrar terminaron siempre con una ventana `active=true`; scripts KWin filtrados por PID y título. |
| Menú de voz diferida | PASS | `/api/config` indicó `capabilities.voice=deferred`; DBusMenu conservó el item «Dictar» con `enabled=false`, `visible=false`; popup propio omite la fila y el micrófono de InputBar está oculto/deshabilitado. No se ejerció el micrófono. |
| Otras ventanas QML | PASS en el alcance automático | Los scripts KWin sólo actuaron sobre el PID y título propios, se descargaron tras cada uso; no se envió señal ni comando a procesos ajenos. |
| Cierre controlado | PASS | SIGINT sólo a padres propios, hijos QML recogidos, puertos libres; logs sin error fatal QML. |

El botón físico del panel, la política de foco al interactuar desde otra aplicación y la lectura humana del menú son **NOT_TESTED**. `StatusNotifierItem.Activate` por DBus y KWin scripting verifican el camino real de Plasma/KWin y la lógica QML, pero no sustituyen esos gestos humanos. No se hizo captura nueva: se usaron estados KWin, journal de QML y layout DBusMenu; ninguna otra ventana fue capturada.

## Perfilado de la causa (con overhead, no benchmark)

`/usr/lib/qt6/bin/qmlprofiler` registró el mismo fixture de 50 deltas en una sesión nueva antes y después. Trazas privadas: `before-profile.qtd`, `profileafter-profile.qtd`. Duraciones inclusivas; las llamadas anidadas se solapan.

| Evento | Antes | Después |
| --- | ---: | ---: |
| Construcciones `MessageBubble` | 105 | 2 |
| Reevaluaciones del binding `ChatView.model` anterior | 56 | No aplica: modelo estable |
| `refreshUI` | 53; ~603 ms inclusivos | 53; ~35,7 ms inclusivos |
| `syncMessages` del modelo estable | No aplica | 57; ~30,4 ms inclusivos |
| Construcciones `Octicon` desde `MessageBubble` | 105 | 2 |
| Eventos `Creating` de la traza completa | 23.739 | 5.922 |
| Eventos `Binding` de la traza completa | 34.540 | 11.287 |
| Eventos `Javascript` de la traza completa | 41.998 | 14.643 |

El profiler identifica repetición de delegates por reasignar el array completo; su propia instrumentación aumenta los tiempos. El resultado de CPU siguiente se midió **sin profiler**.

## CPU, memoria y latencia sin profiler

Fixture: `target/debug`, tema oscuro, 1366×768 escala 1, Intel UHD Graphics 605, cuatro CPU, Qt 6.12.0. Mock emitió exactamente 50 deltas `x` cada 50 ms. Cada columna de streaming corresponde a un turno consecutivo de la misma sesión en perfil XDG fresco para ese lado. CPU es porcentaje **de un núcleo** a partir de `/proc/<pid>/stat`; RSS/PSS salen de `smaps_rollup`. El harness sondeó el primer token cada 20 ms. Se tomó CPU entre ~0,15 s tras comenzar y ~0,10 s antes del final del turno para reducir bordes; duración de cada muestra ~2,27–2,38 s. No se extrapola a otros equipos.

| Métrica | Antes: turnos 1 / 2 / 3 | Después: turnos 1 / 2 / 3 |
| --- | --- | --- |
| CPU QML (% de un núcleo) | 41,25 / 67,41 / 95,96 | 15,44 / 16,34 / 15,40 |
| CPU Rust (% de un núcleo) | 2,10 / 1,69 / 1,68 | 1,76 / 1,77 / 2,20 |
| Primer token desde envío QML (ms, sondeo 20 ms) | 95 / 79 / 94 | 50 / 31 / 33 |
| RSS QML al final (KiB) | 188.200 / 190.548 / 192.008 | 187.712 / 187.844 / 188.252 |
| PSS QML al final (KiB) | 98.657 / 101.005 / 102.465 | 101.401 / 101.533 / 101.941 |
| RSS Rust al final (KiB) | 27.184 / 27.692 / 27.836 | 27.212 / 27.716 / 27.872 |
| PSS Rust al final (KiB) | 22.488 / 22.996 / 23.140 | 22.493 / 22.997 / 23.153 |
| Mensaje final | 50/50 caracteres cada turno | 50/50 caracteres cada turno |

El PSS comparte bibliotecas Qt con otros procesos y varía entre ejecuciones; no se atribuye la diferencia inicial a una fuga ni a un ahorro. Tras los tres turnos del harness posterior el PSS pasó de ~101,1 MiB en reposo inicial a ~101,9 MiB. El perfil XDG `after` ocupó ~8 KiB de config, ~140 KiB de SQLite/data y ~2,1 MiB de caché; los archivos de trazas y harnesses están fuera de esos subdirectorios.

| Reposo | Antes | Después |
| --- | --- | --- |
| QML visible, proceso real, 5 s | W1: 11,0 % y 10,6 %; W1R harness tras turnos: 11,62 % | W1R proceso real: 10,79 % y 11,19 % |
| QML oculto, proceso real, 5 s | W1: 2,2 % (una muestra) | 2,60 % y 2,40 % |
| Rust visible/oculto | W1: 0,0 % | 0,0 % en muestras de 1 s |
| RSS/PSS QML visible del proceso real | W1: ~185 MiB / ~98 MiB | ~187 MiB / ~123 MiB en esta ejecución |

La ablación privada del pulso online dio 10,34 % frente a 11,53 % de referencia; desactivar sólo el temporizador de hotkeys dio 10,75 %. Esas muestras únicas no demuestran una mejora estable. Las rutas de red y supervisión del turno siguieron operativas.

## Regresiones funcionales

| Prueba | Resultado |
| --- | --- |
| `python3 scripts/validate_gate_w1r.py` | PASS: 11 casos QML offscreen aislados. Modelo estable durante 50 cambios UTF‑8, cancelar/terminar, cambiar sesión, proveedor fallido, historial de texto, inserción/eliminación y recarga de igual longitud, badge de aprobación, 120 filas/20 actualizaciones, scroll del lector, cambios rápidos de bandeja y voz diferida. No certifica KWin. |
| `python3 scripts/validate_f1b3b.py --qml` | PASS final, código 0: fmt, check, clippy, seguridad F1A, F1B1, conversación F1B2, persistencia F1B3A, almacenamiento F1B3B, unitarios seguros, rutas y QML offscreen F1B1/F1B2/F1B3B. Evidencia `/tmp/kda-f1b3b-check-znap0ia1`. Una corrida anterior tuvo todos los comandos PASS pero falló su manifest porque se añadió el test W1R durante la ejecución; se repitió después con fuentes estables. |
| Conversación QML en Wayland con mock local | PASS: dos turnos confirmados, streaming parcial y `¡Hola, señor! ñandú 😀` exacto; `Cancelando` → `Cancelado` antes y durante tokens; cambio de sesión sin token antiguo; proveedor 503 con banner y sin assistant falso. Journal PID 30720. |
| SQLite privado del retest de conversación | PASS: `integrity_check=ok`, dos sesiones, dos respuestas assistant confirmadas y seis mensajes user; ninguna respuesta assistant publicada para turnos cancelados o fallidos. |
| Scroll | PASS lógico: con 80 filas y lector al principio, 20 actualizaciones del último mensaje no lo llevaron al final. Fluidez visual humana: NOT_TESTED. |
| Fugas prolongadas, tema claro, atajos globales, voz real y aceptación humana | NOT_TESTED/NOT_SUPPORTED según configuración; no se activaron capacidades diferidas. |

Los logs de evidencia y trazas quedan en el perfil privado indicado; no contienen sesiones personales. Las mediciones de una sola máquina son referencias locales y no fijan límites universales.
