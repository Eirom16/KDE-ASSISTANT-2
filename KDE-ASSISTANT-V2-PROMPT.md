# KDE ASSISTANT v2 - Prompt de Desarrollo (Cupertino & Power Edition)

Eres un desarrollador senior creando **KDE Assistant**, un asistente de escritorio para KDE Plasma Linux. Tu tarea es construir la aplicacion completa desde cero usando **Rust + Qt 6/QML**, con estetica Apple Design (Cupertino), capacidades de agente con tool calling, invocacion por voz estilo Siri y system tray multifuncion.

## Que es KDE Assistant

Es un asistente de IA de escritorio que:
- Se integra nativamente con KDE Plasma (DBus, tray, atajos globales, KWin translucency)
- Usa modelos de IA gratuitos via **OpenRouter API** (compatible con OpenAI, sin claves propias restrictivas)
- Tiene **activacion por voz estilo Siri** con orbe luminoso, chimes y wake word ML
- Puede **hablar (TTS)** y **escuchar (STT con Whisper local)**
- Actua como **agente**: ejecuta herramientas (abrir apps, crear/editar archivos, buscar en web, mostrar imagenes)
- Se adapta al diseno y colores del escritorio (paleta Apple con acentos KDE)
- Funciona en modo ventana flotante translucida con soporte Wayland
- Usa **GitHub Primer Octicons** para toda su iconografia

## Stack Tecnico

- **Backend:** Rust 1.75+ + tokio (async, nunca bloquea el thread de Qt)
- **UI:** Qt 6.11+ / QML con QtQuick Controls 2
- **AI:** OpenRouter API (`https://openrouter.ai/api/v1/chat/completions`) — compatible con tool calling
- **STT:** whisper-rs (local, sin internet)
- **TTS:** piper-tts (motor neural ONNX) (local)
- **Chimes:** rodio (playback de WAVs)
- **Wake Word:** ONNX/openWakeWord (~50MB, deteccion precisa de "Hey KDE")
- **Audio:** cpal (input) + VAD
- **DB:** SQLite via rusqlite
- **HTTP:** reqwest con streaming SSE
- **Iconos:** GitHub Primer Octicons (SVG embebidos en `.qrc`)

## Arquitectura

```
┌──────────────────────────────────────────────────────────────┐
│                    QML Frontend (Cupertino)                  │
│  ┌──────────┐  ┌──────────┐  ┌─────────────┐  ┌──────────┐  │
│  │ Session  │  │  Chat    │  │  InputBar   │  │ VoiceOrb │  │
│  │ Drawer   │  │  View    │  │  capsule    │  │ Siri-like│  │
│  │ chips    │  │ burbujas │  │  pill bar   │  │ glow orb │  │
│  └──────────┘  └──────────┘  └─────────────┘  └──────────┘  │
│  ┌──────────┐  ┌──────────┐  ┌─────────────┐               │
│  │ ToolCall │  │ ImageCard│  │ TypingDots  │               │
│  │ Badge    │  │ zoomable │  │ pulsantes   │               │
│  └──────────┘  └──────────┘  └─────────────┘               │
└───────────────────────┬──────────────────────────────────────┘
                        │ Signals / Properties (Q_INVOKABLE)
┌───────────────────────┴──────────────────────────────────────┐
│                    Rust Backend (Tokio)                       │
│  ┌──────────┐  ┌───────────┐  ┌──────────────┐              │
│  │ AiService│  │ Session   │  │ ToolExecutor │              │
│  │ HTTP+SSE │  │ Manager   │  │ open_app     │              │
│  │ toolcall │  │ SQLite    │  │ files/search │              │
│  └──────────┘  └───────────┘  │ images       │              │
│  ┌──────────┐  ┌───────────┐  └──────────────┘              │
│  │ Speech   │  │ Audio     │  ┌──────────────┐              │
│  │ STT+TTS  │  │ Capture   │  │ Hotword ONNX │              │
│  │ whisper  │  │ cpal+VAD  │  │ "Hey KDE"    │              │
│  │ piper   │  └───────────┘  └──────────────┘              │
│  └──────────┘  ┌───────────┐  ┌──────────────┐              │
│  ┌──────────┐  │ Chime     │  │ KDE Integr.  │              │
│  │ Config   │  │ Player    │  │ DBus+Tray+   │              │
│  │ Manager  │  │ rodio     │  │ Shortcuts    │              │
│  └──────────┘  └───────────┘  └──────────────┘              │
└──────────────────────────────────────────────────────────────┘
```

