# F4-R4 — Resultados y comandos

**Rendimiento: IMPROVED (limitado). TTS técnico: READY_FOR_REVIEW. F4: PARTIAL.** LIVE_TTS T1 conserva PASS histórico de R3; el pipeline R4 audible y T2–T5 humanos permanecen NOT_TESTED.
Fecha: 2026-10-19. Sólo recursos privados existentes, voz `es_ES-sharvard-medium`, Piper 1.8.0 / ONNX Runtime 1.31.0. Sin micrófono ni reproducción audible.

## Entorno y trazabilidad

[Baseline y hashes de los 16 informes anteriores](F4-R4-EVIDENCE/baseline.json), [CPU/afinidad/entorno](F4-R4-EVIDENCE/host.json). Branch/HEAD comprobados antes de iniciar los binarios; índice vacío y un solo worktree. HOME/TMP/XDG en `/var/tmp/kda-f4r4-_rbmkh36`, raíz y subdirectorios privados. `private_env`/`require_private` verifican propiedad, permisos, rutas distintas, socket `f4-unreachable-audio`, bus ausente y ausencia de credenciales/overrides. Las instancias de Backend se crean en subperfiles nuevos con SQLite sintética y puerto efímero, sin reutilizar perfil, sockets o puertos personales.

Piper conserva su instalación autorizada `/var/tmp/kda-f4r3-piper-aqrh6h0h`. Sin pip, paquetes globales, nuevas dependencias, voz, motor ni descargas. `pw-play` sólo conecta al daemon privado de `tests/fixtures/f4/pipewire-tts.conf`: no carga ALSA/udev/Pulse ni un session manager de hardware. Las pruebas de KWin, cuando se indican, sólo usan ventanas propias y mantienen el audio privado.

Build dev, aplicación sin optimizar y `sha2` optimizado según Cargo.toml existente. Dos jobs; antes de compilar se observaron 128 GiB libres y unos 4,3 GiB de memoria disponible. No build release ni compilaciones simultáneas con inferencia. No cambios a flags del modelo, hilos, voz, hablante o duración para fabricar mejoras.

## Comandos reproducibles

Ejecutados por wrappers con `private_env` y comprobación branch/HEAD; no copiar un lanzamiento de Backend al perfil habitual. Los runners de benchmark exigen perfiles explícitos y validan el aislamiento por sí mismos.

```text
cargo fmt
cargo test --locked --lib voice_ -- --test-threads=1
cargo test --locked --test f4_cli -- --test-threads=1
cargo clippy --locked --all-targets -- -D warnings
cargo build --locked --bin kde-assistant --bin bench_piper_warm

python3 -B scripts/benchmark_f4r4.py --profile /var/tmp/kda-f4r4-_rbmkh36 --resources /var/tmp/kda-f4r3-piper-aqrh6h0h --runs 5 --baseline /var/tmp/kda-f4r4-_rbmkh36/baseline-kde-assistant --binary /home/eirom/KDE-ASSISTANT-2/target/debug/kde-assistant --label final-buffered-rerun --candidate buffered
python3 -B scripts/benchmark_f4r4_warm.py --profile /var/tmp/kda-f4r4-_rbmkh36 --resources /var/tmp/kda-f4r3-piper-aqrh6h0h --runs 5 --label warm-experiment-final
python3 -B scripts/benchmark_f4r4_playback.py --profile /var/tmp/kda-f4r4-_rbmkh36 --resources /var/tmp/kda-f4r3-piper-aqrh6h0h --runs 5 --baseline /var/tmp/kda-f4r4-_rbmkh36/baseline-kde-assistant --label playback-final-rerun
python3 -B scripts/benchmark_f4r4_playback.py --profile /var/tmp/kda-f4r4-_rbmkh36 --resources /var/tmp/kda-f4r3-piper-aqrh6h0h --runs 5 --baseline /var/tmp/kda-f4r4-_rbmkh36/baseline-kde-assistant --label playback-paired-final --paired
python3 -B scripts/validate_f4_piper.py --profile /var/tmp/kda-f4r3-piper-aqrh6h0h
```

`baseline-kde-assistant` es una copia del binario R3 exacto, SHA-256 `ed42270cd686109ef5a000f72afb40fa8665615fb6f5b59745ee9c9a2ccd2833`, tomada antes de editar fuentes R4. Conserva el preflight duplicado y reproducción tras PCM completo. No se intenta reconstruir el baseline mezclando los cambios nuevos.

Una exploración inicial medía `/proc` y buscaba compiladores con demasiada frecuencia: se conserva como diagnóstico, fuera de la comparación principal. Otro intento se detuvo al detectar compilación entre inferencias y quedó incompleto; [comando y fallo](F4-R4-EVIDENCE/benchmark-attempt1-commands.json), [log](F4-R4-EVIDENCE/paired-incomplete.log). No se relajó el guard, no se terminó ningún proceso ajeno ni se atribuyó sin evidencia la compilación a una aplicación concreta. El nuevo intento usa otra etiqueta, preservando el anterior.

