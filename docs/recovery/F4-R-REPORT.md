# F4-R — Recuperación de STT y validación

Baseline: `9544a58eba4b6adf6c6343c04db80813b084f76b`. Rama conservada `recovery/f4-voice-coordinator`. Sin commits, staging, push, merge, release, F5 ni F6. Los cinco informes F4 originales permanecen byte a byte intactos; hashes en [baseline](F4-R-EVIDENCE/baseline.json). Este informe es un addendum, no una sustitución del dictamen anterior.

## F4-INC-01: corrección verificable sin acceder al perfil real

Se auditó main: los dos modos internos retornan antes del logger, Tokio, InstanceLock, Config y Backend/SQLite. Los argumentos `--voice-*` mal colocados/desconocidos ya fallaban antes del backend. Existía otra apertura: un argumento desconocido que no coincidiera con ese prefijo se ignoraba y podía iniciar la aplicación. Ahora una lista explícita valida todos los argumentos normales antes de cualquier inicialización. Se conservan los flags documentados y se rechazan puertos inválidos/duplicados y el modo retirado `--send-msg`.

`tests/f4_cli.rs` ejecuta 14 variantes sintéticas con HOME/XDG privados, deadline y reaping: modos válidos con modelo ausente, aridad incorrecta, prefijos mal escritos, flags tardíos, puerto inválido/duplicado. Cada ejecución debe terminar sin crear ningún archivo/directorio del perfil. El caso modelo ausente produce sólo su JSON de diagnóstico. La auditoría de orden y los efectos observables se complementan; no se probó la corrección sobre archivos reales.

`scripts/f4_safety.py` centraliza la creación de HOME, TMP y las cuatro raíces XDG en un perfil propio 0700. Descarta claves, proxies y overrides de aplicación/audio/bibliotecas, fija un socket PipeWire inexistente y un bus inexistente, y usa Cargo offline. CARGO_HOME/RUSTUP_HOME conservan explícitamente los caches de herramientas de desarrollo existentes; no son perfiles de KDE Assistant. Los helpers que pueden lanzar backend/QML exigen esta validación antes de crear sockets o procesos. Se rechazan omisiones, escapes, symlinks, permisos abiertos, credenciales y sockets inesperados.

El worker opt-in de cancelación también crea su propio perfil/socket ficticio, aunque el modo restringido no carga configuración. El daemon PipeWire sintético conserva sólo su socket privado y módulos sin hardware. Los servidores de fixture eligen puerto loopback efímero; los gates heredados con 8765 requieren el puerto libre. Los tests oficiales Plasma usan el bus/display del escritorio para observar únicamente ventanas propias, con perfil y servicio DBus distintos; no reutilizan DB, bearer ni endpoint de una instancia habitual.

No se volvió a abrir, restaurar ni inspeccionar metadatos de configuración/SQLite personales. La incertidumbre histórica de F4-INC-01 sigue vigente: estos tests previenen su repetición, no reconstruyen el estado anterior del usuario.

## Diagnóstico

Whisper-rs 0.16.0 usa whisper-rs-sys 0.15.0 / whisper.cpp 1.8.3. El Rust dev tiene opt-level=0, pero su C++ ya usa RelWithDebInfo, `-O2 -g -DNDEBUG`, `-march=native`, CPU y repack. Lleva WHISPER_DEBUG; no hay BLAS, OpenMP, CUDA ni otro backend de inferencia activado. La CPU N5000 dispone de SSE4.2, pero no AVX/F16C/FMA. Los booleanos de extensiones del cache CMake no sustituyen los flags efectivos ni la CPU detectada.

La reproducción separa conversión, carga, creación de estado, inferencia y extracción de texto. Cargar contexto cuesta décimas de segundo y crear estado decenas de milisegundos; el coste dominante está en la inferencia. Para tres segundos se conserva por defecto el contexto acústico de 1500 pasos del modelo (ventana nominal de 30 segundos). La prueba controlada de audio_ctx=256 reduce mucho el trabajo, manteniendo ocho tokens sobre el tono. Esto identifica una causa fuerte de coste innecesario en entradas cortas, pero no acredita conservación de precisión hablada.

