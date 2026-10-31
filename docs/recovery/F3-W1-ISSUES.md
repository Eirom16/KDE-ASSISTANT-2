# GATE-F3-W1 — Incidencias y límites

## Defectos de producción

**Sin defecto bloqueante demostrado en H1–H7.** Gate ACCEPTED para su alcance.
No se modifica código durante esta validación.

## Observación de experiencia H2 — Foco del campo

Literal: «OK, el campo de texto no recibe el foco a menos que lo haya dejado
asi antes de minimizar la app.» KWin confirma activación de la ventana al
restaurar. Foco de ventana y activeFocus de un control son distintos. El
contrato F3 no exige autofocus del editor al restaurar; no se clasifica esta
observación como rechazo de activación ni se añade un cambio fuera del gate.
Se conserva para revisión posterior de experiencia; no se instrumentó el
activeFocus del campo ni se atribuye una causa adicional.

## Intento H6-01 invalidado

Literal: «Cometi un error en la prueba, iniciala nuevamente». Intento conservado
en H6-attempt-01, sin aceptación humana. Monitor: prepared→denied, sin despacho;
al revisar, la SQLite no contenía ya esas filas. No se infiere cómo ocurrió.
Backend/Qt anteriores estaban terminados y no había autoridad pendiente.
H6-02 completó posteriormente la revocación y el usuario confirmó el resultado;
no se cuentan dos tests independientes ni un fallo productivo por el intento 01.

El usuario ejecutó H6-02 mientras aún se preparaba el mensaje del turno: un
preflight que esperaba cero pendientes se detuvo al encontrar la nueva solicitud.
No modificó datos ni provocó otro intento. La lectura siguiente confirmó
revoked/abandoned; se conservó ese intento y se pidió sólo confirmación humana.

## Defectos y sincronización del harness externo

- H5: el supervisor Python esperaba señales sin recoger Popen al salir
  voluntariamente el backend. Backend ya terminado quedó transitoriamente Z;
  Qt estaba ausente y el puerto cerrado. Terminar sólo ese supervisor propio
  permitió recogerlo. Se añadió un hilo bloqueado en Popen.wait para los
  siguientes arranques. No se cambió OwnedChild ni producción. Limpieza final
  confirma ausencia de los procesos propios y zombies pendientes.
- Limpieza final: se comprobó el puerto mock inmediatamente después de salir
  el backend, antes de completar shutdown del supervisor. Esa primera sonda
  falló. Se esperó su salida y se verificó de nuevo: supervisor ausente,
  listeners cerrados y recursos liberados. No se enviaron señales adicionales
  ni se alteró la aplicación para satisfacer la comprobación.

## Restricciones del compositor y capacidades no ensayadas

- GlobalShortcuts: **NOT_TESTED_BY_USER_CHOICE**, siempre --no-shortcuts.
  Ningún diálogo ni registro; tecla real sin certificar. No es FAIL del gate.
- Blur y pin superior F3 Wayland: **NOT_SUPPORTED** según contrato consolidado.
  No se editan reglas personales ni se ensaya un workaround.
- El compositor conserva la política de foco. No hay force stealing, XTest,
  captura del teclado, APIs privadas de producción ni control de ventanas ajenas.
- HiDPI, múltiples salidas, X11, overlay legado, reinicio Qt en mitad del
  consentimiento y rendimiento cuantitativo: NOT_TESTED en este gate humano.
  Se conservan las evidencias técnicas previas sin atribuirles aceptación humana.
- Las muestras anteriores de CPU no se reinterpretan ni se abre una tarea F5.

## Popup Red histórico

El incidente F2-W1H de popup no visto sigue **sin causa explicada**. En H7 se
confirmó el escenario físico y se recogieron metadatos QML/KWin mientras Red
seguía pending. Después se denegó sin despacho ni modificación. Este éxito no
explica el incidente histórico ni certifica todos los futuros escenarios de foco.
No se autoaprueba ni se ignora consentimiento por visibilidad.

## Seguridad y trabajo futuro

F2 permanece sin cambios: system_info Green; read_file/create_file Yellow;
edit_file Red; 19 herramientas restringidas. No se reactivan adaptadores ni
se cambian UnknownOutcome o recuperación. La aprobación tardía H6 fue una
prueba sobre autoridad ya revocada y resultó rechazada; nunca una aprobación
viva para facilitar la prueba.

Sin pendientes bloqueantes de este gate. Evidencia privada y los tres informes
se conservan para revisión. No commits/push/merge/release. No F4, F5 ni trabajo
de SettingsDialog/Groq/proveedores de F3.5.
