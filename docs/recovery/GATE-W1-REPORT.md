# GATE-W1 — Aceptación real en Plasma Wayland

**Estado: PARTIAL.** Ejecutado el 13 de octubre de 2026 en `America/Santo_Domingo` (14 de octubre UTC). Rama `recovery/f1b3b-history-retention`, HEAD `e00fc1f77322b8fd1dace4c9c4858327a141f82a`. No se cambió código de producción ni se hizo commit, push, merge o release.

## Dictamen

KDE Assistant funciona como chat escrito en esta sesión Plasma Wayland: el binario real inició, Qt mapeó la ventana, el InputBar aceptó eventos de teclado Qt, dos turnos confirmados se mostraron en streaming con UTF-8 correcto, la cancelación produjo estados terminales correctos, el cambio de sesión descartó eventos antiguos y SQLite conservó sólo las respuestas confirmadas. El reinicio recuperó historial y operaciones inciertas sin repetir herramientas. El proveedor fue un servidor HTTP sintético en `127.0.0.1:8877`; no se probó ningún servicio comercial.

La aceptación global queda **PARTIAL** por dos fallos de experiencia reproducidos: un clic en bandeja sobre una ventana minimizada la oculta y exige otro clic para restaurarla; la UI consumió cerca del 100 % de un núcleo durante streaming de 50 tokens a unos 20 tokens/s. El tema claro, los atajos globales y el uso humano prolongado siguen sin probarse. Estos puntos no se atribuyen a una regresión de persistencia o cancelación F1B.

| Área | Estado | Evidencia principal |
| --- | --- | --- |
| Backend/chat | PASS | Health 200, mock loopback, turnos QML con streaming y error 503 explícito. |
| UI Wayland | PARTIAL | Capturas reales de ventana y aviso de recuperación; entrada QtTest correcta; fallo al restaurar desde minimización. |
| Persistencia | PASS | Reinicio, historial, migración de `Dispatched` a `UnknownOutcome`, SQLite íntegra. |
| Cancelación | PASS | Antes del primer token y durante respuesta; `Cancelando` seguido de `Cancelado`. |
| Seguridad | PASS en el perfil ensayado | XDG privado, herramientas y voz desactivadas, red de proveedor sólo loopback, ninguna acción externa. |
| Rendimiento | PARTIAL | Arranque medido; CPU de UI elevada en reposo y streaming. Sin umbral inventado ni conclusión de optimización. |
| KDE/tray/atajos | PARTIAL | StatusNotifier y DBusMenu presentes, ocultar/restaurar y cierre a bandeja; minimización defectuosa; atajos no ejercidos. |
| F3/F4 y voz | NOT_TESTED | Voz diferida deliberadamente; micrófono, modelos, wake word, overlay y descargas fuera del gate. |

No apareció un fallo de corrección del núcleo de texto que bloquee técnicamente F2. La decisión de iniciar F2 puede usar esta evidencia; el defecto de bandeja debe corregirse antes de considerar satisfactoria la integración de escritorio, y la carga de CPU merece análisis antes del uso cotidiano prolongado. El tema claro, la interacción humana con los controles y las funciones F3/F4 requieren otra aceptación. No se inició ninguna de esas fases.

## Método y aislamiento

Se leyeron `AGENTS.md`, `KDE-ASSISTANT-V2-PLAN.md`, `KDE-ASSISTANT-V2-PROMPT.md`, `apple-DESIGN.md`, README, `src/main.rs`, los contratos F1B-2/F1B-3A/F1B-3B y el cierre F1B3B. El comando real descubierto fue `target/debug/kde-assistant --no-shortcuts`, que lanza `qml6 -apptype widget ... qml/Main.qml`. Se usó también `--ui-off --no-shortcuts` para montar ese mismo `Main.qml` en harnesses QML durante pruebas programáticas, sin un segundo backend. Nunca se usó `QT_QPA_PLATFORM=offscreen`; el proceso real registró `QT_QPA_PLATFORM=wayland`.

El perfil `0700` está en `/tmp/kda-gate-w1-jehX1Fa7`. `XDG_CONFIG_HOME`, `XDG_DATA_HOME`, `XDG_CACHE_HOME` y `TMPDIR` apuntaron a sus subdirectorios privados; `WAYLAND_DISPLAY`, `DISPLAY`, `XDG_SESSION_TYPE` y el bus DBus de sesión se heredaron. Se quitaron claves API y proxies del entorno de los procesos lanzados. La primera configuración fue creada por la aplicación con clave vacía, `enable_tool_calling=false`, todas las herramientas de acción desactivadas, `wake_word_enabled=false`, `auto_speak=false`, `auto_listen=false`, `chimes_enabled=false`, memoria y personaje desactivados. Luego se configuró **sólo ese perfil** con `ai.provider=custom`, URL `http://127.0.0.1:8877/v1`, modelo `gate-w1-fixture` y credencial de fixture local. No se activó `--enable-voice`.

Antes del arranque, `127.0.0.1:8765` estaba libre. El equipo reportó 4 CPUs, 7,6 GiB RAM (3,4 GiB disponibles al inicio), Intel UHD Graphics 605, escritorio 1366×768 con escala 1, Qt/QML 6.12.0, Plasma/KWin 6.7.5, Rust/Cargo 1.99.0. `XDG_SESSION_TYPE=wayland`, `WAYLAND_DISPLAY=wayland-0`, Plasma y KWin Wayland activos. El log del primer arranque avisó que faltaba el modelo Whisper y siguió hasta servir chat; `/api/config` informó voz `deferred`, micrófono no inicializado y descargas de arranque desactivadas.

Las pruebas de UI fueron automatizadas **sobre el QML real y el compositor real**: funciones públicas de `Main.qml`, eventos de teclado QtTest, propiedades de `RecoveryHistory.qml`, KWin para geometría/estado, y capturas de Spectacle de la ventana de la aplicación. No equivalen a que una persona haya probado visualmente cada control. **Observación humana del usuario: ninguna.** Las capturas revisadas por el agente muestran el estado inicial de error por falta de clave, el chat oscuro y el aviso de operaciones inciertas; no se obtuvo una captura fiable del diálogo de recuperación abierto. Su contenido y transiciones se verificaron mediante propiedades y señales QML.

## Evidencia y cierre

El perfil conserva `app.log`, `timed-start.log`, `mock-requests.jsonl`, `evidence-journal.log`, métricas JSON, harnesses y las capturas `window.png`, `active-window.png`, `recovery-notice.png`. Los resultados verificables por caso están en [GATE-W1-RESULTS.md](GATE-W1-RESULTS.md); defectos y correcciones mínimas propuestas en [GATE-W1-ISSUES.md](GATE-W1-ISSUES.md). El directorio temporal puede desaparecer según la política del sistema; las mediciones y los hechos decisivos quedan transcritos en esos informes.

Se envió SIGINT únicamente a los PID propios. Los padres esperaron a sus hijos QML, no quedó ningún hijo propio persistente, ambos puertos quedaron libres y todos los scripts temporales de KWin se descargaron. No se cerró ningún QML ajeno, se borró ninguna sesión personal ni se modificó configuración KDE del usuario. La base final privada contiene dos sesiones sintéticas, nueve mensajes y una operación incierta sintética conservada para inspección; `integrity_check=ok`, `foreign_key_check` sin filas, WAL y `synchronous=FULL`. Los archivos de documentación sin seguimiento que ya existían en Git se dejaron intactos.
