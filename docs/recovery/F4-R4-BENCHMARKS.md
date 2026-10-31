# F4-R4 — Benchmarks offline de latencia Piper

Fecha: 2026-10-19. **AUTOMATED_AUDIO_FIXTURE**, sin micrófono ni altavoces. Son tiempos de software/IPC; inicio acústico R4: **NOT_TESTED**.

## Método

Mismo Pentium Silver N5000 x86_64, voz `es_ES-sharvard-medium` (modelo SHA-256 `40febfb1679c69a4505ff311dc136e121e3419a13a290ef264fdf43ddedd0fb1`), Piper 1.8.0, ORT 1.31.0, CPUExecutionProvider, tres hilos intra-op, uno inter-op, spinning desactivado, optimización `all`, hablante 0 y duración 1. No cambios de modelo, cuantización, velocidad, semilla, precisión o proveedor para acelerar el resultado.

Dos textos sintéticos, sin voz personal:

- Corto/T1: «Hola. Soy KDE Assistant y esta es una prueba de voz en español.»
- Varias frases: «Hola. Esta es una prueba de voz en español. La respuesta debe conservar el inicio y el final sin cortes. Terminamos la prueba sin cambiar archivos.»

Cinco repeticiones por configuración/texto (20 workers en la comparación principal), alternando el orden baseline/candidato por repetición. Cada worker crea una sesión ONNX nueva: **cold start de proceso/modelo, con page cache del SO sin vaciar**. No es una prueba de disco frío. El baseline incluye el preflight original; el candidato lo suprime, pero verifica los mismos recursos dentro del worker. El worker ya emitía PCM por frases antes de R4; el coordinador original esperaba EOF, igual que la ruta final conservadora.

RSS/PSS del worker muestreados cada 100 ms; detector de compiladores cada segundo y entre operaciones. CPU mediante `RUSAGE_CHILDREN`; en esta tabla corresponde sólo al worker, **excluye CPU del preflight**. La prueba de coordinador separada incluye procesos y CPU del Backend/hijos. El monitor no compite mediante inspección continua de `/proc`. No se compila durante inferencia, se cambia afinidad/frecuencia, se para software ajeno ni se vacían caches globales. Carga de escritorio no exclusiva; se conserva loadavg, CPU del monitor y datos individuales.

p50/p95 por rango más próximo; con n=5 el p95 es el máximo observado, no una estimación fiable de la cola poblacional. Se proporcionan desviación estándar poblacional y MAD. Las unidades dependen de la columna, no del nombre de una función estadística reutilizada.

## Comparación instrumentada: PCM completo disponible

Esta serie se capturó antes del endurecimiento final que conserva la identidad tomada al terminar cada hash. No se modificaron voz, inferencia, formato ni estrategia de reproducción; los dos checks finales de identidad se incluyen en la repetición posterior del coordinador. El hash del binario medido está en cada inventario. No se etiqueta esta serie como una medición del binario posterior al endurecimiento.

Incluye preflight + worker en baseline y sólo worker verificado en R4. Ninguna fila representa audio oído.

| Texto / ruta | n | p50 s | p95 s | mín–máx s | desviación s | MAD s |
|---|---:|---:|---:|---:|---:|---:|
| short / baseline | 5 | 8.308 | 8.647 | 7.284–8.647 | 0.565 | 0.339 |
| short / buffered | 5 | 7.034 | 7.815 | 6.845–7.815 | 0.356 | 0.179 |
| multi / baseline | 5 | 8.700 | 9.912 | 8.547–9.912 | 0.500 | 0.131 |
| multi / buffered | 5 | 8.474 | 9.308 | 8.095–9.308 | 0.428 | 0.279 |

Diferencias emparejadas baseline menos candidato (no confundir con restar dos medianas):

- short: mediana **0.832 s**, rango 0.439–1.285 s; 5/5 diferencias positivas.
- multi: mediana **0.452 s**, rango 0.180–0.604 s; 5/5 diferencias positivas.

