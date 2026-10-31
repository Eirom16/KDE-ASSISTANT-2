# F4-R5 — Benchmarks integrados

La comparación principal alterna R4 frío y R5 rápido dentro de cinco parejas. Usa VoiceCoordinator/HTTP auténticos y reproducción PCM completa por PipeWire privado. No es el microbenchmark ONNX R4 ni una prueba audible. La serie principal final y sus datos individuales figuran abajo; las pruebas de expiración se registran aparte.

## Método

Mismo equipo N5000, voz `es_ES-sharvard-medium`, Piper 1.8.0/ORT1.31.0, CPUExecutionProvider, hablante 0, escala 1, PCM16LE mono 22050 Hz. Tres hilos intra-op, uno inter-op, spinning OFF. Texto corto idéntico a R4: «Hola. Soy KDE Assistant y esta es una prueba de voz en español.». La prueba de cambios de hablante pertenece a lifecycle y no entra en comparaciones de rendimiento/calidad.

Modelo frío significa sesión ONNX nueva; no se vacía page cache ni se fuerza frío de disco. No se cambia gobernador, afinidad, paquetes, compositor, volumen ni aplicaciones ajenas. No compiladores durante inferencia. Se registran carga del host, memoria, temperatura y procedencia del binario. Las primeras series agrupadas y la serie alternada final se separan; el entorno se reinició entre intentos y no se mezclan datos de arranques distintos para fabricar percentiles.

A: binario R4 exacto copiado antes de editar R5, worker desechable. B: primera síntesis con modo rápido (incluye carga y verificación). C: siguiente solicitud con modelo Ready. D: tercera consecutiva. E: modelo expirado y recarga. F: LLM mock en streaming con demora conocida, comparación con modo apagado. G: cancelar preparación. H: desactivar y liberar. El modo rápido no se activa al iniciar perfil; cada preparación proviene de lectura explícita o autolectura de turno mock.

El sondeo observa Speaking después de PCM completo; la conexión null se mide aparte. `trace.pcm_complete_received_ms` y `player_spawn_ms` son relojes del coordinador; `fast.ready_ms` mide carga/preparación hasta confirmación, `last_engine_ms` mide síntesis dentro del worker. No se suman percentiles independientes. La disponibilidad de respuesta HTTP final y la escritura final del mock son referencias distintas, ambas identificadas.

CPU incluye Backend, hijos ya recogidos y CPU acumulada de hijos todavía vivos; sin este último término se subestimaría el worker preparado. RSS/PSS se muestrean desde `/proc`; no son un límite duro del allocator. Periodos de reposo sin polling se separan del ensayo de TTL con polling, y cero ticks significa por debajo de la resolución del contador, no CPU matemáticamente nula.

Se informa n, mediana, p95 por rango más cercano, min/max y desviación muestral. Con n=5, p95 es el máximo observado, no una garantía de cola poblacional. Una observación no recibe percentiles. La latencia acústica y la calidad perceptiva de R5 permanecen NOT_TESTED.

## TTL y memoria

Se evalúan 15/30/60 s. El contador comienza/reinicia al completar síntesis, de modo que parte transcurre durante reproducción. Una consulta de estado no lo reinicia. Se mide expiración real, procesos recogidos y descriptores liberados. El candidato inicial propuesto es 30 s, revisable tras evidencia y revisión: permite una ventana de diseño de veinte segundos entre solicitudes, a diferencia de 15 s; 60 s retendría aproximadamente el doble de tiempo. Esa ventana es un criterio de diseño, no una distribución medida de consultas del usuario ni un ensayo real de conversación de veinte segundos.

La envolvente R4 de 256 MiB sustenta los márgenes de admisión (3×) y alerta PSI con margen reducido (2× ese margen); el límite observado de RSS del hijo (2× envolvente) es un monitor a 1 Hz. Se conservan rechazo ante métricas inválidas, buffers de 16 MiB y límites de texto. No se fuerza OOM ni se certifican cgroups limitados. La descripción QML se actualiza a aproximadamente 310 MB, orientativa, según el crecimiento de arena observado al reutilizar.

## Series que no se confunden

El primer smoke y el primer lifecycle no alcanzaron Ready; el primero utilizó la ruta fría. El primer ensayo QML mezclaba selecciones de sesión y se corrigió su sincronización. La primera comparación alternada se interrumpió por cancelación y se conserva incompleta. El snapshot posterior prueba un rechazo de política con PSI 0,17% y unos 4,76 GiB disponibles; motivó revisar el criterio excesivamente estricto de PSI aislado. Ninguno de esos intentos se rellena con muestras de otra serie.

