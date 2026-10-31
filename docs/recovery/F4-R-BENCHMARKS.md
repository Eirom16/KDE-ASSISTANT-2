# F4-R — Benchmarks y alternativas

**Rendimiento IMPROVED en experimentos offline; experiencia productiva sin aceptar.** No hay voz humana, micrófono, salida audible ni WER válido. Este documento distingue alternativas rápidas en señales sin habla de reconocimiento español usable.

## Condiciones y reproducción

Pentium Silver N5000, cuatro cores, CPU x86_64 sin AVX/F16C/FMA; toolchain y kernel en [toolchain](F4-R-EVIDENCE/toolchain.json). whisper-rs 0.16.0 / whisper.cpp 1.8.3, modelo tiny multilingüe existente de 77.691.713 B, SHA256 `be07e048e1e599ad46341c8d2a135645097a538221678b7acdd1b1919c6e1b21`. No se volvió a consultar ni modificar el modelo/config/SQLite del perfil personal.

C++ efectivo: CPU/repack, `-O2 -g -DNDEBUG -march=native`, WHISPER_DEBUG; Rust dev opt-level=0. GGML_BLAS/OPENMP/CUDA desactivados. Se conservan [cache CMake](F4-R-EVIDENCE/dev-cmake-cache.txt) y [flags](F4-R-EVIDENCE/dev-native-flags.txt). Native no significa AVX disponible; no se forzó una ISA incompatible.

Greedy best_of=0 (la biblioteca normaliza un decoder), idioma es, translate=false, no_context=true, suppress_blank=true, suppress_nst=false, temperature=0 y fallback incremental 0,2 salvo el caso explícito sin fallback. Contexto por defecto 0 significa usar los 1500 pasos del modelo; no significa cero trabajo. n_max_text_ctx default 16384 queda condicionado por el modelo de 448 y no_context; no se agregó historial entre operaciones.

Scripts no descargan recursos. Exigen rutas explícitas, perfiles privados, Cargo offline y ausencia de procesos de compilación antes/durante el muestreo. Sólo se ejecutó una inferencia/compilación pesada a la vez. El host conserva aplicaciones propias del usuario; no se suspendieron ni cambiaron sus prioridades. Hay actividad de navegador/compositor y variación de frecuencia/carga. Se registran loadavg y CPU de cada hijo; el sampler también consume CPU y las mediciones iniciales no cuantifican todo su coste. Son comparaciones de ingeniería descriptivas, no un laboratorio de CPU dedicada.

Entrada canónica: WAV PCM16 mono 16 kHz, exactamente tres segundos, generador propio CC0, seno 440 Hz/amplitud 0,15, ruido blanco determinista o ceros. Ambos motores reciben los mismos PCM de esos WAV; manifiestos con SHA en las carpetas canónicas/finales. Vosk procesa bloques de 200 ms y Whisper batch: diferencia intrínseca del motor, no una simulación de streaming Whisper. Las primeras dos reproducciones usaron el WAV de 48 kHz convertido por la referencia sinc Rust; están separadas de la comparación canónica a 16 kHz.

```text
cargo build --offline --locked -j 2 --bin kde-assistant --bin bench_voice
python3 scripts/benchmark_f4.py --model /var/tmp/kda-f4-tiny-jaan_09m/ggml-tiny.bin --runs 5 --output docs/recovery/F4-R-EVIDENCE/dev-default --rate 48000
python3 scripts/benchmark_f4.py --model /var/tmp/kda-f4-tiny-jaan_09m/ggml-tiny.bin --runs 5 --audio-ctx 256 --output docs/recovery/F4-R-EVIDENCE/dev-ctx256 --rate 48000
python3 scripts/benchmark_f4.py --model /var/tmp/kda-f4-tiny-jaan_09m/ggml-tiny.bin --runs 3 --rate 16000 --output docs/recovery/F4-R-EVIDENCE/dev-canonical-tone
python3 scripts/benchmark_f4.py --model /var/tmp/kda-f4-tiny-jaan_09m/ggml-tiny.bin --runs 3 --audio-ctx 256 --output docs/recovery/F4-R-EVIDENCE/dev-canonical-ctx256
```

Los comandos completos de hilos/afinidad/ruido/silencio están en cada `process.json`. El modelo tiny requiere elección explícita; su ruta privada histórica no es un default del runner. Los ejecutables y pesos se identifican por hash, no sólo por nombre.

## Qué comparaciones permiten concluir

