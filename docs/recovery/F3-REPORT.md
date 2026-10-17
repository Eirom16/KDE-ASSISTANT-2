# F3 — Integración KDE/Wayland

Baseline comprobado antes de cambios: `recovery/f2-permission-engine`,
`8b21c7fd505c8c6b5d0e9bcb5c7ecbd7455c9835`. Un worktree, índice vacío,
sin modificaciones rastreadas. Ancestry GATE-W1R comprobada. Rama nueva:
`recovery/f3-kde-wayland-integration`. Sin commits, push, merge ni release.
Informes históricos sin seguimiento preservados; manifiesto SHA-256 privado
en `/var/tmp/kda-f3-historical-sha256.json`.

## F3-0: auditoría previa a implementación

| Área | Estado encontrado / mecanismo | Riesgo y dependencia | Recuperación prevista |
|---|---|---|---|
| Instancia | Bind exclusivo 127.0.0.1:8765 previo a bootstrap | No identifica aplicación ni activa ventana; perfiles compiten por puerto | Lock de perfil y activación DBus sin secretos; puerto explícito para pruebas |
| Hijo QML | OwnedChild, kill/wait sólo del Child propio; sondeo 200 ms | Error de UI corta backend; SIGKILL del padre puede dejar hijo; no drenaje explícito | Ownership por Child, parent-death signal Linux y drenaje acotado sin replay |
| Tray | Qt.labs.platform SystemTrayIcon, QApplication vía qml6 -apptype widget; DBusMenu visible:false | Funciona, conservar; runner QGuiApplication aborta con KDE | Harness QApplication propio; conservar producción |
| Ventana | visible/active/Minimized; showNormal sobre cualquier inactiva | Corrige KWin minimized/Qt Windowed, pero puede perder maximizado | Controlador común que conserve maximizado, pruebas propias KWin |
| Consentimiento | Dialog modal QML; cierre/sesión deniega; F2 timeout 120 s | Episodio Red no visible sin diagnóstico durante pending | Diagnóstico local limitado a geometría/estado/tiempo; jamás autoaprobar |
| Atajos | rdev X11 thread sin stop, dbus-send sin listener funcional + hotkey.state cada 300 ms | No nativo Wayland; escucha global y pérdida de eventos | Portal GlobalShortcuts opt-in; canal SSE acotado y sin comandos entrantes |
| Overlay | Personaje opt-in, qml6 separado layer-shell | Fuera de F3/F5; no necesario para chat rápido | Ventana Qt normal; no ampliar avatar |
| Recursos | Resolver app_paths existe pero main usa -I . y qml/Main.qml | Dependencia CWD; override puede caer silenciosamente a otra raíz | Conectar resolver, override estricto y rutas absolutas |
| KDE interno | dbus-send síncrono repetido para tema; Notify con firma defectuosa | Sin timeout; false-success de blur (sólo log) | Cliente DBus Rust con deadline; blur NOT_SUPPORTED explícito |
| Adaptadores legacy | tool_executor tiene kdotool/xdotool, pgrep/pkill por nombre, procesos y red | Peligrosos, algunos subprocess sin ownership/timeout | Mantener los 19 bloqueados por F2; retirar kill por nombre incluso de legacy |
| Autoridad | Cuatro tools activas, plan → dispatch_permission → journal → executor | No permitir rutas UI/DBus como dispatcher LLM | DBus sólo Activate/Toggle de ventana propia, sin strings de comandos |
| Rendimiento | ListModel estable W1R; polling tray 15 s y hotkey 300 ms; voice/agent SSE | Reposo QML ya tiene deuda F5 | Eliminar polling de hotkeys y recents; medir sin rediseñar |

Entorno observado: Plasma 6.8.0, Qt 6.12.0; GlobalShortcuts publica versión 2.
KGlobalAccel público evaluado: añade dependencia KF y persistencia de atajos
personales. Se prefiere portal por consentimiento/ownership de sesión.

## Resultado de entrega — 2026-10-17

**PARTIAL — READY_FOR_REVIEW del código; F3 no está aceptada/completa.**
El usuario eligió continuar sin registrar el atajo y dejar pendiente la prueba
humana. No se abrió el diálogo del portal ni se modificaron sus asignaciones.
La aceptación de foco mediante gesto real de bandeja también sigue pendiente;
una activación DBus sintética no sustituye un token de entrada Wayland.

