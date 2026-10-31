# F4-R4 — Optimización de latencia Piper

**Rendimiento: IMPROVED (limitado). TTS técnico: READY_FOR_REVIEW. F4: PARTIAL.** LIVE_TTS T1 conserva PASS histórico de R3; el pipeline R4 audible y T2–T5 humanos permanecen NOT_TESTED.
Fecha: 2026-10-19. Rama `recovery/f4-voice-coordinator`, HEAD `9544a58eba4b6adf6c6343c04db80813b084f76b`. Cambios locales previos conservados, índice sin preparar; sin commits, push, merge, release, F5 ni wake word.

## Resultado medido

Inicio del reproductor privado, observado con el mismo sondeo HTTP y cinco parejas alternadas: p50 **7,692 → 7,238 s**; ahorro emparejado mediano **453 ms**, 5/5 positivo. CPU Backend+hijos p50 **9,78 → 9,18 s de CPU por operación**. La mejora es limitada: sigue existiendo una espera fría cercana a siete segundos. La serie agrupada previa mostró un p95 peor en R4 y queda visible en BENCHMARKS; no se garantiza una cola estrecha en uso real.

Modelo preparado experimental, segunda petición, n=5: PCM completo p50 **0,991 s**, sin reproducir; retención temporal de 253–256 MiB. No se traslada a la aplicación por defecto ni se presenta el primer fragmento (~0,1 s) como audio continuo disponible.

## Diagnóstico y decisión

El coste dominante es crear una sesión ONNX nueva. La ruta R3 añadía un proceso preflight que calculaba los mismos hashes que el worker volvía a calcular. Después esperaba todo el PCM y sólo entonces lanzaba `pw-play`.

Se conserva una ruta **desechable con PCM completo**, suprimiendo únicamente el preflight duplicado. El worker sigue verificando todos los hashes y las opciones antes de importar las dependencias privadas y cargar la voz. No se utiliza una caché de verificaciones, ni un motor residente por defecto.

Se construyó y midió un protocolo de PCM progresivo, pero **no se conectó a la ruta de producción final**: la entrega por frases de Piper deja un déficit medible entre la breve «Hola.» y la frase siguiente. Adelantar el reproductor con ese primer fragmento podría introducir pausas. Los resultados de primer PCM del experimento no se presentan como una mejora aceptada de reproducción continua.

Un segundo experimento mantiene exactamente una sesión ONNX para dos solicitudes, con TTL fijo de tres segundos de espera. Lo posee un ejecutable Rust, sin Backend, servidor, herramientas, credenciales ni dispositivo de audio. Muestra cuánto se podría ahorrar reutilizando el modelo; **no está disponible desde Configuración, HTTP ni autolectura**, y no cambia el comportamiento predeterminado. Una integración con TTL configurable y política de preparación necesita decisión posterior, considerando la memoria retenida.

Las cifras finales, dispersión y alcance están en [BENCHMARKS](F4-R4-BENCHMARKS.md); comandos y gates en [RESULTS](F4-R4-RESULTS.md), pendientes en [ISSUES](F4-R4-ISSUES.md).

## Cambios de implementación

- `voice_coordinator.rs`: un solo worker validado, PCM completo acotado a 16 MiB, comprobación temprana de recursos presentes y comprobación de cancelación antes de reproducir. Conserva identidad de sesión/turno/generación, admisión y drenado de operaciones anteriores.
- `voice_tts.rs`: hashes completos por operación; rechazo de archivos especiales, enlaces finales, propietario incorrecto o escritura por grupo/otros. Comprueba metadatos antes/después del hash. Conserva la identidad capturada al finalizar cada hash, la comprueba otra vez tras preparar el runtime y mantiene abiertos los descriptores del modelo y JSON verificados hasta que Python carga esos mismos inodos; no vuelve a abrir sus rutas para inferencia. La configuración y el modelo se comprueban de nuevo por identidad antes/después de cargar y se cierran los descriptores heredados.
- `piper_worker.py`: misma API `PiperVoice.synthesize`, voz, hablante 0, duración 1 y CPUExecutionProvider, tres hilos intra-op, uno inter-op y spinning desactivado. Instrumenta importación ORT/Piper, sesión ONNX, fonemización, IDs, inferencia, conversión PCM, disponibilidad y entrega IPC. Los envoltorios devuelven exactamente los resultados originales.
- `voice_process.rs`: métricas monotónicas de spawn, primer stdout, EOF y terminación, manteniendo lectura/escritura concurrentes, límite, cancelación y kill/wait. `data` queda excluido de serialización: los logs de tiempos no incluyen PCM ni texto.
- `mod.rs` exporta el adaptador experimental; `voice_pcm.rs`: adaptador **experimental** con cabecera y frames acotados a 32 KiB, formato PCM16 LE/mono/22050 Hz, límite total 16 MiB, metadatos de hasta 8 KiB, backpressure y un solo propietario de productor/reproductor. Pruebas de orden, errores, salida truncada/excesiva, timeout y cancelación con ambos procesos recogidos. Ningún código Backend lo invoca en la ruta final.
- `main.rs`: modos internos estrictos stream/experimento, despachados antes de Backend/config/SQLite; argumentos inválidos nunca arrancan la aplicación normal.
- `bench_piper_warm.rs`: experimento sin audio, máximo dos peticiones, TTL fijo 3 s y deadline global 60 s, comprobación de perfil privado y recogida del hijo incluso ante error/timeout/Ctrl-C.
- Tres runners `benchmark_f4r4*.py`; `validate_f4.py` incorpora los tests del protocolo sin quitar aserciones previas. `tests/f4_cli.rs` amplía los rechazos de argumentos internos.