| Comparación | Alcance válido / límite |
| --- | --- |
| Histórica F4: base 93,810 s vs tiny 39,891 s | No es selección definitiva: compilación concurrente en base, builds dev y conversiones/condiciones diferentes. No mide precisión española |
| Tiny O2 completo vs contexto 256, mismo WAV/config restante | Coste del contexto en ese workload, con variación de carga registrada. No conservación de precisión hablada |
| Vosk vs tiny canónicos a 16 kHz | Coste de motores ante el mismo PCM sintético; distintos algoritmos y loadavg. No multiplicador garantizado para habla real |
| Hilos/afinidad, n=3 | Exploratorio; no criterio suficiente para pinning global ni presupuesto p95 |
| O2 vs O3 / Rust dev vs release | No hay comparación numérica válida: el build optimizado quedó bloqueado |

## Whisper: inferencia, dispersión y memoria

Percentiles por rango más próximo; desviación estándar poblacional. n=3/5 es exploratorio: p95 coincide con el máximo en estos tamaños, sin validez de SLO ni intervalo de confianza. No se mezclan repeticiones de configuraciones distintas.

| Experimento | n | p50 / p95 (s) | min–max (s) | σ (s) | pico RSS/PSS muestreado (KiB) |
| --- | --- | --- | --- | --- | --- |
| dev-canonical-ctx256 | 3 | 5.468 / 5.636 | 5.359–5.636 | 0.114 | 131364 / 128256 |
| dev-canonical-tone | 3 | 40.833 / 40.954 | 38.094–40.954 | 1.321 | 225860 / 222739 |
| dev-ctx256 | 5 | 8.128 / 9.180 | 6.560–9.180 | 0.849 | 133332 / 130199 |
| dev-ctx256-t1 | 3 | 10.331 / 10.508 | 10.304–10.508 | 0.090 | 131012 / 128147 |
| dev-ctx256-t2 | 3 | 6.099 / 6.145 | 5.995–6.145 | 0.063 | 131384 / 128275 |
| dev-ctx256-t2-affinity | 3 | 5.878 / 6.030 | 5.845–6.030 | 0.080 | 148940 / 144772 |
| dev-default | 5 | 46.930 / 49.733 | 45.248–49.733 | 1.442 | 230072 / 226385 |
| dev-noise-ctx256 | 3 | 5.346 / 5.635 | 5.183–5.635 | 0.187 | 136348 / 133265 |
| dev-noise-no-fallback | 3 | 4.902 / 5.006 | 4.891–5.006 | 0.052 | 131360 / 128266 |
| dev-silence-ctx256 | 3 | 4.597 / 4.610 | 4.575–4.610 | 0.015 | 147080 / 142957 |

[Resumen completo](F4-R-EVIDENCE/whisper-summary.json) contiene MAD, CPU, carga/estado/conversión, loadavg y opciones. Cada carpeta conserva sus tiempos crudos y proceso. La carga fría significa contexto sin cargar en ese proceso, con page cache del OS sin purgar; n=1 por experimento. No confundir contexto caliente del benchmark con un modelo residente en el producto.

La entrada canónica con contexto completo dio 40,833 s p50; audio_ctx=256 dio 5,468 s. Los tokens finales del tono fueron ocho y un segmento en todas esas repeticiones. El tamaño de ventana acústica explica una gran parte del coste, pero no se puede inferir calidad española desde esa equivalencia. El recorte no se habilitó en producción. Tampoco se atribuye toda la diferencia de pared a un único factor del host: la CPU acumulada y el resto de condiciones están disponibles en JSON.

El procesamiento WAV canónico costó ~28–30 ms en Rust dev; la conversión 48→16 kHz costó ~1,0–1,15 s con la referencia sinc offline. Producción usa PipeWire y no esta lectura/resampling del benchmark. La inferencia completa sigue dominando; cargar bajo demanda cuesta décimas de segundo, sin justificar una copia grande permanente en RAM.

Con audio_ctx=256: un hilo dio 10,331 s, dos 6,099 s y tres 5,468 s en los casos canónicos. Afinidad 1,2 con dos hilos dio 5,878 s. No hay evidencia suficiente para fijar una afinidad global o concluir una mejora robusta por pinning; no se aplicó. Los threads del producto siguen limitados a tres.

Silencio y ruido del decoder directo produjeron cinco tokens/un segmento (`[Música]`) después de un fallback 0→0,2. Sin fallback, ruido produjo seis tokens (`(música)`) y p50 4,902 s frente a 5,346 s. El resultado cambió; no se eliminó fallback productivo por esa ganancia. El coordinador ya retorna texto vacío para PCM casi silencioso antes de cargar Whisper; el ensayo directo no representa ese filtro.

