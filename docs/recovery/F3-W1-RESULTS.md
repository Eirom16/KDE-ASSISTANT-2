# GATE-F3-W1 — Resultados y evidencia

Dictamen: **ACCEPTED para H1–H7**. Rama `recovery/f3-kde-wayland-integration`;
HEAD `1d2186e5dae78ae9ddc1dbed01c44c9b7d67f920` sin cambios.
Fecha de pruebas: 2026-10-17; tiempos siguientes en UTC.
Evidencia raíz: `/var/tmp/kda-f3-w1-4g519o2v/` (privada).

## Matriz

| Escenario | HUMAN_VERIFIED | AUTOMATED_QML / AUTOMATED_KWIN y comprobaciones | Evidencia |
|---|---|---|---|
| H1 bandeja | PASS | PASS: superficie ausente al ocultar; activa al mostrar, PIDs conservados | H1-result.json; events.jsonl, última pareja 22:29:38/22:29:42 |
| H2 minimizar/restaurar | PASS para restaurar/activar | PASS: minimizada en precondición; activa/no minimizada 22:31:35 | H2-precondition.json, H2-result.json |
| H3 desenfocada | PASS | PASS para presentación/activación 22:35:01; precondición sin minimizar confirmada por escenario humano | H3-result.json |
| H4 maximizada | PASS | PASS: Qt visibilidad 4 antes de ocultar 22:37:12; restaura con true a 22:37:15; KWin 1366×728, (0,0) | H4-result.json |
| H5 X y Salir | PASS | PASS: oculta y reaparece; Qt salida 0, backend apagado 22:38:58, HTTP cerrado y flock libre | H5-result.json, app-H1-H5.log |
| H6 nueva sesión | PASS, intento 02 | PASS: pending 22:44:11 → revoked/abandoned 22:44:17; sin despacho; replay 409 | H6-result.json, H6-monitor.json |
| H7 Red/Denegar | PASS | PASS: pending 22:47:02 → denied 22:47:16; popup visible/con foco, ventana activa durante pending; sin despacho ni modificación | H7-result.json, H7-monitor.json |
| Limpieza final | No gesto humano adicional | PASS: supervisor/backend/Qt ausentes, listeners y nombres propios liberados, SQLite/FK íntegros | cleanup.json, final-sessions-snapshot.db |
| GlobalShortcuts físicos | NOT_TESTED_BY_USER_CHOICE | Estado disabled; no registro ni solicitud al portal | state*.json; flags de lanzamiento |
| Blur / pin superior F3 Wayland | NOT_SUPPORTED | Según contrato consolidado; no se cambian reglas personales | F3-KDE-CONTRACT |
| HiDPI, múltiples salidas, X11, overlay legado, medición de CPU/RSS | NOT_TESTED | Fuera del gate humano realizado | Límites del alcance |
| UNIT y regresiones técnicas anteriores | NOT_TESTED en este gate | No se repiten: consolidación técnica previa se toma como baseline | F3-CLOSURE / F3-COMMIT-VALIDATION |

No se suman snapshots, repeticiones de clicks, init/cleanup ni intentos invalidados
como tests independientes. Los siete escenarios humanos son H1–H7; no son una
nueva corrida de las suites unitarias ni un reemplazo de ellas. Ninguna prueba
offscreen se usa para este dictamen.

## H6 y H7: seguridad e integridad

- H6 válido: eventos prepared, abandoned; permiso revoked y turno cancelado.
  POST /api/tools/approve con los IDs previos, únicamente después de comprobar
  revoked/abandoned: HTTP 409, turn_not_waiting_for_approval. No despacho.
- H7: eventos prepared, denied; permiso denied. No aprobación tardía ensayada
  para Red y ninguna aprobación real. El archivo edit.txt permanece intacto.
- PRAGMA integrity_check: ok; PRAGMA foreign_key_check: sin filas.
- Al cerrar: cero permisos pending/approved/consumed y cero eventos
  authorized/dispatched/succeeded/unknown_outcome en la SQLite de este gate.
