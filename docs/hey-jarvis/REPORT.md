# Hey Jarvis — aceptación física de la única sesión autorizada

Fecha local: 2026-10-22 (observación UTC hasta 2026-10-23 02:33). **Resultado global: PARTIAL funcional; FAIL del límite de activaciones. No LIVE_WAKE_PASS.** No se repite la sesión ni se reutiliza su consentimiento.

El humano confirma: al decir «jey jarvis» la app apareció automáticamente y pudo editar el borrador. Informa una limitación clara: pronunciado rápido/unido («jeyjarvis») no se reconoce; necesita separar las palabras. No comunicó un tiempo numérico, el contenido exacto de ambos borradores ni confirmó ambos recorridos completos.

## Criterios físicos

| Criterio | Resultado | Evidencia y límite |
|---|---|---|
| Integridad previa, propósito y aislamiento | PASS | Hash del binario `b5149c8c8e4baa7e49db9e2e1f618cea2b5ff4615121c16b7a5bb55a92a7ed7b`, todos los recursos, configuración y QML coincidieron con el escenario preparado; HOME/TMP/XDG separados 0700. DBus privado ausente y socket Wayland absoluto; PipeWire compartido sólo tras validar y consumir el recibo. |
| Una sola sesión, duración máxima 180 s | PASS | Un único lanzamiento. Aborto al observar la tercera detección a los 163,878 s desde arranque. Entre la primera habilitación y el aborto transcurrieron menos de 99 s, con pausas intermedias. No se volvió a abrir el micrófono. |
| Máximo dos activaciones / sin reactivación manual entre órdenes | FAIL | El monitor observó tres detecciones. Tras la primera hubo pausa/reanudación; tras la segunda, otra pausa/reanudación. El monitor reaccionó después de la tercera admisión. No se atribuyen las pausas a un botón específico sin evidencia humana. |
| Aparición automática desde la escucha de fondo | PASS | Confirmación humana de aparición en cuanto pronunció la frase; detecciones nativas inician Recording. No hay medición numérica del enfoque/latencia ni evidencia independiente de que cada intento se hiciera minimizado. |
| Pronunciación natural y variación de cadencia | PARTIAL | «jey jarvis» separado reconoce; rápido/unido falla según el humano. No está resuelto por la aceptación del modelo offline. |
| Continuidad wake → orden sin perder palabras | PARTIAL | Hubo dictado desde la segunda detección y un borrador nativo de 35 caracteres. Falta cotejo humano del texto con la orden completa; no se conservó audio para reconstruirlo. |
| Dos borradores correctos y editables | PARTIAL | El humano confirma edición. Sólo se observó un TranscriptReady: el primer STT fue cancelado; el segundo terminó. No se certifican las dos órdenes exactas. |
| Cancelar y rearmar sin volver al robot | PARTIAL | Hubo cancelación y luego otra detección, pero entre ellas se cerraron y reabrieron captura/detector mediante pausa/reanudación. No valida rearmado automático con los mismos PID. |
| Pausar cierra el micrófono y recoge los hijos | PASS | Pausas observadas con microphone_active=false y ausencia de captura/detector. Cada reanudación produjo otro par de PID. |
| Desactivar explícitamente antes del cierre | PARTIAL | El aborto original entró en cierre del backend y recogió todo; no dejó confirmación API final OFF. El intento posterior de desactivar recibió conexión rechazada porque el proceso ya había terminado. |
| Cierre completo | PASS | Backend y todos los hijos del arnés recogidos. Los diez PID observados por el monitor adicional estaban ausentes de /proc. |
| Sin envíos, herramientas ni audio persistente | PASS | SQLite privada: messages=0 y tool_audit=0. Envío bloqueado en QML privado, proveedor loopback inalcanzable, herramientas apagadas; sin archivos de grabación. Sólo PCM efímero en RAM. |
| Limpieza de campaña | PASS | Perfil privado, recibo consumido, logs, métricas, SQLite y QML de campaña eliminados después de extraer estos datos escalares. Recursos compartidos ONNX/STT conservados. |

## Tiempos observados y recursos reales

Tiempos desde el lanzamiento, no desde el fin acústico de la frase: habilitación a 64,930 s, Listening a 65,729 s, primera Recording a 69,104 s y Transcribing a 71,013 s. Se pausó/canceló ese STT a 96,368 s, tras unos 25,35 s sin borrador. Reanudación a 97,552 s; segunda Recording a 105,221 s, Transcribing a 109,543 s y TranscriptReady a 151,258 s: **41,715 s de transcripción**, con 4,322 s entre Recording y Transcribing. No son cifras de latencia acústica wake→ventana. Hubo pausa a 151,537 s, reanudación a 156,716 s y tercera Recording a 163,878 s, seguida del aborto. No se pidió ningún intento adicional.

Muestreo /proc aproximadamente cada 0,5 s, limitado a los procesos propios, en el N5000 y sobre el binario fijado. CPU expresada como porcentaje de **un núcleo**:

