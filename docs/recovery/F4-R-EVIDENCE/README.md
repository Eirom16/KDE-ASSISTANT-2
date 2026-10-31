# Evidencia F4-R — índice

Código sin commits sobre recovery/f4-voice-coordinator / HEAD 9544a58eba4b6adf6c6343c04db80813b084f76b. Producto F4 NEEDS_FIXES; mejoras de coste sólo experimentales/offline.

- **gates-final/** y **gate-final-driver.log**: evidencia canónica del código final después del probe de cancelación confirmada. results.json contiene comandos/exit/duración; f4/ incluye el runner desglosado, HTTP/PipeWire/Main. f3-results/f35-results son copias seguras de los resúmenes; no se copian perfiles/SQLite/auth.
- **confirmed-inference-cancel.json** y **gates-final/worker.log**: marca después de carga/estado, progreso CPU y cancelación/reaping exactos. Los probes temporizados originales también siguen en el mismo test. No es LIVE_MICROPHONE.
- **whisper-summary.json**, **dev-*/benchmark.json/process.json**: experimentos descriptivos O2. Las primeras carpetas dev-default/dev-ctx256 usan 48→16 kHz; canónicas y el resto usan 16 kHz. Los comandos están completos en process.json. cpp flags/cache y toolchain identifican el backend; compiled-capabilities, n=1, sólo confirma capacidades y no se mezcla en percentiles.
- **vosk-small-es/** y **vosk-final/**: dos ejecuciones independientes, cada una n=10 por señal. La final incluye manifiesto completo y hashes del corpus. No se agrupan como una población homogénea n=20. El helper es sólo benchmark, sin integración del motor en el producto.
- **corpus-equivalence.json** confirma que los WAV canónicos Whisper/Vosk son idénticos por SHA256. **corpus-plan.json** identifica las grabaciones humanas ausentes y sus requisitos de referencia/permiso. WER hablado NOT_TESTED.
- **download-proposal.json / vosk-download.json**: autorización explícita de modelo+runtime Vosk, tamaños/hashes, inventario extraído y eliminación completa de los artefactos privados. No hay pesos ni bibliotecas descargadas en esta entrega; no hubo instalación global/pip.
- **native-release-preflight.json / native-release-build.log / cmake-blocked.json**: intento offline de reconstrucción C++ Release con un job, fallido por CMake ausente. No hay cifras O3 ni clean build certificado. CMake fue propuesto pero no descargado sin respuesta específica.
- **tiny-quantized-metadata.json / sherpa-assets-metadata.json**: consulta de metadatos oficiales; ningún modelo de esas alternativas fue descargado.
- **baseline.json / source-review.json**: hashes F4 preservados, delta correctivo, HEAD/índice y fuentes entregadas. Los cinco informes F4 originales permanecen byte a byte iguales.

**Historia:** gates/, f4-first/ y confirmed-inference-cancel.log conservan comprobaciones anteriores a la instrumentación final; native-release-build.log conserva el fallo real de dependencia. No se ocultan bajo los PASS finales.

Los perfiles de prueba son privados, sin claves reales, micrófono/salida/wake ni cambios a KDE personal. Los gates Plasma usan sólo sus propias ventanas sobre el display/bus real y perfiles de aplicación separados. La incertidumbre del incidente F4-INC-01 anterior no se resolvió inspeccionando datos personales; estas guardas previenen repetición, no certifican retrospectivamente ausencia de cambios.
