# F3 — Incidencias y límites

## Defectos reales corregidos

- Atajos rdev no nativos Wayland y puente de archivo con polling/loss de eventos.
- CWD elegía recursos; el resolver existente no estaba conectado al launcher.
- Activación de ventana inactiva podía perder maximizado; showNormal era no-op
  en el estado real KWin minimized/Qt Windowed de esta versión. Se recupera con
  KWindowSystem público y remapeo Qt diferido.
- El rescate a los 1500 ms podía volver a mostrar una ventana ocultada por el
  usuario durante el arranque. Se conserva sólo si aún no hubo interacción.
- SIGKILL del controlador podía dejar la UI viva; se añade parent-death signal.
- Duplicado no identificaba/activaba instancia; perfiles competían por puerto fijo.
- Tema DBus bloqueante repetido y notificación interna con firma incorrecta.
- Blur declaraba éxito sólo escribiendo un log.
- Harness qmltestrunner con QGuiApplication abortaba al crear menú QWidget KDE.
- Código legacy podía ejecutar pkill por nombre (inaccesible bajo F2): retirado.
- Fallback QML a variables de entorno no soportadas: retirado; AuthToken privado
  es la única fuente del bearer para Main, bandeja, ajustes y auditoría.

## Restricciones de compositor / verificación humana

- Estado de entrega PARTIAL, código READY_FOR_REVIEW. No hay aceptación humana
  F3 ni autorización para registrar un atajo real; no se declara F3 completa.
- El hint Qt de pin superior no produjo keepAbove en KWin. Control deshabilitado
  en Wayland y función NOT_SUPPORTED; no se editan reglas del usuario. Blur
  tampoco está implementado; no se anuncia éxito ficticio.
- Una llamada sintética SNI/DBus no lleva necesariamente token de entrada. El
  remapeo inactivo también preserva maximizado; la corrida final consiguió foco,
  pero el gesto real sigue NOT_TESTED. El harness sincroniza precondiciones Qt
  y registra BLOCKED si KWin rechaza el intento, sin forzar ese resultado.

- GlobalShortcuts disponible v2, pero registro/colisiones/gesto real requieren
  consentimiento del usuario. El usuario eligió explícitamente continuar sin
  registrar el atajo; no se abre el diálogo en esta entrega.
  La prueba sintética no certifica una tecla física Wayland.
- KWin decide posición/foco. Qt requestActivate puede ser rechazado; no se
  introduce force stealing ni dependencia de APIs privadas. El activation_token
  opcional del portal se transmite por SSE autenticado al adaptador público
  KWindowSystem; no se usan mecanismos privados.
- El caso Red no visto en W1H-05 sigue sin causa probada. Los nuevos diagnósticos
  sólo demuestran el estado en el instante registrado.
- Pruebas con una pantalla no certifican HiDPI real, múltiples salidas, cambios
  de escala en vivo ni todos los setups Plasma/X11. No se confunde offscreen
  con Wayland.

## Deuda técnica

- No hay restart automático de Qt: relanzado explícito recupera journal y revoca.
- El escritorio sin bus puede ejecutar backend/Qt, pero no ofrece las mismas
  capacidades de activación/tray/portal.
- El runtime conserva componentes de voz legacy diferidos. No se certifica su
  ownership completo ni se inicia F4.
- Los 19 adaptadores restringidos siguen con distintos déficits de identidad,
  límites o consentimiento. Su presencia no permite despacho productivo.
- Caché del módulo autenticado es privada por perfil; no es un secreto exportado
  por DBus ni se promete sandbox entre aplicaciones con el mismo UID.
- SSE por XMLHttpRequest mantiene responseText durante la conexión, igual que
  los canales existentes; el nuevo stream tiene baja frecuencia y cola limitada.
  No se afirma memoria constante para una conexión con infinitas activaciones.
- Reposo QML, avatar y rediseño visual corresponden a F5; muestras F3 no fijan
  presupuestos universales.
- Muestra visible Qt: 12.27% de un núcleo, oculta 1.99%, backend 0% en intervalos
  de tres segundos. No hay comparación controlada equivalente con baseline;
  no se certifica ausencia de regresión cuantitativa ni se abre F5 aquí.
- Se conservó el polling heredado de supervisión (200 ms; headless 100 ms),
  sin añadir polling de escritorio/teclado; una espera por señal de proceso
  podría reducirlo en una mejora posterior con su gate propio.

## Futuro fuera de alcance

F4 voz/PTT; F5 avatar/idle/visual; F6 instalación/desktop ID empaquetado.
Reactivar multimedia, volumen, brillo, KDE Connect, launch/close de terceros,
notificaciones del modelo o herramientas de red requiere contrato F2 individual,
identidad efectiva del recurso y pruebas completas. Ningún registro DBus o flag
legacy basta para conceder esos permisos.


## Hallazgos de auditoría final corregidos

| ID | Reproducción anterior | Corrección / regresión |
|---|---|---|
| AUD-01 | Activated antes de respuesta Bind se perdía | Suscripción antes de Create/Bind; portal_audit_early_activation_is_not_lost |
| AUD-02 | Closed antes de respuesta Bind se ignoraba y anunciaba éxito | Selección prioritaria Closed durante handshake; portal_audit_early_session_close_is_not_success |
| AUD-03 | Handle sintácticamente válido de otra sesión aceptado | Igualdad exacta con handle calculado de conexión/token; portal_audit_session_handle_is_owned |
| AUD-04 | Binding con toggle y una acción inesperada aceptado | Resultado exacto de una acción; portal_audit_bound_actions_are_exact |
| AUD-05 | Drop del owner dejaba listener vivo y entregando toggles | Drop aborta, estado cerrado; main recoge resultado de carrera abort/completion; portal_audit_discarded_owner_stops_listener |
| AUD-06 | Servicio desaparecía y estado seguía registrado | Señal owner-changed invalida, sin auto-rebind; portal_audit_service_loss_is_not_registered |
| HAR-01 | TMPDIR largo superaba sun_path y el bus de tests no iniciaba | Socket abstracto Linux con UUID; pruebas aisladas en ruta larga |
| HAR-02 | Regresión F1B2 reabría SQLite antes de terminar Drop; Weak vacío no implica flock libre | Fixture espera adquisición efectiva del flock antes de construir otro Backend; no cambia SessionManager ni su exclusión |

Ninguno autoriza herramientas ni cambia el dictamen humano PARTIAL. La prueba
HTTP añadida cubre bearer, Host/Origin, método no permitido, overflow y reconexión
sin replay. El ensayo multihilo caracteriza el límite del kernel; no se convierte
el PPID en una garantía de identidad de hilo. Incidente Red histórico sin causa
confirmada; diagnóstico y consentimiento siguen sin autoaprobación.