El preflight evitable por sí solo ronda medio segundo. Las diferencias adicionales no se atribuyen íntegramente al código: incluyen variación de carga, síntesis estocástica y creación de sesión. La mejora es **limitada** y no resuelve por completo el arranque frío.

| Texto / ruta | CPU worker p50 / p95 s | RSS pico p50 / p95 MiB | PSS pico p50 / p95 MiB |
|---|---:|---:|---:|
| short / baseline | 9.312 / 9.522 | 230.0 / 231.6 | 222.8 / 224.4 |
| short / buffered | 8.694 / 9.390 | 230.6 / 231.0 | 223.5 / 223.8 |
| multi / baseline | 12.131 / 13.117 | 240.4 / 241.5 | 233.3 / 234.3 |
| multi / buffered | 12.435 / 13.350 | 241.7 / 242.8 | 234.5 / 235.6 |

No se incorpora un coste residente nuevo al default. Se mantienen límites de PCM de 16 MiB, texto de 16.000 bytes y timeout de generación 90 s; no se aumentaron para esconder latencia.

## Ruta crítica del worker final, texto corto, n=5

Tiempos de cada fase en milisegundos. Carga/sesión y totales contienen fases anidadas; **no sumar indiscriminadamente las filas**.

| Fase | p50 ms | p95 ms | mín–máx ms |
|---|---:|---:|---:|
| Validación total | 537.265 | 547.141 | 532.412–547.141 |
| Hash modelo + JSON | 140.701 | 143.888 | 139.363–143.888 |
| Verificación runtime | 395.424 | 404.781 | 391.984–404.781 |
| Bootstrap Python, estimación reloj pared | 171.298 | 185.205 | 168.422–185.205 |
| Importación ORT | 749.333 | 861.154 | 740.207–861.154 |
| Importación Piper | 152.938 | 176.234 | 150.533–176.234 |
| Lectura/config lingüística | 0.387 | 0.433 | 0.347–0.433 |
| Creación sesión/carga ONNX | 4134.881 | 4831.983 | 4066.728–4831.983 |
| Fonemización | 9.308 | 9.455 | 9.118–9.455 |
| Conversión IDs | 0.062 | 0.077 | 0.057–0.077 |
| Inferencia total | 1082.275 | 1127.913 | 1040.562–1127.913 |
| Conversión a PCM16 | 0.219 | 0.334 | 0.210–0.334 |
| Escritura IPC | 0.402 | 5.066 | 0.275–5.066 |
| Primer PCM desde entrada script Python | 5218.082 | 5951.868 | 5088.871–5951.868 |
| Síntesis/entrega desde inicio síntesis | 1093.885 | 1139.527 | 1053.419–1139.527 |

`python_bootstrap_wall_estimate_ms` cubre el intervalo entre preparar exec Rust y entrar al script tras imports estándar. Usa reloj de pared sólo para diagnóstico entre lenguajes: puede verse afectado por ajustes de reloj; no gobierna identidad, cancelación ni timeouts. Las demás duraciones usan relojes monotónicos. El tiempo de sesión incluye carga/deserialización y preparación de grafo/CPU; no existe aquí una separación ficticia del cargador interno ONNX.

El coordinador mide por separado spawn, primer stdout, EOF/exit del worker, solicitud/spawn del player y final del player. Los tiempos internos Python empiezan después del exec; no se equiparan sus relojes absolutos ni se confunden con el comienzo de una operación conversacional.

## PCM progresivo: medido y descartado para producción

Cinco pares por texto con el primer prototipo, binarios y métricas registrados en `paired-workers.json`. Disponibilidad inicial aproximada del prototipo:

| Texto | Primer PCM externo p50 / p95 s | Baseline espera PCM completo p50 / p95 s |
|---|---:|---:|
| short | 6.275 / 6.442 | 7.985 / 8.558 |
| multi | 6.141 / 7.008 | 9.113 / 9.847 |

