# F4 — Defectos, límites y trabajo pendiente

**Implementación PARTIAL; aceptación F4 NEEDS_FIXES.** Sin commits y sin continuación a otras fases.

## Defectos reproducibles corregidos en el camino productivo

| ID | Defecto | Corrección / evidencia |
| --- | --- | --- |
| F4-01 | Dictado dependía de wake/Piper/ONNX y no abría micrófono independientemente | Captura PipeWire bajo acción explícita; fixture sintético y coordinador |
| F4-02 | API STT elegida por existencia de key, sin consentimiento de audio | Coordinador sin cliente remoto; routing legacy local; spy loopback y source assertions |
| F4-03 | Estados/transcripciones obsoletos; requests QML sin identidad | Scope revocable, comparaciones de generación/sesión, rechazo de transición inválida, entrega única; UNIT/AUTOMATED_QML |
| F4-04 | Inferencia spawn_blocking no terminable y síntesis sin timeout | Proceso propio por operación, límites, cancelación y kill+wait |
| F4-05 | Barge-in compartía un flag reseteable; síntesis podía sobrevivir al stop | Un owner, cola uno, sin escucha durante TTS; fixtures de cancelación; motor real pendiente |
| F4-06 | Calibración guardaba WAV y sidecar podía inyectar intercambios | Rutas de mutación retiradas; sin grabaciones persistidas nuevas |
| F4-07 | Resampler lineal sin antialias; tasa/canales inválidos | PipeWire productivo; referencia sinc offline; señales 16/44,1/48 kHz y rechazo 12 kHz |
| F4-08 | Tray anunciaba PTT global sin registro | Acción de abrir entrada de voz, sin texto de atajo ficticio |
| F4-10 | Se retiró accidentalmente el registro DBus al extraer el bloque legacy de main | F3 reprodujo timeout de Activate; se restauró exactamente el registro previo y F3 completo pasó sin cambiar aserciones |
| F4-11 | El callback de creación de primera sesión podía iniciar dictado tras ocultar/cancelar | Generación UI y visibilidad en ambas fronteras async; Main.qml real con HTTP demorado, positivo 1 inicio y cancel/hide 0 |
| F4-12 | El nombre piper también puede identificar una aplicación de ratón ajena a TTS | El nuevo camino exige piper-tts y falla cerrado con piper genérico; otras variantes esperan validación explícita |
| F4-09 | Modo interno de comprobación no conectado podía iniciar Backend | Dispatch antes del logger/Backend; rechazo de otros --voice-*; runner comprueba ausencia de config tras esos comandos |

## Incidente de validación (no ocultar)

F4-INC-01: al diagnosticar F4-09 se ejecutó una vez `target/debug/kde-assistant --voice-model-check /nonexistent/f4-model` sin XDG privado. Arrancó perfil personal a las 15:52:05 UTC, backend 10857 y QML 10864. Se envió SIGTERM sólo al backend propio y se comprobó ausencia de esa instancia. No se activó micrófono/atajos ni se envió chat observado. Sí se abrió configuración/SQLite y se pudo modificar bootstrap, permisos o recuperación de arranque. No se dispone de una comparación previa del perfil para certificar que no cambió. No se realizó rollback especulativo ni se inspeccionaron transcripciones/conversaciones privadas. Las pruebas posteriores usan aislamiento explícito y verifican el dispatcher antes de iniciar el backend. Esto requiere revisión del usuario; no se describe toda la sesión como aislada.

## Limitaciones técnicas y falta de recursos

- LIVE_MICROPHONE: NOT_TESTED. Ocupación, permiso denegado, desconexión y cambio de dispositivo físico no están certificados por un EOF sintético. WirePlumber recibe propiedades de no relink/fallback, pendientes de prueba física.
- LIVE_TTS/HUMAN_VERIFIED de audio: NOT_TESTED. No hay Piper ni voces instalados. Compatibilidad CLI, sample rate efectivo, latencia al oído, selección de speaker y limpieza de eventuales nietos de wrappers externos siguen abiertas. El inventario heredado se conserva; la ejecución nativa rechaza el nombre genérico `piper` para no iniciar una aplicación ajena. La variante legítima de TTS con ese nombre necesita verificación explícita futura.
- LIVE_WAKE_WORD: BLOCKED por gates. Hay archivos ONNX, pero no una validación acústica/procedencia/licencia local completa. No se midieron verdaderos positivos/negativos, ruido ni activación por propia TTS. No se añadió escucha productiva.
- Motor STT definitivo: BLOCKED por precisión/performance; Whisper es una implementación provisional reutilizada. Tiny sólo tiene autorización de benchmark offline, sin instalación en el perfil personal.
- Corpus: señales CC0 y textos de referencia creados; faltan grabaciones españolas con derechos/consentimiento, frases largas/números/apps y variantes de pronunciación. WER de habla, precisión y silencios naturales NOT_TESTED.
- No hay consentimiento remoto de grabaciones. Se deshabilita la función; no se reutiliza autorización de claves.
- Modelo con header válido todavía puede tener pesos corruptos. El worker falla sin hacer efectos; no se valida SHA conocido para cualquier modelo elegido por el usuario. Integridad tiny descargado sí verificada.

## Deuda de rendimiento

El experimento base n=5 en N5000 fue muy lento: 81–115 s para una señal de 3 s, con compilaciones concurrentes y build dev (C++ RelWithDebInfo). No es aceptación de habla ni medición release. El timeout limita la espera, no arregla la latencia. Tiny autorizado también sigue lento: p50 39,891 s y p95 45,098 s, n=5 con señal sintética. Es necesario comparar alternativas pequeñas con habla real y build optimizado; estas mediciones no seleccionan un motor definitivo.

Carga por operación libera RAM y permite matar inferencia durante carga; en la muestra la carga base fue pequeña respecto de la inferencia. Caché persistente/TTL, optimización de contexto acústico y cuantización requieren evidencia antes de agregarse. No se fija presupuesto numérico de producto sobre estos resultados exploratorios.

## Fixtures y evidencia intermedia

Los primeros intentos del daemon sintético no negociaban puertos sin session manager. El fixture ahora configura sus puertos y enlaza únicamente nodos del daemon privado. La fuente SPA usa su seno por defecto de 440 Hz; se comprueba frecuencia/amplitud estrictamente, sin afirmar que aceptó un parámetro de frecuencia ignorado.

El primer test de reaping publicaba el archivo PID antes de escribirlo, permitiendo leer una cadena vacía y comprobar `/proc/` por error. Se corrigió con publicación atómica del marcador; se conserva la aserción de desaparición del PID exacto. No se trató un error de fixture como prueba de un huérfano productivo ni se relajó una suite heredada.

El runner de runtime reutilizaba el log en append y podía detectar readiness de una ejecución previa antes de existir el token nuevo. Ahora trunca el log al lanzar; dos ejecuciones finales consecutivas pasan sobre el mismo directorio. El cambio corrige el aislamiento temporal del fixture, sin debilitar comprobaciones.

## Funcionalidades futuras dentro de F4

Comparación Vosk/sherpa con modelos/licencias concretos y permiso para artefactos; corpus humano consentido; presupuestos con p50/p95 suficientes; micrófono real y UX de revisión; Piper real y oído; después wake con cooldown/FP/FN/eco. No corresponden a F5/F6 y no se han iniciado esas fases. Los campos de voz heredados no implementados permanecen deshabilitados o explícitos; no se anuncian como aceptación.
