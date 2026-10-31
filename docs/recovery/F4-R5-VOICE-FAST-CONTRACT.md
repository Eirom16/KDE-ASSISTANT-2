# F4-R5 — Contrato de Voz rápida

Rama `recovery/f4-voice-coordinator`, base `9544a58eba4b6adf6c6343c04db80813b084f76b`. Ampliación interna de F4; no sustituye F1B/F2/F3 ni el contrato de voz. Activación explícita; configuraciones antiguas y nuevas: **OFF**. Sin precarga al iniciar Backend, abrir ventana, iniciar sesión o consultar estado.

## Autoridad y preparación

Un `WarmPiperManager` por `VoiceCoordinator`; actor perezoso y un único hijo ONNX como máximo. QML no posee procesos. Mutex síncrono sólo para estado pequeño, sin I/O ni inferencia bajo ese mutex. El actor serializa carga/reserva/síntesis/evicción; canal de admisión acotado a dos mensajes y una solicitud de síntesis admitida a la vez.

Preparación anticipada requiere modo rápido + autolectura + turno LLM válido de la sesión seleccionada y scope vigente. El coordinador comprueba que no está capturando/transcribiendo/sintetizando. Crear una petición manual «Leer» también permite carga bajo demanda. Activar el switch no carga nada y no activa autolectura. Piper instalado no activa el switch.

La preparación no recibe texto. El LLM sigue entregando su stream habitual. Sólo un turno terminado correctamente puede pasar su respuesta final a `speak`, con sesión/turno/scope vigentes; no se añaden prompts ni herramientas. Fallo o cancelación del turno revoca su preparación. Parar TTS no afirma cancelar LLM. Cambio real de sesión invalida la generación y recoge el worker; consultar de nuevo la misma sesión no prolonga TTL.

## Estados y generaciones

`Disabled → Cold` sólo al habilitar; `Cold → Preparing → Ready` requiere confirmación IPC después de cargar y comprobar ONNX. `Ready → Synthesizing → Ready` requiere reserva, verificación criptográfica nueva, Begin y Done válidos. `Ready → Expiring → Cold` al vencer inactividad o vida máxima. Cualquier revocación pasa por `Stopping`, kill+wait, y termina Cold/Disabled. Fallos de motor/protocolo/integridad producen Error. Presión de memoria antes de despachar produce Fallback y permite la ruta fría existente; error de integridad/IPC **no** se reintenta automáticamente por otra ruta.

Época atómica Rust invalida planes antiguos. Cada hijo tiene UUID de generación; cada síntesis UUID de solicitud, comprobados en Reserved/Begin/Done/Error. La operación conversacional conserva sesión/turno/generación F4. La identidad de selección incluye runtime, modelo, hablante, escala y TTL. Cada guard de operación propaga revocación global también durante reproducción de PCM ya bufferizado. La expiración normal del modelo no revoca audio válido ya sintetizado.

`fast.state`, PID, UUID, contadores y tiempos son observabilidad interna autenticada; Ready no significa micrófono, wake ni reproducción. `phase=load/validation/inference` distingue carga, hash y llamada real a inferencia. Consultar estado no renueva leases.

## IPC privado v1

Pipes heredados, sin socket ni servidor. Ejecutable actual con modo interno estricto `--voice-piper-managed`; argumentos inválidos se rechazan antes de logger/Backend/config/SQLite. Después exec explícito de Python canónico con `-I -B -S`, entorno mínimo sin credenciales, HOME personal, DBus o socket PipeWire. Sin shell, PATH `piper`, daemon, autoridad de conversación ni acceso a herramientas. Sólo `pw-play` de la operación recibe el socket de reproducción autorizado.

Control: longitud u32 LE, 1..131072 B y JSON `protocol=1`. Init contiene generación UUID y TTL 15..60 s. Ready confirma generación, PCM16LE mono 22050 Hz y carga terminada. Reserve/Reserved incluye generación y solicitud; reserva evita que el worker expire mientras Rust recalcula hashes. Synthesize contiene las mismas identidades y texto UTF-8 no vacío, máximo 16000 B. Begin se emite inmediatamente antes de la primera llamada de inferencia.

Después de Begin: frames PCM con longitud u32 LE, 2..32768 B, pares; suma máxima 16 MiB. Cero termina PCM y precede Done con contador, formato, SHA-256 y ambas identidades. `0xffffffff` señala Error terminal. EOF, framing, generación, formato, contador o hash incorrectos nunca producen reproducción exitosa. Backpressure: escritura de pipes bloquea al productor; Rust consume frames pero retiene PCM completo hasta Done. No reproducción progresiva ni archivos WAV/PCM temporales.