## Diseno de UI (Cupertino)

### Principios de Diseno

1. **Nativo de KDE + Apple:** Colores de fondo inspirados en Apple (canvas parchment/ink), acentos KDE/Apple hibridos (Action Blue), tipografia Inter/Noto Sans con tracking negativo.
2. **Translucidez KWin:** La ventana usa blur nativo de KDE Plasma (X11 o Wayland) para un look frosted acrylic.
3. **Radios fluidos:** 18px para cards/burbujas, `pill` (9999px) para botones primarios e InputBar.
4. **Responsivo:** Breakpoints compacto (<360px), normal (360-600px), amplio (>600px).
5. **Accesible:** Contraste suficiente, navegacion por teclado, soporte screen reader.

### Layout de la Ventana

```
┌─────────────────────────────────────────────────┐
│  [hubot] KDE Assistant         [─] [□] [×]      │  ← Titlebar KDE (CSD)
├──────────┬──────────────────────────────────────┤
│          │                                      │
│ SESIONES │         MENSAJES DE CHAT             │
│ [+ nueva]│                                      │
│          │  ┌─────────────────────────────┐    │
│ ┌──────┐ │  │ Tu                          │    │
│ │Chat 1│ │  │ ┌───────────────────────┐  │    │
│ │ 3 msg│ │  │ │ Hola, que puedes     │  │    │
│ └──────┘ │  │ │ hacer?                │  │    │
│ ┌──────┐ │  │ └───────────────────────┘  │    │
│ │Chat 2│ │  └─────────────────────────────┘    │
│ │ 5 msg│ │                                      │
│ └──────┘ │  ┌─────────────────────────────┐    │
│          │  │ KDE Assistant               │    │
│          │  │ ┌───────────────────────┐  │    │
│          │  │ │ Puedo ayudarte con    │  │    │
│          │  │ │ tareas, abrir apps... │  │    │
│          │  │ └───────────────────────┘  │    │
│          │  │ [🚀 open_app dolphin] ←toolcall
│          │  └─────────────────────────────┘    │
│          │                                      │
│          │            ╭─────────╮              │
│          │            │   ORB   │              │  ← VoiceOrb cuando escucha
│          │            ╰─────────╯              │
│          ├──────────────────────────────────────┤
│          │ ┌──[ Escribe...  ][🎤][▲]────────┐ │  ← Capsule InputBar
│          │ └────────────────────────────────┘ │
└──────────┴──────────────────────────────────────┘
```

### Colores (Tema Apple Dark — adaptado a KDE)

```qml
QtObject {
    property color canvas: "#1d1d1f"          // Fondo ventana
    property color surface: "#2a2a2c"          // Burbujas asistente, cards
    property color surfaceHover: "#3a3a3c"     // Hover states
    property color hairline: "#3a3a3c"         // Bordes 1px
    property color primary: "#2997ff"          // Action Blue (dark)
    property color primaryFocus: "#0071e3"     // Focus state
    property color onPrimary: "#ffffff"        // Texto sobre primary
    property color onDark: "#ffffff"           // Texto sobre dark
    property color ink: "#ffffff"              // Texto principal
    property color inkMuted: "#7a7a7a"         // Texto secundario
    property color inkMuted80: "#cccccc"       // Texto muted 80
    property color userBubble: "#2997ff"       // Burbuja usuario (dark)
    property color userText: "#ffffff"         // Texto usuario
    property color error: "#ff453a"            // Errores
    property color success: "#30d158"          // Exito
    property color warn: "#ff9f0a"             // Advertencias
    property color surfaceChip: "#d2d2d7"      // Chips (light) / translucido dark
}
```

### Colores (Tema Apple Light)

```qml
QtObject {
    property color canvas: "#f5f5f7"           // Parchment
    property color surface: "#ffffff"          // Cards blanco puro
    property color surfaceHover: "#e8e9eb"     // Hover
    property color hairline: "#e0e0e0"         // Hairline
    property color primary: "#0066cc"          // Action Blue (light)
    property color primaryFocus: "#0071e3"     // Focus
    property color onPrimary: "#ffffff"        // Texto sobre primary
    property color ink: "#1d1d1f"              // Texto principal
    property color inkMuted: "#6d6f72"         // Texto secundario
    property color inkMuted80: "#333333"       // Texto muted 80
    property color userBubble: "#0066cc"       // Burbuja usuario (light)
    property color userText: "#ffffff"         // Texto usuario
    property color error: "#ff3b30"
    property color success: "#34c759"
    property color warn: "#ff9500"
}
```

