# F4-R5 — Resultados offline

Implementación y validación offline completadas; revisión técnica pendiente. Aceptación humana F4-R5: **NOT_TESTED**. F4 global: **PARTIAL**. No inferir aceptación acústica de PCM, Speaking, spawn o conexión a sumidero nulo.

## Aislamiento y procedencia

Perfil nuevo `/var/tmp/kda-f4r5-3y7zg8fs`, HOME/TMP/XDG distintos, privados y verificados por `private_env`/`require_private`. SQLite sintética, bearer privado, puertos efímeros, proveedor loopback mock, PipeWire privado `f4-unreachable-audio` con sink nulo sin ALSA/udev/Pulse/session manager de hardware. Sin perfil habitual, inspección de conversaciones/credenciales personales, root, configuración KDE, micrófono, salida física, Groq, descargas o instalaciones.

Recursos existentes `/var/tmp/kda-f4r3-piper-aqrh6h0h`; revalidación nativa completa `--voice-piper-check` devuelve null/exit 0. Se usa el entrypoint explícito y verificado, sin buscar `piper` en PATH. Snapshot basal, veinte hashes de informes, hash del binario R4 y condiciones del host: F4-R5-EVIDENCE. El binario R4 se copió antes de editar R5; no se reconstruye mezclando cambios nuevos.

Build dev con dependencias bloqueadas/offline y jobs 1 en las comprobaciones finales. Sin release costoso ni builds durante inferencia. Una compilación ajena se dejó terminar antes de medir; no se detuvo ninguna aplicación ajena. Los checks de branch/HEAD preceden los lanzamientos. Los argumentos internos inválidos se prueban antes de Backend/config/SQLite, conservando F4-INC-01.

## Cobertura focalizada

UNIT: IPC válido y rechazos por versión/generación/request obsoletos, EOF, JSON corrupto, control excesivo, PCM impar/excesivo/truncado, contador/hash/formato incorrectos, error terminal y worker bloqueado; kill+wait de los hijos de fixture. ResourceWatch: cambios de tamaño idéntico, sustitución y permisos, ignorando archivos no relacionados. Manager: default/configuración sin precarga, deduplicación, época revocada, rotación de token y política de memoria fail-closed. Configuración: migración OFF, roundtrip y TTL inválido. Coordinator: scope/estado STT, no preparación sin autolectura, cancelación LLM antigua sin afectar lectura nueva y pruebas heredadas de ownership.

AUTOMATED_AUDIO_FIXTURE: Piper real a PCM completo y reproducción sólo en null sink, reutilización, TTL, cancelación durante inferencia observada, cambio de sesión, worker muerto/congelado, recursos privados modificados, recuperación, autolectura mock y chat independiente. Resultados y n por escenario figuran en el inventario final; no sustituye interacción humana.

AUTOMATED_QML / AUTOMATED_KWIN: gates oficiales PASS; pruebas de switch con teclado/tema, guardado/recarga y no forzar autolectura. No se etiqueta automatización como HUMAN_VERIFIED ni LIVE_PTT.

### Evidencia offline final del motor integrado

| Escenario | Resultado / evidencia | Categoría |
|---|---|---|
| R4 frío frente a primera/Ready/consecutiva R5 | 5 parejas, 20 lecturas; [datos](F4-R5-EVIDENCE/paired-final.json) | AUTOMATED_AUDIO_FIXTURE |
| Autolectura con streaming mock, ON/OFF | 5 turnos por modo; 1 llamada LLM por turno, texto persistido sin cambios; [warm](F4-R5-EVIDENCE/mock-warm-final.json), [cold](F4-R5-EVIDENCE/mock-cold-final.json) | AUTOMATED_AUDIO_FIXTURE |
| Desactivar Preparing/Ready, cancelar inferencia, sesión durante reproducción, muerte/congelación, cambio de hablante/recurso, recuperación, modelo ausente, toggles, texto OFF y cierre durante carga | 14 casos PASS; [lifecycle](F4-R5-EVIDENCE/lifecycle-final.json) | AUTOMATED_AUDIO_FIXTURE |
| Expirar durante LLM pendiente, vida máxima, timeouts reales, 5 cancelaciones LLM, nueva lectura/Busy, cierre en reproducción, identidad del binario, TTL durante PCM válido, admisión STT sin modelo | 14 casos PASS; [batería extendida](F4-R5-EVIDENCE/extended-final.json) | AUTOMATED_AUDIO_FIXTURE |
| Expiración y recarga 30 s, polling sin renovación | 5 ciclos PASS, todos recogidos; [TTL30](F4-R5-EVIDENCE/ttl30-final.json) | AUTOMATED_AUDIO_FIXTURE |
| QML real offscreen + Piper/null sink | Primera/Ready/consecutiva PASS, desactivación y recogida; [datos](F4-R5-EVIDENCE/qml-coexist-final.json) | AUTOMATED_QML + AUTOMATED_AUDIO_FIXTURE |

