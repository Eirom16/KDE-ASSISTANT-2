# GATE-W1 — Resultados y evidencia

**Leyenda:** PASS = comprobado en este entorno; FAIL = comportamiento reproducible contrario al esperado; NOT_TESTED = no se ejerció; NOT_SUPPORTED = fuera de la configuración de esta ejecución. `QML` indica automatización de los componentes reales sobre Wayland, `API` el backend real con proveedor de fixture, `KWin` la sesión gráfica, `captura` una imagen real revisada por el agente. No hubo observación humana del usuario.

## Entorno y arranque

| Comprobación | Estado | Método y resultado |
| --- | --- | --- |
| Rama/HEAD y árbol previo | PASS | `recovery/f1b3b-history-retention`, `e00fc1f...`; sólo documentación previa sin seguimiento. |
| Sesión gráfica | PASS | `wayland`, `wayland-0`, KDE; `kwin_wayland` y `plasmashell` activos. |
| Dependencias | PASS | `qml6` 6.12.0, Qt 6.12.0, Plasma/KWin 6.7.5; UI real cargó sin error QML fatal. |
| Puerto 8765 al inicio | PASS | Libre; después escuchó sólo en `127.0.0.1:8765`. No se terminó instancia ajena. |
| Arranque del binario/QML | PASS | `target/debug/kde-assistant --no-shortcuts`; hijo `qml6 ... qml/Main.qml` en Wayland nativo. |
| Ventana visible | PASS | KWin: título `KDE Assistant v2`, geometría 575×708 en (395,10), 1366×768 escala 1; capturas `window.png` y `active-window.png`. |
| Backend health | PASS | `GET /api/health` → 200 `{"app":"kde-assistant","status":"ok"}`. |
| Primera configuración restrictiva | PASS | Clave vacía, tool calling false, herramientas de acción false, voz y personaje false; captura con error visible de falta de clave. |
| Voz opcional | PASS | Sin `--enable-voice`; `/api/config` devolvió voz `deferred`, micrófono no inicializado y sin descarga de arranque; ausencia de Whisper sólo produjo warning. |
| Cierre y recogida | PASS | SIGINT a padres propios; hijo QML recogido, puerto 8765 y mock 8877 libres. |

## Chat y recuperación

| Caso | Estado | Evidencia |
| --- | --- | --- |
| Crear sesión desde QML | PASS | `GATE_CHAT QML_SESSION_CREATED`, UUID backend nuevo. |
| Dos mensajes escritos | PASS | `TURN_1_PASS`, `TURN_2_PASS`; 2 respuestas `assistant` confirmadas en SQLite. |
| Streaming incremental | PASS | Harness observó `¡Hola` mientras `streaming=true`, antes de `😀`; mock emitió deltas separados. |
| Tildes, ñ y emoji | PASS | `¡Hola, señor! ñandú 😀` exacto en QML y SQLite; input `áéíóú ñ 😀`. |
| Cancelar antes del primer token | PASS | Mock retrasó 3 s; QML mostró `Cancelando` y después `Cancelado`; sin assistant persistido. |
| Cancelar durante respuesta | PASS | Tras `Inicio`, QML mostró ambos estados; sin assistant completado en SQLite. |
| Cambiar de sesión durante respuesta | PASS | `GATE_SWITCH` en curso, nueva sesión vacía; tras 3,6 s no apareció token viejo. |
| Fallo del proveedor | PASS | Mock 503; banner `Error del asistente`, detalle `provider_http...503`, bubble `⚠`; ningún assistant falso. |
| Entrada y Enter | PASS | QtTest inyectó teclas en `Main.qml` sobre Wayland; `GATE_KEYBOARD` llegó al proveedor y obtuvo respuesta. No sustituye teclado físico humano. |
| Reinicio/historial | PASS | Nueva instancia leyó siete mensajes de la primera sesión; dos assistant conservados y cinco inputs, incluidos los fallidos/cancelados. |
| Recuperación de crash sintético | PASS | 51 filas sembradas como `Dispatched`; al reiniciar: 51 `UnknownOutcome`, turno `Failed`, snapshot en `turn_recovery`, cero replay. |
| Aviso y explicación en UI | PASS | `recoveryCount=1` como indicador de presencia; `RecoveryHistory` mostró 50 filas y texto exacto «La operación pudo haberse realizado, pero no se pudo confirmar su resultado.» Captura real del aviso `recovery-notice.png`. |
| Paginación de operaciones | PASS | Página de 50 con `hasOlder=true`; cursor `audit_id` devolvió la operación 51 sola. |
| API vacía/no disponible | PASS | `RecoveryHistory.status=empty` para sesión sin operaciones; URL loopback sin servidor produjo `unavailable`, no ausencia falsa. |
| Confirmación/cancelación de borrado | PASS | Primer DELETE → 409, QML abrió explicación; cerrar confirmación conservó siete mensajes. |
| Borrado explícito y aislamiento | PASS | Señal del botón `eraseEvidenceButton.clicked()` causó segundo DELETE; sesión objetivo y su journal se retiraron, la otra conservó su mensaje. SQLite íntegra. |
| Apariencia visual del diálogo abierto | NOT_TESTED | Transiciones y texto comprobados por QML; no hay captura fiable ni observación humana del diálogo. |

