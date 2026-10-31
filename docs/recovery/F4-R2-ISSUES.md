# F4-R2 — Incidencias, límites y trabajo pendiente

**STT_SELECTED provisional / LIVE_PTT_PASS (flujos probados) / F4 NEEDS_FIXES.** No consolidación ni commits.

| ID | Tipo | Estado y evidencia |
|---|---|---|
| R2-CAP-01 | Defecto reproducible del grabador nuevo | CORREGIDO. `--target=auto` publicaba destino literal auto junto a no-fallback; dos intentos EOF sin PCM/WAV. Cada reintento autorizado. Omitir target usa entrada predeterminada, fixture privado y cuatro capturas autorizadas positivas. La ruta productiva ya omitía target |
| R2-SAFE-01 | Validación incompleta de entorno externo | CORREGIDO. HOME/TMP y cuatro XDG deben ser privados, distintos, UID propietario y sin escapes. Guardas ampliadas; ningún nuevo arranque personal observado. No equivale a auditar/reparar el incidente histórico |
| R2-LIFE-01 | Parada durante Preparing | CORREGIDO. Una parada antes de PCM podía abrir captura brevemente o dejar transición Preparing→Transcribing inválida. Ahora regresa Idle sin audio; checks antes de preflight/dispositivo/spawn y consulta de dispositivos cancelable. Test sin modelo/captura real |
| R2-PERF-01 | Hashing dev en ruta crítica | MEJORADO. SHA-256 O0: preflight p50 2,896 s, worker largo 7,295 s. Optimizar sólo dependencia sha2 mantiene verificaciones y baja a 0,215 / 3,758 s, n=3 por configuración. No es una mejora de precisión ni una medición PTT completa |
| R2-TEST-01 | Socket de fixture demasiado largo | CORREGIDO. TMPDIR privado anidado excedía sockaddr_un (108 bytes). Daemon privado: ruta 121 B falla, 71 B funciona. Fixture usa directorio 0700 corto en /var/tmp, sin aflojar aserciones ni abrir hardware. [Diagnóstico](F4-R2-EVIDENCE/socket-diagnostic.json) |
| R2-TEST-03 | Carrera de teardown del fixture SQLite | CORREGIDO. Weak no actualizable podía observarse antes de cerrar flock. Test controla el destructor final con Arc::try_unwrap, mismo plazo y aserciones; tres HTTP focales y F1B3B completo PASS. Sin cambio productivo de DB ni F2 |
| R2-TEST-02 | Clippy int_plus_one | CORREGIDO. Límite de texto expresado como suma < MAX_TEXT, equivalente a suma+1 <= MAX_TEXT. Sin aumento de buffers, excepciones lint ni cambios semánticos |
| R2-ACC-01 | Defecto de reconocimiento reproducible | ABIERTO. Vosk falla Dolphin/LibreOffice y configurar en la frase de aplicaciones (85,71% WER de esa muestra). Dictado siempre editable. No usar voz como autorización exacta ni asumir nombres correctos |
| R2-ACC-02 | Precisión Whisper/números | ABIERTO. Completo 0/3 cantidades correctas, reducido 1/3 frente a 3/3 Vosk en esta toma. Diferencias no explicables sólo por formato numérico. WER completo/reducido agregado 27,5/30%; no seleccionados para integración inicial |
| R2-CORPUS-01 | Límite de evaluación | Cuatro WAV, un hablante, 40 palabras. Referencias confirmadas y n=3 inferencias, sin triplicar corpus. No hay habla con ruido, otros acentos ni estadísticas amplias. No grabar las referencias adicionales sin nueva autorización |
| R2-CTX-01 | Riesgo de truncamiento | Reducido proporcional a duración+1 s en benchmark. Los tres conservaron el final largo; una frase no elimina el riesgo general. Default Whisper completo intacto, sin imponer contexto 256 |
| R2-PTT-01 | Falta de aceptación física productiva | PASS para lo probado. Dictado/edición y Cancelar físico confirmados por el usuario; registro de Cancelar con cero LLM/mensajes, estado final idle y todos los procesos recogidos. Falta latencia completa cronometrada y cobertura de otros dispositivos/desconexión. Las capturas anteriores de corpus siguen siendo evidencia distinta |
| R2-RES-01 | Recursos y limpieza | Corpus cerrado y eliminado, con resúmenes sanitizados conservados; Vosk y ambos perfiles PTT también eliminados al cierre, sin Git ni instalación global. Usuario autorizó retener Vosk temporalmente para PTT aislado. WAV/hipótesis/logs ya eliminados al cerrar comparación; Vosk/perfiles ya eliminados al cerrar evaluación PTT; sin borrado forense. Registrar la limpieza real, sin afirmar anticipadamente que ocurrió |
| R2-NATIVE-01 | Alcance del adaptador | Sólo libvosk 0.3.45 x86_64 y small es 0.42 evaluados, hashes fijados. No admite modelos alternativos ni autodetección. Preflight y worker verifican recursos de nuevo; no se retiene modelo caliente. Probar otras versiones exige revisión |
| R2-BUILD-01 | Deuda de optimización | Whisper O2 CPU/SSE, sin comparación release nueva. No instalar CMake ni compilar varias variantes pesadas para esta etapa; sin aumento de timeout como solución. El costo C++ sigue material |
| R2-DIST-01 | Distribución futura | Vosk/modelo Apache-2.0 y libloading ISC documentados. Falta inventario completo de avisos/licencias de dependencias nativas y empaquetado antes de F6. No compromiso de distribución en esta etapa |
| F4-INC-01 | Incidente histórico | No inspección de archivos personales ni rollback ciego. Modos internos antes de Backend y allowlist estricta verificados sobre perfiles sintéticos. No certificación retrospectiva de efectos del incidente |
| R2-TTS-WAKE | Gate pendiente / fuera de alcance correctivo | Sin motor/voz Piper real ni prueba audible. Sin nuevo desarrollo Hey Jarvis. No descargas ni instalación; F4 no aceptada ni asistente manos libres |

