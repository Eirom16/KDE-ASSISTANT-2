# F4-R2 — Benchmark de habla española y worker nativo

## Alcance y condiciones

Un hablante, cuatro grabaciones voluntarias, referencias confirmadas por separado: 40 palabras únicas, 43 segundos de WAV (incluido silencio). Cada toma se autorizó antes de abrir el micrófono. No es una evaluación estadística de todos los acentos, ruido o entornos. No constituye aceptación del PTT de la aplicación.

WAV idéntico por muestra para los tres motores: PCM16 LE, mono, 16 kHz. Referencias propias CC0-1.0; la autorización de evaluación **no** licencia la redistribución del audio personal. WAV, hipótesis y logs de motores se mantuvieron fuera de Git, en directorios 0700/archivos 0600; ya se eliminaron al cerrar la comparación offline. Los resúmenes sanitizados permanecen. [Autorizaciones y capturas](F4-R2-EVIDENCE/corpus-plan.json).

Whisper tiny multilingüe existente: 77.691.713 B, SHA256 `be07e048e1e599ad46341c8d2a135645097a538221678b7acdd1b1919c6e1b21`, MIT. Vosk small español 0.42 y runtime 0.3.45: descarga nuevamente autorizada **sólo para evaluación offline**, Apache-2.0 según metadatos oficiales. [Inventario, hashes y límites de verificación](F4-R2-EVIDENCE/vosk-download.json). No pip, instalación global ni nueva descarga de Whisper, CMake, voces o dependencias.

La comparación española usó el binario dev previo a la integración nativa; Rust O0, C++ RelWithDebInfo/O2, CPU SSE/repack, sin AVX/AVX2/CUDA. La integración recompiló dependencias locales ya disponibles; no se ha hecho release/O3 de Whisper. No se atribuye una mejora de Whisper a la optimización posterior de SHA-256. La caché CMake actual conserva RelWithDebInfo, GGML_NATIVE=ON y AVX/AVX2/AVX512=OFF.

Whisper: idioma es, 3 hilos, temperature_inc=0.2; completo audio_ctx=0. Reducido experimental: duración + 1 s redondeada a múltiplos de 256: 512/768/768/1024. Nunca 256 para frases de 8–15 s. Vosk: sin gramática restringida, bloques 200 ms, OMP/OPENBLAS=1. No se adaptó el motor a las referencias.

Tres inferencias por combinación, secuenciales, sin compiladores concurrentes. Orden completo/reducido/Vosk salvo aplicaciones, cuyo orden se invirtió. No se cerraron aplicaciones ajenas. Las métricas incluyen carga del host, no prometen aislamiento CPU total. Carga fría significa proceso/contexto nuevo con page cache del SO sin purgar. Inferencias repetidas no agregan hablantes ni muestras nuevas.

Percentiles por rango más próximo, n=3: p95 es el máximo descriptivo, no un SLO. Se registran min/max, desviación estándar poblacional y MAD. No se mezclan duraciones en un percentil de corpus.

## Precisión y latencia emparejadas

WER estricto normaliza NFC/caso/puntuación; conserva acentos y formatos numéricos. S/D/I y checks de cantidades/apps/final de frase se informan por separado. Corpus puntuado con run0 de cada WAV único, sin triplicar referencias. No hubo variación textual entre las tres repeticiones propias de cada motor ni vacíos injustificados.