Las cifras anteriores R4 —0,991 s de segundo PCM en experimento y 253–256 MiB— no se presentan como mediciones de producto R5. Tampoco T1 R3 (~8 s) es una observación humana de esta nueva ruta. Las comparaciones finales se enlazan con sus datos brutos y hashes en el cierre de esta entrega.

## Comparación alternada final (binario Notify)

Cinco parejas, veinte lecturas. Datos completos: [paired-final.json](F4-R5-EVIDENCE/paired-final.json); estadísticos y unidades: [paired-summary.json](F4-R5-EVIDENCE/paired-summary.json). Cada fila tiene cinco muestras; p95 es el máximo por el tamaño de la serie.

| Caso | Speaking p50 / p95 (s) | min–max (s) | SD (s) | CPU total p50 (s) | PCM completo p50 (s) |
|---|---:|---:|---:|---:|---:|
| A: R4 frío | 7.280 / 7.865 | 7.170–7.865 | 0.277 | 9.26 | 7.240 |
| B: R5 primera lectura | 7.850 / 7.908 | 7.741–7.908 | 0.068 | 9.66 | 7.767 |
| C: R5 Ready | 1.696 / 1.712 | 1.637–1.712 | 0.033 | 3.46 | 1.606 |
| D: R5 consecutiva | 1.677 / 1.702 | 1.651–1.702 | 0.023 | 3.46 | 1.598 |

Tiempos individuales hasta Speaking (s):

- r4-cold-cold: 7.865, 7.331, 7.280, 7.170, 7.262.
- r5-fast-first: 7.850, 7.741, 7.818, 7.908, 7.901.
- r5-fast-ready: 1.637, 1.657, 1.712, 1.696, 1.707.
- r5-fast-consecutive: 1.651, 1.659, 1.677, 1.701, 1.702.

El camino Ready reduce la mediana técnica aproximadamente un 76,7% respecto de R4 frío. El primer uso R5 sigue pagando carga y verificación y resulta algo más lento; no hay promesa de inicio audible de un segundo. No se suma p50 de etapas para inferir latencia total.

Las veinte salidas son PCM16LE mono 22.050 Hz, con contador terminal válido y sin frames sobrantes aceptados. Duraciones observadas: R4 3,541–3,738 s; R5 primera 3,529–3,657 s; Ready 3,541–3,646 s; consecutiva 3,587–3,750 s. La variación de duración del sintetizador se conserva; ni estos contadores ni hashes distintos prueban equivalencia perceptiva. No se cambia voz, hablante, escala ni parámetros acústicos. Escucha humana de R5: NOT_TESTED.

### Reposo y RAM de esta serie

- r5-fast-first: RSS máximo del worker 237.2 MiB; PSS máximo 226.3 MiB; Backend RSS máximo 30.8 MiB.
- r5-fast-ready: RSS máximo del worker 261.5 MiB; PSS máximo 250.6 MiB; Backend RSS máximo 31.5 MiB.
- r5-fast-consecutive: RSS máximo del worker 297.0 MiB; PSS máximo 286.0 MiB; Backend RSS máximo 31.6 MiB.

Cinco ventanas de siete segundos sin polling: Piper cero ticks en las cinco; Backend CPU individual (s): 0.02, 0.01, 0.01, 0.00, 0.01. RSS retenido máximo 297.0 MiB; PSS 286.0 MiB. Desactivar recogió cada worker. La corrección Notify reduce el gasto observado del Backend frente a los 0,29–0,36 s/7 s del sondeo previo; son series diagnósticas distintas y no una prueba causal aislada de todas las condiciones del host.


## F: preparación durante LLM mock

Cinco turnos por modo, demora sintética de nueve segundos, streaming habitual y una única llamada por turno. Texto persistido idéntico antes/después de la lectura en las diez respuestas. [Datos warm](F4-R5-EVIDENCE/mock-warm-final.json), [cold](F4-R5-EVIDENCE/mock-cold-final.json) y [resumen](F4-R5-EVIDENCE/mock-summary.json). Referencia: escritura final del mock; la respuesta HTTP final se registra aparte.

| Modo | Final → Speaking p50 / p95 (s) | min–max (s) | SD (s) | CPU total p50 (s) |
|---|---:|---:|---:|---:|
| warm | 1.695 / 1.718 | 1.676–1.718 | 0.018 | 9.52 |
| cold | 6.979 / 11.974 | 6.911–11.974 | 2.396 | 9.10 |

Individuales (s):
- warm: 1.695, 1.676, 1.716, 1.690, 1.718.
- cold: 6.979, 6.911, 6.932, 11.974, 10.425.