## Vosk autorizado: dos ejecuciones independientes

Vosk API 0.3.45, libvosk del wheel oficial manylinux x86_64 (7.173.758 B), modelo `vosk-model-small-es-0.42` (ZIP 39.817.833 B), ambos Apache-2.0 según las fuentes oficiales. Se extrajeron sólo la biblioteca/licencias y modelo a un directorio 0700, sin pip ni instalación. El wheel se verificó contra SHA de PyPI; el ZIP tiene tamaño y SHA registrado, sin afirmar una firma/hash upstream que no se obtuvo. [Inventario y cleanup](F4-R-EVIDENCE/vosk-download.json).

C API mediante ctypes por proceso de benchmark. OPENBLAS_NUM_THREADS=1/OMP_NUM_THREADS=1, sin dispositivos ni credenciales. ELF requiere libstdc++, libm, libgcc_s y libc. La ejecución completa en el N5000 es evidencia real de compatibilidad para este caso; no se certifica todo el modelo/ISA con sólo el nombre del wheel. El runtime empaquetado es antiguo: debe revisarse mantenimiento/actualización antes de distribuir una integración.

Cada ejecución hizo diez repeticiones de cada señal (30 inferencias). Se conservan por separado: [primera](F4-R-EVIDENCE/vosk-small-es/benchmark.json), [final](F4-R-EVIDENCE/vosk-final/benchmark.json). No se suman para fabricar un n=20 homogéneo.

| Señal, ejecución final | n | p50 / p95 (ms) | min–max (ms) | σ (ms) |
| --- | --- | --- | --- | --- |
| tone | 10 | 376.836 / 410.453 | 364.564–410.453 | 12.847 |
| silence | 10 | 351.020 / 379.941 | 344.606–379.941 | 10.812 |
| noise | 10 | 470.124 / 475.887 | 464.877–475.887 | 3.071 |

Carga de biblioteca 38.316 ms y modelo 664.569 ms (n=1 en esta ejecución). Pico RSS/PSS muestreado 160596/150042 KiB. CPU ticks muestreados 1551, HZ=100; puede faltar el tramo final entre muestras. CPU y pared del monitor se registran por separado. Reposo cargado: 2.004 s, 1 ticks, RSS/PSS 147740/137232 KiB: observación corta cuantizada, no promesa de CPU cero.

Las tres señales devolvieron siempre texto vacío. Eso es comportamiento correcto ante estos fixtures, no una prueba de que reconoce palabras españolas. Las condiciones de carga del host no fueron idénticas entre todos los procesos; no se presenta un multiplicador de aceleración de voz real. Latencia de texto con palabras, WER y RTF de habla: NOT_TESTED.

Cancelación: tras observar procesamiento de un bloque PCM se terminó el hijo conocido y se hizo wait; PID ausente al retornar. [Primera](F4-R-EVIDENCE/vosk-small-es/cancel.json), [final](F4-R-EVIDENCE/vosk-final/cancel.json). Es un experimento externo de proceso; no certifica integración de Vosk en VoiceCoordinator ni cancelación de un turno LLM. Whisper conserva su test opt-in con el supervisor productivo para cancelar/timeout y recoger el worker. F4-R añade un probe del mismo decoder: publica una marca atómica después de crear el estado, observa al menos dos ticks CPU dentro de whisper_full y cancela mediante el supervisor productivo; comprueba el PID exacto recogido. No clasifica una fase sólo por esperar 500 ms. La instrumentación de archivo sólo existe en el binario de benchmark; la ruta productiva usa un observer vacío.

```text
python3 scripts/benchmark_vosk_f4.py --lib /var/tmp/kda-f4r-vosk-f97mnvsm/vosk/libvosk.so --model /var/tmp/kda-f4r-vosk-f97mnvsm/vosk-model-small-es-0.42 --runs 10 --output docs/recovery/F4-R-EVIDENCE/vosk-final
```

Los artefactos de esa ruta ya fueron eliminados conforme a la autorización. Repetir el benchmark exige aportar artefactos locales explícitos o nueva autorización de descarga; el script no los obtiene automáticamente.

## Compilación optimizada y alternativas no medidas

El preflight encontró ~120 GiB libres y ~4,46 GiB de MemAvailable, con un solo job previsto. Se intentó aislar únicamente el C++ optimizado, dejando Rust dev, mediante:

```text
cargo build --offline --locked -j 1 --config 'profile.dev.package.whisper-rs-sys.debug-assertions=false' --bin bench_voice
```

Ese override seleccionó CMAKE_BUILD_TYPE=Release sin WHISPER_DEBUG, pero terminó con exit 101 por ausencia de `cmake`. [Log](F4-R-EVIDENCE/native-release-build.log), [preflight](F4-R-EVIDENCE/native-release-preflight.json). No se llegó a un binario O3 válido; no se comparan números Release ficticios. Se inspeccionaron los comandos generados en caché, pero no se reconstruyó manualmente un sustituto del build Cargo ni se descargó la herramienta sin autorización explícita. Cargo release completo/LTO: NOT_TESTED/BLOCKED; no se presenta este ensayo como aplicación release.

| Alternativa | Artefacto / licencia / dependencias | Estado y decisión |
| --- | --- | --- |
| Whisper tiny optimizado | Mismos pesos MIT; CPU/SSE disponible, sin asumir AVX; native Release requería CMake | Medido O2 y parámetros experimentales. O3 pendiente, contexto reducido sin WER. No cambiar default |
| Tiny q5_1 | 32.152.673 B; MIT; GGML soporta cuantización | [Metadatos oficiales](F4-R-EVIDENCE/tiny-quantized-metadata.json), SHA/revisión registrados. Ausente; quantizador no está en las fuentes empaquetadas de la crate, no se añadió uno ni se descargaron pesos. RAM/CPU/precisión NOT_TESTED |
| Tiny q8_0 | 43.537.433 B; MIT | Disponible upstream, no instalado/descargado; NOT_TESTED. Menor disco no demuestra menor latencia SSE |
| Vosk small es 0.42 | ZIP 39.817.833 B + runtime 0.3.45 7.173.758 B; Apache-2.0; C API/Kaldi | Candidato provisional por coste; sólo benchmark autorizado, no integración. Precisión publicada upstream no es WER local |
| sherpa-onnx + moonshine-base-es-quantized-2026-02-27 | Archivo 50.846.902 B, encoder+decoder ORT ~62 MiB; sherpa Apache-2.0; pesos españoles bajo licencia Community de Moonshine, no asumir MIT | Español concreto confirmado por documentación. Runtime/binario no instalado y licencia de pesos requiere revisión para distribución. Sin descarga/inferencia/precisión/CPU/RAM local: NOT_TESTED. No se integra otro motor a ciegas |

Fuentes primarias: [Whisper](https://github.com/ggml-org/whisper.cpp), [pesos/revisión](https://huggingface.co/ggerganov/whisper.cpp), [Vosk modelos](https://alphacephei.com/vosk/models), [Vosk instalación](https://alphacephei.com/vosk/install), [runtime 0.3.45](https://pypi.org/project/vosk/0.3.45/), [sherpa Moonshine español](https://k2-fsa.github.io/sherpa/onnx/moonshine/models-v2.html#sherpa-onnx-moonshine-base-es-quantized-2026-02-27-spanish), [licencias Moonshine](https://github.com/moonshine-ai/moonshine/blob/main/LICENSE). Los tamaños sherpa se verificaron por [metadatos de release](F4-R-EVIDENCE/sherpa-assets-metadata.json), sin descargar el archivo. El mantenimiento activo del código no acredita rendimiento de ningún modelo ausente.

La comprobación adicional [compiled-capabilities](F4-R-EVIDENCE/compiled-capabilities/benchmark.json), n=1, no se agrega a los percentiles: el backend informa CPU/SSE3/SSSE3/repack, COREML=0 y OPENVINO=0. No contiene AVX/F16C. Se usa para verificar el backend compilado, no como nueva muestra estadística.

## Corpus y límites de aceptación

[Plan/consentimiento](F4-R-EVIDENCE/corpus-plan.json): referencias españolas cortas/largas, números, aplicaciones, consultas naturales y variantes autorizadas; grabaciones humanas todavía ausentes. El generador no simula una persona mediante TTS. WER permanece NOT_TESTED. Los fixtures son propios CC0, con semillas y hashes reproducibles, separados del plan de habla. No se utilizó audio personal ni se obtuvo un corpus externo silenciosamente.

PTT físico, precisión con habla, TTS real/audible e integración conversacional completa de voz siguen pendientes. No hay presupuesto/SLO numérico de producto ni certificación manos libres desde estas medidas.