Esos números describen disponibilidad, no continuidad. En el worker final del texto corto se mide déficit de suministro de **565.0–684.6 ms** si se empieza al primer chunk; la primera «Hola.» dura sólo unas décimas de segundo. `supply_gap_ms` compara tiempo de llegada de cada chunk con duración acumulada previa, sin dispositivo de sonido. Es una predicción de inanición, no audio escuchado. Basta para rechazar la premisa de streaming continuo sin un diseño adicional de prebuffer/planificación. No se lanzó por altavoces.

El protocolo experimental conserva orden, formato, límites, backpressure y recogida de ambos hijos. No se activa desde conversaciones. La ruta final espera PCM completo y no introduce esos huecos de generación durante reproducción.

## Modelo preparado: experimento, sin cambiar default

Cinco procesos distintos, cada uno carga una sola sesión y atiende dos solicitudes; espera un segundo entre ellas y vence a los tres segundos de inactividad después de la segunda. Rust posee stdin/stdout, timeout global, cancelación y wait. Sin Backend ni audio. El texto y las opciones son los mismos. Los tiempos de carga/importación de la segunda respuesta identifican recursos reutilizados, no una segunda carga; `load_per_request_ms=0` en esa respuesta.

| Solicitud | n | Primer PCM p50 / p95 s | PCM completo p50 / p95 s | desviación PCM completo s |
|---|---:|---:|---:|---:|
| Fría | 5 | 5.794 / 5.951 | 6.695 / 6.871 | 0.094 |
| Preparada | 5 | 0.109 / 0.124 | 0.991 / 1.029 | 0.020 |

Memoria retenida después de la segunda síntesis, muestras n=5: RSS 253.0–255.5 MiB; PSS 245.3–247.8 MiB. Los dos segundos de espera muestreados consumieron 2–2 ticks de CPU por worker (CLK_TCK=100: 0.02–0.02 s); no se afirma CPU exactamente cero. La memoria puede crecer por arenas/caches con la segunda síntesis; no son dos modelos cargados.

Todos terminaron por TTL con stdin del padre aún abierto y fueron recogidos. Tras esperar dos segundos, el resto hasta exit/wait fue 1096.5–1154.2 ms; incluye aproximadamente el segundo de TTL restante y cleanup, **no equivale al coste puro de liberar memoria**. CPU global de cada experimento (dos síntesis, espera y owner Rust) p50/p95 11.272/11.479 s; pico RSS worker p50/p95 254.2/255.5 MiB, PSS 246.5/247.8 MiB.

La cifra relevante para una futura ruta conservadora preparada es **PCM completo**, no los primeros ~100 ms de «Hola.». Persisten déficit entre chunks y arranque frío costoso. El experimento no implementa una preferencia de producto ni autoriza retención en RAM al iniciar KDE Assistant; TTL configurable e integración con sesión/turno quedarían sujetos a decisión/revisión.

## Resultado final después de endurecer la identidad

Se repitieron cinco pares alternados con el binario que conserva la identidad del hash hasta exec y la compara antes de cargar. El resto del pipeline, voz y opciones permaneció igual. No se mezcla esta serie con las anteriores. Es la comparación final de inicio de reproducción, mediante el mismo sondeo HTTP, PCM real y sumidero privado.

| Ruta | n | Speaking observado p50 / p95 s | mín–máx s | desviación / MAD s | CPU Backend+hijos p50 / p95 s |
|---|---:|---:|---:|---:|---:|
| baseline | 5 | 7.692 / 8.195 | 7.656–8.195 | 0.201 / 0.036 | 9.780 / 9.910 |
| buffered | 5 | 7.238 / 7.260 | 7.135–7.260 | 0.044 / 0.006 | 9.180 / 9.310 |