| Proceso | CPU escucha normal (15,0 s muestreados) | RSS / PSS máximo de ese proceso en escucha |
|---|---:|---:|
| Detector ONNX | 14,60 % | 52.984 / 44.475 KiB |
| PipeWire pw-record | 0,33 % | 8.396 / 3.828 KiB |
| Backend | 4,33 % | 35.452 / 26.713 KiB |
| QML | 8,13 % | 202.220 / 150.451 KiB |

Pico simultáneo del árbol durante Listening: RSS 298.580 KiB, PSS 224.977 KiB. STT no residía en escucha normal y Piper no se cargó. Durante las interacciones el detector bloqueado usó 0 ticks de CPU entre muestras comparables. STT bajo demanda alcanzó 258,60 % de un núcleo durante 66,16 s muestreados de ambos intentos; pico STT RSS/PSS 229.728/220.364 KiB. Pico simultáneo de todo el árbol: RSS 526.580 KiB, PSS 440.636 KiB. Es una muestra breve con voz real, pausas y actividad de ventana; no una medida de batería ni una tasa de falsos positivos.

## Defecto del arnés y corrección offline

El arnés original esperaba **dos borradores**, en vez de terminar al completar/cancelar la segunda **detección**. Si se cancelaba el primer STT, seguía esperando y podía aceptar reanudaciones manuales. Su guarda `activations > 2` sólo abortaba después de admitir una tercera. Éste es un incumplimiento real del límite, no un PASS por haber abortado.

Se corrigió `scripts/live_hey_jarvis.py`: una pausa/desactivación después de detectar termina la campaña; al completar o cancelar la segunda detección se pausa y cierra aunque exista un solo borrador. El finally intenta Desactivar aun si falla el monitor, conserva el motivo del fallo y registra la apertura real observada, en lugar de marcarla incondicionalmente. Cinco regresiones puras de política pasan en `scripts/test_live_hey_jarvis.py`, sin backend, audio, modelos ni repetir benchmarks. **No se ejecutó físicamente esta corrección.** El monitor sigue siendo externo por sondeo: para garantizar una cuota estricta frente a cualquier carrera, una próxima campaña requiere además un presupuesto aplicado dentro de la admisión del backend. Este punto queda como bloqueo antes de otra prueba física limitada.

## Cadencia: diagnóstico offline tras la observación humana

Se revisó el adaptador existente: entrega PCM continuo al clasificador y no tokeniza ni exige una pausa entre «Hey» y «Jarvis». Como comprobación nueva y acotada de este defecto, se sintetizaron únicamente dos variantes **unidas** de `[[hˈeɪdʒˈɑːɹvɪs]]` con el Piper ya autorizado, speaker 0, ruido 0, semillas fijas y `length_scale` 0,65 / 1,05. No se repitió el corpus ni un benchmark; no hubo micrófono, reproducción, Backend, servicios remotos ni descargas. HOME/TMP/XDG privados, socket de audio inalcanzable y eliminación automática de los fixtures al terminar.

| Variante sintética unida | Score máximo del modelo existente | Umbral 0,8 |
|---|---:|---|
| Rápida, length_scale 0,65 | 0,894867 | PASS |
| Más lenta, length_scale 1,05 | 0,855523 | PASS |

Estas dos muestras **no reproducen el fallo humano** ni acreditan tolerancia al acento/cadencia del usuario. No existe evidencia suficiente para afirmar si su fallo viene del score por debajo del umbral, de la pronunciación, del entorno acústico o de hablar mientras el detector estaba ocupado/pausado. La sesión física no guardó scores por frase y su audio ya no existe. **La dificultad con «jeyjarvis» rápido sigue abierta; no se modificó el umbral ni se añadió una transformación acústica sin validación.** El binario probado permanece intacto.

La acción concreta necesaria es una calibración local breve con scores por frase, sólo en RAM, comparando pronunciación natural rápida y separada mientras el indicador esté en Listening; ajustar después el umbral configurable sólo si esos scores separan voz válida y fondo. Si no los separan, un ajuste de umbral no basta y habrá que evaluar otro clasificador/adaptación con evidencia. Esa calibración física necesita consentimiento nuevo, una cuota de admisión estricta y un recibo nuevo. No se abre audio para prepararla ni se identifica una descarga como necesaria todavía.

## Recomendaciones y estado siguiente

1. Corregir la tolerancia a cadencia con los recursos existentes y verificaciones offline específicas; no declarar resuelta la pronunciación humana mediante un ajuste especulativo del umbral ni descargar pesos nuevos.
2. Antes de cualquier nueva prueba física, aplicar la cuota en la admisión del backend, preparar hashes/recibo nuevos y obtener consentimiento nuevo. El permiso actual está consumido.
3. Mantener separadas cancelación de interacción y pausa global en la explicación/indicadores: esta sesión no valida el rearmado automático porque se observaron pausas intermedias.
4. Reducir la espera de STT del N5000 mediante una optimización local posterior respaldada por medidas; no cargar STT/TTS durante la escucha normal.