Los dos timeouts extendidos son observaciones únicas: reserva 15,039 s y carga congelada 89,889 s desde el punto instrumentado. No se les asignan percentiles. Cinco cancelaciones LLM durante preparación: p50 19,115 ms, p95 19,583 ms; cierre durante reproducción nula 165,2 ms y revocación del binario privado 52,2 ms (n=1 cada una). Son acknowledgements técnicos y recogida, no latencia acústica.

## Intentos que se conservan

Primer smoke usó la ruta fría y falló la expectativa Ready; primer intento de lifecycle tampoco llegó a Ready. El arnés inicial no conservó snapshot suficiente para atribuir cada fallo a una única causa. La posterior captura explícita muestra Fallback `memory_pressure`, cero workers y PSI 0,17% con MemAvailable 4.997.476 KiB. La política revisada combina margen disponible con alerta PSI, manteniendo fallo cerrado ante información inválida y presión con margen reducido.

La primera coexistencia QML/API terminó en Idle por revocación de contexto y agotó el timeout del arnés. Se conserva FAIL. Se corrigió el escenario para alinear su sesión con la selección inicial de Main.qml y esperar estabilización; la repetición pasó con la UI real offscreen. No se cambió F3 ni se amplió el deadline de reproducción para obtener PASS.

La primera comparación alternada quedó incompleta por una operación cancelada; se conserva fuera de los percentiles de la serie final. Se añadieron causas internas de revocación y se corrigió la recuperación de tokens. Los datos agrupados con mayor dispersión/temperatura se publican aparte; no se mezclan con la comparación alternada ni se suman percentiles de etapas independientes.

La primera batería extendida falló antes de Backend: el socket privado anidado medía 119 bytes y excedía sun_path (108 incluyendo NUL). PipeWire conservó el diagnóstico `File name too long`. Se acortaron los nombres privados y se añadió rechazo explícito de rutas demasiado largas; no se usó el runtime personal ni se amplió el timeout. La repetición tiene evidencia separada. Un primer orquestador también comprobó el ledger de build antes de su escritura final entre comandos; tras verificar las cinco salidas 0 se relanzó. Ese error de coordinación no se cuenta como fallo del motor ni como una prueba ejecutada.

La primera repetición de gates usó un perfil exterior más largo (`recheck-profile`): los 17 grupos F4 pasaron, pero el socket `f4-synthetic-manager` del fixture oficial excedió de nuevo sun_path. Se conserva ese FAIL en `gates-recheck`; se repite con un perfil exterior corto `rc`, sin modificar el runner oficial, su timeout ni sus aserciones. El precheck del arnés R5 contempla también el sufijo `-manager`.

## Comandos

Siempre dentro de entorno privado explícito. Los scripts de Piper rechazan perfil implícito y compiladores activos.

```text
cargo fmt
cargo test --locked --lib piper -- --test-threads=1
cargo test --locked --lib warm_piper::tests -- --test-threads=1
cargo test --locked --lib voice_coordinator::tests -- --test-threads=1
cargo test --locked --lib voice_fast -- --test-threads=1
cargo build --locked --bin kde-assistant
python3 -B scripts/validate_f4_r5.py --profile /var/tmp/kda-f4r5-3y7zg8fs --resources /var/tmp/kda-f4r3-piper-aqrh6h0h --runs 5
python3 -B scripts/benchmark_f4r5.py --profile /var/tmp/kda-f4r5-3y7zg8fs --resources /var/tmp/kda-f4r3-piper-aqrh6h0h --baseline /var/tmp/kda-f4r5-3y7zg8fs/baseline-r4 --runs 5
```

Variantes `--cold`, `--binary`, `--mock`, `--ttl 15/30/60`, `--lifecycle`, `--smoke --qml-coexist` se registran con sus comandos exactos y resultados. `--mock --cold` conserva autolectura pero apaga preparación. El timeout HTTP del arnés permite una demora mock deliberadamente mayor que el TTL; no modifica deadlines del producto.

Gates requeridos, secuenciales:

