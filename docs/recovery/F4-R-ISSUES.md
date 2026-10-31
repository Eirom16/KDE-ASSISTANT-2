# F4-R — Incidencias, bloqueos y revisión

**Rendimiento IMPROVED sólo en experimentos offline; F4 NEEDS_FIXES.** No hay consolidación ni commits.

## Corregido y probado

| ID | Hallazgo | Resolución |
| --- | --- | --- |
| R-SAFE-01 | Un typo fuera del prefijo --voice- podía ignorarse e iniciar la app | Lista explícita de CLI normal validada antes del logger/Config/Backend; 14 escenarios en fixture privado |
| R-SAFE-02 | Helpers F4 aceptaban un env arbitrario suministrado por el caller | Creación/validación común de HOME/TMP/XDG, permisos, socket/bus y overrides; tests rechazan omisión antes de spawn/listen |
| R-SAFE-03 | El test opt-in del worker heredaba el socket runtime de quien lo ejecutara | Perfil propio, bus/audio ficticios, entorno acotado y comprobación de directorios sin efectos |
| R-STT-01 | language=None conservaba el default inglés de FullParams | Se transmite None a set_language; default español no cambia. Calidad de autodetección hablada NOT_TESTED |
| R-BENCH-01 | Mezcla de conversión, creación de estado e inferencia impedía atribuir coste | Etapas separadas, CPU/memoria, tokens/segmentos y parámetros; fixtures/compilaciones seriales |

El alcance de R-SAFE-02 son los runners/helpers de validación F4 y sus experimentos. Los scripts de lanzamiento normal del producto siguen siendo lanzadores del perfil del usuario por diseño; no se convierten en fixtures ni se ejecutaron aquí. Los gates heredados fueron auditados para sus perfiles propios y ejecutados con un entorno exterior privado adicional. Plasma requiere bus/display de sesión para observar ventanas propias; esto no es reutilizar una DB o bearer habitual.

## F4-INC-01 histórico

No se volvió a abrir el perfil real ni a consultar sus metadatos para reconstruir el incidente. No se intentó rollback. El daño/ausencia de daño histórico no puede certificarse sin estado previo; la incertidumbre de los informes F4 se conserva. Las guardas actuales previenen nuevas omisiones y son evidencia independiente, no una reparación retrospectiva de archivos personales.

## Bloqueos y límites técnicos

- **Precisión española / WER: BLOCKED por corpus hablado autorizado ausente.** Hay señales CC0 y referencias/plan, sin grabaciones de personas. No se permite afirmar calidad desde `[Música]` o texto vacío.
- **Rendimiento productivo: pendiente.** Default Whisper/contexto completo conserva su demora; audio_ctx reducido sólo es experimental. Vosk es candidato, no un motor instalado/activo. La autorización fue exclusivamente para benchmark; los artefactos se eliminaron después.
- **O3 / release: BLOCKED por herramienta de build ausente.** CMake no existe en PATH ni en el cache estándar de paquetes consultado. El intento offline falló con exit 101; se solicitó permiso para un wheel privado de CMake 4.4.0, sin tomar «continúa» como autorización de descarga. No se descargó. El PASS de build usa dependencias nativas cacheadas; no certifica un clean build/release en el host actual.
- **Tiny q5_1/q8_0: NOT_TESTED.** Fuentes/tamaños/hash identificados; no hay artefactos ni quantizador empaquetado. No se descargaron ni se escribió un conversor improvisado.
- **Sherpa/Moonshine español: NOT_TESTED.** Modelo concreto y tamaño identificados; runtime ausente. La licencia Community de los pesos españoles debe revisarse antes de distribución; no es automáticamente Apache/MIT por la licencia del runtime.
- **Mantenimiento Vosk:** wheel 0.3.45 probado sólo en este experimento; antes de una integración distribuible hay que revisar versión nativa, reproducibilidad, avisos y corpus de habla. No se añadió un proceso Python privilegiado ni permanente.
- **Condiciones del host:** sin compilaciones concurrentes en inferencia, pero navegador/compositor y frecuencia/carga variables. El monitor tiene coste. n=3/5 Whisper y n=10 por señal/ejecución Vosk describen estos ensayos; no establecen SLO/presupuestos ni colas p95 de producto.
- **Cancelación:** Whisper probado con supervisor productivo, incluyendo probe del decoder compartido con entrada a inferencia y progreso CPU confirmados; Vosk terminado tras progreso PCM en un helper experimental. No confundirlo con cancelación de una integración Vosk inexistente.
- **PTT físico / LIVE_MICROPHONE: NOT_TESTED.** Los WAV y PipeWire privado no son un micrófono físico.
- **Piper / LIVE_TTS: NOT_TESTED.** No se instaló motor/voz ni hubo reproducción audible.
- **Wake / LIVE_WAKE_WORD: BLOCKED por gates anteriores.** Sin desarrollo nuevo, escucha continua ni atajos globales.

## Orden de continuación propuesto para revisión

1. Revisar los cambios de aislamiento y el banco de pruebas.
2. Autorizar un corpus de habla española con referencias/derechos explícitos y decidir el recurso de compilación ausente si se requiere O3.
3. Comparar precisión y latencia de Vosk, tiny completo y contexto reducido con condiciones emparejadas; sólo entonces elegir/integrar un motor.
4. PTT físico consentido; después Piper real y audible, integración de conversación y finalmente wake.

No se aumenta timeout para ocultar la demora, no se cambia Configuración personal y no se altera F2. Se conserva la rama sin commits para la siguiente revisión técnica.