### Tipografia (escala Apple, sustituto Inter)

```qml
QtObject {
    property string fontFamily: "Inter, Noto Sans, system-ui, sans-serif"
    property int fontSizeHero: 28
    property int fontSizeTagline: 21
    property int fontSizeBodyStrong: 17
    property int fontSizeBody: 15
    property int fontSizeCaption: 13
    property int fontSizeFinePrint: 12
    property real lhHero: 1.14
    property real lhBody: 1.47
    property real lsHero: -0.28
    property real lsBody: -0.224
    property int fontWeightBold: Font.DemiBold
    property int fontWeightNormal: Font.Normal
}
```

### Radios de Borde

```qml
QtObject {
    property int radiusSm: 8
    property int radiusMd: 11
    property int radiusLg: 18
    property int radiusPill: 9999
}
```

### Iconografia (GitHub Primer Octicons)

Todos los iconos se cargan desde el repositorio clonado de `primer/octicons` y se embeben en `resources.qrc`. SVGs vectoriales escalables.

| Funcion | Octicon |
|---|---|
| Enviar mensaje | `paper-airplane-16.svg` |
| Microfono | `unmute-16.svg` |
| Buscar web | `search-16.svg` |
| Crear archivo | `file-added-16.svg` |
| Editar archivo | `pencil-16.svg` |
| Leer archivo | `file-16.svg` |
| Abrir app | `rocket-16.svg` |
| Mostrar imagen | `image-16.svg` |
| Bandeja | `hubot-16.svg` |
| Configuracion | `gear-16.svg` |
| Nueva sesion | `plus-16.svg` |
| Panel sesiones | `sidebar-collapse-16.svg` / `sidebar-expand-16.svg` |
| Detener TTS | `stop-16.svg` |
| Reintentar | `sync-16.svg` |
| Cerrar | `x-16.svg` |
| Minimizar | `dash-16.svg` |
| Maximizar | `square-16.svg` |
| Refresh | `sync-16.svg` |
| Ojo (ver) | `eye-16.svg` |
| Reloj (recientes) | `history-16.svg` |
| Salir | `sign-out-16.svg` |

### Burbujas de Mensaje

```
USUARIO (alineado derecha):
┌─────────────────────────────┐
│                     Tu     │  ← inkMuted, 11px
│      ┌─────────────────┐   │
│      │ Hola, abre      │   │  ← Fondo primary, texto onPrimary
│      │ el navegador    │   │  ← Radius 18px (uniforme, esquinas suaves)
│      └─────────────────┘   │
└─────────────────────────────┘

ASISTENTE (alineado izquierda):
┌─────────────────────────────┐
│  KDE Assistant              │  ← inkMuted, 11px
│  ┌───────────────────────┐  │
│  │ Abriendo Firefox...   │  │  ← Fondo surface, texto ink
│  │                       │  │  ← Radius 18px
│  │ [🚀 open_app firefox] │  │  ← Tool call pill incrustado
│  └───────────────────────┘  │
│  ┌───────────────────────┐  │
│  │ [imagen adjunta]      │  │  ← ImageCard (si show_image)
│  └───────────────────────┘  │
└─────────────────────────────┘
```

### Tool Call Badge (Pill de Accion)

```
┌──────────────────────────┐
│ [🚀] open_app: dolphin   │  ← Fondo surface, icono + texto, radius pill
└──────────────────────────┘
```

Estados:
- **running:** icono girando (RotationAnimation)
- **success:** fondo tintado verde, check icon
- **error:** fondo tintado rojo, x icon

### Voice Orb (Siri-like Glowing Orb)

```qml
Rectangle {
    id: orb
    width: 120; height: 120
    radius: width / 2
    gradient: Gradient {
        GradientStop { position: 0.0; color: "#2997ff" }  // Action Blue
        GradientStop { position: 0.5; color: "#5e5ce6" }  // Indigo
        GradientStop { position: 1.0; color: "#bf5af2" }  // Violet
    }
    // Spring animation reactive to mic amplitude
    scale: audioLevel * 0.3 + 1.0  // breathing effect
    // Glow effect via DropShadow or RadialGradient
    Behavior on scale { SpringAnimation { spring: 3.0; damping: 0.4 } }
}
```