Prioridad siguiente: revisión técnica de R2; Cancelar/liberación ya confirmados y evaluación PTT cerrada. El resumen final del primer launcher sigue ausente, sin reconstrucción ficticia. La primera prueba PTT humana de dictado editable ya fue positiva. Sólo tras aceptación de reconocimiento preparar propuesta Piper con recursos/licencias y pedir aprobación. No F5/F6, herramientas adicionales o cambios de seguridad F2.

| ID | Tipo | Estado y evidencia |
|---|---|---|
| R2-EVID-01 | Resumen final del primer launcher ausente | Limitación histórica de evidencia: causa no determinada. No se inventan llamadas al mock ni latencia. Snapshot previo de transcript_ready/cero mensajes y confirmación humana disponibles; tres PID ya desaparecidos, sin audio en disco |
| R2-EVID-02 | Observación dependiente del cierre | CORREGIDO para pruebas siguientes: progreso atómico sin transcripción, log vaciado y cleanup con reaping explícito. Fixture simula fallo del observador y recoge proceso propio. 12 tests focales y runner F4 completo PASS, sin hardware |

La solicitud separada de Cancelar recibió del usuario «continua». Se preparó el perfil `/var/tmp/kda-f4r2-ptt-gpfkwmrz` y un recibo de un uso para esa prueba manual; el usuario abrió la ventana e inició físicamente los ciclos. Confirmó Cancelar correcto. El registro describe tres ciclos reales, sin fingir una única toma ni scoring de precisión. La captura nunca se inició automáticamente.

## Propuesta posterior Piper — sin instalación ni autorización implícita

Candidatos para una evaluación privada posterior a la revisión técnica, no un cambio ya aceptado:

| Recurso | Fuente/versión | Tamaño de descarga conocido | Licencia/condición |
|---|---|---:|---|
| Motor piper-tts 1.8.0, wheel Linux x86_64 cp39-abi3 | [OHF v1.8.0](https://github.com/OHF-Voice/piper1-gpl/blob/v1.8.0/README.md), [metadatos oficiales PyPI](https://pypi.org/pypi/piper-tts/1.8.0/json) | 34.131.442 B | GPL-3.0-or-later, Python >=3.9; no confundir con etiquetas históricas Apache/MIT de otros componentes |
| es_ES-sharvard-medium ONNX | [Modelo](https://huggingface.co/rhasspy/piper-voices/blob/main/es/es_ES/sharvard/medium/es_ES-sharvard-medium.onnx), [inventario fijado](https://huggingface.co/rhasspy/piper-voices/blob/2525f22960f62209936f0ae3cce3ffe7b2c18f1f/voices.json) | 76.733.615 B | [Model card](https://huggingface.co/rhasspy/piper-voices/raw/main/es/es_ES/sharvard/medium/MODEL_CARD): español de España, 2 hablantes, 22.050 Hz; dataset CC BY 3.0. Revisar avisos y licencia del modelo, sin sustituirla por el badge genérico del repositorio |
| Configuración ONNX JSON correspondiente | Mismo inventario | 7.045 B | Necesaria para sample rate/speakers; se fijará revisión y hash antes de pedir descarga |

Subtotal conocido 110.872.102 B (≈105,7 MiB), **no total instalable**: faltan resolver versiones/tamaños de ONNX Runtime CPU, pathvalidate y dependencias transitivas compatibles con Python/CPU del host. No se descargó ningún wheel, modelo ni dependencia: sólo documentación/metadatos. [Inventario documental](F4-R2-EVIDENCE/piper-proposal-metadata.json). RSS, CPU, carga y latencia Piper siguen NOT_TESTED; tamaño de ONNX no equivale a RAM.

Si se autoriza después, usar directorio nuevo `/var/tmp/kda-f4-piper-*` 0700 y entorno privado, sin instalación global, sin servidor HTTP Piper ni Python residente. Rust mantendría procesos y cancelación. La [CLI 1.8.0](https://raw.githubusercontent.com/OHF-Voice/piper1-gpl/v1.8.0/src/piper/__main__.py) admite las opciones raw/velocidad usadas, pero el coordinador exige nombre verificado piper-tts: falta revisar la selección del ejecutable privado/entrypoint y dependencias antes de preparar una instalación concreta. No se implementa esa adaptación ahora.

Alternativas: mantener TTS pendiente o auditar una distribución nativa concreta, sin asumir que una release histórica o el nombre genérico piper son seguros/compatibles. Revisar condiciones GPL/modelo para futura distribución antes de F6; F6 no se inicia. Tras resolver inventario total se pediría permiso específico de descarga y, por separado, reproducción audible con volumen controlado, texto sintético y parada. Limpieza de entorno/modelo/audio al cerrar, sin promesa forense. Hey Jarvis permanece fuera de esta corrección.
