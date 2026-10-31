# F4-R2 — Selección provisional e integración inicial STT

**STT_SELECTED (provisional): Vosk small español 0.42, runtime 0.3.45. LIVE_PTT_PASS (flujos probados). F4: NEEDS_FIXES.** La evidencia permite integrar dictado editable experimental; no permite aceptar F4 ni afirmar precisión general o uso manos libres. La primera prueba PTT ya tuvo valoración humana positiva de rapidez y edición; Cancelar físico también fue confirmado; falta la salida audible y ampliar precisión/medición.

## Baseline y conservación

Rama `recovery/f4-voice-coordinator`, HEAD/base consolidada `9544a58eba4b6adf6c6343c04db80813b084f76b`; un worktree, índice vacío. Sin commits, ramas nuevas, push, merge, release, F5/F6, reset, stash o descarte. Los cinco informes F4 y cuatro F4-R se conservan; [hashes iniciales](F4-R2-EVIDENCE/baseline.json). Se leyeron AGENTS.md, PLAN/PROMPT/diseño, los nueve informes anteriores y el contrato de voz.

No se inspeccionó ni cambió configuración, SQLite, credenciales o metadatos del perfil personal. No se intentó restaurar el incidente histórico F4-INC-01. La prevención actual no convierte aquel arranque en una ejecución aislada retrospectivamente.

## Evidencia española y decisión

El usuario autorizó cuatro tomas por separado y confirmó después las palabras realmente pronunciadas. Un hablante, cuatro WAV idénticos entre motores, 40 palabras únicas, tres inferencias por configuración. WER agregado observado: Vosk 17,5%, Whisper tiny completo 27,5%, reducido 30,0%. Vosk conservó las tres cantidades y el final de la frase larga; confundió Dolphin y LibreOffice. Inferencia Vosk p50 por muestra: 1,042–2,530 s; Whisper completo 36,985–38,320 s. [Diseño, resultados, recursos y límites](F4-R2-BENCHMARKS.md).

La selección combina latencia, recursos y errores observados; no se decide por el tono ni sólo por velocidad. El resultado de aplicaciones es un defecto material de reconocimiento. El corpus pequeño no representa todos los acentos, entornos o ruido. No se adaptó el motor a las referencias. Las repeticiones no triplican hablantes ni referencias.

Se integró Vosk como **opción explícita experimental**; Whisper conserva el default y no recibe cambios de decodificación productiva. La reducción de contexto Whisper sigue siendo una opción de benchmark; no se impone 256 a frases largas ni se presenta como mejora de precisión.

## Autorización y datos

Vosk se descargó de nuevo con permiso **sólo para evaluación offline**: modelo small es 0.42 (39.817.833 B) y wheel oficial runtime 0.3.45 (7.173.758 B), destino `/var/tmp/kda-f4r2-vosk-pruy5scb` 0700, archivos 0600, sin pip ni instalación global. SHA del runtime contrastado con PyPI; SHA del modelo con inventario previo, sin afirmar firma/hash upstream. [Fuentes y verificación](F4-R2-EVIDENCE/vosk-download.json).

Las cuatro tomas voluntarias de 8/10/10/15 s se evaluaron privadas y offline en `/var/tmp/kda-f4r2-corpus-gjw89ydm`; al cerrar la comparación se eliminaron WAV, hipótesis y logs privados. Dos primeros intentos cortos fallaron con cero PCM y sin WAV; cada reintento tuvo permiso explícito. [Registro sanitizado](F4-R2-EVIDENCE/corpus-plan.json). No se infiere autorización para nuevas tomas, PTT o audio remoto. Referencias propias CC0; no se concede licencia para redistribuir las grabaciones.

La limpieza del corpus ya se realizó ([registro](F4-R2-EVIDENCE/cleanup-corpus.json)). Vosk y ambos perfiles PTT también se eliminaron al cerrar la evaluación, sin prometer borrado forense. La evaluación PTT ya está cerrada y su limpieza se completó ([registro](F4-R2-EVIDENCE/cleanup-final.json)); no quedó instalación permanente. Después el usuario amplió expresamente el permiso: «Autorizar uso temporal de Vosk para PTT aislado». Esa ampliación no activó el micrófono. El usuario autorizó después una primera prueba PTT y confirmó estar listo; la primera prueba se ejecutó: el usuario confirmó rapidez y edición. Después de la solicitud separada de Cancelar el usuario indicó continua, ejecutó la prueba y confirmó que funcionó correctamente. Ambos perfiles privados y Vosk se eliminaron al cerrar; ninguno contenía audio crudo.