Estados:
- **idle:** anillo sutil (radius pequeño, opacidad 0.4)
- **listening:** orbe completo, escala reactiva a amplitud del mic
- **processing:** orbe contraido con RotationAnimation lenta
- **speaking:** ondas expansivas concetricas

### Indicador de Escritura

```
Tres puntos pulsantes con timing escalonado:
●  (delay 0ms)    -> scale 1.0 -> 1.2
●  (delay 150ms)  -> scale 1.0 -> 1.2
●  (delay 300ms)  -> scale 1.0 -> 1.2
Pulso cada 400ms total.
```

### InputBar (Floating Capsule Bar)

```
┌────────────────────────────────────────────┐
│ ┌──────────────────────┐  ╭───╮  ╭───╮  │
│ │ Escribe un mensaje...│  │🎤 │  │ ▲ │  │  ← pill completo (radius 9999px)
│ └──────────────────────┘  ╰───╯  ╰───╯  │
│   Enter para enviar · Shift+Enter nueva  │  ← Caption hint
└────────────────────────────────────────────┘
```

- **Auto-resize:** TextArea crece hasta 4 lineas maximo
- **Microfono circular 44x44** (button.icon.circular Apple spec)
- **Enviar circular 44x44** — solo activo cuando hay texto
- **Press effect:** Scale 0.95 con Behavior (100ms)

### Panel de Sesiones (SessionDrawer)

```
┌──────────────────┐
│ [+ Nueva sesion] │  ← Pill button
├──────────────────┤
│ ┌──────────────┐ │
│ │ Chat 1       │ │  ← Seleccionado: fondo primary, texto onPrimary
│ │ 3 mensajes   │ │
│ └──────────────┘ │
│ ┌──────────────┐ │
│ │ Chat 2       │ │  ← No seleccionado: hover surfaceHover
│ │ 5 mensajes   │ │
│ └──────────────┘ │
└──────────────────┘
```

- Items en chip pill (radius 9999px)
- Ancho 220px fijo, colapsable
- Seleccion: fondo `primary` con texto `onPrimary`
- Hover: fondo `surfaceHover`

### Welcome Screen

```
┌────────────────────────────────────────────┐
│                                            │
│          [hubot-64.svg]                    │  ← 64x64 Octicon
│                                            │
│        KDE Assistant                       │  ← fontSizeHero, ink
│                                            │
│   Escribe un mensaje para comenzar.        │  ← fontSizeBody, inkMuted
│   Puedo ayudarte con preguntas,            │
│   tareas del sistema, y mas.               │
│                                            │
│   [🚀 Abrir navegador]  [🔍 Buscar]       │  ← Sugerencias pill buttons
│                                            │
└────────────────────────────────────────────┘
```

## Comportamiento del Asistente

### Personalidad

- **Amigable pero profesional** — Saluda cuando apropiado, sin excesos
- **Conciso** — Respuestas cortas y directas
- **Util** — Prioriza accionables
- **Honesto** — Si no puede hacer algo, lo dice
- **Multilenguaje** — Detecta y responde en el idioma del usuario
- **Orientado a acciones** — Cuando sea posible, ejecuta herramientas en vez de solo describir

### System Prompt por Defecto

```
Eres KDE Assistant, un asistente de escritorio para KDE Plasma Linux.
Responde de forma concisa y util en el idioma del usuario.
Puedes ayudar con:
- Preguntas generales
- Tareas del sistema (abrir apps, crear/editar archivos)
- Busquedas en la web
- Mostrar imagenes relevantes
- Configuracion del escritorio
Si el usuario te pide algo que no puedes hacer, se honesto al respecto.
Respuestas cortas y directas son preferidas.
Formato: usa markdown cuando sea util (listas, codigo, negrita).
Cuando ejecutes herramientas, describe brevemente lo que hiciste.
```

### Modelo por Defecto

```
openrouter/z-ai/glm-5.2:free
```

Configurable desde SettingsDialog. Otros modelos gratuitos disponibles:
- `openrouter/mimo-v2.5-free`
- `openrouter/hy3-free`
- `openrouter/nemotron-3-ultra-free`
- `openrouter/ling-3.0-flash-fin-free`