| Etapa organizativa | Resultado | Evidencia |
|---|---|---|
| F3-A infraestructura KDE | IMPLEMENTED | Lock por perfil, activación de duplicado, tray/DBusMenu, parent-death, reaping, salida controlada; UNIT y procesos reales |
| F3-B ventanas y atajos | PARTIAL | Mostrar/ocultar/minimizar/restaurar/maximizado y ventana normal probados en KWin; portal implementado y probado sintéticamente; registro/tecla/foco humanos NOT_TESTED |
| F3-C adaptadores y robustez | IMPLEMENTED dentro del alcance restringido | DBus Rust de tema/notificación, recursos absolutos, matriz de 23 tools, regresiones y Plasma real; sin reactivar herramientas |

Los apartados funcionales F3-D/E/F de la solicitud están incluidos en estas
tres etapas organizativas. HiDPI físico, múltiples pantallas y X11 no se
certificaron. Blur y pin superior de esta integración Wayland son NOT_SUPPORTED.
La UI deshabilita el pin en Wayland en vez de anunciar un estado que KWin ignora.

## Arquitectura final y código recuperado

- Rust/Tokio/Axum conserva autoridad, SQLite, journal, SSE y cancelación. F2
  sigue admitiendo sólo system_info Green, read_file/create_file Yellow y
  edit_file Red. Las 19 restantes no se anuncian ni despachan por el agente.
- flock sobre archivo privado persistente identifica la instancia por perfil;
  DBus Desktop1 sólo transporta Activate/Toggle sin argumentos. No contiene bearer ni
  prompts. Perfiles privados usan puerto efímero y módulo QML autenticado propio.
- Se conserva la implementación funcional Qt SystemTrayIcon/DBusMenu. Un
  controlador común presenta la ventana propia, conserva maximizado y remapea
  asíncronamente superficies inactivas para cubrir la discrepancia Qt/KWin.
  KWindowSystem público opcional solicita foco; KWin mantiene la decisión final.
- GlobalShortcuts sustituye la escucha global rdev: sesión/conexión propias,
  autorización del portal, consulta previa de bindings, deadlines, cola acotada,
  activación por SSE autenticado y cierre explícito. Está apagado por defecto;
  --no-shortcuts prevalece. Sin registro real durante esta entrega.
- OwnedChild administra únicamente handles de hijos creados por la app.
  PR_SET_PDEATHSIG evita Qt huérfano tras SIGKILL del padre. Error Qt cierra el
  backend; relanzado explícito recupera persistencia sin replay. No hay restart
  automático de operaciones ni cambios en UnknownOutcome.
- Notificaciones internas y lectura de tema usan DBus asíncrono con timeout.
  Notify no tiene caller del modelo. Los adaptadores legacy permanecen aislados
  por F2; se retiró incluso de fixtures el cierre por pkill/nombre ambiguo.
- Recursos se resuelven por override absoluto validado, instalación o raíz
  compilada, sin CWD. Launcher privado y arranque desde otro directorio pasan.
- Harness Qt usa QApplication; corrige el fallo QWidget de qmltestrunner KDE
  manteniendo la bandeja productiva habilitada.

## APIs y dependencias

zbus 5.19 con Tokio (sin runtime async-io) añade cliente/servicio DBus; Cargo.lock
registra sus dependencias nuevas sin actualizar dependencias previas. Su MSRV
1.87 es compatible con Rust 1.99 fijado por el repositorio; no se afirma soporte
Rust 1.75. No se añade backend privilegiado, Python productivo ni WebEngine.

APIs: org.freedesktop.DBus, StatusNotifierItem/DBusMenu de Qt,
org.freedesktop.portal.GlobalShortcuts/Request/Session/Settings,
org.freedesktop.Notifications, org.kde.kwindowsystem opcional, flock y prctl Linux.
Los scripts KWin son sólo del harness, temporales, con nombre único y filtro
de PID/caption de hijos propios; se comprueba su descarga. No se instalan en KDE.
El harness requiere Qt6Widgets/Qt6QuickTest/pkg-config/c++; CMake se obtuvo en un
directorio privado porque faltaba en el entorno, sin paquetes globales.

