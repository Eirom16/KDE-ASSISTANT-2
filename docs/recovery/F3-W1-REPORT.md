# GATE-F3-W1 — Aceptación humana del escritorio

## Dictamen

**ACCEPTED para el alcance humano H1–H7 ensayado.** Se completaron los siete
escenarios con interacción física del usuario y evidencia complementaria sobre
la ventana/procesos propios. No se identificó un defecto bloqueante en ellos.
La consolidación técnica F3 previa permanece CONSOLIDATED; no se repitió su
auditoría ni se cambió implementación.

GlobalShortcuts: **NOT_TESTED_BY_USER_CHOICE**. No se abrió ningún diálogo del
portal, no se registraron atajos y no se certifica una tecla física. Blur y pin
superior de esta implementación Wayland permanecen NOT_SUPPORTED. Estas reservas
no se convierten en funcionalidades aceptadas mediante este gate.

## Baseline y preparación

- Rama: `recovery/f3-kde-wayland-integration`.
- HEAD inicial/final: `1d2186e5dae78ae9ddc1dbed01c44c9b7d67f920`.
- Un solo worktree; índice y archivos rastreados sin cambios.
- Leídos F3-CLOSURE, F3-RESULTS, F3-ISSUES y F3-KDE-CONTRACT.
- Sesión KDE Plasma Wayland; backend Rust y QML originales del repositorio.
- Perfil XDG_CONFIG_HOME, XDG_DATA_HOME, XDG_CACHE_HOME y TMPDIR privados;
  SQLite nueva y únicamente read.txt/edit.txt sintéticos.
- Proveedor mock ThreadingHTTPServer en 127.0.0.1, sin credenciales comerciales.
  Los valores fixture-local/f3-w1-synthetic son identificadores de prueba.
- Voz/modelos/avatar desactivados; siempre `--no-shortcuts --http-port 0`.
- CWD de ejecución privado distinto del repositorio. El bootstrap inicial
  `--ui-off` se terminó y recogió antes del arranque con UI.

No se modificaron SettingsDialog, proveedores, Rust, QML ni configuración KDE
personal. No hubo root, inyección de teclas, control de aplicaciones ajenas,
notificaciones de prueba ni accesos a documentos reales. Los scripts KWin de
observación filtraron PID y título propios y se descargaron individualmente;
no se usaron para accionar ventanas. H3 dejó la elección y activación de la otra
aplicación al usuario, sin inspeccionar su contenido.

## Resultados humanos y observaciones literales

| Escenario | Resultado | Observación literal del usuario |
|---|---|---|
| H1 — Bandeja oculta/muestra | HUMAN_VERIFIED: PASS | «Funciona» |
| H2 — Minimizar/restaurar con una activación | HUMAN_VERIFIED: PASS para restauración y activación de ventana | «OK, el campo de texto no recibe el foco a menos que lo haya dejado asi antes de minimizar la app.» |
| H3 — Activación desde otra aplicación | HUMAN_VERIFIED: PASS | «Ok, esta vez si funciono» |
| H4 — Conservar maximizado | HUMAN_VERIFIED: PASS | «si, si vuelve maximizado» |
| H5 — X a bandeja y Salir | HUMAN_VERIFIED: PASS | «Si, la x solo lo oculto y salir cerro la app» |
| H6 — Nueva sesión con Yellow pendiente, repetición válida | HUMAN_VERIFIED: PASS | «Ahora si, funciono perfectamente. Se abrio una nueva sesion y se quito el aviso» |
| H7 — Popup Red visible/legible y Denegar | HUMAN_VERIFIED: PASS | «funciono» |

Las confirmaciones breves se interpretan dentro del escenario que se acababa de
solicitar; no se inventan citas más detalladas. H2 distingue foco KWin de ventana
y foco interno del editor: no se certifica autofocus del campo. H4 cuenta con
confirmación de maximizado y geometría coherente; no hubo inspección automatizada
de clipping ni informe humano explícito adicional sobre cada borde.

