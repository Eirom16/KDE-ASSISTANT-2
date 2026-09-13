# KDE Assistant 2 — Plan: Conversación por Voz en Tiempo Real

> Plan técnico para integrar una experiencia de conversación por voz natural en el Desktop Agent de KDE Assistant 2. Complementa el sistema del Desktop Agent, Character, animaciones, sonidos y herramientas ya definidos.

---

## 1. Objetivo

Convertir KDE Assistant 2 en un asistente de escritorio capaz de mantener conversaciones de voz naturales:

- Activación mediante **“Hey Jarvis”**.
- Escucha en tiempo real.
- Voice Activity Detection (VAD).
- Speech-to-Text (STT).
- LLM con streaming.
- Text-to-Speech (TTS) con streaming.
- Interrupción inmediata de la voz del asistente (**barge-in**).
- Conversaciones de varios turnos.
- Contexto conversacional.
- Ejecución de herramientas del escritorio.
- Reacciones visuales y sonoras sincronizadas.
- Recuperación ante errores.

**La meta no es añadir un micrófono a un chatbot. La meta es construir un sistema de conversación bidireccional en tiempo real.**

---

# 2. Arquitectura general

```text
Usuario
  ↓
Micrófono
  ↓
WakeWordController
  ↓
AudioInputController
  ↓
VAD / Turn Detection
  ↓
Streaming STT
  ↓
ConversationManager
  ↓
LLM + Tool Calling
  ↓
ResponseController
  ├──→ Streaming TTS → AudioOutputController
  └──→ CharacterController
             ├── expression
             ├── mood
             ├── gaze
             ├── movement
             ├── scene
             └── sound
```

### Regla fundamental

El LLM decide:

- qué responder;
- qué acción solicitar;
- la intención semántica de la respuesta.

La aplicación decide:

- cuándo escuchar;
- cuándo dejar de escuchar;
- cuándo interrumpir;
- cuándo reproducir audio;
- cómo manejar buffers;
- cuándo ejecutar herramientas;
- cómo cambiar el estado del personaje.

**No permitir que el LLM controle directamente el loop de audio ni cada frame de animación.**

---

# 3. Máquina de estados

Implementar un estado central:

```text
IDLE
 ↓
WAKE_DETECTED
 ↓
ATTENTIVE
 ↓
LISTENING
 ↓
THINKING
 ↓
RESPONDING
 ├──→ INTERRUPTED → LISTENING
 ↓
IDLE
```

Estados adicionales:

```text
NO_SPEECH
TOOL_EXECUTING
CANCELLED
AUDIO_ERROR
STT_ERROR
LLM_ERROR
TTS_ERROR
```

Reglas:

- `IDLE`: comportamiento normal.
- `WAKE_DETECTED`: se detectó “Hey Jarvis”.
- `ATTENTIVE`: el personaje despierta.
- `LISTENING`: captura la voz.
- `THINKING`: procesa.
- `RESPONDING`: reproduce TTS.
- `INTERRUPTED`: el usuario habló durante la respuesta.
- `TOOL_EXECUTING`: ejecuta una herramienta.
- Los errores deben dejar el sistema en un estado recuperable.

---

# 4. Wake Word — “Hey Jarvis”

Crear un `WakeWordController` desacoplado:

```qml
WakeWordController {
    property string wakeWord: "Hey Jarvis"
    property bool enabled: true
}
```

No distribuir `"Hey Jarvis"` por todo el proyecto.

### Flujo

```text
IDLE
 ↓
"Hey Jarvis"
 ↓
WAKE_DETECTED
 ↓
wake animation
 ↓
ATTENTIVE
 ↓
LISTENING
```

Al detectar el wake word:

- despertar al personaje;
- cambiar expresión;
- mirar hacia el usuario cuando sea posible;
- reproducir un micro-chime;
- iniciar escucha;
- mostrar feedback visual;
- no robar el foco de la ventana actual.

