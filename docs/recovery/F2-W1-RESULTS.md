# GATE-F2-W1 — Resultados por evidencia

**Leyenda:** `LIVE_BACKEND` = binario y SQLite reales bajo perfil privado; `AUTOMATED_QML` = propiedades/señales/harness, sin gesto humano; `AUTOMATED_KWIN` = estado del compositor; `SCREENSHOT_REVIEW` = popup renderizado revisado; `UNIT` = fixture Rust aislado; `NOT_TESTED` = conducta subyacente no ejercida. Un PASS de `UNIT` no se cuenta como prueba visual real.

## Entorno y controles

| Comprobación | Estado | Evidencia |
| --- | --- | --- |
| Git | PASS | Rama y HEAD requeridos; cambios rastreados iniciales/finales: ninguno. Informes históricos sin seguimiento conservados. |
| Wayland auténtico | PASS | DBus KWin PID 834, `wayland-0`, Plasma/KWin 6.7.5, Qt 6.12.0, 1366×768 escala 1. |
| Aislamiento | PASS | Perfil `/tmp/kda-f2-w1-arv7tefj` 0700; XDG y TMPDIR privados; backend 127.0.0.1:8765; mock 127.0.0.1:8877; sin voz/modelos/credenciales comerciales. |
| Tema KDE, aplicación real | PASS | `qml6 -apptype widget` inició sin aborto; popup Yellow/Red renderizado. |
| Tema KDE, runner | FAIL de infraestructura | `SIGABRT` en `QMenu::QMenu` bajo `KDEPlasmaPlatformTheme6.so`; reproducer mínimo con bandeja visible. Tema genérico: F2 QML 4/4. |
| Cuatro herramientas | PASS | Green/Yellow/Red ensayadas con backend real, proveedor mock y datos sintéticos. |
| Catálogo restante | PASS en restricción | Seis nombres forzados desde mock fueron rechazados; `UNIT` F2 cubre las 19 aun con flags legados. |

## Adversariales 1–20

| # | Escenario | Resultado y nivel de evidencia |
| ---: | --- | --- |
| 1 | Denegar Yellow | PASS `LIVE_BACKEND`: `read_file`/`create_file` resultaron `success=false`; sin lectura publicada ni archivo creado. |
| 2 | Aprobar Yellow | PASS `LIVE_BACKEND` y `AUTOMATED_QML`: lectura exacta, creación exacta; popup real visible. |
| 3 | Denegar Red | PASS `LIVE_BACKEND`: original intacto, sin `.bak` de esa operación. |
| 4 | Aprobar Red | PASS `LIVE_BACKEND` y `AUTOMATED_QML`: nuevo texto, `.bak` con contenido anterior; popup Red revisado. |
| 5 | Cerrar diálogo sin responder | PASS `AUTOMATED_QML` genérico (`tst_f2_consent`); cierre con `X`/Escape físicos en KDE `NOT_TESTED`. |
| 6 | Cambiar sesión en confirmación | PASS `AUTOMATED_QML` genérico (`detachChat` descarta); cambio en ventana KDE real `NOT_TESTED`. |
| 7 | Cancelar turno durante espera | PASS `LIVE_BACKEND`: `/api/chat/cancel` produjo terminal `cancelled` y `Abandoned`; gesto de UI real `NOT_TESTED`. |
| 8 | Aprobar después de cancelar | PASS `LIVE_BACKEND`: HTTP 409, ninguna operación despachada. |
| 9 | Doble clic Aprobar | PASS `UNIT` F2, segunda respuesta rechazada; doble clic físico `NOT_TESTED`. |
| 10 | Reenviar aprobación consumida | PASS `LIVE_BACKEND`: HTTP 409. |
| 11 | Consent ID de otra operación | PASS `LIVE_BACKEND`: ID falso HTTP 404; `UNIT` cubre dos consentimientos pendientes. |
| 12 | Turn ID incorrecto | PASS `LIVE_BACKEND`: HTTP 404. |
| 13 | Session ID incorrecto | PASS `LIVE_BACKEND`: HTTP 404. |
| 14 | Argumentos alterados tras aprobar | PASS `UNIT` F2; mutación del mensaje HTTP real `NOT_TESTED`. |
| 15 | Consentimiento expirado | PASS `UNIT` F2; espera real de 120 s `NOT_TESTED`. |
| 16 | Reiniciar con aprobación pendiente | PASS `UNIT` F2; reinicio del backend real con pendiente `NOT_TESTED`. |
| 17 | Fallo SQLite antes de despacho | PASS `UNIT` F2 con trigger sintético; no se inyectó fallo en la SQLite del proceso visual. |
| 18 | Recuperar `UnknownOutcome` sintético | PASS `UNIT` F2/F1B-3A; en la SQLite privada de esta aceptación hubo 0 `UnknownOutcome`. |
| 19 | Cerrar inesperadamente QML propio | PASS `LIVE_BACKEND`/`AUTOMATED_QML`: `SIGKILL` sólo al PID propio 90223 durante `pending`; backend siguió; fila `revoked`; HTTP 409 tardío. |
| 20 | Reactivar QML con backend vivo | PASS `LIVE_BACKEND`/`AUTOMATED_QML`: nuevo QML vivo, fila siguió `revoked`, sin replay. |

