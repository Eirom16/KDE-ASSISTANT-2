# Evidencia F4 — índice de revisión

Baseline `9544a58eba4b6adf6c6343c04db80813b084f76b`; rama `recovery/f4-voice-coordinator`; sin commits ni staging. Resultado de producto **PARTIAL / NEEDS_FIXES**. Los PASS automáticos no aceptan hardware, precisión española ni rendimiento.

## Evidencia final

- [FINAL/results.json](FINAL/results.json): compilación, UNIT, conversión y Qt. [runtime.json](FINAL/runtime.json), [pipewire.json](FINAL/pipewire.json) y [main-lifecycle.json](FINAL/main-lifecycle.json) completan la integración aislada.
- [final-validation.json](final-validation.json): dos ejecuciones consecutivas del mismo runner sobre FINAL, ambas exit 0. [Primera](f4-delivery-1.log) y [segunda](f4-delivery-2.log). La segunda verifica la corrección de readiness al reutilizar la salida.
- `final-checks/`: fmt, ambos Clippy, build y worker Whisper real con cancelación/timeout. El modelo tiny debe elegirse explícitamente; el test no descarga modelos.
- `regressions/`: F3.5, F2 y selecciones F3/F1B. `review-f3.log`, `f3-runtime-final.json` y `review-f1b3b.log` corresponden a las últimas ejecuciones documentadas en F4-RESULTS.
- `final-ui/` y `visual-plasma/`: Settings Qt y capturas propias claras/oscuras en Wayland/OpenGL. `visual/` conserva el render software con la limitación ColorOverlay documentada.
- `installed-models.json`, `tiny-download.json`, `bench-base-debug*.json` y `bench-tiny/`: inventario y benchmark exploratorio. Tiny se descargó con permiso únicamente para benchmark offline, a un directorio privado fuera del repositorio; no se instaló en el perfil personal. No hay pesos en esta carpeta.
- `corpus/manifest.json`: señales propias y referencias CC0; no contiene grabaciones humanas. Los WAV generados pueden reproducirse con scripts/voice_corpus.py; no se almacenan aquí.
- `source-review.json`: hashes del código entregado y comprobaciones de rama/HEAD/índice para revisión, sin crear un commit.

## Historia y privacidad

Los demás directorios (`validation*`, `final`, `accepted-automation`, `final-automation`, `verified-final`, `review-final`, etc.) conservan intentos intermedios; algunos fallaron. No todos son resultados aceptados. La evidencia canónica es FINAL y los archivos identificados arriba. F4-RESULTS y F4-ISSUES explican los fallos de fixtures y la regresión DBus corregida sin relajar aserciones heredadas.

No se copian perfiles XDG, bases SQLite, claves, bearer ni archivos auth a esta entrega. Las pruebas automatizadas finales usan perfiles privados, datos sintéticos, loopback mock y PipeWire privado sin módulos de hardware. No se certifica LIVE_MICROPHONE/LIVE_TTS/LIVE_WAKE_WORD.

F4-INC-01 es una excepción real al aislamiento de la sesión: un diagnóstico arrancó una instancia propia sobre el perfil personal antes de corregir el dispatcher. Se detuvo; no se observó audio ni chat remoto, pero config/SQLite se abrieron y no hay snapshot previo para certificar ausencia de cambios. El incidente está descrito íntegramente en F4-ISSUES/F4-REPORT; los PASS posteriores no lo eliminan.