Se conserva debajo la implementación, licencias y evidencia offline anterior en el mismo informe. F4-VOICE-ACCEPTANCE.md permanece intacto. Sin instalaciones, descargas, perfil personal, servicios remotos, commits, push ni merge.

---

# Hey Jarvis — escucha en segundo plano integrada

## Estado vigente y alcance

La experiencia de un solo uso queda sustituida por **activación única de una opción de escucha en segundo plano**. Mientras la opción sigue habilitada, decir «Hey Jarvis» presenta la interfaz, inicia dictado con audio del mismo flujo y permite una siguiente activación tras completar o cancelar. La implementación y verificaciones offline están terminadas. **LIVE_WAKE_PASS no concedido**: la única prueba física autorizada terminó con aceptación parcial y un fallo del límite de activaciones; véase el resultado humano al inicio de este informe.

Se conserva la misma rama `recovery/f4-voice-coordinator` y HEAD `9544a58eba4b6adf6c6343c04db80813b084f76b`. No hay commits, push, merge, descargas, instalaciones ni cambios al perfil personal. La aceptación audible R5 permanece intacta, con su T3 PARTIAL y excepción T5. No se repitieron T1–T5 ni el corpus/benchmark sintético anterior.

Binario final preparado: SHA-256 `b5149c8c8e4baa7e49db9e2e1f618cea2b5ff4615121c16b7a5bb55a92a7ed7b`.

## Comportamiento actual

- **OFF al arrancar.** Se habilita desde el robot, Configuración → Avanzado o la bandeja, una sola vez durante esa ejecución de la aplicación. No instala servicios, autorranque ni escucha entre reinicios. La preferencia antigua `wake_word_enabled` no inicia captura por sí sola.
- **Oculta/minimizada:** el backend conserva la captura y el detector. Ocultar deja de desvincular la sesión de voz cuando la opción está habilitada. La detección presenta la ventana sin un clic por cada invocación.
- **Audio continuo entre wake y dictado:** un único `pw-record` produce PCM16 mono 16 kHz. Un buffer circular de 50 frames conserva cuatro segundos, 128.000 bytes, sólo en RAM. Cada detección ancla dos segundos de pre-roll al frame clasificado y entrega esos bytes y los siguientes al dictado F4. No se cierra/reabre el dispositivo en ese relevo. Los resultados viejos de la sesión se revocan al cambiar de conversación.
- **Borrador y rearmado:** el texto se publica mediante una cola local acotada con identidad y ACK, se inserta una sola vez en el borrador editable y no se envía. Se reconoce y elimina sólo el prefijo literal de activación cuando STT lo transcribe. El detector se rearma automáticamente después del turno: al menos 25 frames de espera y ocho frames de silencio, con reset de features sin recargar modelos. Las activaciones se bloquean durante grabación, STT, TTS y prewarm explícito de una respuesta vocal.
- **Cancelar interacción:** detiene/revoca esa operación y vuelve a esperar mientras la opción permanezca habilitada. Conserva el borrador. El detector y el input ligero siguen residentes por diseño. **Pausar** cierra captura y detector y conserva la opción pausada; **Desactivar** cierra los hijos y deja OFF. Cerrar realmente la aplicación también cierra todo. Pausar/Desactivar están disponibles en ventana y bandeja; un botón Cancelar sigue visible tras obtener un borrador.
- **Privacidad:** texto explícito «Micrófono abierto» en la ventana, tooltip y menú de bandeja. Pausa/OFF indican micrófono cerrado sólo después del cierre. No se escriben PCM/WAV. La cola de borradores admite ocho entradas; si no hay consumidor se pausa, sin crecimiento ilimitado. Un retraso que exceda la historia circular falla en vez de descartar audio silenciosamente.
- **Fallos:** EOF, falta de datos, worker caído, score inválido o bloqueo de inferencia cierran los recursos y dejan la opción en error/pausa. No hay reconexión automática a otro dispositivo ni bucle de reapertura. Reanudar es una acción explícita. Cambiar configuración pausa la escucha y requiere Reanudar con los ajustes nuevos.

