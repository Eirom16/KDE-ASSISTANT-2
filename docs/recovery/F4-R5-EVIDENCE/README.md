# Evidencia F4-R5

- `baseline.json`: rama/base, binario R4 y veinte hashes históricos.
- `paired-final.json` / `paired-summary.json`: cinco parejas integradas R4/R5.
- `mock-*-final.json` / `mock-summary.json`: autolectura con proveedor loopback.
- `lifecycle-final.json` / `extended-final.json`: 28 casos de lifecycle/fallos.
- `ttl30-final.json` / `ttl-candidates-final.json`: expiración/recarga real.
- `qml-coexist-final.json`: UI real offscreen y null sink.
- `final-build-check.json` / `final-source.json`: binario y fuente finales, smoke y reposo OFF.
- `gates/results.json`: primera secuencia, incluido FAIL Clippy original.
- `gates-recheck/`: FAIL del perfil largo del arnés (todos los grupos previos PASS).
- `gates-final/results.json`: cierre F4/F2/build/fmt PASS. F3/F3.5/F1B3B y Clippy explícitos están en `gates/`.
- `diagnostic-before-*`, `ttl-before-zombie-fix.json`, `pressure-policy-diagnostic.json`: intentos anteriores separados de estadísticas finales.
- `cleanup.json`, `secret-scan.json`, `socket-precheck.json`: cleanup, canarios y rechazo previo a Backend.

UNIT, AUTOMATED_AUDIO_FIXTURE, AUTOMATED_QML y AUTOMATED_KWIN según cada runner. Ningún archivo acredita escucha humana R5 o micrófono físico. No config, SQLite, bearer, PCM/WAV o recursos de modelos en este directorio.