### Casos a probar

- wake word sin voz posterior;
- wake word duplicado;
- wake word mientras Jarvis habla;
- wake word mientras ejecuta una herramienta;
- wake word desactivado;
- micrófono no disponible.

---

# 5. AudioInputController

Crear una capa dedicada para entrada de audio.

Responsabilidades:

- dispositivo de entrada;
- captura;
- sample rate/formato;
- frames;
- VAD;
- STT;
- errores;
- reconexión.

Requisitos:

- micrófono predeterminado;
- selección manual;
- cambio de dispositivo;
- indicador de entrada;
- desconexión/reconexión.

No mezclar la captura de audio directamente con `ConversationManager`.

---

# 6. VAD — Voice Activity Detection

Implementar detección de:

```text
silencio
voz
ruido
inicio de turno
fin de turno
```

Debe:

- detectar inicio de voz;
- detectar fin;
- tolerar pausas naturales;
- evitar cortes demasiado rápidos;
- evitar esperar demasiado;
- trabajar junto con STT.

Implementar endpointing configurable.

No depender exclusivamente de un timeout fijo.

---

# 7. Streaming STT

Flujo:

```text
Audio
 ↓
VAD
 ↓
STT stream
 ↓
partial transcript
 ↓
final transcript
```

Distinguir claramente:

```text
partial transcript
final transcript
```

Los parciales sirven para feedback y preparación.

**No ejecutar acciones peligrosas únicamente con un transcript parcial.**

El transcript final pasa al `ConversationManager`.

---

# 8. ConversationManager

Crear o reutilizar un componente central:

```text
ConversationManager
```

Responsabilidades:

- historial;
- turnos;
- contexto;
- follow-ups;
- cancelación;
- referencias recientes;
- coordinación STT/LLM/TTS.

Debe ser independiente de QML siempre que sea posible.

---

# 9. Conversación multi-turno

Ejemplo:

```text
Usuario:
"Hey Jarvis"

Jarvis:
"Sí?"

Usuario:
"Busca mi carpeta de proyectos."

Jarvis:
"Encontré varias. ¿Quieres la de KDE Assistant?"

Usuario:
"Sí."

Jarvis:
"La estoy abriendo."
```

No exigir el wake word antes de cada frase cuando el modo conversacional esté activo.

Configurar:

```text
Single Turn
Follow-up
Continuous
```

Después de responder puede existir una ventana de follow-up:

```text
RESPONDING
 ↓
FOLLOW_UP_WINDOW
 ↓
usuario habla
 ↓
LISTENING
```

Si no habla:

```text
FOLLOW_UP_WINDOW
 ↓
IDLE
```

---

# 10. Streaming LLM

El LLM debe poder entregar la respuesta progresivamente.

```text
Transcript
 ↓
LLM
 ↓
tokens/chunks
 ↓
ResponseController
```

No esperar necesariamente a la respuesta completa antes de preparar TTS.

Objetivo principal:

**reducir la latencia hasta la primera palabra.**

---

# 11. Streaming TTS

Arquitectura:

```text
LLM stream
 ↓
sentence/phrase buffer
 ↓
TTS stream
 ↓
audio buffer
 ↓
speaker
```

Requisitos:

- streaming cuando esté disponible;
- baja latencia;
- buffering pequeño;
- cancelación;
- stop inmediato;
- evitar reproducir respuestas antiguas.

---

# 12. Barge-in — interrupción natural

Esta es una función crítica.

Ejemplo:

```text
Jarvis:
"El archivo que encontré está en..."

Usuario:
"No, el otro."

→ Jarvis se detiene inmediatamente.

Jarvis:
"Entendido. Busco el otro."
```

Flujo obligatorio:

```text
TTS hablando
 ↓
VAD detecta voz
 ↓
cancel TTS
 ↓
stop audio
 ↓
flush audio buffer
 ↓
cancel respuesta obsoleta si corresponde
 ↓
LISTENING
 ↓
STT
 ↓
nuevo turno
```