Un escenario humano se preparó por turno y se esperó la respuesta antes del
siguiente. H6 se repitió a petición del usuario: «Cometi un error en la prueba,
iniciala nuevamente». El primer intento queda INVALIDATED_BY_USER, sin PASS/FAIL
de aceptación. Su monitor registró prepared→denied, sin despacho; posteriormente
SQLite ya no contenía esas filas. No se infieren los gestos exactos ni su borrado.
Se preservaron logs, monitor y backup SQLite en H6-attempt-01. No se cuentan
intentos ni clicks repetidos como escenarios nuevos.

## Garantías observadas

- H1: KWin deja de encontrar la superficie propia al ocultar y la encuentra
  activa/no minimizada al presentar; backend y Qt mantienen sus identidades.
- H2: precondición KWin minimizada; primera presentación restaura y activa.
- H3: usuario confirma precondición/gesto desde otra app; Qt estaba visible e
  inactiva al presentar y KWin la encuentra activa después. Qt Windowed previo,
  por sí solo, no certifica ausencia de minimización en KWin.
- H4: Qt Maximized antes de ocultar, restoreMaximized al presentar y KWin
  1366×728 en (0,0), activa/no minimizada. Sin imponer keepAbove.
- H5: X oculta, bandeja recupera; Salir produce Qt código 0 y apagado del backend.
- H6 válido: pending→revoked, prepared→abandoned; ningún Authorized/Dispatched;
  aprobación tardía HTTP 409 después de revocar, SQLite/FK íntegras, dos sesiones,
  hashes sintéticos conservados y marcador de lectura ausente de mensajes.
- H7: Red pending→denied, prepared→denied; no despacho ni cambio del archivo.
  Durante pending, QML mostró popup_visible/popup_focus y KWin ventana activa.
  La muestra se verificó contra pending antes y después; no se usa una muestra
  posterior a expiración como evidencia del foco durante la solicitud.

No se autoaprobó ningún consentimiento vivo. El único POST aprobado=true fue
la regresión de aprobación tardía H6, después de comprobar revoked/abandoned;
fue rechazado. H7 nunca ejecutó una aprobación. El incidente Red histórico
sigue sin causa explicada; el éxito de esta muestra no lo explica retroactivamente.
Las cuatro herramientas F2 y las 19 restringidas conservan el contrato consolidado;
este gate no cambia flags productivos ni habilita adaptadores.

## Cierre y límites

Se detuvo únicamente el supervisor de pruebas identificado por ruta exacta y
pidfd; éste terminó/recogió sus handles propios. Al cierre están ausentes
supervisor, backend y Qt, sin zombies propios pendientes. Ambos listeners cerrados,
puerto backend reocupable, flock libre y nombres DBus propios liberados. SQLite
final integrity_check=ok, sin violaciones FK, permisos activos ni eventos de
despacho. Evidencia conservada; no se borra el lock ni el módulo privado de token.
Los 44 archivos históricos del manifiesto inicial conservan SHA-256 exactos.

Se corrigió sólo el harness externo: recogida Popen.wait tras salida voluntaria.
Una primera sonda final de puerto ocurrió antes de terminar el mock; se repitió
tras desaparecer el supervisor y pasó. No fueron cambios ni fallos demostrados
de producción. Detalle en F3-W1-ISSUES.

Evidencia privada: `/var/tmp/kda-f3-w1-4g519o2v/`.
Sólo los tres informes GATE-F3-W1 se añaden sin seguimiento al repositorio.
Sin commits, push, merge, release, F4 ni F5. F3.5 no se aborda.
No se ensayaron HiDPI/múltiples pantallas, X11, overlay legado ni rendimiento
controlado en este gate. No se extrapola aceptación a esos ámbitos.

**Recomendación: aceptar GATE-F3-W1 en el alcance ensayado, conservando
NOT_TESTED_BY_USER_CHOICE para atajos físicos y los límites documentados.**