| Muestra / palabras | Motor / contexto | WER | S/D/I | Inferencia p50 / p95, ms | min–max, ms | σ / MAD, ms |
|---|---|---:|---|---:|---:|---:|
| short / 4 | whisper_full / 0 | 0.00% | 0/0/0 | 36984.625 / 37019.434 | 31376.644–37019.434 | 2651.870 / 34.809 |
| short / 4 | whisper_reduced / 512 | 0.00% | 0/0/0 | 10950.977 / 11134.070 | 10820.496–11134.070 | 128.615 / 130.481 |
| short / 4 | vosk_small_es / None | 0.00% | 0/0/0 | 1042.443 / 1048.652 | 1038.830–1048.652 | 4.056 / 3.613 |
| applications / 7 | vosk_small_es / None | 85.71% | 3/0/3 | 1404.221 / 1425.245 | 1400.314–1425.245 | 10.949 / 3.907 |
| applications / 7 | whisper_reduced / 768 | 14.29% | 1/0/0 | 15637.487 / 17779.583 | 14746.197–17779.583 | 1272.985 / 891.290 |
| applications / 7 | whisper_full / 0 | 14.29% | 1/0/0 | 37425.023 / 37436.515 | 36576.480–37436.515 | 402.743 / 11.492 |
| numbers / 8 | whisper_full / 0 | 87.50% | 4/3/0 | 37573.070 / 38228.222 | 32121.380–38228.222 | 2737.469 / 655.152 |
| numbers / 8 | whisper_reduced / 768 | 100.00% | 4/4/0 | 18112.086 / 18289.986 | 17244.629–18289.986 | 456.667 / 177.900 |
| numbers / 8 | vosk_small_es / None | 0.00% | 0/0/0 | 1339.764 / 1368.379 | 1338.349–1368.379 | 13.835 / 1.415 |
| long / 21 | whisper_full / 0 | 14.29% | 2/0/1 | 38319.843 / 38353.461 | 33344.726–38353.461 | 2353.257 / 33.618 |
| long / 21 | whisper_reduced / 1024 | 14.29% | 2/0/1 | 25694.620 / 25795.248 | 25650.789–25795.248 | 60.475 / 43.831 |
| long / 21 | vosk_small_es / None | 4.76% | 1/0/0 | 2529.766 / 2572.316 | 2505.008–2572.316 | 27.797 / 24.758 |

| Motor | Errores / 40 palabras | S/D/I | WER observado |
|---|---:|---|---:|
| Vosk small es | 7 / 40 | 4/0/3 | 17,5% |
| Whisper tiny completo | 11 / 40 | 7/3/1 | 27,5% |
| Whisper tiny reducido | 12 / 40 | 7/4/1 | 30,0% |

Aplicaciones fue el punto débil de Vosk: Firefox conservado, Dolphin/LibreOffice incorrectos; 85,71% WER en esa muestra. Whisper conservó Dolphin/LibreOffice pero falló Firefox. No se declara ganador universal por el WER agregado.

Números: Vosk conservó las tres cantidades 23/105/2026. Completo produjo cantidades equivocadas, no sólo cifras en lugar de palabras; checks 0/3. Reducido conservó 2026 pero combinó las primeras cantidades; 1/3. El WER estricto no se reescribe para favorecer ningún motor.

Muestra larga: Vosk tuvo una sustitución esta→esa; ambos Whisper, dos sustituciones y una inserción. Ninguno omitió el final «sin cambiar mis archivos». Esta frase no demuestra ausencia general de truncamiento ni estabilidad con ruido.

Fuentes sanitizadas por muestra: [short](F4-R2-EVIDENCE/short-summary.json), [applications](F4-R2-EVIDENCE/applications-summary.json), [numbers](F4-R2-EVIDENCE/numbers-summary.json), [long](F4-R2-EVIDENCE/long-summary.json). [Agregado de referencias únicas](F4-R2-EVIDENCE/corpus-summary.json).

## Etapas y recursos del comparador

La conversión aquí es lectura/procesamiento del WAV canónico; no se atribuye a un resampler separado. PipeWire negoció 16 kHz al capturar. El coste de conversión de captura física no se aisló. La creación de estado y extracción de texto están en los JSON; disponibilidad interna caliente no incluye captura ni transporte QML. No se suman percentiles independientes para inventar un extremo a extremo.