Nunca terminar la respuesta anterior antes de escuchar la interrupción.

---

# 13. Sistema de cancelación

Las operaciones largas deben poder cancelarse:

- STT;
- LLM;
- TTS;
- herramientas;
- búsquedas;
- descargas;
- procesamiento.

Usar IDs de operación:

```text
requestId = 42
TTS(42)

usuario interrumpe

cancel(42)

new requestId = 43
```

Una operación cancelada no puede publicar resultados posteriormente.

---

# 14. AudioOutputController

Responsabilidades:

- dispositivo de salida;
- volumen;
- reproducción;
- streaming;
- stop;
- buffer;
- ducking;
- estado.

Estados:

```text
STOPPED
BUFFERING
PLAYING
INTERRUPTED
ERROR
```

---

# 15. Ducking y mezcla de sonidos

La voz tiene prioridad sobre los SFX.

```text
TTS speaking
 ↓
SFX volume ↓
 ↓
TTS finishes
 ↓
SFX restored
```

Los sonidos de:

- pasos;
- whooshes;
- chimes;
- idle;

no deben competir con la voz.

---

# 16. Integración con CharacterController

Estados visuales:

```text
sleeping
waking
attentive
listening
thinking
tool_executing
speaking
interrupted
confused
error
success
idle
```

Mapeo:

```text
WAKE_DETECTED → wake
LISTENING → attentive/listening
THINKING → thinking
TOOL_EXECUTING → tool scene
RESPONDING → speaking
INTERRUPTED → stop speaking + attentive
SUCCESS → happy
ERROR → concerned/confused
```

---

# 17. Sincronización de voz y personaje

No animar el personaje con cada token.

El personaje reacciona al estado:

```text
LLM started
→ thinking

TTS started
→ speaking

TTS interrupted
→ attentive/listening

TTS finished
→ idle
```

Inicialmente usar:

- speaking animation;
- blink;
- gaze;
- pequeños movimientos.

No implementar lip-sync/visemas complejos hasta estabilizar la base.

---

# 18. Tool Calling + voz

Ejemplo:

```text
Usuario:
"Hey Jarvis, abre Dolphin."

WakeWord
 ↓
LISTENING
 ↓
STT
 ↓
LLM
 ↓
open_app("dolphin")
 ↓
Character Scene
 ↓
TTS
```

La herramienta real ejecuta la acción; la animación representa visualmente esa acción.

---

# 19. Visual Intent

Mantener:

```text
Tool result
 ↓
Visual Intent
 ↓
CharacterController
```

Ejemplo:

```json
{
  "tool": "search_file",
  "visual": "search",
  "target": "dolphin",
  "mood": "focused"
}
```

La aplicación debe validar cualquier intención visual.

No permitir animaciones arbitrarias generadas por el modelo.

---

# 20. Conversación + búsqueda de archivos

Ejemplo:

```text
"Hey Jarvis, busca el PDF de matemáticas."

Wake
 ↓
Listening
 ↓
STT
 ↓
LLM
 ↓
search_file
 ↓
filesystem / KIO
 ↓
resultado
 ↓
thinking → searching → inspecting → found
 ↓
TTS
```

La animación nunca sustituye la búsqueda real.

---

# 21. Contexto conversacional

Debe soportar referencias como:

```text
"abre ese"
"el segundo"
"el de ayer"
"no, el otro"
"ponlo en Descargas"
"ahora bórralo"
```

El `ConversationManager` debe conservar resultados recientes y referencias estructuradas, no depender solamente del historial textual.

---

# 22. Seguridad de herramientas

### Acciones normalmente seguras

- abrir aplicación;
- buscar archivo;
- consultar información;
- navegar;
- cambiar una vista.

### Acciones con confirmación

- eliminar archivos;
- mover grandes cantidades de archivos;
- cerrar aplicaciones con trabajo sin guardar;
- cambios críticos del sistema;
- comandos potencialmente peligrosos.

