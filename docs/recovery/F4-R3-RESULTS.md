# F4-R3 — Resultados de validación

Fecha: 2026-10-19. **PARTIAL / LIVE_TTS NOT_TESTED / NEEDS_FIXES.** La síntesis es real; ninguna prueba ha abierto altavoces ni micrófono. T1–T5 necesitan permisos y confirmaciones separados.

## Preparación autorizada

Comando ejecutado tras «Lo autorizo»:

```text
python3 -B scripts/prepare_f4_piper.py --inventory docs/recovery/F4-R3-EVIDENCE/download-inventory.json --authorization docs/recovery/F4-R3-EVIDENCE/download-authorization.json
```

PASS: diez archivos, 151.879.200 B verificados; siete wheels + ONNX/config/ficha. Instalación sólo en venv privado, sin índice ni compilación Python; versiones e importaciones coinciden: [instalación](F4-R3-EVIDENCE/installation.json), [importaciones](F4-R3-EVIDENCE/import-check.json). No paquetes globales o descargas adicionales. Los 1799 recursos de `piper_resources.json` se derivaron de los wheels verificados y coincidieron byte a byte con lo instalado (168.960.626 B, excluyendo RECORD regenerado por pip y COPYING bajo .data; los avisos se conservan instalados).

Los tags x86_64/CPython 3.14 son sólo comprobación de distribución. La compatibilidad real N5000 se acredita mediante las inferencias siguientes, no por los tags.

## UNIT / AUTOMATED_QML / AUTOMATED_KWIN

Primera selección, con HOME/TMP/cuatro XDG privados, audio/bus inaccesibles, Cargo offline y jobs=2:

- `cargo test --locked --offline --lib voice_coordinator::tests -- --test-threads=1`: 13 PASS.
- `cargo test --locked --offline --lib voice_process::tests -- --test-threads=1`: 3 PASS.
- `cargo test --locked --offline --lib voice_tts::tests -- --test-threads=1`: 2 PASS.
- `cargo test --locked --offline --test f4_cli -- --test-threads=1`: PASS; incluye Piper inválido/ausente y argumentos internos mal colocados antes de Backend/config/SQLite.
- Formato, build y Clippy default PASS. [Logs y comandos de build focalizado](F4-R3-EVIDENCE/integration-build.json), [focales previos](F4-R3-EVIDENCE/focused-results.json).

Primera pasada secuencial de gates completos: todos PASS, [comandos/tiempos](F4-R3-EVIDENCE/gates/results.json). Incluye:

```text
python3 -B scripts/validate_f4.py --output <directorio de evidencia>
python3 -B scripts/validate_f35.py --plasma
python3 -B scripts/validate_f2.py
python3 -B scripts/validate_f3.py --runtime-only --plasma
python3 -B scripts/validate_f1b3b.py --qml
cargo fmt --check
cargo clippy --locked --all-targets -- -D warnings
cargo clippy --locked --features legacy-fixtures --all-targets -- -D warnings
cargo build --locked --bin kde-assistant
```

Los dos Clippy están dentro de F2. Se restaura el binario default después de legacy-fixtures y antes de runtime/pruebas reales. Los gates no compiten por puertos ni compilaciones. Las pruebas Plasma usan sólo sus propias ventanas, Wayland absoluto y el bus existente; HOME/TMP/cuatro XDG privados, audio inaccesible, sin credenciales/atajos. No son pruebas humanas de TTS/PTT.

Pasada final tras deshabilitar lectura manual sin recursos y corregir el significado del indicador de inicialización: **todos PASS**, [resultado final](F4-R3-EVIDENCE/final-gates/results.json). QML de voz: 9 PASS; QML Configuración: 17 PASS (incluye runtime/hablante Piper guardados y recargados), con pruebas Wayland/KWin propias: [detalle F3.5](F4-R3-EVIDENCE/final-gates/f35-details.json). No se atribuyen PASS históricos R2 a R3.