### Tool Calling (Bucle de Agente)

```rust
async fn agent_loop(messages: Vec<Message>, tools: Vec<Tool>) -> Result<String> {
    loop {
        let response = ai_service.chat(messages.clone(), tools.clone()).await?;
        if let Some(tool_call) = response.tool_call {
            let result = tool_executor.execute(&tool_call).await?;
            messages.push(Message::Tool {
                tool_call_id: tool_call.id,
                content: result,
            });
            continue;  // Vuelve a llamar al LLM con el resultado
        }
        return Ok(response.content);
    }
}
```

### Manejo de Errores

```
┌────────────────────────────────────┐
│  [⚠] Error de conexion            │  ← Label error color
│  ┌──────────────────────────────┐ │
│  │ No se pudo conectar con      │ │  ← Fondo error suave, borde error
│  │ OpenRouter. Verifica tu API  │ │
│  │ key y conexion a internet.   │ │
│  │                  [Reintentar]│ │  ← Button secondary pill
│  └──────────────────────────────┘ │
└────────────────────────────────────┘
```

## Funcionalidades de Voz (Siri-like)

### Wake Word ML

- **Modelo:** ONNX openWakeWord (~50MB), descarga opcional
- **Wake word:** "Hey KDE" (configurable)
- **Umbral:** 0.8 confianza (configurable)
- **Cooldown:** 2 segundos entre detecciones
- **Visual:** Orbe luminoso aparece con fade-in + chime de activacion

### STT (Speech-to-Text)

- **Motor:** Whisper (whisper-rs) corriendo local
- **Modelo:** `ggml-base.bin` (~140MB, descarga opcional)
- **Idiomas:** es, en, pt, fr, de, ja, zh, ko
- **Modo:** Single-shot con auto-VAD (silence detection)
- **Max duracion:** 30 segundos
- **Feedback:** Orbe luminoso escala con amplitud del mic

### TTS (Text-to-Speech)

- **Motor:** piper-tts (motor neural ONNX)
- **Voces:** Configurables
- **Velocidad:** Configurable (1.0 = normal)
- **Trigger:** Automatico despues de respuesta (configurable)
- **Barge-in:** Si el usuario habla o pulsa Escape, TTS se detiene inmediato

### Chimes (rodio)

- **activate.wav:** Tono ascendente ~150ms al detectar wake word
- **process.wav:** Tono suave al terminar transcripcion
- **deactivate.wav:** Tono descendente al cancelar
- **WAVs embebidos** en `assets/chimes/`

### Push-to-Talk

- **Shortcut:** Super+Shift+V (mantenido)
- **Behavior:** Mientras se mantiene, graba. Al soltar, procesa.
- **Visual:** Orbe luminoso aparece

## Responsive Design

### Breakpoints

```
Compacto (< 360px):
- SessionDrawer oculto
- Chat ocupa todo el ancho
- InputBar minimalista

Normal (360px - 600px):
- SessionDrawer colapsado (icono)
- Chat con ancho completo
- InputBar normal

Amplio (> 600px):
- SessionDrawer visible (220px)
- Chat con max-width 720px centrado
- InputBar con max-width
```

### Comportamiento de Ventana

- **Translucidez:** `Qt.WA_TranslucentBackground` + KWin blur
- **Siempre arriba:** `Qt.WindowStaysOnTopHint`
- **Posicion inicial:** Centro-inferior de pantalla
- **Tamaño minimo:** 360x500
- **Tamaño maximo:** 800x900
- **Minimizar:** A system tray, no a taskbar
- **Cerrar:** Oculta ventana, no cierra la app

### Animaciones

- **Fade-in mensajes:** 200ms
- **Slide SessionDrawer:** 150ms
- **Scale botones press:** 100ms
- **Typing dots:** Pulso cada 400ms
- **Voice orb scale:** Spring (damping 0.4)
- **Voice orb fade:** 200ms

## KDE Integration

### System Tray Multifuncion