Ejemplo:

```text
"Elimina ese archivo."

"¿Quieres eliminar definitivamente archivo.pdf?"

"Sí."

→ ejecutar
```

Nunca interpretar silencio como confirmación.

---

# 23. Estado de escucha

El usuario siempre debe saber cuándo Jarvis está escuchando.

Posibles indicadores:

- personaje atento;
- aura sutil;
- micrófono;
- transcript parcial;
- animación;
- nivel de audio.

Debe ser claro sin resultar molesto.

---

# 24. Estado de pensamiento

Durante procesamiento:

```text
LISTENING → THINKING
```

El personaje puede:

- mirar;
- pensar;
- cambiar expresión;
- hacer un movimiento corto.

No fingir progreso inexistente.

---

# 25. Estado de habla

Durante TTS:

```text
RESPONDING
```

Usar:

- speaking animation;
- microgestos;
- gaze;
- movimiento suave.

El lip-sync puede quedar para una fase posterior.

---

# 26. Eco y procesamiento de audio

Investigar e implementar cuando corresponda:

- Acoustic Echo Cancellation (AEC);
- noise suppression;
- automatic gain control;
- routing correcto.

Esto es especialmente importante con los altavoces integrados de laptops.

---

# 27. Latencia

Medir:

```text
wakeLatency
sttFirstPartialLatency
sttFinalLatency
llmFirstTokenLatency
ttsFirstAudioLatency
totalFirstResponseLatency
bargeInLatency
```

No optimizar a ciegas.

El objetivo es reducir especialmente:

```text
fin de frase
→ primera palabra de Jarvis
```

---

# 28. Buffers

Diseñar explícitamente:

```text
Microphone Buffer
STT Buffer
LLM Text Buffer
TTS Buffer
Audio Output Buffer
```

Buffers demasiado grandes aumentan latencia.

Buffers demasiado pequeños provocan cortes.

---

# 29. Errores

### Micrófono

```text
"El micrófono no está disponible."
```

### STT

```text
"No pude entenderte. ¿Puedes repetirlo?"
```

### LLM

```text
"No pude procesar la solicitud."
```

### TTS

```text
"No puedo reproducir mi respuesta ahora."
```

### Tool

```text
"No pude completar esa acción."
```

El personaje debe reflejar el estado con expresiones apropiadas.

---

# 30. Recuperación

Siempre que sea seguro:

```text
error
 ↓
cleanup
 ↓
reset subsystem
 ↓
IDLE
```

Nunca dejar permanentemente:

- micrófono bloqueado;
- TTS reproduciendo audio viejo;
- `LISTENING` congelado;
- personaje congelado;
- tool call pendiente;
- conversación bloqueada.

---

# 31. Configuración

### Voice

- Wake word.
- Micrófono.
- Altavoz.
- Voz TTS.
- Velocidad.
- Volumen.

### Conversation

- Single Turn.
- Follow-up.
- Continuous.
- Timeout.
- Barge-in.

### Audio

- Voice volume.
- SFX volume.
- Ducking.
- Silent mode.

### Privacy

- Wake word.
- Indicador de micrófono.
- Procesamiento local/remoto.
- Almacenamiento de transcripciones.

---

# 32. Privacidad

Documentar claramente:

```text
¿Qué audio permanece local?
¿Qué audio se envía externamente?
¿Cuándo está activo el micrófono?
¿Se almacenan transcripciones?
```

No grabar conversaciones completas por defecto.

---

# 33. Arquitectura de componentes

Adaptar a la estructura real del repositorio:

```text
src/
├── audio/
│   ├── AudioInputController
│   ├── AudioOutputController
│   ├── AudioDeviceManager
│   └── AudioSession
│
├── voice/
│   ├── WakeWordController
│   ├── VADController
│   ├── STTController
│   ├── TTSController
│   └── VoiceSession
│
├── conversation/
│   ├── ConversationManager
│   ├── TurnManager
│   ├── ResponseController
│   ├── ContextManager
│   └── CancellationManager
│
├── agent/
│   ├── AssistantBrain
│   ├── ToolManager
│   ├── ToolExecutor
│   └── VisualIntentController
│
└── character/
    ├── CharacterController
    ├── ExpressionController
    ├── AnimationController
    ├── MovementController
    ├── GazeController
    ├── MoodController
    └── SceneController
```

No crear duplicados si el repositorio ya posee componentes equivalentes.

---

# 34. QML y backend

QML:

- UI;
- estados visuales;
- animaciones;
- feedback;
- personaje;
- configuración.

Backend:

- audio;
- STT;
- TTS;
- networking;
- herramientas;
- contexto;
- lógica de conversación.

No convertir QML en el backend completo.

---

# 35. KDE / Qt / Wayland

Priorizar:

- Qt 6;
- Qt Multimedia cuando sea apropiado;
- APIs nativas;
- integración KDE;
- Wayland;
- Plasma.

Evitar Electron/WebView si existe una solución Qt/KDE adecuada.

---

# 36. Rendimiento

El agente debe funcionar razonablemente en hardware modesto:

- Intel iGPU;
- 8 GB RAM;
- Plasma/Wayland;
- pantallas de resolución moderada.

Evitar:

- polling agresivo;
- loops innecesarios;
- renderizado excesivo;
- procesos duplicados;
- modelos de voz innecesariamente pesados.

---

# 37. Lightweight Voice Mode

Crear un modo ligero:

```text
Lightweight Voice Mode
```

Características:

- menos animaciones;
- menos SFX;
- personaje simplificado;
- buffers optimizados;
- menor consumo;
- sin servicios innecesarios.

---

# 38. Integración con el Desktop Agent

No crear un asistente de voz separado.

La arquitectura debe ser:

```text
KDE Assistant 2
├── Text Chat
├── Voice Conversation
├── Desktop Agent
└── Character
```

Todos comparten:

```text
AssistantBrain
ConversationManager
ToolManager
CharacterController
ContextManager
```

---

# 39. Flujo objetivo — abrir aplicación

```text
Usuario:
"Hey Jarvis."

Jarvis:
"Sí?"

Usuario:
"Abre Dolphin."

→ STT
→ LLM
→ open_app("dolphin")
→ Character Scene
→ TTS
→ idle
```

---

# 40. Flujo objetivo — interrupción

```text
Usuario:
"Hey Jarvis."

Jarvis:
"Sí?"

Usuario:
"Busca el archivo..."

Jarvis:
"Encontré un archivo llamado..."

Usuario:
"No, espera."

→ VAD
→ cancel TTS
→ stop audio
→ LISTENING

Usuario:
"Busca el otro."

→ STT
→ LLM
→ nueva acción
```

Este escenario es obligatorio.

---

# 41. Flujo objetivo — conversación multi-turno

```text
"Hey Jarvis."

"Busca mis imágenes de hoy."

"Encontré varias."

"Abre la segunda."

"La estoy abriendo."

"Ahora muévela a Descargas."

"¿Quieres moverla a Descargas?"

"Sí."

"Hecho."
```

Debe conservar contexto durante toda la secuencia.

---

# 42. Fases de implementación

## Fase 0 — Auditoría

- [ ] Inspeccionar arquitectura actual.
- [ ] Identificar backend del agente.
- [ ] Identificar sistema de herramientas.
- [ ] Identificar sistema de audio/SFX.
- [ ] Identificar `FloatingOrb.qml` / `AssistantCharacter`.
- [ ] Identificar estados existentes.
- [ ] Documentar puntos de integración.
- [ ] No implementar antes de completar la auditoría.

## Fase 1 — Audio básico