Endurecimiento posterior, limitado a ejecutar el intérprete canónico del sistema evitando que pyvenv.cfg redirija la biblioteca estándar: F4, ambos Clippy, build y formato repetidos PASS, [comandos](F4-R3-EVIDENCE/hardened/results.json). Cuatro pruebas Python de admisión T1 PASS: permiso ausente/incompleto, recibo usado o configuración alterada se rechazan antes de acceder al socket del host o lanzar un binario. El instalador quedó además con entorno mínimo y pip --isolated para futuras preparaciones; no se repitió la instalación ni hubo nuevas descargas. El comando efectivamente instalado es el registrado en installation.json.

## AUTOMATED_AUDIO_FIXTURE — motor real

Texto sintético conocido: «Hola. Esta es una prueba de voz en español de KDE Assistant.» No grabaciones humanas ni cálculo de WER; la inteligibilidad TTS requiere audición. PCM16LE mono 22.050 Hz, no vacío y dentro de 16 MiB; se borran los PCM instrumentales tras medir. Los pesos/paquetes son los del inventario aprobado; ni CUDA ni otro proveedor de inferencia.

```text
python3 -B scripts/benchmark_piper_f4.py --profile /var/tmp/kda-f4r3-piper-aqrh6h0h --runs 3
python3 -B scripts/benchmark_piper_f4.py --profile /var/tmp/kda-f4r3-piper-aqrh6h0h --runs 3 --optimization-comparison
python3 -B scripts/benchmark_piper_f4.py --profile /var/tmp/kda-f4r3-piper-aqrh6h0h --runs 3 --prepack-comparison
```

Exploración de hilos: [datos](F4-R3-EVIDENCE/threads-exploratory.json). CLI oficial p50 6613,1 ms; helper con 1/2/3 hilos 7561,9 / 6541,2 / 6201,2 ms. Se conserva como exploración, no comparativa final: durante la primera observación también se generó el manifiesto de recursos; además esa primera implementación leía cachés de bytecode del sistema y la final las evita. El primer intento de runner se interrumpió por duplicar una clave del diccionario de métricas; no se utilizó para percentiles, se corrigió el colector y se repitió. No fue fallo del motor ni prueba audible.

Comparaciones homogéneas siguientes, n=3 por configuración, tres hilos, `-I -B -S -X pycache_prefix=/dev/null`, sin compilación/inferencia concurrente:

| Configuración | p50 pared ms | p95 ms | Min–max ms | Desv. estándar ms |
|---|---:|---:|---:|---:|
| ORT ALL | 6830,0 | 6871,3 | 6724,3–6871,3 | 61,9 |
| ORT BASIC | 7137,0 | 7168,9 | 7087,8–7168,9 | 33,4 |
| ORT DISABLED | 7153,9 | 7223,3 | 7151,4–7223,3 | 33,3 |
| ALL, ensayo prepacking independiente | 6759,7 | 7019,8 | 6750,6–7019,8 | 124,8 |
| ALL sin prepacking | 6669,9 | 6859,7 | 6643,1–6859,7 | 96,4 |
| ALL con prepacking paralelo | 6726,4 | 6779,3 | 6685,5–6779,3 | 38,4 |