```
┌────────────────────────────────────────────┐
│  [hubot] KDE Assistant                     │
├────────────────────────────────────────────┤
│  [eye] Mostrar / Ocultar    [Super+Sh+A]  │
│  [plus] Nueva sesion        [Ctrl+N]      │
│  [unmute] Dictar / Push-Talk [Super+Sh+V] │
├────────────────────────────────────────────┤
│  ⚡ Acciones Rapidas                       │
│     [search] Buscar en Google...           │
│     [rocket] Abrir aplicacion...           │
│     [file] Abrir carpeta del asistente     │
├────────────────────────────────────────────┤
│  [pin] Siempre visible      [✔]           │
│  [history] Sesiones recientes ▶           │
│     Conversacion sobre Rust                │
│     Organizacion de archivos               │
├────────────────────────────────────────────┤
│  [gear] Configuracion                      │
│  [sign-out] Salir                          │
└────────────────────────────────────────────┘
```

### Global Shortcuts (configurables)

| Shortcut | Accion default |
|----------|---------------|
| Super+Shift+A | Mostrar/Ocultar ventana |
| Super+Shift+V | Push-to-talk |
| Escape | Minimizar a tray / detener TTS |
| Ctrl+N | Nueva sesion |
| Ctrl+, | Configuracion |

### DBus

- **Nombre:** `org.kde.assistant`
- **Interfaz:** `org.kde.assistant.Chat`
- **Metodos:** `SendMessage(string)`, `GetSessions()`, `NewSession()`, `ShowWindow()`

### Notificaciones

Usar `QSystemTrayIcon::showMessage()` con notificaciones nativas KDE.

## Configuracion

### Archivo de Config

Ubicacion: `~/.config/kde-assistant/config.json`

```json
{
  "ai": {
    "baseUrl": "https://openrouter.ai/api/v1",
    "apiKey": "",
    "model": "openrouter/z-ai/glm-5.2:free",
    "temperature": 0.7,
    "maxTokens": 2048,
    "systemPrompt": "Eres KDE Assistant...",
    "enableToolCalling": true,
    "maxToolIterations": 8
  },
  "speech": {
    "sttModel": "base",
    "ttsEngine": "piper",
    "piperModel": "es_ES-sharvard-medium",
    "piperLengthScale": 1.0,
    "ttsVoice": "es",
    "ttsRate": 1.0,
    "wakeWord": "hey kde",
    "wakeWordThreshold": 0.8,
    "autoSpeak": false,
    "chimesEnabled": true,
    "wakeWordModelPath": "assets/models/wake_word.onnx"
  },
  "ui": {
    "theme": "system",
    "windowWidth": 420,
    "windowHeight": 680,
    "showSessionPanel": false,
    "alwaysOnTop": true,
    "translucent": true
  },
  "tools": {
    "openApp": true,
    "createFile": true,
    "editFile": true,
    "readFile": true,
    "webSearch": true,
    "showImage": true,
    "allowedPaths": ["~/Documentos", "~/Descargas", "~/Escritorio"]
  },
  "shortcuts": {
    "toggle": "Super+Shift+A",
    "pushToTalk": "Super+Shift+V"
  }
}
```

## Pasos de Implementacion

### Fase 1: Scaffold (Dias 1-2)
1. `git init` + .gitignore
2. `cargo init` con `kde-assistant`
3. Agregar dependencias a Cargo.toml
4. Configurar Qt/QML build system (`build.rs` + `qmake`)
5. Clonar `primer/octicons` a `assets/octicons/`
6. Crear `resources.qrc` con QMLs + Octicons
7. Crear estructura de directorios
8. Verificar que `cargo build` compila
9. Commit inicial

### Fase 2: Backend Core (Dias 3-6)
1. `AiService` con HTTP streaming SSE a OpenRouter
2. Implementar tool calling (bucle de agente)
3. `SessionManager` con SQLite
4. `ConfigManager` (lectura/escritura de config.json)
5. `ToolExecutor` con open_app, create_file, edit_file, read_file, web_search, show_image
6. Tests unitarios
7. Verificar que el chat funciona via terminal

### Fase 3: QML UI Apple (Dias 7-10)
1. `Main.qml` con ventana translucida + KWin blur
2. `Theme.qml` con todos los tokens Apple
3. `Octicon.qml` componente reutilizable
4. `ChatView` con message bubbles
5. `MessageBubble` con radios Apple
6. `InputBar` capsule bar con auto-resize
7. `SessionDrawer` con chips pill
8. `TypingIndicator` tres puntos pulsantes
9. Conectar QML con Rust via signals/slots
10. Implementar streaming visual

### Fase 4: Tool Calling UI (Dias 11-12)
1. `ToolCallBadge` pill informativa
2. `ImageCard` para imagenes en chat
3. Integrar pills de accion en MessageBubble
4. Manejo de errores visual
5. Boton de reintentar