El harness inicial de recuperación esperaba erróneamente `recoveryCount=51`; esa propiedad de `Main.qml` sólo indica presencia y valió 1. Se corrigió el harness temporal y se repitió completo con exit 0. La primera corrida de teclado comparaba índices absolutos en una sesión que ya tenía historial; la repetición comparó los dos últimos mensajes y pasó. Esos errores de fixture no se atribuyen a producción ni se ocultan como PASS inicial.

## KDE y presentación

| Característica | Estado | Observación |
| --- | --- | --- |
| Posición/escala KWin | PASS | Ventana mapeada centrada, sin quedar fuera del monitor; escala 1. |
| Foco/teclado | PASS | KWin `active=true`; QtTest escribió y Enter envió. Entrega por teclado físico humano: NOT_TESTED. |
| Tema oscuro | PASS | `dark=true` en log, captura con canvas y componentes oscuros legibles. |
| Tema claro | NOT_TESTED | No se cambió el tema KDE personal. |
| Panel y bandeja | PASS | StatusNotifier registrado, `Menu=/MenuBar`; `GetLayout` devolvió Mostrar/Ocultar, Nueva sesión, Configuración y Salir. Click primario DBus ocultó/restauró. |
| Minimizar y restaurar con primer clic de bandeja | FAIL | KWin `minimized=true`; primer `Activate` dejó cero ventanas mapeadas; segundo `Activate` restauró. |
| Cerrar y restaurar | PASS | `closeWindow()` de KWin ocultó sólo esta ventana; backend/QML siguieron vivos; `Activate` la restauró. |
| Error visible | PASS | Captura de primera ejecución muestra aviso de clave ausente; mock 503 expuso error en propiedades QML. |
| Atajos globales | NOT_TESTED | Deshabilitados con `--no-shortcuts` para no invadir preferencias del usuario. |
| Voz, avatar y modelos F3/F4 | NOT_SUPPORTED | Deliberadamente deshabilitados en este perfil; no son requisitos de chat escrito GATE-W1. |

## Rendimiento observado

Hardware: cuatro CPUs, Intel UHD Graphics 605, 7,6 GiB RAM; ~3,4 GiB disponibles antes de iniciar. Medidas de `target/debug` y `qml6` con perfil aislado, tema oscuro, 1366×768 escala 1. CPU expresa porcentaje **de un núcleo** mediante deltas `/proc/<pid>/stat`; RSS/PSS de `smaps_rollup`. Son muestras, no benchmarks estables ni umbrales de aceptación.

| Condición | Backend | QML |
| --- | --- | --- |
| Arranque cronometrado, perfil ya creado | Health a 0,026 s desde `Popen`; KWin registró ventana a ~1,187 s | Única muestra de arranque; proceso/QML cerrados con código 0. |
| Visible en reposo, 5 s | 0,0 % CPU; RSS 25.652 KiB, PSS 20.824 KiB | 11,0 % CPU; RSS 185.128 KiB, PSS 98.081 KiB. |
| Visible en reposo, segunda muestra 5 s | 0,0 % CPU | 10,6 % CPU. |
| Oculta desde bandeja, 5 s | 0,0 % CPU | 2,2 % CPU. |
| Streaming, 50 deltas/2,5 s, muestra 1 de 1,7 s | 2,35 % CPU; RSS 28.548 KiB, PSS 23.651 KiB | 102,93 % CPU; RSS 187.564 KiB, PSS 99.548 KiB. |
| Streaming, muestra 2 de 1,7 s | 1,76 % CPU; RSS 28.604 KiB, PSS 23.707 KiB | 96,46 % CPU; RSS 187.308 KiB, PSS 99.537 KiB. |
| Tras varios turnos, proceso real final | RSS 25.612 KiB, PSS 20.763 KiB | RSS 185.756 KiB, PSS 98.157 KiB. |

Al cierre, XDG privado ocupó ~12 KiB config, ~180 KiB data SQLite y ~3,4 MiB caché (incluye shader cache). El directorio total con logs/harnesses/capturas fue ~3,9 MiB. No quedó hijo propio persistente. Los cambios de memoria entre procesos no prueban ausencia de fuga; no se midió una sesión prolongada.

## Límites de evidencia

No hubo prueba de proveedor comercial, red externa, tool calling, acciones sobre el sistema, micrófono, wake word, TTS, tema claro, atajos globales ni aceptación humana. La inyección de 51 operaciones fue **sólo SQLite sintético del perfil privado** con backend detenido; no se ejecutó `read_file` ni otra herramienta. La captura DBus directa `ScreenShot2` respondió `NoAuthorized`; Spectacle sí capturó la ventana real. Los resultados automatizados y las capturas no certifican cada detalle perceptual para una persona.