La regla central se sostuvo en los escenarios ejecutados: ningún ID falso, permiso cancelado, consumido o revocado despachó la operación. Una prueba física de doble clic o de expiración real queda pendiente.

## QML/KWin y capturas

| Aspecto | Estado | Evidencia/límite |
| --- | --- | --- |
| Apertura y contenido | PASS `AUTOMATED_QML` | `approval_needed` de read/create/edit; herramienta, acción, destino, riesgo y alcance; lectura avisa del proveedor. |
| Visibilidad y tamaño | PASS acotado `AUTOMATED_KWIN` | PID propio 89917: una ventana `(395,10) 575×708`, activa, no minimizada; popup 420 px de ancho y sin recorte en captura. |
| Legibilidad/contraste | PASS acotado `SCREENSHOT_REVIEW` | Popups claros sobre ventana oscura, texto legible, botones distinguibles. Riesgo `yellow`/`red` sin color o traducción: incidencia UX. |
| Aprobación desde QML | PASS `AUTOMATED_QML` | Harness llama `approveTool`; SQLite consumió y despachó. No equivale a clic del botón físico. |
| Teclado/foco/Escape | PARTIAL | Código QML declara `focus: true` y `CloseOnEscape`; interacción real/orden Tab `NOT_TESTED`. |
| Streaming, sesión, backend sin respuesta, ventanas duplicadas | PARTIAL | KWin vio una ventana propia en muestras; cambio de sesión genérico pasó. Casos completos durante diálogo real `NOT_TESTED`. |
| Observación humana | `NOT_TESTED` | Ninguna persona interactuó con el diálogo. |

Capturas sintéticas: [read](F2-W1-EVIDENCE/consent-read.png), [create](F2-W1-EVIDENCE/consent-create.png), [edit](F2-W1-EVIDENCE/consent-edit.png). Sólo recogen el popup propio. No se tomó captura del escritorio entero.

## Regresión exacta

| Gate de `validate_f2.py` | Estado | PASS / FAIL / IGNORED / FILTERED |
| --- | --- | --- |
| `cargo fmt --check` | PASS | exit 0 |
| `cargo check --locked --all-targets` | PASS | exit 0 |
| Clippy default y `legacy-fixtures`, `-D warnings` | PASS / PASS | exit 0 / 0 |
| F1A | PASS | 19 / 0 / 1 / 0 |
| F1B-1 | PASS | 13 / 0 / 1 / 0 |
| F1B-2 | PASS | 20 / 0 / 3 / 0 |
| F1B-3A | PASS | 26 / 0 / 1 / 0 |
| F1B-3B | PASS | 33 / 0 / 3 / 0 |
| F2 Rust | PASS | 17 / 0 / 0 / 0 |
| F2 HTTP | PASS | 1 / 0 / 0 / 160 |
| F2 QML offscreen | PASS | 4 / 0 / 0 / 0 |
| W1R desde F2 | PASS | 11 / 0 / 0 / 0 |
| `validate_f1b3b.py --qml` independiente | PASS | todos sus pasos exit 0 |
| `validate_gate_w1r.py` independiente | PASS | 11 / 0 / 0 / 0 |

Las tres invocaciones principales fueron secuenciales; logs privados en `/tmp/kda-f2-w1-arv7tefj/evidence/regression-*.log`. Los `ignored` son exclusiones heredadas y `filtered` corresponde al filtro del test HTTP.

## SQLite, tiempos y cierre

SQLite final: `integrity_check=ok`, `foreign_key_check=[]`, WAL, `synchronous=FULL`; `permission_requests` 30 `consumed`, 15 `denied`, 5 `revoked`, 0 pendientes/aprobadas; `operation_journal` 30 `Succeeded`, 21 `Denied`, 5 `Abandoned`, 0 `Failed`, 0 `UnknownOutcome`. No se interpreta `0 UnknownOutcome` como prueba de recuperación física: la cubren fixtures separados.

Muestras: solicitud QML→evento de consentimiento 40–42 ms en dos turnos; aprobación QML→marcador `Dispatched` ~4 ms en uno. En una ventana de ~0,8 s mientras esperaba: CPU backend 0,0 % y QML ~25,0 % de un núcleo; RSS/PSS ~28/24 MiB backend, ~196/138 MiB QML. No hay serie suficiente para inferir fugas o regresión perceptible. Tras cerrar sólo procesos propios y descargar el script KWin, los puertos 8765 y 8877 quedaron libres.