Carga/síntesis con deadline 90 s existente; reserva y preflight 15 s. El deadline externo de solicitud incluye espera/carga/validación/inferencia. Cancelación prioritaria y revocación terminan hijo propio mediante kill+wait; no basta con abandonar futures. Actor recoge watch/monitor además del hijo. El hijo también aplica deadlines a su entrada y vida máxima; PDEATHSIG/kill-on-drop son defensas adicionales, no sustitutos de wait.

## Integridad

Antes de carga se conservan hashes completos de modelo, JSON y manifiesto de 1799 recursos aprobados R3/R4; UID, permisos, archivos regulares, NOFOLLOW y metadatos antes/después de hash. Modelo/config se abren y fijan por descriptores; identidades originales se comprueban antes de exec y antes/después de carga Python. Cada síntesis preparada repite preflight criptográfico completo; ninguna caché por nombre/mtime/tamaño sustituye hashes.

Inotify complementa hashes: cambios de contenido/atributos/inodo, sustitución, movimiento, borrado, pérdida/overflow de watch revocan; se vigilan recursos, cadena de runtime, Python y ejecutable actual. Eventos ajenos en directorios no renuevan ni invalidan innecesariamente. Cambiar configuración, modelo, voz, escala, runtime o desactivar modo revoca y espera recogida antes de confirmar configuración. Cualquier guard invalida resultados pendientes antes de reproducción.

**No es un sandbox** contra otro proceso malicioso del mismo UID. Las importaciones nativas/Python usan paths verificados y watches, pero no existe transacción atómica sobre todo el filesystem ni prohibición de syscalls de red/archivos. No se promete resistencia completa a manipulación simultánea del mismo UID. El worker recibe sólo recursos aprobados y texto final, sin secretos ni autoridad adicional.

## Retención y presión de memoria

TTL inicial propuesto 30 s, validado 15..60; sin control extra en QML. Comienza en Ready y se reinicia sólo tras síntesis exitosa. Vida máxima del hijo: 2×TTL desde Ready; obliga a renovar incluso con solicitudes sucesivas. Una síntesis ya reservada puede terminar dentro de su deadline; TTL no corta arbitrariamente una inferencia activa. Al quedar ocioso se aplica el menor deadline de inactividad/vida. Consultas de estado y eventos duplicados de preparación no reinician ninguno.

Admisión conserva la ruta fría cuando falta información de memoria o hay presión. Política conservadora basada en envolvente R4 medida de 256 MiB: MemAvailable debe cubrir tres envolventes (modelo, scratch y crecimiento de escritorio); PSI some avg10 positivo bloquea preparación si MemAvailable está por debajo de dos márgenes de admisión (1536 MiB); información inválida bloquea siempre. Monitor a 1 Hz revoca ante presión o RSS del hijo superior a dos envolventes. Son decisiones de admisión/evicción revisables, no límites duros de heap ni garantía frente a OOM; no se fuerza presión real del sistema. No se precarga durante STT; iniciar PTT recoge Piper antes de cargar/capturar. No se mantiene una segunda sesión ONNX. Métricas y justificación final: BENCHMARKS.

## Datos y salida

Sin audio remoto, descargas, micrófono, wake, shortcuts o cambios F2. Los 19 tools bloqueados permanecen bloqueados. Texto/PCM no aparecen en métricas; sólo números e identidades opacas. PCM vive en buffers acotados y se descarta al cancelar o terminar. Los perfiles de prueba son privados HOME/TMP/XDG con SQLite sintética, puerto efímero, proveedor mock loopback y PipeWire nulo sin drivers de hardware. Una prueba audible futura exige nueva autorización; T1 R3 no certifica este pipeline.

La cancelación utiliza flag persistente y Notify, sin timers de sondeo de 10 ms. Cada waiter se registra antes de comprobar el flag, evitando perder una revocación entre comprobación y espera. Una retirada por salud cancela la generación antigua y renueva el token únicamente si su época sigue vigente; no puede cancelar el token ya renovado por una configuración concurrente. El monitor distingue un hijo terminado (Z/X o /proc ausente) de métricas inválidas de un hijo vivo; Child::wait conserva la autoridad sobre la salida y la expiración normal no revoca la siguiente generación.