El primer intento del nuevo runner de reproducción recibió 409 en su segunda petición porque conservaba un scope ya vencido. Se corrigió el helper para obtener un scope vigente por operación, conforme al contrato existente; no se cambió la aceptación del Backend. [Intento y resultado](F4-R4-EVIDENCE/benchmark-commands.json), [repetición corregida](F4-R4-EVIDENCE/playback-commands.json). La serie agrupada mostró dispersión, por lo que se añadió una comparación alternada con Backend/perfil nuevo en cada operación; se conservan ambas, sin mezclar muestras.

## Cobertura focalizada

- UNIT: 47 tests `voice_` en la comprobación focal inicial, incluidos coordinator, proceso, STT, estados y recursos Piper; CLI interno aislado PASS. La suite final F4 registra cada grupo por separado.
- UNIT: integridad rechaza corrupción/tamaño/enlace/especial; el descriptor conserva el inodo verificado tras sustituir la ruta. Verificación repetida por operación, sin caché de mtime.
- UNIT: protocolo experimental conserva orden y final; rechaza cabecera/PCM truncados, frames impares/excesivos, audio vacío, contadores incorrectos, exceso total y fallo del productor. Cancelación y timeout recogen productor y reproductor bloqueados por backpressure. No prueba de altavoces.
- AUTOMATED_AUDIO_FIXTURE: síntesis real y comparación sobre texto sintético, sin archivos de audio; formato/tamaño/duración/hash medidos y procesos esperados recogidos. Sumidero nulo separado de cualquier aceptación humana.
- AUTOMATED_QML / AUTOMATED_KWIN: sólo según los resultados de los runners finales; no se cuentan como interacción humana.
- HUMAN_VERIFIED / LIVE_TTS: únicamente T1 histórico R3. Pipeline R4 audible NOT_TESTED; T2–T5 humanos NOT_TESTED. LIVE_MICROPHONE y LIVE_WAKE_WORD no ejecutados en R4.

## Endurecimiento final y fallo intermitente

La comprobación de identidad se corrigió para comparar contra los metadatos capturados al terminar el hash, sin volver a tomarlos como referencia después de preparar el runtime. [Build y cuatro tests de integridad](F4-R4-EVIDENCE/inode-fix-build.json). Las series anteriores siguen siendo evidencia de rendimiento con binarios identificados; el inicio de reproducción y el lifecycle se repiten en el binario endurecido.

Primera secuencia: F4/F3.5/F2 PASS; F3 falló «Active own window did not hide». [Secuencia completa](F4-R4-EVIDENCE/gates/results.json), [fallo](F4-R4-EVIDENCE/gates/f3.log). El mismo comando oficial, sin alterar runner ni aserciones, pasó al repetirlo: [comando](F4-R4-EVIDENCE/f3-diagnostic-command.json), [resultados propios](F4-R4-EVIDENCE/f3-diagnostic-results.json). No se ha demostrado una causa ni relación con TTS. Se conserva como intermitencia observada; no se borra el fallo inicial ni se lo transforma en PASS.

Última comprobación de reproducción y lifecycle: [comandos](F4-R4-EVIDENCE/post-inode-commands.json), [datos n=5 por ruta](F4-R4-EVIDENCE/playback-inode-safe-final.json), [siete escenarios PASS](F4-R4-EVIDENCE/piper-inode-safe-lifecycle.json). Speaking p50 7,692 → 7,238 s, ahorro emparejado mediano 453 ms, 5/5 positivo. Cancelar reproducción: 6,06 ms en un caso; cambio de sesión: 14,17 ms en un caso. Son datos de sumidero nulo, no escucha humana. [Smoke preparado final n=1](F4-R4-EVIDENCE/warm-inode-safe-smoke.json), sin percentiles nuevos.

## Gates finales

Los comandos completos, salidas y estados definitivos están registrados en `F4-R4-EVIDENCE/gates-final/results.json`. Se ejecutan secuencialmente, restaurando el build default tras los fixtures legacy:

```text
cargo fmt --check
python3 -B scripts/validate_f4.py --output docs/recovery/F4-R4-EVIDENCE/gates-final/f4
python3 -B scripts/validate_f35.py --plasma
python3 -B scripts/validate_f2.py
cargo build --locked --bin kde-assistant
python3 -B scripts/validate_f3.py --runtime-only --plasma
python3 -B scripts/validate_f1b3b.py --qml
cargo clippy --locked --all-targets -- -D warnings
cargo clippy --locked --features legacy-fixtures --all-targets -- -D warnings
cargo build --locked --bin kde-assistant
cargo fmt --check
```