- [ ] AudioInputController.
- [ ] AudioOutputController.
- [ ] Selección de dispositivo.
- [ ] Captura.
- [ ] Reproducción.
- [ ] Indicadores.
- [ ] Errores.
- [ ] Prueba en Plasma/Wayland.

## Fase 2 — Wake word

- [ ] WakeWordController.
- [ ] `"Hey Jarvis"`.
- [ ] `WAKE_DETECTED`.
- [ ] Wake animation.
- [ ] SFX.
- [ ] Timeout.
- [ ] Recuperación.
- [ ] Pruebas con ruido.

## Fase 3 — VAD

- [ ] VADController.
- [ ] Inicio de voz.
- [ ] Fin de voz.
- [ ] Endpointing.
- [ ] Sensibilidad.
- [ ] Pausas naturales.
- [ ] Ruido.

## Fase 4 — STT

- [ ] STTController.
- [ ] Streaming.
- [ ] Partial transcript.
- [ ] Final transcript.
- [ ] Cancelación.
- [ ] Errores.
- [ ] Latencia.

## Fase 5 — ConversationManager

- [ ] ConversationManager.
- [ ] TurnManager.
- [ ] ContextManager.
- [ ] Historial.
- [ ] Follow-up.
- [ ] Cancelación.
- [ ] IDs de operación.
- [ ] Multi-turno.

## Fase 6 — LLM streaming

- [ ] Streaming.
- [ ] ResponseController.
- [ ] Buffer.
- [ ] Cancelación.
- [ ] Respuestas parciales.
- [ ] Tool calling.
- [ ] First-token latency.

## Fase 7 — TTS streaming

- [ ] TTSController.
- [ ] Streaming.
- [ ] Buffer.
- [ ] Reproducción incremental.
- [ ] Stop inmediato.
- [ ] Cancelación.
- [ ] First-audio latency.

## Fase 8 — Barge-in

- [ ] Detectar voz durante TTS.
- [ ] Cancelar TTS.
- [ ] Vaciar buffers.
- [ ] Cancelar respuesta obsoleta.
- [ ] Cambiar a `LISTENING`.
- [ ] Procesar nuevo turno.
- [ ] Pruebas repetidas.

**No marcar esta fase como completada hasta comprobar una interrupción realmente inmediata y estable.**

## Fase 9 — Character Integration

- [ ] Listening.
- [ ] Thinking.
- [ ] Speaking.
- [ ] Interrupted.
- [ ] Tool executing.
- [ ] Error.
- [ ] Success.
- [ ] Wake.
- [ ] Microgestos.
- [ ] Gaze.
- [ ] Movimiento.
- [ ] Sincronización.

## Fase 10 — Tool Integration

- [ ] open_app.
- [ ] search_file.
- [ ] file operations.
- [ ] system actions.
- [ ] Visual Intent.
- [ ] Tool scenes.
- [ ] Confirmaciones.
- [ ] Errores.
- [ ] Contexto de resultados.

## Fase 11 — Natural Conversation

- [ ] Follow-up.
- [ ] Continuous mode.
- [ ] Referencias como "ese", "el otro", "el segundo".
- [ ] Interrupciones.
- [ ] Correcciones.
- [ ] Preguntas de seguimiento.
- [ ] Confirmaciones.
- [ ] Cancelaciones naturales.
- [ ] Contexto entre herramientas.

## Fase 12 — Audio avanzado

- [ ] Ducking.
- [ ] Priorización SFX.
- [ ] AEC.
- [ ] Noise suppression.
- [ ] AGC cuando corresponda.
- [ ] Recovery de dispositivos.
- [ ] Buffer tuning.
- [ ] Latency profiling.

## Fase 13 — Rendimiento

- [ ] RAM.
- [ ] CPU.
- [ ] GPU.
- [ ] Consumo en idle.
- [ ] Consumo escuchando.
- [ ] Consumo durante TTS.
- [ ] Lightweight mode.
- [ ] Eliminar polling innecesario.

## Fase 14 — Privacidad