- Marker de read.txt ausente en mensajes; sin contenido entregado por la lectura.
- SHA-256 read.txt: `78ff48c9da52e26e007f5344d97420170b0b8561463c581fe0ebb993402ff87c`.
- SHA-256 edit.txt: `612a2ac1988d046a2884597044f8c81915d5ffeb6a3b345559b970d9c8fd9a83`.
- H7 KWin durante pending: activa, no minimizada, 575×708, keepAbove=false;
  popup QML visible y con foco. popup_z no se interpreta como stacking KWin.
- H6-01 invalidado por el usuario: prepared→denied, sin despacho; snapshot y
  monitor preservados en H6-attempt-01. No es FAIL de aceptación ni PASS H6.

Las observaciones literales completas se conservan en F3-W1-REPORT y en
H1-result.json … H7-result.json. Los monitores son registros automáticos originales;
el campo placeholder user_observation de esos monitores no sustituye los result
posteriores que registran la confirmación humana.

## Ejecución y aislamiento

Comandos de arranque del harness externo:

```bash
python3 /var/tmp/kda-f3-w1-4g519o2v/supervisor.py
```

Primera versión preservada como supervisor-H1-H5.py. El supervisor invoca el
binario original con argv explícitos:

```text
/home/eirom/KDE-ASSISTANT-2/target/debug/kde-assistant --no-shortcuts --http-port 0
```

Sólo el bootstrap inicial añadió --ui-off. Los cuatro directorios XDG/TMPDIR
apuntan a subdirectorios privados de la evidencia. QT_QPA_PLATFORM=wayland,
QT_FORCE_STDERR_LOGGING=1, QT_ASSUME_STDERR_HAS_CONSOLE=1 y
QML_DISABLE_DISK_CACHE=1. Se retiran variables *_API_KEY y proxies; no se exporta
el bearer. AuthToken y server.token permanecen privados, sin publicarlos aquí.
Las llaves de proveedores de la configuración sintética están vacías salvo
fixture-local para el mock loopback. No se inicia descarga de modelos ni audio.

El usuario envió H6/H7 desde el chat real. El mock emitió una sola llamada
read_file/edit_file por escenario; las siguientes respuestas son texto sintético.
El monitor externo se ejecutó como:

```text
python3 /var/tmp/kda-f3-w1-4g519o2v/consent_watcher.py H6
python3 /var/tmp/kda-f3-w1-4g519o2v/consent_watcher.py H7
```

El watcher consulta SQLite sólo durante su escenario, con plazo acotado; no es
polling productivo ni listener de teclado. Las observaciones de bandeja se
activan por logs F3_WINDOW_HIDE/PRESENT; nunca se envía SNI.Activate/Event para
sustituir un click físico. Las sondas KWin temporales son sólo observe sobre PID
y título propios. Estado de proceso se contrasta con start_ticks de /proc.
La aprobación tardía se envía autenticada sin imprimir token y sólo bajo guard
revoked/abandoned. No hay wrapper ni modificación de Main.qml.

PIDs primera sesión: backend 41726, Qt 41734. H6 intento 01: 42982/42991.
H6 intento 02 y H7: 43253/43262. Todos terminados y recogidos. Salida final
backend 0 a las 22:48:16; el mock y supervisor terminan también.

La primera sonda final comprobó el mock antes de que terminara el supervisor y
falló; el control final tras su salida pasó. El primer harness demoraba el wait
del backend ya salido en H5; se recogió y se corrigió en el propio harness.
Detalles conservados, sin presentarlos como regresiones productivas.

## Recursos y preservación final

cleanup.json: backend/Qt/supervisor ausentes; puerto backend cerrado y bind
posible con SO_REUSEADDR; puerto mock cerrado; flock adquirible sin borrar lock;
DBus Desktop1/SNI propios sin owner. Históricos: 44 archivos con SHA-256 inicial
sin cambios. Índice y rastreados limpios; sólo tres nuevos informes del gate.
No commits, push, merge ni release. La evidencia permanece privada y se conserva
para revisión; no se promete borrado seguro de la caché autenticada.