## Arquitectura y contrato suplementario R2

El único propietario sigue siendo `VoiceCoordinator`: identidad sesión/generación/turno, captura PipeWire existente, cancelación y reaping antes de aceptar otra operación. Los modelos se cargan sólo bajo demanda en un proceso hijo terminable, uno por operación; no hay residente Python ni modelo duplicado permanente.

`voice_stt::Selection` selecciona Whisper o Vosk desde configuración local. No hay fallback remoto ni entre motores: desconocido, idioma incompatible o rutas inválidas fallan de forma comprensible. Vosk small es exige `es`/variante española; no autodetecta idioma. Preflight valida antes de abrir el micrófono. Worker recibe PCM16 LE mono 16 kHz por stdin, máximo 120 s / 3,84 MB; emite una cadena JSON limitada. No recibe tokens, credenciales, autoridad de conversación, herramientas, DBus ni configuración personal.

`vosk.rs` usa la API C nativa con ownership RAII de biblioteca/modelo/recognizer. Sólo acepta el libvosk evaluado (tamaño y SHA256), carga el inode abierto verificado mediante `/proc/self/fd`, y verifica tamaño/SHA de los 13 archivos del modelo evaluado. No es sandbox frente a otros procesos del mismo UID. La aceptación de otras versiones/modelos requiere revisión; no se permite dlopen arbitrario desde un campo visual. Los errores del worker llegan sin volcar PCM ni transcripciones a logs.

Los modos `--voice-vosk-check` y `--voice-vosk-worker` se despachan antes de logger, runtime, Backend, configuración o SQLite. Aridad inválida, modo desconocido o argumento interno mal colocado falla antes del arranque normal. La guarda F4-INC-01 se conserva y amplía con casos nuevos.

El texto reconocido llega a la previsualización/borrador editable existente; no envía al LLM, persiste mensajes ni ejecuta herramientas. El envío deliberado sigue el mismo backend, sesiones y consentimiento F2. Sin cambios al catálogo vigente o las 19 herramientas bloqueadas. El pipeline de voz no otorga autorización adicional.

## Correcciones y código

- `src/backend/vosk.rs`, `voice_stt.rs`, `main.rs`, `backend/mod.rs`: adaptador C confinado al worker, selección, modos internos estrictos.
- `voice_coordinator.rs`, `voice_process.rs`: selección local y consulta de dispositivos cancelable; Detener durante Preparing vuelve a Idle sin abrir captura. Se distingue parada sin audio de transcripción completada. La cancelación recoge el proceso; no deja una transición Preparing→Transcribing inválida bloqueada.
- `models/config.rs`, `http_server.rs`, `qml/Main.qml`, `SettingsDialog.qml`: tres campos de voz (`stt_engine`, `vosk_library`, `vosk_model`), presencia del recurso seleccionado y controles existentes de tema. Sin cambio personal ni otros campos F3.5.
- `Cargo.toml`/lock: `libloading=0.8.9` ya disponible, ISC, sin descarga; dependencia root añadida al lock sin actualizar paquetes. `sha2` O3 sólo en dev, aplicación O0. El primer worker gastaba segundos en hashing O0: preflight p50 2,896→0,215 s; worker largo completo 7,295→3,758 s, n=3. Se conservan todas las comprobaciones y límites.
- `scripts/f4_spanish.py`, `compare_f4_spanish.py`, `record_f4_spanish.py`, `benchmark_native_vosk_f4.py`: corpus con consentimiento, WER acotado S/D/I, comparación emparejada, grabador de un uso y medición nativa por etapas. Herramientas de evaluación, sin autoridad de agente.
- `f4_safety.py`: exige HOME/TMP y cuatro XDG privados, distintos y sin escapes; antes sólo comprobaba cuatro XDG cuando recibía un entorno externo. Los generadores ya creaban HOME/TMP privados; no se ha observado un nuevo arranque personal.
- `live_f4_ptt.py`: escenario privado manual preparado; modo live exige permisos separados para uso Vosk, micrófono y preparación, recibo de un uso. Sin clics ni grabación automática. Sólo mock loopback, sin herramientas, lectura automática, wake o chimes. Primera ejecución live realizada por el usuario, con resultado editable y valoración cualitativa positiva.
- Tests Rust de selección, integridad, CLI, preparación, paridad/cancelación de workers; QML de serialización/persistencia de voz y runner secuencial R2. El mock F3.5 conserva ahora `speech` tras una escritura reconocida, para poder comprobar recarga real del formulario; rechazos/pending no persisten.

