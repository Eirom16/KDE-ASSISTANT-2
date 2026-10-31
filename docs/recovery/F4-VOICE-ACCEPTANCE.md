# F4 — Aceptación funcional de voz R5

Campaña humana FAST TRACK cerrada. **Voz R5 propuesta para revisión funcional con las excepciones documentadas de T3 y del arnés T5. F4 global permanece PARTIAL, sin consolidar.** No F4-R6, nuevas optimizaciones, Hey Jarvis ni F5/F6.

## Resultado por prueba

| Prueba | Resultado técnico | Confirmación humana | Dictamen |
|---|---|---|---|
| T1. Lectura española con modelo preparado | Ready confirmado; mismo worker/generación reutilizados; una lectura terminada; procesos recogidos | «La escuche completa y la calidad fue excelente» | LIVE_TTS_PASS |
| T2. Detener durante reproducción | Speaking → Idle; identidad revocada; worker y reproductor recogidos, ausencia comprobada | «Funciono perfectamente» | LIVE_TTS_PASS |
| T3. Nueva reproducción después de detener | Nueva lectura iniciada; detención manual confirmada. El arnés falló su aserción de final natural, coherente con esa intervención. Reinicio de aplicación entre T2 y T3 | «Lo detuve, ya que me confundi de prueba. Pero de todas formas se escucha perfecto y en la anterior prueba se escucho completo. Asi que no creo que haya necesidad de repetirla» | PARTIAL; no repetir por decisión del usuario |
| T4. Cambiar conversación durante reproducción | Sesión vocal inicial verificada; cancelación, identidad revocada y procesos recogidos | «Funciono perfectamente, al cambiar de sesion se detuvo la voz» | LIVE_TTS_PASS |
| T5. Autolectura de respuesta LLM mock | Ready durante espera del mock, una llamada, una síntesis, una reutilización; identidad con Turn ID conservada al finalizar. Runner FAIL por comparación estricta del mensaje de entrada; respuesta persistida exacta verificada después | «Sí, se leyó automáticamente completa y correctamente» | LIVE_TTS_PASS para la escucha; excepción técnica del fixture preservada |

T3 no acredita nueva lectura completa dentro del mismo proceso después de Stop. La escucha completa de T1 no se atribuye a T3. No hay evidencia de fallo espontáneo de Piper en esa interrupción: el usuario confirmó que pulsó Detener. Se respeta su decisión de no repetir. El PASS histórico de T1 R3 sigue separado de esta aceptación R5.

## Baseline, recursos y aislamiento

Rama `recovery/f4-voice-coordinator`; HEAD `9544a58eba4b6adf6c6343c04db80813b084f76b`; un worktree e índice vacío. Cambios locales F4/R/R2/R3/R4/R5 conservados, sin commits/push/merge/release. Binario idéntico al aceptado R5, SHA-256 `b8a977a6bd86d1d1bd7e208e051c0a4ba82401f5d2929d65521d9bfd5f9778bf`.

Piper 1.8.0, ONNX Runtime 1.31.0, voz `es_ES-sharvard-medium`, speaker 0, length scale 1, PCM16LE mono 22.050 Hz. Misma instalación privada previamente autorizada; integridad completa verificada por el modo interno nativo antes de cada lanzamiento. Sin nuevos recursos, instalación global, cambios acústicos ni rebuilds.

Perfiles exclusivos de campaña bajo `/var/tmp/kda-f4-live-sg_4h88k`, con HOME, TMP y cuatro XDG privados y distintos. Preparación con PipeWire inalcanzable; sólo después de cada permiso se comparte el socket de salida consentido y el socket Wayland. DBus personal permanece inaccesible; `--no-shortcuts`; puerto HTTP efímero y bearer de perfil privado. SQLite sintética; único proveedor mock loopback sin herramientas, claves reales, memoria ni resumen automático. STT configurado con modelo ausente, micrófono/wake/chimes apagados. No se inició captura, proveedor remoto, acción del sistema ni perfil personal.

Las ventanas muestran títulos privados y usan los controles productivos. En las copias QML sólo se cambió el título para distinguirlas. Los clics Leer, Detener, cambio de conversación y Enviar pertenecen al usuario. No se presentan gestos simulados como evidencia humana.

## Consentimiento y ejecución