Ahorro emparejado mediano: **453.3 ms**, rango 396.7–1060.3 ms; 5/5 positivo. p95 sigue siendo sólo el máximo de cinco observaciones. Se confirma una mejora limitada: el cold start continúa cerca de siete segundos. No se midió inicio acústico.

| Ruta | RSS pico hijos p50 / p95 MiB | PSS pico hijos p50 / p95 MiB | enlace null sink p50 / p95 s | fin p50 / p95 s |
|---|---:|---:|---:|---:|
| baseline | 229.8 / 231.0 | 222.6 / 223.7 | 7.731 / 8.247 | 11.484 / 12.009 |
| buffered | 230.0 / 232.8 | 222.7 / 225.6 | 7.277 / 7.298 | 10.959 / 11.021 |

Trazas del coordinador final, n=5, milisegundos monotónicos relativos a la operación. Los tiempos de Python se mantienen separados en las series de instrumentación anteriores.

| Evento | p50 ms | p95 ms |
|---|---:|---:|
| Coste spawn worker | 1.359 | 1.588 |
| Primer stdout PCM | 6068.842 | 6106.961 |
| EOF worker | 7135.203 | 7178.884 |
| Exit/wait worker | 7135.534 | 7179.211 |
| PCM completo recibido | 7135.784 | 7179.478 |
| Spawn pw-play desde inicio | 7137.152 | 7180.736 |
| Exit/wait pw-play desde inicio | 10904.060 | 10997.512 |

Lifecycle final PASS, siete escenarios: cancelación inmediata 17.52 ms, preparación activa 18.01 ms, reproducción 6.06 ms, nueva reproducción después de cancelar, cambio de sesión 14.17 ms, autolectura mock (una llamada, texto sin cambios) y modelo ausente. Tiempos de cancelación individuales, no percentiles. Todos los hijos recogidos; reposo default 3.004 s sin hijos de voz y CPU observada 0,0 % de un núcleo. No se atribuye el cancel de Preparing a una subfase ONNX exacta.

Comprobación del experimento preparado tras el endurecimiento: **n=1, smoke**, primera respuesta PCM completo 6.997 s y segunda 1.017 s; TTL y reaping correctos. No se calculan percentiles de esta observación. Los percentiles n=5 de la segunda síntesis proceden del experimento anterior, identificado por versión y sin cambiar el código Python de generación.

[Comandos posteriores al endurecimiento](F4-R4-EVIDENCE/post-inode-commands.json), [serie final](F4-R4-EVIDENCE/playback-inode-safe-final.json), [lifecycle final](F4-R4-EVIDENCE/piper-inode-safe-lifecycle.json), [smoke preparado](F4-R4-EVIDENCE/warm-inode-safe-smoke.json), [hashes del binario/fuentes](F4-R4-EVIDENCE/post-inode-source-hashes.json). Nada de esta sección constituye LIVE_TTS ni utiliza permiso audible de T1.

## Coordinador anterior al endurecimiento y PipeWire privado

Comparación alternada adicional: cinco pares con Backend/perfil/SQLite nuevo por operación, puerto efímero, mismo texto T1, sin LLM ni micrófono. Cada operación acaba y recoge sus hijos antes de la siguiente. El reloj común observa el estado Speaking por HTTP cada 100 ms; **no demuestra que alguien haya oído audio**. El reproductor sólo conecta a `f4-tts-sink` del daemon privado. No es una simulación de Speaking sin PCM: todas las reproducciones terminan exit 0 y vuelven a idle.

| Ruta | n | Speaking observado p50 / p95 s | desviación / MAD s | conexión null sink p50 / p95 s | fin p50 / p95 s |
|---|---:|---:|---:|---:|---:|
| baseline | 5 | 7.436 / 7.443 | 0.004 / 0.003 | 7.473 / 7.480 | 11.138 / 11.249 |
| buffered | 5 | 7.013 / 7.021 | 0.047 / 0.008 | 7.051 / 7.058 | 10.706 / 10.858 |

