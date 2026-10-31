# F1A — Revisión del diff

Rama `recovery/f1a-containment`; base/HEAD `12da960ed4722401f1cb42b7a96d9cef912cf90a`. Sin commit/push. Estado de implementación: **IMPLEMENTED** en SEC-01/02; validación global con fallos heredados. Ver [decisiones y riesgos](F1A-REPORT.md) y [resultados exactos](F1A-RESULTS.md).

## Archivos de F1A

| Archivo | Responsabilidad | Aspecto principal de revisión |
| --- | --- | --- |
| `Cargo.toml` | Dependencia `rustix` con feature fs; newline final | Usa interfaz segura de syscalls, sin dependencia descargada nueva; resolución local 1.1.5. |
| `src/backend/mod.rs` | Registro privado de safe_fs | Ningún cambio de inicialización/voz. |
| `src/backend/ai_service.rs` | SEC-01 y proveedor válido | Un snapshot por completion/turno, pasado a cada call_once; contención/unknown provider rechazan antes del HTTP. |
| `src/backend/tool_executor.rs` | SEC-01 y conexión al límite seguro | Un snapshot a los 23 despachos; retirada de validate_path; operaciones en spawn_blocking; bloqueos explícitos F2. |
| `src/backend/safe_fs.rs` (nuevo) | SEC-02 | Root FD, openat2 restringido, creación exclusiva, backups/destinos derivados y publicación con renameat2; sin fallback. |
| `src/models/config.rs` | Contención de ausencia/corrupción | Carga devuelve error; campos de seguridad omitidos restrictivos; proveedor explícito y reconocido. Save/migración persistente no reconstruidos. |
| `tests/f1a_security.rs` (nuevo) | Fixtures privados de regresión | 15 tests: flags, concurrencia, symlinks, legítimos, no efectos y prueba reejecutada con kernel denegado. |
| `docs/recovery/F1A-REPORT.md` (nuevo) | Decisiones/alcance/riesgos | Separa contención de autorización pendiente F2 y aceptación del producto. |
| `docs/recovery/F1A-RESULTS.md` (nuevo) | Evidencia repetible | Fallos antes, resultados finales, filtros y prueba seccomp con fuente. |
| `docs/recovery/F1A-DIFF-REVIEW.md` (nuevo) | Revisión por responsabilidad | Este documento. |

Cinco tests nuevos adicionales están junto a las unidades afectadas: dos en ai_service, dos en config y uno en tool_executor. No se movieron los módulos restantes ni se alteró QML, arquitectura de voz, DB, sidecar Python, auth, servidor HTTP o registry.

`Cargo.lock` local se actualizó al declarar rustix directo, pero sigue ignorado por la regla existente `*.lock` y no versionado. No se cambió .gitignore ni se abrió la tarea de reproducibilidad de F1B. No se afirma build limpio publicado/MSRV con este lock ausente del repositorio.

## Trabajo previo preservado

`git status` también muestra `src/backend/hotkey_listener.rs` y los cuatro documentos F0 sin versionar. Son trabajo previo, **no cambios de esta fase**. SHA-256 del hotkey antes y después: `2622363c9d4323d81822d529edc95a449b75b387a379d831ae9e3b37988f40ab`. No se formateó ese archivo. `src/lib.rs` tampoco se modificó. Sus dos problemas de formato se reportan como heredados.

## Revisión de límites

- Una única entrada pública despacha ToolCall: execute; falla antes del despacho si no puede obtener snapshot. Los métodos que aceptan snapshots son privados. Todos los flags individuales se comprueban con la misma referencia. Record_denial sólo escribe auditoría.
- No quedan canonicalize/validate_path en las rutas activas de estos módulos. Origen, destino derivado y backup pasan por SafePath. Los errores de primitives no se convierten en fs::write/fs::copy por pathname.
- La escritura principal publica un temporal de otro inode y no trunca el objeto que pudo ser reemplazado por un link. NOREPLACE evita perder datos cuando aparece un destino durante creación/copia exclusiva.
- Preview/local show_image reciben snapshots privados; búsquedas resuelven entradas desde dirfds. Las dos delegaciones no representables se deniegan con mensaje explícito F2.
- El flag ai.enable_tool_calling continúa controlando catálogo, no acciones manuales. Consentimiento por operación/origen/argumentos no se reconstruyó; no confundir ToolCtx con prueba de aprobación.
- No se añadieron unwrap en producción de esta implementación. Unwrap/expect de fixtures no son usados por el producto. Check all-targets, Clippy -D warnings y formato de archivos tocados pasan.

## Regresiones posibles / restricciones conscientes

Los principales cambios de compatibilidad son fail-fast durante guardado, configuración inicial/antigua que ahora requiere intervención explícita, bloqueos temporales de open_file/share_file, rechazo de symlinks/hardlinks/montajes/roots inexistentes y publicación de archivos nuevos 0600. No se conserva inode/ACL/xattrs/modo ejecutable. Pérdida de actualizaciones en append concurrente, durabilidad conjunta de backup/principal y limpieza de vistas privadas siguen pendientes. Ver justificación y límites del adversario en REPORT.

La suite existente independiente de hardware/red/modelos no mostró regresiones nuevas. Una primera implementación del atacante de la prueba de directorio falló por DirectoryNotEmpty durante su propio rename de restauración; se sustituyó por EXCHANGE atómico. La prueba corregida y las otras 14 pasan; el fallo intermedio queda registrado en RESULTS.

## Pendientes de aceptación

No integrar esta rama como una certificación del producto completo. El formato global sigue fallando en dos archivos preexistentes y Piper sigue ausente (12 fallos). Revisar las restricciones de compatibilidad, la política de primer arranque, retención de snapshots, consentimiento central y delegación segura en sus fases correspondientes. No se hizo commit, push, merge ni trabajo de Fase 1B.