## Revisión de identidad antes del cierre

La revisión detectó que el primer prototipo volvía a capturar la identidad del modelo después de verificar los demás recursos. Una modificación in-place durante ese intervalo podía convertirse en la identidad esperada sin un nuevo hash. Se corrigió para conservar los metadatos originales tomados al terminar la verificación y rechazarlos si cambian antes del exec. Python recibe esos mismos valores, no una identidad recalculada después del intervalo. Cuatro tests de recursos/integridad PASS, incluido modificar el archivo después del hash.

Los recursos usados en las mediciones no cambiaron y no hubo incidente con datos personales. Se conservan las series de instrumentación previas identificadas por hash del binario; el inicio del reproductor y el lifecycle se repiten con el binario endurecido. El experimento preparado también recibe una comprobación final, n=1, sin atribuir percentiles a ese smoke.

## Ownership, seguridad y recursos

El Backend sigue siendo el único propietario del ciclo conversacional y de la reproducción real de la aplicación. No se modifica F2, sus permisos, las 19 herramientas bloqueadas, SQLite, consentimiento, STT, QML ni parámetros de la voz. La autolectura sigue ocurriendo al cerrar un turno: no produce prompts adicionales ni altera texto persistido. Parar audio no afirma cancelar un LLM activo.

Los hijos siguen sin shell y con entorno mínimo, PDEATHSIG, kill-on-drop y wait explícito. El worker no recibe el socket de escritorio, claves, bearer ni HOME del usuario. Sólo el reproductor autorizado por la operación recibe el socket de audio; en estas pruebas es exclusivamente el daemon privado con sumidero nulo. No se mantiene un modelo grande en espera por defecto.

Los descriptores protegen frente a sustitución de las rutas del modelo/configuración. Las comprobaciones de metadatos rechazan modificaciones detectadas antes de emitir PCM. Esto no es un sandbox frente a un atacante del mismo UID que modifica memoria/archivos durante ejecución: los módulos Python y recursos de espeak siguen cargándose de rutas privadas verificadas. No se promete inmutabilidad completa del filesystem ni borrado forense.

## Alcance humano y privacidad

Se reutilizaron sólo los recursos privados ya autorizados en `/var/tmp/kda-f4r3-piper-aqrh6h0h`. Sin descargas, instalaciones, paquetes globales, micrófono, altavoces, Groq/APIs reales, cambios KDE ni acceso al perfil personal. HOME/TMP y los cuatro XDG son privados. Las pruebas de compositor sólo observan ventanas propias de los runners.

No se guardaron PCM/WAV de las síntesis; los buffers de benchmark se desechan tras calcular tamaño/hash. Los archivos sintéticos mínimos de tests de orden se eliminan al finalizar. Los informes anteriores y sus apéndices T1 quedan íntegros, con hashes de conservación.

**LIVE_TTS T1 histórico: PASS exclusivamente para R3.** Se conserva literalmente la valoración «Se escucho perfectamente, sin cortes y sin distorcion. El unico problema son los 8s.». No se traslada esta aceptación al pipeline modificado. T2–T5 humanos: NOT_TESTED. Cualquier nueva escucha requiere permiso independiente después de revisión; no se reutilizó el recibo consumido de T1. F4 sigue PARTIAL.