[Optimización](F4-R3-EVIDENCE/graph-optimization.json), [prepacking](F4-R3-EVIDENCE/prepacking.json). Se conserva ALL y prepacking estándar: BASIC/DISABLED empeoran el tiempo; la pequeña variación de prepacking no justifica cambiarlo con n=3. Las claves se verificaron en el [código oficial ORT 1.31.0](https://github.com/microsoft/onnxruntime/blob/v1.31.0/include/onnxruntime/core/session/onnxruntime_session_options_config_keys.h). p95 es el máximo por nearest-rank con tres observaciones, no una estimación robusta de cola. Todas las series incluyen n, media, dispersión/MAD y CPU/RSS/PSS muestreados cada 25 ms en su JSON; no se mezclan como una única distribución.

En la primera exploración de tres hilos: importación ≈0,60–0,63 s, carga ≈4,34–4,45 s, síntesis ≈0,99–1,06 s, primer PCM ≈5,12–5,20 s (reloj interno), RSS máximo muestreado ≈209–211 MiB, PSS ≈202–204 MiB, CPU acumulada ≈7,68 s. Audio generado ≈3,15–3,22 s, variable por ruido del sintetizador. No son latencias audibles ni del coordinador final: faltan preflight/validación, HTTP y reproducción. La carga domina; no se retiene un modelo ni se crean derivados/cache de pesos para ocultarla.

## Coordinador real, sumidero privado sin altavoz

```text
python3 -B scripts/validate_f4_piper.py --profile /var/tmp/kda-f4r3-piper-aqrh6h0h
```

Primera ejecución PASS: [evidencia](F4-R3-EVIDENCE/coordinator-real-first.json). Daemon PipeWire privado sólo con driver dummy y null sink, sin ALSA/udev/Pulse/sesión de hardware. No se llama pw-record. Se prueba síntesis real→pw-play→sumidero nulo, cancelación, nueva reproducción sin mezcla, cambio de sesión y respuesta mock completada→autolectura. Una sola llamada `/v1/chat/completions`, sin tools, texto persistido idéntico antes/después de leer. Modelo ausente devuelve error y no deja hijos. Preparación cancela en 16,2 y 13,9 ms observados; reproducción 9,7 ms y sesión 9,3 ms, una observación por caso, sin p50/p95 inventados.

El nombre original `cancel-inference` de ese primer JSON NO demuestra inferencia: se canceló a 1,5 s dentro de Preparing, que incluye hash/import/carga/síntesis. Se conserva el dato con nota correctiva; el runner usa ahora `cancel-active-preparation`. La comprobación post-carga del worker final se medirá por un marcador real de carga.

Reposo del backend tras operaciones: ventana de 3,002 s, 29.280 KiB RSS, 24.683 KiB PSS, 0,0 % de un núcleo, cero hijos de voz. Sólo ese perfil/ventana, no promesa de consumo general. Cada proceso propio observado se recoge al finalizar. No hay worker/modelo permanente.

## Worker nativo final: medición separada

```text
python3 -B scripts/benchmark_native_piper_f4.py --profile /var/tmp/kda-f4r3-piper-aqrh6h0h --runs 3
```

PASS, [datos completos y hash del binario](F4-R3-EVIDENCE/native-final.json). Proceso/sesión ONNX nuevos en cada observación; páginas del filesystem pueden estar calientes. No mide modelo residente caliente (NOT_TESTED). Comparación final independiente de las series exploratorias: intérprete canónico, importación aislada sin cachés de bytecode, ALL/3 hilos, hashes conservados. No hubo compilación concurrente.

| Etapa | n | p50 ms | p95 ms | Min–max ms | Desv. estándar ms |
|---|---:|---:|---:|---:|---:|
| Preflight nativo independiente | 3 | 536,0 | 1100,9 | 534,2–1100,9 | 266,7 |
| Worker completo (validación + import + carga + síntesis + salida) | 3 | 7835,3 | 7909,5 | 7628,3–7909,5 | 119,0 |
| Primer PCM observado desde iniciar el worker | 3 | 6688,9 | 6786,3 | 6525,7–6786,3 | 107,5 |
| Kill + wait tras procesamiento posterior a carga | 3 | 32,0 | 32,0 | 16,7–32,0 | 7,2 |

**No sumar medianas como si fueran una medición del coordinador.** Las parejas preflight+worker observadas suman 9010,4 / 8162,5 / 8371,3 ms, antes de HTTP/pw-play. La producción espera el PCM completo: disponer de primer PCM no significa haber iniciado reproducción. Tiempo hasta primer audio audible: NOT_TESTED. La espera de unos ocho segundos sigue siendo una limitación material de experiencia; no se declara aceptable sin evaluación humana ni se amplían timeouts.

Reloj interno del helper final: importación 971,5–1038,9 ms, carga ONNX 4692,5–4883,8 ms, síntesis 1109,8–1140,0 ms. CPU acumulada 9,009–9,198 s por worker; RSS máximo muestreado 215.212–215.636 KiB (≈210 MiB), PSS 207.306–207.730 KiB (≈203 MiB), intervalo 25 ms. Son máximos observados, no garantía de haber capturado el pico instantáneo. Los JSON registran loadavg e intervalos. No se atribuye toda diferencia entre exploración y medición final a una única causa.

Cancelación: tres procesos reales emitieron un marcador `loaded` tras crear la sesión ONNX; se cancelaron 250 ms después, con 61–63 ticks CPU adicionales de procesamiento. Todos recogidos. Prueba directa del worker y procesamiento posterior a carga; el enrutado por identidad/cancelación HTTP se comprueba separadamente con el coordinador. No se identifica mediante esos ticks la instrucción concreta del kernel ONNX interrumpida.

Pesos corruptos: copia privada del ONNX con cuatro bytes alterados, tamaño idéntico; preflight devuelve «Integridad de recurso Piper inválida» antes de cargar el modelo. Fixture eliminado. Los errores de ausencia/corrupción y los timeouts/límites del supervisor tienen evidencia distinta; no se provocó una espera real de 90 s para declarar el timeout unitario como modelo colgado real.

## Pendiente humano, límites y limpieza

La ejecución final del coordinador con el intérprete canónico también pasó: [resultado](F4-R3-EVIDENCE/coordinator-real-final.json). Cancelación de preparación 16,4 / 23,6 ms; reproducción a null sink 9,3 ms; cambio de sesión 12,7 ms, cada caso una observación. Nueva reproducción y autolectura mock PASS, una sola llamada al proveedor, texto persistido intacto, todos los hijos observados recogidos. Reposo final: 3,003 s, 29.832 KiB RSS / 25.494 KiB PSS, 0,0 % de un núcleo, cero hijos de voz. Esto no demuestra interrupción audible física.

T1 escuchar/volumen; T2 Detener; T3 nueva reproducción; T4 cambio de sesión; T5 chat mock con aceptación humana: **NOT_TESTED**. Nadie ha confirmado oír audio en R3. No se mide «primer audio audible» mediante un PCM ni se cuenta null sink como LIVE_TTS. Tampoco se repite PTT humano o se certifica wake.

T1 preparada mediante `python3 -B scripts/live_f4_tts.py --prepare --engine-profile /var/tmp/kda-f4r3-piper-aqrh6h0h`. Perfil `/var/tmp/kda-f4r3-piper-aqrh6h0h/live-t1-1792419192735966309`, [escenario sin token/credenciales](F4-R3-EVIDENCE/prepared-t1.json): una lectura de «Hola. Soy KDE Assistant y esta es una prueba de voz en español.», backend headless privado, sin proveedor activo, tools, micrófono o audio guardado. Preparación completada y cerrada; **NO lanzada para audio**. Permiso audible independiente solicitado y pendiente. Las muestras personales R2 siguen eliminadas; no se reutiliza ninguna grabación.

Tamaño lógico de archivos regulares observado: venv 180.670.633 B, wheels 75.140.290 B, voz/config/ficha 76.738.910 B (no mide bloques asignados ni RAM).

Los PCM instrumentales y el modelo corrupto se han eliminado: cero WAV/PCM en la raíz privada. Venv/wheels/voz/perfiles se conservan temporalmente mientras la evaluación autorizada permanece abierta y se eliminarán al cerrarla, sin borrado forense. Los datos de rendimiento son de CPU N5000, no extrapolación a otro equipo. [Checkpoint](F4-R3-EVIDENCE/checkpoint.json): 13 informes anteriores idénticos por SHA-256, rama/HEAD e índice vacío conservados, diff --check PASS. Sin commits ni perfil personal. [Procedencia del manifiesto de recursos](F4-R3-EVIDENCE/resource-manifest-provenance.json).

## Apéndice T1 — Revalidación previa a permiso

[Registro sin secretos](F4-R3-EVIDENCE/T1/preparation-check.json): PASS de hashes y escenario sin usar, sin repetir benchmarks. Binario SHA-256 `ed42270cd686109ef5a000f72afb40fa8665615fb6f5b59745ee9c9a2ccd2833`, idéntico al native-final registrado. Preflight `kde-assistant --voice-piper-check <venv privado> <voz privada>` devolvió null y exit 0: valida recursos, no infiere ni reproduce.

Backend privado comprobado por API local: idle, recursos presentes, sesión sintética preparada existente sin mensajes, sin hijos de voz; shutdown limpio y proceso recogido. Se verificaron las rutas y permisos de los seis directorios privados y el socket inaccesible. La clave de prueba está en provider_keys.custom por la migración F3.5 existente; la primera aserción esperaba el campo legado. Se corrigió la comprobación estricta de credencial sintética sin modificar configuración ni relajar el requisito de aislamiento.

**LIVE_TTS: NOT_TESTED.** Salida real, reproducción, tiempos de esta toma y valoración humana pendientes del permiso audible. No se abrió micrófono ni altavoz. Se conservan íntegros los resultados R3 anteriores; [hashes de sus prefijos antes del apéndice](F4-R3-EVIDENCE/T1/report-prefixes.json).

### T1 — Autorización, salida y ejecución técnica (valoración humana pendiente)

El usuario autorizó explícitamente «si, lo autorizo» y después confirmó «Volumen cómodo; reproducir ahora». Se consultaron únicamente la salida predeterminada y su volumen con wpctl inspect/get-volume, sin modificarlos: Audio Interno Estéreo analógico / ALC257 Analog. Antes del ajuste manual del usuario figuraba Volume: 1.15; no se impuso ni cambió volumen, mute, ruta o dispositivo. [Autorización](F4-R3-EVIDENCE/T1/audible-authorization.json), [salida](F4-R3-EVIDENCE/T1/output-check.json), [preparación física confirmada](F4-R3-EVIDENCE/T1/readiness.json).

Una única ejecución del escenario preparado: código de salida 0, sin error técnico observado; Preparing 79,4 ms → Speaking 8038,4 ms → Idle 11883,7 ms desde el reloj del launcher. Pared del comando completo 12,241 s. La ventana observada de Speaking fue ≈3,845 s; no es una medición del instante acústico de primer/último audio. No se calculan p50/p95 de esta observación.

[Procesos y ejecución](F4-R3-EVIDENCE/T1/technical-run.json), [estados de reproducción](F4-R3-EVIDENCE/T1/playback-observation.json). Se observaron Backend 11837, preflight 11845, worker 11846 (exec Rust→Python 3.14, mismo PID) y reproductor 11855 (pw-play resuelve a pw-cat). Todos recogidos. El backend volvió a idle, la sesión sigue sin mensajes y no hubo archivos PCM/WAV, micrófono, descarga o proveedor remoto. El recibo de una sola toma quedó consumido; no se repitió la reproducción ni se inició T2–T5.

Finalización técnica correcta; **clasificación LIVE_TTS pendiente de respuesta humana** sobre escucha completa, claridad, naturalidad, volumen y posibles defectos. No se declara LIVE_TTS_PASS por el estado idle o el exit 0.

### T1 — Confirmación humana y clasificación final de esta toma

**HUMAN_VERIFIED / LIVE_TTS_PASS (T1).** Respuesta literal: «Se escucho perfectamente, sin cortes y sin distorcion. El unico problema son los 8s.» [Registro de confirmación y hashes de la evidencia técnica](F4-R3-EVIDENCE/T1/human-confirmation.json). Se cumplen los dos criterios: reproducción técnica exitosa (exit 0, idle final, procesos recogidos) y confirmación humana de escucha.

Calidad audible valorada positivamente, sin cortes ni distorsión. El usuario había confirmado volumen cómodo y ahora da una valoración global positiva; no se inventan calificaciones independientes de naturalidad/volumen. La única objeción indicada es la espera de unos ocho segundos. Preparación→Speaking se observó en esta toma cerca de 8,04 s desde iniciar el launcher; el instante acústico no se instrumentó. Una sola observación, sin percentiles nuevos.

**Latencia: pendiente de corrección/revisión. TTS global: PARTIAL. F4: NEEDS_FIXES.** La aceptación audible no implica aceptación del tiempo de arranque ni certificación de T2–T5. No nuevas reproducciones, benchmarks o pruebas físicas; se conservan íntegros los resultados anteriores.