Esta comparación mock es agrupada: warm precedió a cold y las dos últimas muestras cold tienen mayor demora. Se conserva la dispersión y las condiciones del host en cada registro; no se presenta como ensayo aleatorizado ni se descartan observaciones lentas. La comparación alternada A–D sigue siendo la evidencia principal.

## E / H: cinco expiraciones productivas de 30 s

[Datos TTL30](F4-R5-EVIDENCE/ttl30-final.json). Los cinco workers se recogieron, cero ticks de CPU del worker durante el tramo ocioso, y descriptores Backend 19 → 15 en cada caso. La expiración se observó 26.163, 26.263, 26.180, 26.350, 26.084 s después de terminar la reproducción; el TTL comenzó al terminar síntesis, aproximadamente cuatro segundos antes. El polling continuo no renovó el lease.

Recarga posterior hasta Speaking (s): 10.311, 13.331, 9.376, 12.285, 8.355. n=5, p50 10.311, p95 13.331, min 8.355, max 13.331, SD 2.051. Son recargas frías reales con mayor dispersión; no se sustituyen por la cifra Ready ni se mezclan con A–D.

### Candidatos 15 y 60 s (una observación cada uno)

[Datos candidatos](F4-R5-EVIDENCE/ttl-candidates-final.json). No se calculan percentiles con n=1.

| TTL (s) | Espera tras reproducción hasta recogida (s) | RSS / PSS Ready (MiB) | CPU worker ocioso (ticks) | Recarga → Speaking (s) |
|---|---:|---:|---:|---:|
| 15 | 11.160 | 239.6 / 228.8 | 0 | 9.172 |
| 60 | 56.316 | 237.8 / 227.0 | 0 | 9.019 |

Ambos recogieron el worker y bajaron descriptores 19 → 15 pese al polling. La repetición primaria de 30 s tiene n=5; los candidatos tienen sólo comprobación funcional acotada. Se propone 30 s para evitar duplicar la retención de 60 s conservando más margen que 15 s. No se ha estudiado el patrón de uso humano ni se declara un óptimo universal.

### Coexistencia QML

Una secuencia real con Main.qml offscreen (n=1 por posición), perfil privado y null sink: Speaking 7.767, 1.649, 1.643 s (primera, Ready, consecutiva). Hijos combinados —incluidos Qt/reproductor/worker— RSS máximo 382.6 MiB, PSS 325.6 MiB. Son máximos muestreados, no suma de máximos independientes. El consumo del navegador/compositor no se atribuyó por proceso: se conservó MemAvailable/carga/PSI global sin inspeccionar perfiles personales. No certifica experiencia acústica o rendimiento de compositor.

### Huella simultánea de hijos en A–D

| Caso | RSS hijos p50 / máximo (MiB) | PSS hijos p50 / máximo (MiB) |
|---|---:|---:|
| r4-cold-cold | 230.8 / 234.1 | 223.0 / 226.2 |
| r5-fast-first | 244.0 / 244.5 | 228.3 / 228.7 |
| r5-fast-ready | 266.3 / 268.8 | 250.7 / 253.0 |
| r5-fast-consecutive | 275.1 / 304.3 | 255.5 / 288.5 |

Son máximos de sumas simultáneas muestreadas de hijos del Backend; incluyen el reproductor cuando coexiste con el modelo. No incluyen al daemon PipeWire privado ni al compositor/navegador.

## Verificación del binario final y modo apagado

La serie principal se obtuvo con el binario registrado en `benchmark-binary.sha256`. El único cambio Rust posterior fue la forma `while let` del recorrido del watcher, conservando sus condiciones y efectos; también se actualizó el texto QML a 310 MB. Los hashes del binario final y las fuentes están en [final-source.json](F4-R5-EVIDENCE/final-source.json). No se mezclan observaciones de binarios distintos para calcular percentiles.

[Comprobación final](F4-R5-EVIDENCE/final-build-check.json): integridad completa PASS; una secuencia primera/Ready/consecutiva dio 7.782, 1.659, 1.651 s hasta Speaking en null sink. Son observaciones únicas de cada posición, sin p50/p95. Todos los hijos propios se recogieron al cerrar.

Con Voz rápida apagada, cinco ventanas de siete segundos registraron CPU Backend (s): 0.00, 0.00, 0.01, 0.00, 0.00; p50 0.00, p95 0.01. Cero hijos de voz en las cinco; RSS Backend máximo 30.5 MiB y PSS 27.6 MiB. No se declara CPU exactamente nula por debajo de la resolución de ticks. No hubo modelos preparados al iniciar el perfil.