Ahorro emparejado mediano hasta Speaking observado: **426.3 ms**, rango 415.0–519.6 ms, 5/5 positivo. El sondeo introduce hasta aproximadamente un intervalo de incertidumbre por observación. La conexión al sumidero incluye los comandos de enlace del harness y no representa latencia de un altavoz.

| Ruta | CPU Backend + hijos p50 / p95 s | RSS hijos pico p50 / p95 MiB | PSS hijos pico p50 / p95 MiB |
|---|---:|---:|---:|
| baseline | 9.440 / 9.510 | 229.9 / 230.5 | 222.3 / 222.8 |
| buffered | 8.970 / 9.050 | 230.8 / 231.9 | 223.3 / 224.4 |

Instrumentación interna de la ruta final, n=5, relojes monotónicos desde la operación (salvo coste de spawn aislado):

| Evento | p50 ms | p95 ms |
|---|---:|---:|
| Spawn worker (coste local) | 1.266 | 1.665 |
| Primer stdout PCM worker | 5882.561 | 5908.526 |
| EOF worker | 6925.267 | 6935.166 |
| Exit/wait worker | 6925.579 | 6935.488 |
| PCM completo recibido coordinador | 6925.832 | 6935.735 |
| Spawn player desde inicio operación | 6927.099 | 6936.957 |
| Exit/wait player desde inicio operación | 10674.847 | 10775.310 |

La serie anterior agrupada (primero cinco baseline y después cinco R4) también se conserva: baseline Speaking p50/p95 7.571/7.596 s; buffered Speaking p50/p95 7.278/8.302 s; tuvo dos esperas atípicas en R4. No se elimina ese resultado desfavorable de cola. El orden agrupado y la carga no exclusiva impiden atribuirlo sólo al código; motivó la comparación alternada, que tampoco garantiza una cola de latencia estrecha en uso futuro. No se mezclan ambos diseños en un p95 único.

[Evidencia alternada](F4-R4-EVIDENCE/playback-paired-final.json), [serie agrupada](F4-R4-EVIDENCE/playback-final-rerun.json). Ambas son AUTOMATED_AUDIO_FIXTURE. No se mide primer audio acústico y no se fabrica un percentil humano de la única T1 histórica.

Reposo default tras las pruebas de lifecycle: 3,003 s, sin hijos de voz, CPU observada 0,0 % de un núcleo, Backend RSS 29.888 KiB/PSS 25.913 KiB. Cancelaciones observadas: preparación 17,50/21,00 ms; playback 12,25 ms; cambio de sesión 12,18 ms. Son casos individuales, no p50/p95; los procesos se recogieron antes del retorno y una operación nueva completó después. Autolectura mock: una única llamada LLM loopback, cero prompts extra y texto persistido sin cambios.

## Integridad y calidad

PCM16 LE, mono, 22050 Hz, número de bytes par, no vacío y consistente con el contador del worker. Los runners conservan duración y hash por muestra, no archivos PCM/WAV. Las variaciones estocásticas de Piper impiden exigir un hash idéntico entre síntesis independientes. No se cambió normalización, fonemizador, voz, hablante, velocidad ni algoritmo de generación para obtener latencia menor. Esto no demuestra equivalencia audible R4: requiere nueva aceptación humana autorizada después de revisión.

Los datos detallados (CPU/RSS/PSS, duración, hash, formato, reaping, loadavg, dispersión y fases) están en [serie principal](F4-R4-EVIDENCE/final-buffered-rerun.json), [experimento preparado final](F4-R4-EVIDENCE/warm-experiment-final.json) y [prototipo progresivo](F4-R4-EVIDENCE/paired-workers.json). La exploración `baseline-workers.json`, con monitor más frecuente, no se mezcla estadísticamente con estas series; el intento interrumpido no se incluye. Tampoco se combinan los resultados R3 históricos con estas muestras.