| Muestra | Motor | Carga de contexto n=1, ms | Conversión WAV, ms | Texto caliente p50 / p95, ms | Pico RSS / PSS, KiB | CPU proceso, s |
|---|---|---:|---:|---:|---:|---:|
| short | whisper_full | 144.568 | 62.035 | 37013.180 / 37045.359 | 227188 / 223070 | 309.48 |
| short | whisper_reduced | 257.279 | 63.189 | 10974.833 / 11161.375 | 157016 / 152866 | 96.09 |
| short | vosk_small_es | 723.949 | 0.257 | 1123.535 / 1138.863 | 165236 / 154093 | 4.41 |
| applications | vosk_small_es | 617.459 | 0.054 | 1484.191 / 1511.437 | 174956 / 163781 | 5.44 |
| applications | whisper_reduced | 116.565 | 84.500 | 15662.046 / 17802.660 | 171272 / 167117 | 140.61 |
| applications | whisper_full | 113.017 | 84.239 | 37447.860 / 37465.140 | 227800 / 223645 | 326.89 |
| numbers | whisper_full | 117.110 | 85.037 | 37601.652 / 38251.642 | 229488 / 225327 | 314.61 |
| numbers | whisper_reduced | 113.762 | 85.494 | 18137.675 / 18308.866 | 169864 / 165744 | 155.70 |
| numbers | vosk_small_es | 618.650 | 0.052 | 1418.865 / 1451.615 | 175524 / 164444 | 5.24 |
| long | whisper_full | 112.889 | 118.868 | 38349.606 / 38385.500 | 227692 / 223601 | 320.63 |
| long | whisper_reduced | 114.554 | 119.614 | 25722.792 / 25815.665 | 189632 / 185578 | 224.29 |
| long | vosk_small_es | 616.590 | 0.080 | 2609.887 / 2655.469 | 191036 / 179763 | 8.76 |

CPU de proceso muestreada incluye su carga y las tres inferencias; Vosk además pausa un segundo con modelo cargado. No comparar esa columna como CPU por inferencia. Vosk registra CPU por inferencia explícita y reposo cargado: 0–1 ticks por aproximadamente un segundo (HZ=100), RSS 150932–155220 KiB, PSS 139842–144140 KiB. La precisión de esa breve observación no permite concluir consumo cero. RSS/PSS incluye el intérprete del comparador Vosk; éste no forma parte del producto Rust.

## Coste real del worker Rust y corrección conservadora

El primer worker nativo reprodujo los cuatro textos del comparador pero añadió segundos de hashing dev sin optimizar. Sobre el mismo WAV largo se midieron tres procesos nuevos por configuración, separados de la comparación de precisión anterior. Misma inferencia/configuración/binarios nativos salvo optimización de la dependencia Rust `sha2`:

```
[profile.dev.package.sha2]
opt-level = 3
```

No cambia los algoritmos de reconocimiento, hashes esperados, límites, timeouts ni cancelación. Aplicación Rust sigue O0; no se hizo release ni se optimizó todo C++. Build incremental de esta corrección: 43,46 s, jobs=2, recursos disponibles comprobados, sin compilaciones paralelas pesadas. Esta es una mejora de la preparación y verificación; no una nueva cifra de precisión.

| Configuración | Etapa | n | p50 / p95 pared, ms | min–max, ms | σ / MAD, ms |
|---|---|---:|---:|---:|---:|
| native-before | preflight | 3 | 2896.090 / 2899.437 | 2891.263–2899.437 | 3.355 / 3.347 |
| native-before | worker | 3 | 7295.100 / 7472.880 | 7277.992–7472.880 | 88.116 / 17.108 |
| native-after | preflight | 3 | 214.998 / 286.533 | 214.480–286.533 | 33.845 / 0.518 |
| native-after | worker | 3 | 3758.350 / 3812.041 | 3739.467–3812.041 | 30.744 / 18.883 |

Pared del monitor: muestreo 50 ms y comprobación de procesos, con cuantización/sobrecoste; preparación e inferencia internas tienen reloj de alta resolución. `preflight` sólo verifica lib/modelo, sin dlopen ni captura. `worker` incluye lectura WAV, verificación, carga, inferencia y liberación; **no incluye el preflight independiente ni latencia PTT/QML**. El worker verifica de nuevo; no conserva modelos en RAM entre operaciones.