Cada prueba tuvo propósito, nonce, frase, hash de binario y número de prueba propios en recibo privado consumible una vez. No se reutilizó R3 ni un permiso de otra prueba. T1 recibió «lo autorizo»; T2 «Ok, procede»; T3 «Si, lo autorizo»; T4 y T5 «OK, procede». Se esperó confirmación humana antes de preparar la siguiente escucha.

T1: mock SSE pendiente sin respuesta final; Rust prepara ONNX sin hablar. Después de Ready y aviso, se solicita una sola lectura manual de «Hola. Soy KDE Assistant y esta es una prueba de voz en español.». Preparación observada 5,976 s, solicitud preparada → Speaking 1,696 s, Idle 5,860 s desde la solicitud. Son observaciones únicas, no percentiles ni tiempo acústico medido. Volumen y latencia percibida no fueron valorados explícitamente.

T2/T3/T4 usan el texto sintético largo «Esta es la prueba de detención de voz…». Autolectura OFF. T2 valida Stop. La ventana T2 expiró y fue recogida mientras se esperaba T3; se informó de ello y se reabrió el mismo perfil. T3 fue interrumpida por confusión y se conserva su resultado incompleto. T4 recibió permiso que incluía reabrir si expiraba; seleccionando «F4 voz · Sesión alternativa» se detuvo la respuesta antigua. La atribución del gesto de cambio de conversación proviene de la confirmación humana, además del estado técnico; Idle por sí solo no demuestra ese gesto.

T5: nuevo perfil con conversación vacía, Voz rápida y autolectura ON sólo para el escenario. Arranque Idle/Cold, cero llamadas y cero síntesis. El usuario pulsa Enviar una vez; mock SSE local con espera artificial de 12 s y respuesta completa fija: «Esta es una respuesta de prueba. La lectura automática comienza cuando termina la respuesta del asistente.». No se pulsa Leer ni se envía un prompt adicional para TTS. Ready se observó durante la espera, antes de síntesis. Una llamada mock, una síntesis y una reutilización; identidad de sesión/turno/generación conservada hasta el final natural. El backend y los hijos se cierran después de esta única lectura.

Observación T5: Ready a 26,813 s del monitor; Preparing de síntesis a 32,156 s; Speaking a 34,664 s; Idle a 40,947 s. El monitor incluye el tiempo que el usuario tardó en enviar. El intervalo Preparing → Speaking fue aproximadamente 2,508 s y no es primer audio audible. La espera de 12 s es del fixture; no mide rendimiento de un proveedor real. No se repitieron benchmarks ni se calculan percentiles con esta toma.

## T5 — FAIL del arnés y diagnóstico

Se conserva íntegro el FAIL/exit 1 de `live_f4_auto_tts.py`: sus campos `persisted_before_speaking` y `persisted_after_speaking` evaluaban conjuntamente el prompt exacto y la respuesta exacta. El usuario envió la frase de prueba sin punto final (21 caracteres frente a 22 esperados). No se modifica esa aserción para obtener PASS y no se repite audio.

Auditoría posterior de sólo la SQLite sintética, en modo lectura: exactamente un mensaje user y uno assistant; el prompt coincide al omitir el punto; la respuesta assistant coincide exactamente con el texto del mock; turno completed; campos de herramientas vacíos y `tool_audit` con cero filas. Una única petición al mock y una única síntesis excluyen un prompt adicional de voz en esta ejecución. Identidad conservada, salida terminada y procesos recogidos. La respuesta persistida correcta queda verificada después; no se registró un hash independiente del contenido assistant durante Speaking, por lo que no se certifica retrospectivamente ese subcheck con un dato inexistente. La implementación conserva el orden publicar turno → solicitar lectura en Rust.

La excepción observada es de correspondencia del fixture de entrada, no evidencia de texto alterado por TTS. La escucha completa confirmada permite LIVE_TTS_PASS audible de T5, separado del runner FAIL. [Resultado original](F4-VOICE-ACCEPTANCE-EVIDENCE/t5-technical.json) y [diagnóstico](F4-VOICE-ACCEPTANCE-EVIDENCE/t5-diagnostic.json).

## Pruebas y código reutilizado

Sin cambios al núcleo Rust/QML productivo durante la campaña. Se reutilizaron `f4_safety`, helpers de perfiles/archivos/recibos, backend/API/log acotado de `live_f4_ptt`, verificación Piper existente y mock de `live_f4_fast_tts`. Nuevos arneses de aceptación: `live_f4_voice_gui.py` para controles físicos T2–T4 y `live_f4_auto_tts.py` para el único Enviar T5. Mock HTTP con Python stdlib (PSF), sin autoridad de agente. REUSE FIRST aplicado sin nuevas bibliotecas, descargas ni reinstalación de componentes funcionales; procedencia/licencias del motor y voz permanecen en R3/R5.