## Archivos modificados y nuevos de F3

- Dependencias: Cargo.toml, Cargo.lock.
- Infraestructura: src/desktop.rs (nuevo), src/owned_process.rs, src/main.rs,
  src/lib.rs, src/app_paths.rs, src/models/config.rs.
- Backend: src/backend/{mod,auth,conversation_turn,http_server,kde_integration,
  hotkey_listener,tool_executor}.rs; global_shortcuts.rs nuevo.
- UI: qml/Main.qml, qml/auth/AuthToken.qml,
  qml/components/{TrayMenu,SettingsDialog,AuditDialog,AgentDashboard}.qml,
  qml/components/qmldir; DesktopWindowController.qml y KdeWindowActivation.qml
  nuevos. qml/agent/AgentOverlay.qml sólo recibe backendUrl resuelto.
- Pruebas: tests/app_paths.rs, tests/f1b1_text_startup.rs,
  tests/qml/tst_gate_w1r.qml; tests/f3_desktop.rs,
  tests/qml/tst_f3_desktop.qml y tests/qt/quicktest_main.cpp nuevos.
  La auditoría agrega tests/f3_process_owner.rs y ajusta el teardown del fixture
  existente tests/f1b2_conversation.rs.
- Runners: scripts/validate_f2.py, scripts/validate_gate_w1r.py;
  scripts/{validate_f3,f3_runtime,qml_test_harness}.py nuevos.
- Documentación: los cinco F3-*.md de esta entrega. Los informes históricos no
  versionados no se incorporan al índice ni se reescriben.

## Pendientes y revisión

Pruebas, conteos, muestras y limitaciones: F3-RESULTS.md. Contrato y garantías:
F3-KDE-CONTRACT.md. Estado individual de cada adaptador: F3-ADAPTER-MATRIX.md.
Defectos/reservas/deuda: F3-ISSUES.md. No se inicia F4, F5 ni F6.
La rama queda con cambios sin commit, a la espera de dirección técnica.


## Auditoría final y consolidación local autorizada

La revisión adversarial del diff contra el baseline reprodujo y corrigió seis
fallos del protocolo/lifetime de GlobalShortcuts: activación adelantada perdida,
cierre temprano tomado como éxito, handle de sesión sin binding a la conexión,
acciones de binding adicionales aceptadas, listener separado de su owner al
hacer Drop y estado registrado tras desaparecer el servicio. No se identificó
un bypass del motor F2 en estos casos: son defectos de integración/ownership.
Las reproducciones FAIL anteriores a corrección están en
`/var/tmp/kda-f3-audit-577vs_tr/reproduce-*.log` y se agregaron regresiones.

También se corrigió el socket del bus sintético para TMPDIR largos: dirección
abstracta Linux aleatoria, sin service directories ni autostart. No se cambió
un servicio de la sesión personal. Se añadieron pruebas de lock simultáneo,
cancelación de conexión dedicada, HTTP/SSE autenticado y parent-death por hilo.
No se modificaron temporizadores de supervisión ni el ListModel/streaming.

Dependencias auditadas contra Cargo.lock baseline: cero paquetes previos
eliminados o con versión cambiada. Los registros de tracing, Tokio y uuid sólo
cambian las listas de dependencias por unificación de features; kde-assistant
agrega zbus. ctrlc habilita termination para SIGTERM/SIGHUP sin cambiar versión.
Se conserva rdev únicamente como tipo/parser legacy, sin listener global.

La división local es infraestructura/producción y pruebas/documentación. El
primer commit incluye tres ajustes mínimos de fixtures existentes:
tests/app_paths.rs y tests/f1b1_text_startup.rs validan recursos/argv sin CWD;
tests/f1b2_conversation.rs espera la liberación real del flock tras teardown,
en lugar de confundir Weak::upgrade() == None con fin del destructor. Las suites nuevas, harnesses, runners y cinco informes
van en el segundo. Los manifiestos exactos, árboles, hashes y validación de cada
archivo staged se registran en F3-COMMIT-VALIDATION.md después de consolidar;
F3-CLOSURE.md contiene el dictamen técnico y la aceptación humana separada.
Los 41 históricos se excluyen de ambos commits y se preservan por SHA-256.