| Configuración | Conversión p50, ms | Verificación + carga p50, ms | Inferencia + liberación p50, ms | CPU worker p50, s | Pico RSS / PSS worker, KiB |
|---|---:|---:|---:|---:|---:|
| native-before | 163.564 | 4510.522 | 2564.472 | 7.278 | 165168 / 162310 |
| native-after | 164.575 | 907.637 | 2645.213 | 3.756 | 168600 / 165741 |

[Antes](F4-R2-EVIDENCE/native-before.json) y [después](F4-R2-EVIDENCE/native-after.json) incluyen comandos, hashes de binarios, CPU, carga, RSS/PSS y PID recogido. El worker de producción, sin el lector WAV del benchmark, verificó paridad en las cuatro muestras tras la optimización: 1992,465 / 2362,149 / 2321,234 / 3502,306 ms respectivamente, **n=1 por muestra**, sin fabricar p50/p95 para esos valores. [Log](F4-R2-EVIDENCE/worker-native-final.log).

Cancelación confirmada después de entrar en inferencia y observar progreso CPU: Vosk 12,479 ms hasta reaping, n=1; Whisper 15,416 ms, n=1. Timeout, modelo corrupto y ninguna creación en HOME/TMP/XDG del worker: PASS. No equivale a pulsar Cancelar con micrófono físico; tampoco es un presupuesto garantizado de cancelación.

## Interpretación

STT_SELECTED **provisional: Vosk small español 0.42 / runtime 0.3.45** para dictado editable inicial, por combinación de latencia, cantidades y WER observado. El error en aplicaciones es material y queda abierto. Whisper sigue disponible y sigue siendo el default de configuración; no se cambió el perfil personal ni se retuvieron recursos como instalación productiva.

Comparaciones inválidas: tonos F4-R frente a WER humano; dev/release no equivalentes; un contexto fijo 256 frente a esta ventana proporcional; inferencia caliente frente a worker frío; sumar percentiles; tratar tres repeticiones como tres personas. Silencio/ruido sintéticos anteriores comprueban coste/comportamiento, no español.

Primera prueba PTT en aplicación: usuario confirmó que funcionó bastante rápido y pudo editar; evidencia cualitativa, sin asignar segundos ni puntuar el texto editado. Cancelar físico posterior confirmado por el usuario y registro de procesos. NOT_TESTED: latencia completa cronometrada, habla con ruido y otros acentos, Piper audible, wake. No se fija presupuesto numérico de usabilidad sin la prueba completa y revisión humana. La aplicación no queda certificada como manos libres.

La primera prueba PTT no dejó transiciones finales del launcher: latencia cuantitativa LIVE_PTT NOT_TESTED. Se conserva únicamente la valoración humana de rapidez y edición y el snapshot autenticado previo, sin convertirlos en segundos o percentiles. El registro del helper se corrigió y validó sin volver a abrir el micrófono.

## Observación física posterior de Cancelar

Segunda ventana: tres ciclos físicos, dos recording→idle y uno que transcribió. Cero peticiones LLM/mensajes y todos los procesos recogidos; usuario confirmó Cancelar correcto. El ciclo auxiliar transcribing→transcript_ready fue 1451,779 ms observado por polling de 100 ms, n=1: no es tiempo de clic a UI ni percentil. Cancelar no tiene timestamp del clic; su latencia numérica sigue NOT_TESTED. No se calcula WER sin referencia confirmada de ese ciclo. [Registro](F4-R2-EVIDENCE/live-ptt-cancel.json).

La comparación y las pruebas físicas están cerradas. Corpus, hipótesis/logs privados, Vosk y perfiles temporales se eliminaron; los comandos conservados documentan lo ejecutado y requieren nuevos recursos consentidos para repetirse. [Cleanup](F4-R2-EVIDENCE/cleanup-final.json).