En silencio/ruido, el decoder directo puede reintentar con temperatura 0.2 y producir etiquetas musicales ficticias. Desactivar fallback ahorra algo de tiempo pero cambia el resultado; no se cambió ese default. El coordinador productivo ya omite la inferencia para PCM prácticamente silencioso. Ese filtro no convierte ruido en habla ni certifica detección de voz.

Las pruebas de hilos/afinidad son exploratorias, con cifras y dispersión en BENCHMARKS. No se fija una afinidad global ni se cambia el scheduler, gobernador o aplicaciones ajenas. La actividad del host y el sampler se registran como límites de comparación. Los datos históricos F4 con compilación concurrente no son una comparación definitiva.

La comparación O3 quedó bloqueada por CMake ausente. El preflight verificó espacio/memoria y el intento offline con un job seleccionó CMAKE_BUILD_TYPE=Release, pero falló antes de generar un artefacto válido. Se inspeccionaron comandos de compilación guardados; no se reconstruyó un sustituto manual ni se descargó CMake sin autorización específica. No se confunde una biblioteca C++ optimizada con un build release completo de Rust.

## Implementación correctiva

- main: rechazo temprano de cualquier CLI desconocida, antes de tocar un perfil.
- STT: opciones experimentales y medición compartida con el mismo decoder; validación de límites/valores no finitos. El camino productivo mantiene contexto completo, tres hilos como máximo y los parámetros de decodificación previos. Se corrige `language=None` para transmitir autodetección real en lugar de conservar el default inglés de FullParams.
- Benchmark Whisper: tiempos separados, tokens/segmentos, CPU ticks y recursos; entrada canónica a 16 kHz, control de compilaciones concurrentes, hilos y afinidad sólo del hijo.
- Benchmark Vosk: adapter ctypes exclusivamente de inferencia offline, usando libvosk extraída del wheel autorizado, sin pip/servidor/autoridad de escritorio ni integración productiva. Recursos recogidos y proceso terminado durante procesamiento PCM.
- Runner/corpus: guardas privadas, fixtures reproducibles CC0 con hashes, referencias españolas y plantilla de permisos para las grabaciones que faltan.

La prueba de cancelación confirmada del decoder compartido publica una marca atómica al entrar en whisper_full, observa progreso CPU y cancela con el supervisor productivo, verificando el PID recogido. El producto utiliza observer vacío, sin archivo/log de diagnóstico.

No se redujeron buffers ni seguridad/cancelación para fabricar velocidad; tampoco se aumentó timeout. VoiceCoordinator/F2/sesiones/herramientas siguen siendo la autoridad anterior. No se tocó Configuración personal, Piper, TTS ni wake word.

## Selección provisional

Vosk small español 0.42 es el candidato prioritario para la siguiente evaluación de habla por el coste observado en señales sin habla y por su modelo específico español. No es una selección definitiva: aún faltan WER, consultas habladas, nombres/números, ruido real y experiencia física. La autorización recibida permite sólo benchmark offline; no se instala ni activa Vosk en el producto.

Whisper tiny se conserva como alternativa local existente. Reducir su contexto es una opción experimental prometedora, todavía sin validación de precisión; no se hizo default. Tiny cuantizado y sherpa-onnx con Moonshine español se evaluaron documentalmente, sin descargarlos ni integrarlos. Véanse tamaños/licencias/dependencias y bloqueos en BENCHMARKS/ISSUES.

**Rendimiento: IMPROVED en opciones de benchmark offline, sin aceptación de rendimiento productivo. Estado F4: NEEDS_FIXES.** PTT físico, precisión española, Piper audible y wake real siguen sin aceptar. La implementación queda sin consolidar y espera revisión técnica.