### Fase 5: Voz Siri (Dias 13-16)
1. `audio_capture` con cpal + VAD
2. `speech_service` con whisper-rs + piper-tts
3. `chime_player` con rodio
4. `hotword` con ONNX/openWakeWord
5. `VoiceOrb.qml` con SpringAnimation
6. Pipeline completo voz -> LLM -> voz
7. Barge-in (cancelar TTS al hablar)
8. Push-to-talk con atajo global

### Fase 6: KDE Integration (Dias 17-18)
1. `QSystemTrayIcon` con menu multifuncion
2. Acciones rapidas en tray
3. Sesiones recientes en tray
4. `KGlobalAccel` para atajos globales
5. DBus `org.kde.assistant`
6. Auto-detectar tema (dark/light)
7. Soporte Wayland + KWin blur

### Fase 7: Polish (Dias 19-20)
1. Animaciones finas
2. Error handling completo
3. `SettingsDialog` con todas las opciones
4. Testing en KDE Plasma real
5. Build final + README

## Notas Tecnicas Importantes

### Streaming SSE con reqwest

```rust
use futures_util::StreamExt;

let response = client.post(url).json(&body).send().await?;
let mut stream = response.bytes_stream();

while let Some(chunk) = stream.next().await {
    let text = String::from_utf8_lossy(&chunk?);
    for line in text.lines() {
        if line.starts_with("data: ") {
            let data = &line[6..];
            if data == "[DONE]" { break; }
            let chunk: StreamChunk = serde_json::from_str(data)?;
            if let Some(content) = chunk.choices[0].delta.content {
                tx.send(content).await?;  // Enviar token al UI
            }
            // Detectar tool_call
            if let Some(tool_call) = chunk.choices[0].delta.tool_calls {
                tx_tool.send(tool_call).await?;
            }
        }
    }
}
```

### Tool Calling Schema (OpenAI-compatible)

```json
{
  "tools": [
    {
      "type": "function",
      "function": {
        "name": "open_app",
        "description": "Abre una aplicacion del sistema por nombre",
        "parameters": {
          "type": "object",
          "properties": {
            "name": { "type": "string", "description": "Nombre o alias de la app" }
          },
          "required": ["name"]
        }
      }
    }
  ]
}
```

### SQLite Schema

```sql
CREATE TABLE sessions (
    id TEXT PRIMARY KEY,
    title TEXT NOT NULL,
    created_at TEXT NOT NULL,
    updated_at TEXT NOT NULL
);

CREATE TABLE messages (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    session_id TEXT NOT NULL,
    role TEXT NOT NULL,            -- 'system', 'user', 'assistant', 'tool'
    content TEXT NOT NULL,
    tool_name TEXT,                -- Nombre de herramienta ejecutada
    tool_call_id TEXT,             -- ID del tool call
    tool_result TEXT,              -- Resultado serializado
    image_url TEXT,                -- URL/path de imagen
    timestamp TEXT NOT NULL,
    FOREIGN KEY (session_id) REFERENCES sessions(id)
);

CREATE INDEX idx_messages_session ON messages(session_id);
CREATE INDEX idx_sessions_updated ON sessions(updated_at DESC);
```

### Whisper Model Download

```rust
let model_path = dirs::data_local_dir()
    .unwrap().join("kde-assistant/models/ggml-base.bin");

if !model_path.exists() {
    let url = "https://huggingface.co/ggerganov/whisper.cpp/resolve/main/ggml-base.bin";
    let response = reqwest::get(url).await?;
    let bytes = response.bytes().await?;
    std::fs::write(&model_path, bytes)?;
}
```

## Recursos

- Qt 6 Documentation: https://doc.qt.io/qt-6/
- Qt Quick Controls 2: https://doc.qt.io/qt-6/qtquickcontrols-index.html
- cxx-qt (Rust + Qt): https://github.com/KDAB/cxx-qt
- OpenRouter API: https://openrouter.ai/docs
- Whisper.cpp: https://github.com/ggerganov/whisper.cpp
- openWakeWord: https://github.com/dscripka/openWakeWord
- piper-tts: https://github.com/rhasspy/piper
- GitHub Primer Octicons: https://github.com/primer/octicons
- Apple Design: https://developer.apple.com/design/
