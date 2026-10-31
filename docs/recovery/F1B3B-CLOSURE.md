# F1B-3B — Cierre de consolidación local

Fecha de cierre: 2026-10-13. **CONSOLIDATED**. Recomendación: aceptar la consolidación local de F1B-3B y realizar aceptación real de Plasma Wayland antes de dar por certificada la experiencia de escritorio. F2/F3 no iniciados.

## Baseline, commits y responsabilidad

Rama `recovery/f1b3b-history-retention`. Baseline obligatorio **d8a6972b5bc3977408c8d0956fce3196623d5458**, árbol `5f724bb939fc0e95d4fe197b2e519b8f5e02b6fd`, padre `690c5d65b48a2fb19564c842a12677b129822a70`. La rama anterior `recovery/f1b3a-persistence` sigue en el baseline.

| Commit | SHA | Árbol validado exacto | Responsabilidad |
| --- | --- | --- | --- |
| A | `b07240f936b0abde098c42d1c32a0189955026b2` | `ccd12e739b70f1fc850b11c8157b06913c8521cc` | 19 archivos: Rust de producción/contexto/retención/caché, SQL 004, QML y adaptaciones mínimas heredadas. |
| B | `e00fc1f77322b8fd1dace4c9c4858327a141f82a` | `2ee7dd18e778b10a8344aa42bef7eb61f11fd82b` | 7 archivos: suite/fixtures F1B3B, runner y los tres informes. |

A tiene como único padre el baseline; B tiene como único padre A. Commits normales locales, autor Codex <codex@openai.com>, misma identidad de agente de los commits precedentes y usada con opciones locales de comando; no se modificó configuración Git global. Sin amend, reset, clean, stash destructivo, push, merge ni release.

El usuario autorizó explícitamente las adaptaciones F1B1/F1B3A en A: espera del último owner DB y marcador 003 comprobado por versión. Los unitarios embebidos de voz y render se adaptan al selector/ownership nuevo sin quitar assertions; no hay hooks nuevos exclusivos de prueba. Todas las pruebas nuevas están en B. Manifest completo por archivo en [F1B3B-COMMIT-VALIDATION.md](F1B3B-COMMIT-VALIDATION.md).

## Defectos reproducidos y corregidos

1. Lease identificado únicamente por pathname: sustituir un workspace por otro con lease activo permitía publicar y borrar su recurso. Test `replaced_workspace_cannot_publish_or_delete_another_live_resource`: FAIL antes (publish=Ok, cleanup=Ok), PASS después. Ahora se fijan y comparan descriptores/device/inode de raíz, directorio y marker; la limpieza reclama un nombre nuevo y valida antes del borrado.
2. Escritura de descarga por pathname: symlinks plantados durante la respuesta loopback cambiaban los bytes del target privado de fixture a PNG. Test `image_download_does_not_follow_planted_output_symlinks`: FAIL antes, PASS después. Descargas/SVGs se crean mediante openat2 CREATE|EXCL; PDF entrega stdout a un descriptor creado igual. Una página PDF sintética fue renderizada por pdftoppm local y verificada como PNG.

Las evidencias antes/después se conservan en `/tmp/kda-f1b3b-consolidation-f24ll7kk`. Un import de fixture incorrecto, un import tokio::fs ya sin uso y dos fixtures unitarias sin lease se corrigieron durante validación; no se ocultan como IGNORED ni se confunden con los dos defectos reales. No quedan FAIL finales.

## Gates y garantías verificadas

Fuentes finales: `python3 scripts/validate_f1b3b.py --qml`, 13 gates PASS en `/tmp/kda-f1b3b-check-4js24vaa`, 269 Rust + 6 QML. Árbol A archivado: 11 gates disponibles PASS, 236 Rust + 4 QML. Árbol B archivado: 13 gates PASS, 269 Rust + 6 QML en `/tmp/kda-f1b3b-check-jipsj57o`. Sólo se excluyeron los dos unitarios externos heredados; mount namespace F1A permanece IGNORED. Workers controlados y Qt opt-in se distinguen en la validación. No se suman tests hijo ni ejecuciones repetidas como cobertura distinta.

