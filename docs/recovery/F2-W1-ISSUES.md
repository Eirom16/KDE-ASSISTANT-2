# GATE-F2-W1 — Incidencias y límites

## F2-W1-01 — `qmltestrunner` aborta con bandeja y tema KDE

**Severidad: infraestructura de test; no bloquea la aplicación auténtica.** En Wayland, `qmltestrunner` con tema KDE aborta (`SIGABRT`) en `QMenu::QMenu`, llamado desde `KStatusNotifierItem` y `KDEPlasmaPlatformTheme6.so` al materializar `SystemTrayIcon visible: true`. GDB confirmó el fatal `QWidget: Cannot create a QWidget without QApplication`. Reproducer mínimo sin bandeja: 3/3 PASS; con bandeja visible: exit -6; con tema genérico: F2 4/4 PASS. `qml6 -apptype widget` real ejecutó el mismo `Main.qml` y mostró consentimientos. El runner no constituye prueba KDE positiva; mantener el gate visual en proceso `qml6` con `QApplication` o dotar al runner de ese tipo de aplicación. No se cambiaron paquetes, tema ni código productivo.

## F2-W1-02 — Riesgo y rechazo visibles con lenguaje técnico

**Severidad: UX media; sin evasión de permisos.** En capturas reales, `Riesgo: yellow` y `Riesgo: red` son cadenas inglesas minúsculas sin señal cromática o explicación diferenciada. El botón Aprobar de Red no destaca que la operación altera un archivo. La solicitud de herramienta deshabilitada llega como `malformed_tool: herramienta no ofrecida en este turno`, claro para diagnóstico pero técnico para una persona. Las acciones, destino, bytes/digest y botones sí son legibles; no se demostró confusión humana. Corregir sólo tras revisión de lenguaje/diseño; este gate no introduce cambios F3 ni altera la política.

## F2-W1-03 — Interacción humana y estados de UI aún sin ensayar

**Estado: NOT_TESTED, no fallo afirmado.** El harness invocó `approveTool` en QML; los tests genéricos cerraron el diálogo y separaron sesión. No hubo clic/teclado físico, Tab/Escape bajo KDE, cambio de sesión desde la ventana auténtica, diálogo mientras llegan tokens, backend sin respuesta ni observación prolongada de foco/ventanas duplicadas. KWin registró una sola ventana propia activa en muestras, no todos los estados posibles. Falta observación humana antes de etiquetar la interfaz `HUMAN_VERIFIED` o cerrar el gate como ACCEPTED.

## F2-W1-04 — Rendimiento y recuperación física limitados

**Estado: evidencia insuficiente, no regresión demostrada.** Dos muestras de ~0,8 s durante espera dieron QML ~25 % y backend 0 % de un núcleo; incluyen render del popup y no son comparables directamente con el ~11 % de reposo visible de W1R. Memoria se midió en una sesión corta y no permite descartar crecimiento prolongado. El perfil real tuvo 0 `UnknownOutcome`; el recovery y fallo SQLite pasaron en fixtures sintéticos F2/F1B-3A. No se simuló fallo físico de filesystem ni se desactivó polling/supervisión. El consumo visible en reposo sigue asignado a F5.

## Cierre

No se reprodujo bypass de permisos. El backend merece PASS en el alcance ensayado; UI y aceptación humana permanecen PARTIAL/NOT_TESTED. Mantener restringidas las otras 19 herramientas y no tomar este gate como aprobación final de F3 hasta completar las comprobaciones humanas y de interacción pendientes.