- [ ] Documentar audio local/remoto.
- [ ] Indicador de micrófono.
- [ ] Configuración del wake word.
- [ ] Almacenamiento.
- [ ] Revisar logs.
- [ ] Revisar datos sensibles.

---

# 43. Tests obligatorios

- [ ] `Hey Jarvis` → respuesta.
- [ ] Wake → comando → herramienta → respuesta.
- [ ] Conversación de varios turnos.
- [ ] Interrupción durante TTS.
- [ ] Pausas naturales sin cortes.
- [ ] Ruido ambiental.
- [ ] TTS error.
- [ ] STT error.
- [ ] LLM error.
- [ ] Tool error.
- [ ] Micrófono desconectado.
- [ ] Altavoz desconectado.
- [ ] Recuperación después de cada error.
- [ ] No reproducir respuestas canceladas.
- [ ] No ejecutar dos turnos simultáneamente.

---

# 44. Debugging

Logs estructurados:

```text
[VOICE] Wake detected
[VOICE] Listening started
[STT] Partial transcript
[STT] Final transcript
[LLM] Request started
[LLM] First token
[TOOL] Executing
[TTS] Stream started
[TTS] First audio
[VOICE] Barge-in detected
[TTS] Cancelled
[VOICE] Listening resumed
```

No registrar audio crudo ni información sensible innecesariamente.

---

# 45. Reglas de implementación

- [ ] Inspeccionar el repositorio real antes de modificarlo.
- [ ] Reutilizar componentes existentes.
- [ ] No duplicar sistemas ya existentes.
- [ ] Mantener QML como capa visual.
- [ ] Mantener audio y lógica pesada fuera de QML cuando corresponda.
- [ ] No añadir dependencias pesadas sin justificación.
- [ ] Priorizar Qt/KDE.
- [ ] Mantener compatibilidad con Wayland.
- [ ] Probar después de cada fase importante.
- [ ] Marcar `[x]` solo después de implementar y probar.
- [ ] Si algo no puede implementarse, marcarlo como bloqueado y explicar por qué.

---

# 46. Criterio de éxito

KDE Assistant 2 debe poder realizar una conversación como:

```text
Usuario:
"Hey Jarvis."

Jarvis:
"Sí?"

Usuario:
"Busca el PDF de matemáticas."

Jarvis:
"Encontré uno. ¿Quieres que lo abra?"

Usuario:
"Sí."

Jarvis:
"Abriéndolo."

[abre el PDF]

Usuario:
"No, espera. Abre el otro."

[Jarvis interrumpe cualquier respuesta apropiada]

Jarvis:
"Entendido. Abriendo el otro."

[ejecuta la acción]

Jarvis:
"Listo."
```

Mientras:

- wake word funciona;
- VAD detecta turnos;
- STT reconoce voz;
- LLM mantiene contexto;
- herramientas se ejecutan realmente;
- TTS responde con baja latencia;
- el usuario puede interrumpir;
- el personaje reacciona;
- los SFX acompañan sin molestar;
- los errores se recuperan;
- el sistema continúa siendo usable en hardware modesto.

---

# 47. Resultado final

```text
                 KDE ASSISTANT 2
                       │
             ┌─────────┴─────────┐
             │                   │
          Texto                Voz
             │                   │
             └─────────┬─────────┘
                       │
              ConversationManager
                       │
                 AssistantBrain
                       │
          ┌────────────┼────────────┐
          │            │            │
        LLM          Tools       Context
          │            │            │
          └────────────┼────────────┘
                       │
                Desktop Agent
                       │
                Character Agent
                       │
        ┌──────────────┼──────────────┐
        │              │              │
    Expression       Motion          Sound
        │              │              │
        └──────────────┼──────────────┘
                       │
                 KDE / Desktop
```

> **Principio final:** KDE Assistant 2 no debe sentirse como un chatbot al que se le agregó voz. Debe sentirse como un agente de escritorio que puede conversar naturalmente.