Diez fixtures UNIT de consentimiento: PASS. Cubren aislamiento, autorización ausente, permisos cruzados T1–T5, consumo único, cambio de configuración y recibos symlink antes de tocar binario/socket. No demuestran calidad audible. `git diff --check`: PASS. No se repiten compilaciones, Clippy, suites completas o benchmarks R5 ya aceptados: esta campaña conserva el binario y sólo añade arneses/evidencia. Los gates offline R5 mantienen su evidencia previa, sin atribuirles una nueva ejecución.

Comandos de campaña (históricos; recibos consumidos y perfiles ya eliminados):

```text
python3 -B scripts/live_f4_fast_tts.py --launch --profile /var/tmp/kda-f4-live-sg_4h88k/t1 --receipt /var/tmp/kda-f4-live-sg_4h88k/t1/permission.json
python3 -B scripts/live_f4_voice_gui.py --launch --profile /var/tmp/kda-f4-live-sg_4h88k/t2 --receipt /var/tmp/kda-f4-live-sg_4h88k/t2/permission.json
python3 -B scripts/live_f4_voice_gui.py --launch --trial 3 --profile /var/tmp/kda-f4-live-sg_4h88k/t2 --receipt /var/tmp/kda-f4-live-sg_4h88k/t2/permission-t3.json
python3 -B scripts/live_f4_voice_gui.py --launch --trial 4 --profile /var/tmp/kda-f4-live-sg_4h88k/t2 --receipt /var/tmp/kda-f4-live-sg_4h88k/t2/permission-t4.json
python3 -B scripts/live_f4_auto_tts.py --prepare --profile /var/tmp/kda-f4-live-sg_4h88k
python3 -B scripts/live_f4_auto_tts.py --launch --profile /var/tmp/kda-f4-live-sg_4h88k/t5 --receipt /var/tmp/kda-f4-live-sg_4h88k/t5/permission.json
python3 -B scripts/test_f4_fast_tts.py
git diff --check
```

T1/T2/T4 terminan técnicamente según su escenario. T3 conserva FAILED en la aserción de final natural y PARTIAL en aceptación, por detención humana. T5 conserva exit 1 de la comparación estricta descrita. No se ocultan fallos ni se cambia su semántica.

## Limpieza, limitaciones y decisión

Todos los procesos observados propios de backend, GUI, worker y reproductor están ausentes; las rutas de cierre hicieron terminate/kill+wait según los helpers existentes. Perfiles, SQLite sintéticas, recibos consumidos y logs privados de campaña eliminados. No se guardó PCM/WAV. Se conserva la instalación privada autorizada de Piper, externa al perfil eliminado; no se retira ni instala nada global. No se promete borrado forense. [Evidencia de limpieza](F4-VOICE-ACCEPTANCE-EVIDENCE/campaign-cleanup.json).

Evidencia técnica y humana escalar en `F4-VOICE-ACCEPTANCE-EVIDENCE/`, sin bearer, credenciales, grabaciones ni contenido de conversaciones personales. No se inspeccionó ni restauró el perfil del incidente histórico F4-INC-01. F2 conserva su catálogo y 19 herramientas bloqueadas; no se tocó consentimiento ni configuración personal.

**Decisión:** capacidad de leer, detener, cancelar al cambiar de conversación y autolectura mock con Piper preparado observadas y confirmadas por la persona. Campaña cerrada con T3 PARTIAL y la excepción de fixture T5, sin defecto productivo espontáneo confirmado y sin nueva repetición solicitada. No equivale a certificación exhaustiva de todos los dispositivos, sesiones, carga del host o acentos. STT/PTT conserva su aceptación previa R2 sin nuevas capturas.

**Aceptación audible R5:** LIVE_TTS_PASS para T1/T2/T4/T5; T3 PARTIAL. **F4 global:** PARTIAL, pendiente de revisión técnica; sin commits ni consolidación. Se solicita revisar este informe y sus excepciones antes de autorizar continuar con Hey Jarvis, aplicando REUSE FIRST. No se inicia wake word ni se solicitan recursos nuevos en esta campaña.