```text
python3 -B scripts/validate_f4.py
python3 -B scripts/validate_f35.py --plasma
python3 -B scripts/validate_f2.py
python3 -B scripts/validate_f3.py --runtime-only --plasma
python3 -B scripts/validate_f1b3b.py --qml
cargo fmt --check
cargo clippy --locked --all-targets -- -D warnings
cargo clippy --locked --features legacy-fixtures --all-targets -- -D warnings
cargo build --locked --bin kde-assistant
```

F3.5-W1 formal sigue omitido por decisión del usuario y no se presenta como certificación exhaustiva. PTT R2 y T1 R3 conservan su evidencia humana histórica; no se repiten. R5 audible, T2–T5, wake, OOM físico/cgroups restringidos y precisión STT nueva: NOT_TESTED. Limpieza y dictamen definitivos figuran al final.

## Gates finales

Todos los comandos se ejecutaron secuencialmente, jobs 1 y perfiles privados. Se conserva la primera ejecución F2 FAIL por Clippy y el segundo F4 FAIL por longitud del socket del arnés. El cierre usa [gates finales](F4-R5-EVIDENCE/gates-final/results.json), junto a [gates originales](F4-R5-EVIDENCE/gates/results.json), sin borrar intentos.

| Comando | Resultado final | Categoría / alcance |
|---|---|---|
| `python3 -B scripts/validate_f4.py` | PASS, repetición completa tras corrección | UNIT / AUTOMATED_AUDIO_FIXTURE / AUTOMATED_QML |
| `python3 -B scripts/validate_f35.py --plasma` | PASS; QML 18 passed y 9 capturas | AUTOMATED_QML / AUTOMATED_KWIN, perfil sintético |
| `python3 -B scripts/validate_f2.py` | PASS, 13 subgates; [estructura](F4-R5-EVIDENCE/gates-final/f2-structured.json) | UNIT / AUTOMATED_QML, consentimiento y regresiones |
| `python3 -B scripts/validate_f3.py --runtime-only --plasma` | PASS | AUTOMATED_KWIN; sin modificar F3 ni registrar tecla global |
| `python3 -B scripts/validate_f1b3b.py --qml` | PASS; unit-safe 197 passed, 1 ignored, 2 filtered | UNIT / AUTOMATED_QML; selección oficial, exclusiones externas heredadas |
| `cargo fmt --check` | PASS | Formato |
| `cargo clippy --locked --all-targets -- -D warnings` | PASS | Default |
| `cargo clippy --locked --features legacy-fixtures --all-targets -- -D warnings` | PASS | Legacy |
| `cargo build --locked --bin kde-assistant` | PASS, default restaurado al final | Build dev |

Ningún test nuevo R5 fue ignorado. Se verificó visualmente la captura propia de Voz a 1366×768; control y descripción encajan en el scroll existente. Los cambios de tema/teclado se comprobaron automáticamente. La intermitencia histórica F3 no se reprodujo en esta ejecución; eso no la elimina del registro histórico.

## Integridad y limpieza final

[Inventario final](F4-R5-EVIDENCE/final-source.json): misma rama/HEAD, un worktree, índice vacío, veinte informes previos sin cambios; hashes de fuentes y binario. Check nativo final de los recursos Piper PASS. [Limpieza](F4-R5-EVIDENCE/cleanup.json): 68 perfiles sintéticos de esta fase eliminados mediante allowlist bajo el root privado, incluidas copias privadas de modelos y configuraciones; no se tocaron recursos R3 ni el perfil habitual. Cada escenario espera y recoge sus hijos; además, 22 PIDs capturados en métricas finales están ausentes. Se retienen el baseline R4 y evidencia sin PCM para reproducibilidad/revisión. No se promete borrado forense.

[Escaneo de canarios privados](F4-R5-EVIDENCE/secret-scan.json): cero coincidencias de bearer de los perfiles de prueba en la evidencia exportada. No se copian config, SQLite, tokens ni audio. El [check final](F4-R5-EVIDENCE/final-build-check.json) confirma integridad, lectura primera/Ready/consecutiva por null sink y cinco ventanas OFF sin hijos de voz; las cifras están en BENCHMARKS. [Precheck AF_UNIX](F4-R5-EVIDENCE/socket-precheck.json): ruta manager de 108 bytes rechazada antes de crear perfil o iniciar Backend.

**Implementación: IMPLEMENTED. Rendimiento: IMPROVED. Seguridad y lifecycle: PASS en las pruebas automatizadas documentadas. Aceptación humana F4-R5: NOT_TESTED. F4 global: PARTIAL.** Revisión técnica pendiente; no consolidación, pruebas audibles o fases nuevas.