El primer Clippy de desarrollo señaló dos detalles del nuevo adaptador experimental (retorno redundante y comprobación de paridad); se corrigieron conservando las condiciones del protocolo. No se deshabilitó ningún lint ni aserción. Los resultados finales se distinguen de esos intentos de desarrollo.

## Privacidad y cierre

Siete escenarios reales de Piper/sumidero nulo PASS: cancelar preparación inmediata (17,50 ms), preparación activa a los 1,5 s (21,00 ms), detener reproducción (12,25 ms), nueva reproducción, cambio de sesión (12,18 ms), autolectura mock tras cierre y modelo ausente. Son observaciones individuales de cancelación, no percentiles. El estado Preparing incluye validación/importación/carga/inferencia: el instante de cancelación no identifica por sí solo una subfase interna. La cancelación de procesos largos está cubierta con fixtures de supervisión; no se identificó con un marcador la subfase ONNX exacta durante el cancel real. No se etiqueta esa observación como prueba específica de cancelación dentro de una llamada ONNX.

Una sola llamada al mock loopback, cero prompts adicionales, texto persistido sin cambios y todos los hijos observados recogidos. Reposo default: tres segundos, cero hijos de voz, Backend RSS 29.888 KiB/PSS 25.913 KiB, CPU observada 0,0 % de un núcleo; no es una garantía de CPU nula para toda duración. [Evidencia](F4-R4-EVIDENCE/piper-lifecycle.json).

Las pruebas no utilizaron conversaciones, claves, configuración ni permisos personales. Sin audio remoto, llamadas LLM reales, herramientas, consentimiento F2 alterado ni registro de GlobalShortcuts. La prueba mock verifica una sola llamada loopback, texto persistido idéntico, cancelación/cambio de sesión y chat operativo con modelo ausente. Los informes previos y T1 permanecen intactos.

Se conserva la evidencia numérica y los recursos autorizados para revisión; no se guardan PCM/WAV de síntesis. Cada runner recoge sus propios hijos y cierra Backend, daemon y mock al terminar. No se reutilizó la autorización audible consumida ni se inició otra prueba humana. Sin commits/push/merge/release.

## Dictamen y cierre final

**Rendimiento: IMPROVED (limitado). TTS técnico: READY_FOR_REVIEW. F4: PARTIAL.** LIVE_TTS T1 conserva PASS histórico de R3; el pipeline R4 audible y T2–T5 humanos permanecen NOT_TESTED.

Todos los gates finales terminaron exit 0, secuencialmente y después del último endurecimiento:

| Gate | Resultado | s |
|---|---|---:|
| fmt-check | PASS | 1.072 |
| f4 | PASS | 43.299 |
| f35 | PASS | 24.869 |
| f2 | PASS | 104.032 |
| default-before-f3 | PASS | 0.471 |
| f3 | PASS | 32.865 |
| f1b3b | PASS | 38.763 |
| clippy-default | PASS | 0.567 |
| clippy-legacy | PASS | 0.517 |
| build-default | PASS | 0.516 |
| fmt-final | PASS | 0.968 |

[Evidencia completa final](F4-R4-EVIDENCE/gates-final/results.json). El F3 inicial FAIL se conserva; después hubo dos ejecuciones oficiales PASS sin modificar el runner. Causa de aquella intermitencia no aislada; no se afirma que el fallo no pudiera reaparecer.

[Auditoría final](F4-R4-EVIDENCE/final-audit.json): HEAD/branch conservados, índice vacío, un worktree, 16 informes históricos idénticos a sus hashes iniciales, hashes finales de código/binarios y comprobación de recogida de hijos conocidos. No se borraron recursos Piper autorizados ni evidencia previa. Se conservan perfiles privados sintéticos para revisión, sin grabaciones ni PCM/WAV de síntesis; ningún servicio de voz de estas pruebas queda activo. Sin cambios al perfil personal ni restauración del incidente histórico.

Se espera revisión técnica antes de una nueva prueba audible o de decidir una política de preparación temporal. No se solicita ni reutiliza permiso de micrófono/altavoces en esta entrega. No hay commits, push, merge, release, F5 ni wake word nuevos.

El smoke final del modelo preparado se lanzó una vez, desde `private_env` verificado, con `KDA_WARM_EXPERIMENT_ROOT=/var/tmp/kda-f4r4-_rbmkh36` y este argv explícito (no ejecutar con un entorno personal):

```text
/home/eirom/KDE-ASSISTANT-2/target/debug/bench_piper_warm /home/eirom/KDE-ASSISTANT-2/target/debug/kde-assistant /var/tmp/kda-f4r3-piper-aqrh6h0h/venv /var/tmp/kda-f4r3-piper-aqrh6h0h/voice/es_ES-sharvard-medium.onnx
```