La ventana utiliza `DesktopWindowController` y la API pública KDE ya existentes. No se fabrican tokens de activación ni se usa un script KWin para forzar foco. [KWindowSystem documenta que el compositor puede rechazar activación para evitar robo de foco](https://api.kde.org/kwindowsystem.html#activateWindow). Se prueba la presentación oculta/minimizada offscreen; visibilidad, enfoque y comportamiento físico de Plasma Wayland quedan por comprobar. No se promete foco irrestricto.

## Reutilización e implementación

Se mantienen openWakeWord/ONNX y sus tres pesos existentes; origen, hashes y licencias están en la evidencia anterior de este mismo documento. No se entrenó otro detector ni se instaló una biblioteca. Los pesos siguen bajo CC BY-NC-SA 4.0, separados del código Apache-2.0; la restricción NC sigue siendo relevante para distribución comercial.

`wake_audio.rs` añade el propietario del input y su historia circular; `background_voice.rs`, integrado dentro de `VoiceCoordinator`, añade la política de sesión, admisión, pausa, rearmado y cola de resultados. `wakeword.rs` amplía el adaptador con el modo restringido `--voice-wake-stream`: handshake `R`, comando `F` más un frame PCM y score f32, y `X` para reset temporal con ACK. Las sesiones ONNX permanecen cargadas mientras está habilitado, y se bloquean en stdin cuando no deben inferir. El modo anterior se conserva sólo por compatibilidad/diagnóstico; el control principal de la aplicación usa escucha de fondo.

STT reutiliza la decodificación supervisada F4 sobre el PCM compartido; se carga por interacción y se recoge al terminar/cancelar. Piper no se carga al habilitar Hey Jarvis. Su preparación opcional existente conserva su política explícita de sesión y hace que wake espere durante respuesta/preparación vocal, evitando activación por la propia salida. No se cambió el motor STT ni Piper para obtener este resultado.

Todos los hijos usan el supervisor existente sin shell, entorno limpiado, terminación con el padre y kill/wait. La captura añade cancelación al perder su propietario. El control autenticado es `POST /api/voice/background` con `enable`, `pause`, `resume`, `disable` o ACK de identidad. No admite audio, transcripciones ni eventos de detección enviados por el cliente. Detectar no llama a chat, no escribe mensajes SQLite, no ejecuta herramientas ni modifica confirmaciones F2.

La lógica acústica nueva de energía se limita a esperar silencio para rearmar; **no sustituye al clasificador neural**. Se conserva umbral 0,8 por defecto y rechazo de scores inválidos. No se certifica una tasa ambiental de falsos positivos sin prueba real.

## Verificación actual

| Verificación | Resultado |
|---|---|
| Build offline, Clippy de todos los targets con `-D warnings`, formato y diff | PASS |
| VoiceCoordinator, incluidas cuatro regresiones de escucha de fondo | 21 PASS |
| Buffer circular, atraso, cierre y cancelación de lectores | 2 PASS |
| HTTP F4, incluido fallo de preflight antes de capturar y ausencia de autoridad de chat/herramientas | 4 PASS |
| CLI inválida, incluidos nuevos modos stream | PASS |
| Arranque/configuración/lifecycle aislados | 16 PASS; un test GUI histórico ignorado |
| QML voz: activación sin clic, hide, privacidad, entrega/ACK único, Cancelar conservando borrador | 15 PASS |
| QML escritorio: presentación automática de Main oculta/minimizada y consentimientos conservados | 13 PASS |
| QML ajustes con su mock | 19 PASS |
| Guardas del arnés físico: falta de permiso, nonce/purpose/límite incorrectos | 4 PASS; no alcanzan socket de audio ni lanzamiento |

Dos interacciones completas con fuentes y STT sintéticos de prueba usan **el mismo PID de captura y detector**, devuelven dos identidades distintas y comprueban bytes de orden inmediatamente posteriores al wake. El VAD no termina al consumir silencio anterior a la activación. Otra prueba cancela la primera interacción, mantiene la opción habilitada y detecta la segunda. Pausar comprueba ausencia de ambos PID en `/proc`; EOF recoge ambos sin reconectar. Son pruebas de integración del supervisor y continuidad de bytes, no aceptación de pronunciación humana.

Las pruebas usan HOME/TMP/XDG privados, PipeWire inalcanzable y DBus privado ausente. Los procesos de fuente de esos tests sólo escriben PCM por stdout. No se abrió un micrófono real ni se ejecutó una captura continua en el equipo.

### Residencia y CPU observadas, sin dispositivo

Una verificación breve del protocolo persistente alimentó 75 frames de silencio por stdin, a ritmo de 80 ms, sin PipeWire ni Backend. No repite el corpus anterior. Se midió el proceso real ONNX del N5000:

- 6,0001 s de stream; 0,82 s de CPU, **13,67 % de un núcleo** durante ese intervalo.
- RSS 49.300 KiB y PSS 46.293 KiB; ningún STT/Piper cargado en ese proceso.
- Dos segundos sin frames: **0 ticks de CPU**. Reset y nueva inferencia con el mismo proceso; cierre y recogida PASS.
- Score de silencio máximo 0,000060; no activó.

Medición realizada con SHA-256 `5022e6da7781888f3855d26518283df8837e3279a41a724653ff354db77b1e04`; el build final posterior incorpora la cancelación de Capture al perder su propietario y conserva el mismo motor/protocolo ML. No se presenta esa cifra como medición del conjunto final Qt/PipeWire, batería o escucha ambiental física. Ese consumo y la latencia acústica real siguen NOT_TESTED hasta obtener permiso.

## Preparación anterior de la prueba física (consentimiento ya consumido)

Perfil exclusivo: `/var/tmp/kda-hey-jarvis-human-_k74hdac`. HOME, TMP y cuatro XDG distintos, privados y verificables. Nonce `5668d4ef-e33a-4717-bb28-a23ff8757c55`; binario, configuración, QML privado y recursos fijados por SHA-256. La preparación validó los tres pesos, ONNX Runtime y Whisper tiny multilingüe ya disponible (77.691.713 bytes, hash histórico `be07e048e1e599ad46341c8d2a135645097a538221678b7acdd1b1919c6e1b21`). Se comprobó el handshake ONNX con stdin cerrado y el validador STT, sin audio. Arranque privado headless: Idle, OFF, cero hijos de voz; backend después recogido.

El perfil mantiene herramientas, claves de proveedor, autolectura, autoescucha heredada, memoria y avatar apagados. El envío se bloquea además en la copia privada de Main; el dictado manual alternativo se deshabilita para ejercitar sólo el recorrido solicitado. El backend de texto remoto apunta a loopback inalcanzable, sin credenciales. No se compartió ningún socket de audio durante la preparación.

`scripts/live_hey_jarvis.py` exige un recibo de autorización explícita ligado a este nonce/hash y de un solo uso **antes** de obtener el socket real. Propone una única prueba de hasta 180 s y dos activaciones: habilitar una vez, minimizar, decir naturalmente «Hey Jarvis, abre el navegador», revisar/editar y Cancelar; esperar el rearmado y decir «Hey Jarvis, qué hora es» sin tocar controles de activación. Tras el segundo borrador se pausa; al terminar se desactiva y se comprueba cierre de hijos. Whisper tiny puede tardar decenas de segundos en este N5000; se reutiliza sin nueva optimización ni descarga.

En aquel momento quedaban pendientes pronunciación natural, pérdida de palabras, tiempo percibido wake→interfaz/Grabando, foco Wayland y consumo con micrófono real; el resultado posterior está al inicio. El monitor recoge estados y contadores; el tiempo total esperando una frase no se hará pasar por latencia del detector. Se pedirá confirmación humana de lo pronunciado y del borrador. Esta descripción corresponde a la preparación anterior. El consentimiento posterior se consumió en la única prueba documentada al inicio; el perfil fue eliminado y no es reutilizable.

---

# Evidencia anterior conservada: base offline de un solo uso

La sección siguiente documenta la base técnica aceptada antes de corregir la experiencia. Sus instrucciones de activación por dictado, caducidad de 120 s y relevo entre dos capturas **no describen el modo de fondo vigente**. Se conserva como historia de resultados, licencias y fallos; no se repitió su campaña.

# Hey Jarvis — implementación y aceptación offline

Fecha: 2026-10-22. Rama `recovery/f4-voice-coordinator`, HEAD `9544a58eba4b6adf6c6343c04db80813b084f76b`. Cambios locales anteriores conservados; índice vacío. Sin commits, push, merge, instalaciones ni descargas.

**Resultado:** activación opcional de Hey Jarvis implementada con openWakeWord local y el coordinador F4. Aceptación ML offline: **6/6 PASS**. Compilación, Clippy y verificaciones de estados, cancelación, HTTP y QML: PASS. Captura y aceptación acústica reales: **NOT_TESTED**, porque no están autorizadas. No se activó escucha continua ni se abrió un micrófono real.

La [aceptación audible R5](../recovery/F4-VOICE-ACCEPTANCE.md) se conserva sin editar: T1/T2/T4/T5 PASS audible, T3 PARTIAL por interrupción manual y excepción del fixture T5. SHA-256 actual: `0219c20a96f8fc5474b8c2279c856b2358129508b4c092ef617124accc6914a2`. No se repitieron esas pruebas ni se abrió F4-R6; F4 global conserva su estado previo.

## Uso y límites de autorización

1. Seleccionar una conversación con STT local configurado. El botón del robot activa **un dictado mediante Hey Jarvis**. Al arrancar siempre está OFF, incluso con preferencias heredadas que antes pedían escucha automática.
2. Esperar al indicador «Hey Jarvis activo». Decir la frase con pronunciación inglesa y esperar al indicador «Grabando» antes de dictar. El botón del robot o Cancelar desactivan la espera; Cancelar sirve también durante preparación y dictado.
3. La transcripción se añade al borrador editable. **Sólo Enviar crea un turno de chat**; detectar una palabra no autoriza mensajes, herramientas ni excepciones a F2.
4. Una detección consume la activación. No hay rearmado automático después del dictado, STT o TTS. Sin detección, la espera caduca a los 120 segundos. Cambiar conversación, ocultar/cerrar la ventana, cambiar configuración o cerrar el backend revoca la operación.

Configuración → Avanzado permite indicar el directorio local de los tres modelos y el umbral. Default nuevo: 0,8; los valores existentes se conservan. Directorio vacío resuelve `XDG_DATA_HOME/kde-assistant/models/wakeword`; el antiguo placeholder `assets/models/wake_word.onnx` sigue aceptándose como ese directorio. Guardar ajustes no activa captura. Los campos heredados de saludo, enrollment, autoescucha y `wake_word_enabled` no conceden activación automática; no se generan saludos ni chimes.

La entrega es deliberadamente de un solo uso y con caducidad. No implementa un daemon que permanezca escuchando en la bandeja. Tampoco promete reconocer «Hey KDE», «oye Jarvis» ni cualquier frase editable con el clasificador fijo Hey Jarvis.

## Inspección y reutilización

Se leyeron PLAN, PROMPT, apple-DESIGN y el código F4 disponible. Había un adaptador Rust `OwwDetector`, tres modelos locales y ONNX Runtime. El adaptador antiguo `hotword.rs` estaba desconectado de F4 y conservaba detección por energía/ZCR, normalización adicional, tareas sin supervisión F4 y política de acumulación propia. Se retiró esa ruta; queda una sonda inerte para compatibilidad con los tests de arranque. La producción reutiliza `OwwDetector`, sin detector alternativo escrito desde cero.

| Motor evaluado | Licencias y compatibilidad | Coste y decisión |
|---|---|---|
| [openWakeWord](https://github.com/dscripka/openWakeWord) | Código Apache-2.0; modelos preentrenados CC BY-NC-SA 4.0. Frase inglesa, inferencia ONNX local; independiente del compositor. | Elegido: adaptador, pesos y runtime ya presentes. Bloques de 80 ms; medición N5000 abajo. Sin Python adicional para detección. |
| [sherpa-onnx KWS](https://github.com/k2-fsa/sherpa-onnx), [modelos](https://k2-fsa.github.io/sherpa/onnx/kws/pretrained_models/index.html) | Código Apache-2.0, Linux x64 y APIs nativas/Rust. La licencia de cada paquete de pesos requiere comprobarse por separado. | Alternativa neural válida, pero precisa nueva integración y recursos ausentes. CPU/RAM/latencia locales NOT_TESTED; no se afirma que sea más lento. No se instaló. |
| [Rhasspy Raven](https://github.com/rhasspy/rhasspy-wake-raven), [licencia MIT](https://raw.githubusercontent.com/rhasspy/rhasspy-wake-raven/master/LICENSE) | Plantillas WAV del usuario y comparación MFCC; sin necesidad de pesos de frase. Repositorio archivado en octubre de 2025. | Requiere al menos tres muestras y dependencias no integradas; no hay muestras autorizadas disponibles. Consumo local NOT_TESTED. No se incorporó. |

La licencia no comercial de los pesos openWakeWord **no es una licencia FOSS permisiva** y no se confunde con Apache-2.0 del código. Se reutilizan archivos existentes para esta integración local, sin redistribuir pesos. Cualquier distribución/uso comercial necesita resolver esas condiciones o elegir pesos compatibles. No se solicita una descarga porque no falta ningún recurso para las pruebas realizadas.

Procedencia declarada por el catálogo existente: [release openWakeWord v0.5.1](https://github.com/dscripka/openWakeWord/releases/tag/v0.5.1), archivos con los mismos nombres. La inspección local acredita estos bytes y hashes; no se hizo una descarga para cotejarlos:

| Archivo local | Bytes | SHA-256 |
|---|---:|---|
| `melspectrogram.onnx` | 1.087.958 | `ba2b0e0f8b7b875369a2c89cb13360ff53bac436f2895cced9f479fa65eb176f` |
| `embedding_model.onnx` | 1.326.578 | `70d164290c1d095d1d4ee149bc5e00543250a7316b59f31d056cff7bd3075c1f` |
| `hey_jarvis_v0.1.onnx` | 1.271.370 | `94a13cfe60075b132f6a472e7e462e8123ee70861bc3fb58434a73712ee0d2cb` |

Total de pesos: 3.685.906 bytes. Runtime reutilizado: [ONNX Runtime, MIT](https://raw.githubusercontent.com/microsoft/onnxruntime/main/LICENSE), `libonnxruntime.so.1.29.0`, 28.497.752 bytes, ya instalado en el directorio de datos de la aplicación. Binding `ort` ya presente, MIT/Apache-2.0 según su manifiesto. No se agregaron dependencias a Cargo.toml/Cargo.lock durante este hito.

Para generar fixtures se reutiliza la instalación privada Piper 1.8.0 autorizada en R3, su runtime y `es_ES-sharvard-medium`. Se conserva la evaluación de licencias de [R3](../recovery/F4-R3-REPORT.md): Piper GPL-3.0-or-later y condiciones específicas de voz/dataset, sin presentar el motor como Apache-2.0. No se modificaron esos recursos ni la configuración productiva de Piper.

## Implementación

| Archivo | Cambio de este hito |
|---|---|
| `src/backend/wakeword.rs` | Selección/validación de recursos existentes, worker restringido y supervisor Rust. Protocolo fijo: PCM16LE mono 16 kHz por stdin; `R` al estar listo y un único `D` al detectar. |
| `src/backend/wakeword_ml.rs` | Reutiliza mel → embedding → clasificador; propaga errores del warmup, corrige índice relativo de embeddings por lotes y hace descubrimiento del runtime de sólo lectura. El test histórico que descargaba queda ignorado con requisito explícito de autorización. |
| `src/backend/hotword.rs` | Retira el detector por energía/ZCR y su loop paralelo antiguo. |
| `src/backend/voice_coordinator.rs` | Entrada `start_wake` bajo el mismo bloqueo de admisión, sesión/generación y cancelación F4. Transiciones `Preparing → WakeWaiting → Preparing → Recording → Transcribing → TranscriptReady`. |
| `src/backend/voice_process.rs` | Comparte el constructor PipeWire con dictado: PCM16/16 kHz, selección validada del micrófono y propiedades que impiden reconexión/fallback silenciosos. |
| `src/backend/http_server.rs`, `src/main.rs`, `src/backend/mod.rs` | `wake: true` opcional en el start autenticado existente y modo interno `--voice-wake-worker`. No acepta transcripciones/eventos de activación fabricados por clientes. No hay arranque automático. |
| QML `Main`, `InputBar`, `VoiceController`, `SettingsDialog` | Control explícito, indicador de espera/cancelación, ajustes de recursos/umbral y entrega al borrador. Usa tokens Theme y Octicon `hubot-16`. |
| `src/bin/test_wakeword.rs`, `scripts/test_hey_jarvis.py`, tests Rust/QML | Diagnóstico sólo offline con rutas explícitas, fixture reproducible y regresiones de autoridad/cierre. El diagnóstico anterior deja de sintetizar usando rutas personales hardcodeadas. |

Antes de capturar, se comprueban STT y recursos wake; el worker debe terminar su carga/warmup y responder `R`. ONNX corre en un hijo con entorno limpiado y un hilo de inferencia por sesión, fuera de Qt/Tokio. F4 usa el mismo supervisor sin shell, con terminación al morir el padre y `kill + wait` sobre los hijos propios. No se abre el adaptador cpal antiguo en paralelo.

Los frames tienen 1.280 muestras/80 ms. Se usa el score ML original contra el umbral; no se añaden heurísticas acústicas. La protección contra duplicados consiste en una activación consumible, salida única del worker, admisión exclusiva y revocación de identidades. NaN, scores fuera de rango y respuestas inválidas fallan sin activar.

Hay timeout de preparación de 15 s, vigilancia de lectura/escritura de 3 s, espera máxima de 120 s y backpressure con buffers acotados. El detector conserva sólo sus ventanas de trabajo en RAM, incluido un máximo de 10 s de PCM; no escribe grabaciones. Al detectar, ambos hijos wake se recogen **antes** de entrar al dictado F4. La captura de dictado se abre de nuevo; por eso se pide esperar a «Grabando». No se garantiza preservar una orden pegada a la misma frase de activación.

Se revoca Piper preparado antes de armar. Durante WakeWaiting se rechazan dictado duplicado, TTS y prewarm; STT empieza después de liberar ONNX wake. Se evita sumar sus modelos residentes y el TTS no puede reactivar el micrófono. Detener/cambiar sesión/apagar invalida resultados tardíos. No se modificaron la síntesis ni la autorización de herramientas del pipeline anterior.

## Pruebas y resultados

Pruebas con HOME/XDG/TMP privados mediante `f4_safety.private_env`, socket PipeWire deliberadamente inalcanzable y DBus inaccesible. Los hijos acústicos de los tests de supervisión producen bytes sintéticos desde Python; no ejecutan `pw-record` real. La aceptación ML invoca sólo los modos worker/diagnóstico, sin crear Backend, chat, tray ni proveedor externo. Los mocks HTTP son loopback.

| Verificación | Resultado |
|---|---|
| `cargo build --offline --bin kde-assistant --bin test_wakeword` | PASS |
| `cargo clippy --offline --all-targets -- -D warnings` | PASS |
| `cargo fmt --check`; `git diff --check` | PASS |
| Unit wakeword | 6 PASS; 1 test de descarga ignorado explícitamente |
| Unit VoiceCoordinator | 17 PASS, incluidos cancelación wake, rechazo de duplicados/TTS/prewarm, sesión obsoleta y shutdown |
| Unit HTTP F4 | 3 PASS; activación explícita sin mensajes, turnos ni auditoría de herramientas; rechazo de transcripción inyectada |
| Unit voice_process | 4 PASS; cancelación, timeout, límites, entorno y recogida de hijos |
| CLI inválida | 1 PASS con múltiples casos, incluidos argumentos wake inválidos y NaN antes del bootstrap |
| Arranque/configuración `f1b1_text_startup` | 16 PASS, 1 QML histórico ignorado; el caso OFF sin recursos se volvió a comprobar tras cambiar el default de ruta |
| QML F4 | 12 PASS; dos casos nuevos de activación/cancelación sin autosend |
| QML ajustes con mock correcto | 19 PASS; ruta/umbral persisten sin activar captura; nueve capturas temporales generadas por el arnés |
| Aceptación con modelos reales y PCM sintético | 6/6 PASS, tabla siguiente |

En el test específico de cancelación se comprobaron los dos PID propios antes de cancelar y su ausencia en `/proc` después de esperar la operación. Una observación: **2,635 ms** para cancelar y recoger ambos hijos sintéticos. No es una garantía de latencia del hardware o de toda la UI.

### ML offline en el N5000

Fixture positiva: fonemas ingleses `[[hˈeɪ dʒˈɑːɹvɪs]]`, sintetizados localmente con Piper español, speaker 0, escala 1, noise/noise_w 0 y semillas fijas. Recursos verificados primero con el validador Piper existente. Estas opciones pertenecen exclusivamente al generador de fixtures; no cambian Piper F4. SHA-256 del PCM positivo remuestreado: `77a58387c45bd3bb6938aca41685e1552fb62cb56bc49daaa436b6ce86c58d81`.

Umbral fijo 0,8 en todos los casos:

| Caso | Audio (s) | Score máximo | Protocolo del worker | Dictamen |
|---|---:|---:|---|---|
| Silencio | 6,00 | 0,000060 | `R` | PASS, sin detección |
| Tono 440 Hz | 6,00 | 0,003018 | `R` | PASS, sin detección |
| Ruido con semilla fija | 6,00 | 0,001690 | `R` | PASS, sin detección |
| Hey Jarvis inglés sintético | 3,76 | 0,901202 | `RD` | PASS, una detección |
| Hey Jarvis repetido | 5,44 | 0,990417 | `RD` | PASS, una detección total |
| Habla española no relacionada | 4,88 | 0,000224 | `R` | PASS, sin detección |

Host: Pentium Silver N5000, cuatro núcleos; build debug y ONNX Runtime CPU. Carga más warmup: 331–334 ms; p95 de inferencia por bloque: 10,48–10,74 ms, frente a 80 ms de audio. Pico RSS del diagnóstico: 39.744–40.112 KiB. Para 6 s de silencio/tono/ruido: 765–772 ms de procesamiento de stream y 1,135–1,141 s de CPU total **incluyendo arranque/warmup**. La relación de procesamiento/audio ronda 0,13. No es CPU total del escritorio ni latencia acústica extremo a extremo; excluye PipeWire, UI y STT. No demuestra tasas de falsos positivos por hora ni precisión en acentos humanos.

Se observaron también p95 de hasta ~86 ms mientras compilaban otras verificaciones, con throughput agregado todavía menor que tiempo real. No se oculta esa sensibilidad a carga del N5000; el resultado final anterior se midió sin esa compilación concurrente. El supervisor falla/cancela si el consumidor se atasca, en lugar de acumular audio sin límite.

### Fallos encontrados y corregidos durante el trabajo

- «Hey Jarvis» escrito normalmente para la voz **española** dio score ~0,0012: no sirve como fixture de pronunciación inglesa. La síntesis con ruido aleatorio y fonemas ingleses varió entre ejecuciones y llegó a 0,5355 en un positivo aislado. Se conservan aquí esos FAIL; el corpus final fija ruido y semillas para ser reproducible. No se bajó el umbral ni se atribuye aceptación humana a ese corpus.
- Un guard inicial de dos scores consecutivos descartaba activaciones breves válidas. Se retiró y se usa el umbral por frame del motor; los duplicados se evitan mediante autoridad/consumo de la activación.
- Clippy detectó `chunks_exact(2)`; se corrigió a `as_chunks` y Clippy final pasa.
- Una invocación directa del test de ajustes, sin sustituir sus placeholders HTTP, expiró a los 60 s. Después se usó su helper de mock existente. Un método mal nombrado en el test nuevo (`reloadConfig`) se corrigió a `loadConfig`; el arnés final pasa 19 casos. No fueron fallos de un dispositivo ni de una sesión personal.

## Reproducción y bloqueos

Desde la raíz, con **rutas de recursos existentes**; el script no descarga ni instala nada:

```text
cargo build --offline --bin kde-assistant --bin test_wakeword
python3 -B scripts/test_hey_jarvis.py --binary target/debug/kde-assistant --diagnostic target/debug/test_wakeword --models /ruta/wakeword --onnx-runtime /ruta/libonnxruntime.so.1.29.0 --piper-runtime /ruta/venv-piper --piper-model /ruta/es_ES-sharvard-medium.onnx
```

Alternativa: `--positive-wav /ruta/fixture-autorizada.wav` sustituye la síntesis de la frase. El generador usa Python stdlib y la API Piper ya instalada; no captura ni reproduce. Las pruebas Rust/QML deben ejecutarse con el entorno privado existente; ajustes QML requiere `validate_f35.qml_tests` para preparar su mock y sus placeholders. No se debe ejecutar el test histórico ignorado de descarga sin autorización específica.

**Bloqueos para afirmar aceptación real completa:** no hay autorización para probar micrófono/Wayland físico, ruido real, acentos, cierre con dispositivo real o el intervalo wake→dictado. No se abrió ese permiso ni se fingió evidencia. La orden debe empezar después del indicador de grabación por el relevo actual entre capturas. La licencia NC de pesos limita una futura distribución comercial; no bloqueó reutilizar los archivos locales en estas pruebas.

Todos los procesos de los arneses se esperaron/cerraron; los perfiles, WAV/PCM sintéticos y capturas temporales se eliminaron con sus directorios privados. Se mantiene la instalación privada previamente autorizada de Piper y los modelos ONNX existentes. Ningún perfil personal se abrió para escritura. Esta entrega añade un único informe nuevo y deja Hey Jarvis OFF.