El nuevo grabador falló al pasar `--target=auto` con no-fallback: PipeWire publicaba literalmente target.object=auto y WirePlumber no encontraba ese nodo. Se corrigió omitiendo el target por defecto. La ruta productiva ya lo omitía. Fixture con daemon privado comprueba la propiedad efectiva, sin hardware; la captura de corpus corregida fue autorizada por separado.

## Dependencias y distribución

API de audio productiva: clientes nativos PipeWire `pw-record`/`pw-dump`, formato negociado 16 kHz mono; sin permisos X11 ni global shortcuts. Vosk runtime 0.3.45 Linux x86_64 funcionó en este N5000 sin asumir AVX/CUDA; únicamente C API, sin importar/instalar su paquete Python en producción. Modelo small es 0.42 Apache-2.0 y runtime Apache-2.0 según fuentes oficiales; licencias de dependencias nativas/redistribución del binario y empaquetado deben revisarse antes de F6, que no se inicia aquí.

## Estado por ámbito

| Ámbito | Estado | Límite |
|---|---|---|
| Corpus humano y comparación offline | IMPLEMENTED | Cuatro frases, un hablante; sin ruido/acentos amplios |
| Selección inicial STT | STT_SELECTED provisional | Vosk, con fallos materiales de aplicaciones |
| Integración nativa y lifecycle | IMPLEMENTED para revisión | Pruebas offline, fixtures y PTT humano de dictado/cancelación; no todos los fallos físicos |
| PTT físico en KDE Assistant | LIVE_PTT_PASS para lo probado | Dictado/edición y Cancelar HUMAN_VERIFIED; cero mensajes y llamadas LLM en la ventana de Cancelar, procesos recogidos. Sin certificación general de micrófonos/desconexiones |
| Piper audible | NOT_TESTED | Sin motor/voz real instalados o autorizados |
| Hey Jarvis | Sin trabajo nuevo | F4 continúa sin certificación wake |

**F4 NEEDS_FIXES**: faltan medición completa de latencia hasta UI, precisión más amplia y voz sintetizada audible. Cancelar/liberación física ya tienen evidencia positiva. Hay una primera prueba de producto con micrófono y valoración cualitativa positiva; no se generaliza a otras frases o entornos. No se instala Piper ni se desarrolla wake para ocultar esos gates. Los resultados finales de regresión se detallan en [F4-R2-RESULTS](F4-R2-RESULTS.md).

La regresión F1B3A expuso una carrera en el teardown del fixture: Weak no actualizable podía observarse antes de acabar el destructor que cierra SQLite/flock. El test conserva el último Arc y completa la destrucción antes de reabrir, manteniendo plazo y aserciones. Tres casos focales y F1B3B completo PASS; no se modificó código productivo de DB ni F2.

El primer launcher no dejó resumen final; no se inventan contadores de mock ni tiempos. Su observador se endureció para guardar progreso atómico sin transcripciones y completar cleanup aun si falla la consulta. Doce tests Python y el gate F4 afectado PASS; sólo se ejercitaron procesos y audio sintéticos. La valoración humana y el snapshot privado previo del backend se conservan como evidencia distinta.

## Cierre físico y dictamen

La segunda ventana dejó registro completo: el usuario inició tres ciclos físicos, dos que regresaron de recording a idle y uno que llegó a transcript_ready. Se informa lo observado, sin presentarlo como una única toma ni como un corpus de referencia. Cero peticiones al mock LLM, cero mensajes, sin PCM/WAV y todos los procesos propios recogidos. El usuario confirmó «funciono correctamente» al preguntarle por Cancelar y borrador vacío. [Evidencia](F4-R2-EVIDENCE/live-ptt-cancel.json).

STT_SELECTED: Vosk provisional. LIVE_PTT_PASS: Grabar/Detener, edición, no autoenvío y Cancelar/liberación, dentro del escenario privado probado. F4 NEEDS_FIXES: Piper audible aún NOT_TESTED y precisión/latencia completa con límites documentados. No se declara F4 aceptada. Los cuatro informes R2 están disponibles para revisión técnica; no hay commits ni continuación de otra fase.

La propuesta posterior de Piper/voz española, sólo documental y sin descargas, está en ISSUES. El motor actual propuesto es GPL-3.0-or-later; no se hereda la etiqueta Apache-2.0 histórica para ese componente. Faltan revisión de dependencias/licencias, autorización de recursos y prueba audible.