Historial: unidades de input completas, pares tool/result validados, orden por ID, outputs Failed/Cancelled rechazados, incertidumbre esencial, system preservado, límites de esquemas/input/respuesta y validación antes de cada request ReAct. JSON legacy inválido de 4 MiB se rechaza por tamaño antes de parsear. No UUID inventados, reparaciones silenciosas, resúmenes sustitutos ni replay.

Eliminación: igualdad parametrizada exacta; Registry reservado/supervisor vivo, turnos persistidos y Prepared/Authorized/Dispatched bloquean borrado. UnknownOutcome exige consentimiento adicional. Dos operaciones inciertas y dependientes se eliminan en una transacción dejando cero huérfanos; otra sesión conserva cinco mensajes. RAISE(ABORT) intermedio restaura filas/eventos; integrity_check=ok y foreign_key_check vacío. Facts/recordatorios globales no se atribuyen a una sesión. Producción nunca desactiva constraints; el seed legacy sólo los desactiva deliberadamente en su conexión sintética y afirma la corrupción antes de probar producción.

Locks inspeccionados: Registry -> Turn -> SQLite, y Registry -> SQLite en borrado; no se encontró SQLite -> Registry ni await bajo el callback de borrado. Manual/voz que preparan un turno revalidan sesión transaccionalmente; late inserts no recrean contenido eliminado. Scheduler sólo usa SQLite de recordatorios globales y no accede a Registry.

Caché: ownership/UID/marker, descriptores sin symlinks/hardlinks/submontajes, leases vivos/referencias/gracia, reservas de generaciones simultáneas dentro del backend y límites de admisión. Los errores de recurso no publicado intentan recogerlo; fallos de limpieza conservan o intentan restaurar ownership. No se borran /tmp legacy, HOME ni cachés ajenas.

QML: dos harnesses nuevos y cuatro heredados PASS offscreen. Incertidumbre honesta, Failed con efectos parciales, API vacía/error, filas antiguas, callbacks obsoletos, aviso de sesión, 409 inicial, conservación antes del consentimiento y borrado explícito posterior. Sin retry/replay. Audit API redacta args al responder; no modifica auditorías antiguas en disco.

## Parche y estado Git final

[F1B3B-CHANGES.patch](F1B3B-CHANGES.patch): **193463 bytes**, SHA256 `b44a3910eefe2d8b15ddafffa7852faf20cd79bd1ddcb9aa9793d23fe7033ad8`. Cubre exclusivamente los 26 archivos del diff baseline -> HEAD. `git apply --check` PASS en un git archive limpio del baseline. Tras aplicar, SHA256 de los 236 archivos coincide con archive B; un índice privado reconstruye exactamente `2ee7dd18e778b10a8344aa42bef7eb61f11fd82b`. No se alteró el índice de trabajo para esa reconstrucción.

Tracked y staged limpios. Los 20 documentos/parches históricos que ya estaban sin versionar permanecen intactos, verificados por SHA256. Estos dos recibos de cierre/validación y el parche quedan como tres entregables nuevos **sin versionar**: se generaron después de los commits para registrar sus hashes reales y evitar autorreferencia del propio parche. No forman parte del diff consolidado. No se añadieron documentos históricos al índice.

## Riesgos y aceptación pendiente

Conteo conservador por bytes, no tokenizer exacto ni descubrimiento de ventana del modelo; deben configurarse sus límites reales. La UI legacy sigue cargando mensajes completos. Caché no es una cuota física frente a escritores arbitrarios del mismo UID; reservas pertenecen al backend, no a un servicio global de cuotas de varios perfiles/procesos. Renderers pueden consumir espacio transitorio antes de validar salida. Leases no certifican QML huérfano fuera del lifecycle cooperante ni una defensa general contra control hostil del mismo UID sobre el namespace.

Borrar filas no destruye inmediatamente WAL, backups o bloques ni revierte efectos externos. Voz real/supervisión completa del pipeline de audio, proveedores/tokenizadores remotos, Plasma Wayland/tray real, mount namespace y fallos físicos: NOT_TESTED. No exactamente una ejecución externa ni permisos definitivos F2.

Antes de aceptación de escritorio: comprobar en una sesión Plasma real, con perfil de fixture, tray/selección/cancelación/recuperación y confirmación de borrado; conservar la distinción con offscreen. Después, dirección técnica podrá autorizar F2 como fase separada. Esta consolidación no la inicia ni extiende F3.
