# KDE Assistant 2 — Prompt maestro: Desktop Agent Character System

## Rol de la IA

Actúa como **arquitecto de software, desarrollador senior de Qt 6/QML, diseñador UX/UI y especialista en integración con KDE Plasma/Wayland**.

Vas a evolucionar KDE Assistant 2 para convertir su actual avatar/orbe flotante en un **Desktop Agent Character**: un personaje de escritorio vivo, expresivo, reactivo y visualmente integrado con KDE.

El resultado debe sentirse como un asistente de escritorio premium y vivo, inspirado conceptualmente en la **presencia, expresividad y comportamiento de Miss Minutes de Loki**, pero **sin copiar su diseño, personalidad, voz, assets ni identidad visual**.

---

# 0. REGLAS FUNDAMENTALES

- [ ] Inspeccionar primero el repositorio real antes de modificar código.
- [ ] No asumir que una función existe: comprobarla en el código.
- [ ] No eliminar funcionalidades existentes de KDE Assistant 2.
- [ ] Mantener la arquitectura Qt 6/QML existente siempre que sea razonable.
- [ ] Priorizar tecnologías nativas de KDE/Qt sobre HTML/Electron/WebView.
- [ ] Mantener compatibilidad con KDE Plasma y Wayland.
- [ ] Diseñar para múltiples monitores.
- [ ] No permitir que las animaciones interfieran con el uso normal del escritorio.
- [ ] Mantener un modo de accesibilidad/reducción de movimiento.
- [ ] Mantener buen rendimiento incluso en hardware Intel integrado de gama baja.
- [ ] Evitar que el LLM controle cada frame de animación.
- [ ] El LLM decide intención; el sistema determinista decide cómo representarla.
- [ ] Cada tarea completada debe marcarse en este documento.
- [ ] No marcar una tarea como completada si solamente se diseñó: debe estar implementada, integrada y probada.
- [ ] Si una función no puede implementarse todavía, marcarla como `⚠️ BLOQUEADA` y explicar exactamente por qué.
- [ ] No saltarse ningún apartado de este checklist.
- [ ] Antes de finalizar, realizar una auditoría completa de este documento.

---

# 1. OBJETIVO DEL SISTEMA

Convertir el actual `FloatingOrb` en un personaje llamado provisionalmente **Assistant Character**.

El personaje debe:

- [x] Tener una identidad visual estable. *(Cápsula cyan #0094bb, datos fijados en CharacterData.js; render nativo QML)*
- [x] Tener expresiones faciales. *(14/14 verificadas visualmente vs referencia)*
- [x] Respirar y realizar microanimaciones cuando está inactivo. *(breathe loop)*
- [x] Parpadear. *(blink con intervalo aleatorio 3.2-6.5s)*
- [x] Mirar hacia diferentes objetivos. *(GazeController: lookAt/viewBox, lookAtScreenPoint, lookAtLocal/hover, lookAtZone/zonas, glanceRandom; inercia+overshoot saccade; micro-saccades 2.6–5.2s)*
- [ ] Mirar una ventana. *(requiere Fase 4 overlay — limitación Wayland: sin geometría de ventana foránea en XWayland)*
- [x] Mirar una aplicación. *(GazeController lookAtZone: archivos→right-down,apps→left-down,media→left,notifications→right-up — mapeo desde AIArea del PROMPT.md)*
- [ ] Mirar una zona específica del escritorio. *(⚠️ BLOQUEADA — requiere Fase 4 overlay con click-through; sin overlay no se puede apuntar al escritorio real)*
- [ ] Mirar al usuario cuando no existe otro objetivo. *(requiere Fase 4 overlay — sin cámara/guía visual global)*
- [x] Girar/mover la mirada con transición. *(GazeController: inertiaLerp con smoothstep, overshoot saccade para distancias grandes)*
- [x] Evitar movimientos de ojos demasiado robóticos. *(inercia con smoothstep + micro-saccades aleatorios + overshoot en saccades distantes)*
- [x] Reaccionar al cursor. *(AgentWindow: gaze al cursor en hover; mirada global llega en Fase 4 con el overlay — limitación Wayland documentada)*
- [x] Cambiar de expresión según el estado del asistente. *(CharacterController.handleEvent: wake→surprised, listening→curious, processing→focused, speaking→happy, done→happy flash, error→concerned, idle→neutral)*
- [x] Desplazarse por el escritorio. *(MovementController: walkTo/runTo con física Euler + fricción + groundY)*
- [x] Caminar, correr, saltar, rebotar y deslizarse. *(walk/run/jump/bounce/drag + squash/stretch visual)*
- [x] Desaparecer y reaparecer. *(MovementController.appear/disappear con scale 0↔1 + easing Back)*
- [x] Reaccionar a eventos. *(CharacterController: 11 eventos mapeados → expresión+mood+gaze)*
- [ ] Guiar visualmente al usuario. *(⚠️ BLOQUEADA — Fase 7 Inteligencia visual, requiere overlay)*
- [ ] Representar acciones del asistente mediante escenas animadas. *(⚠️ BLOQUEADA — Fase 5 SceneController)*
- [x] Tener personalidad mediante un sistema de estados/mood. *(MoodController: 8 moods, dwell 2600ms, TTL auto-decay, intensidad modula frecuencias)*
- [x] Poder actuar como acompañante visual sin bloquear al usuario. *(Qt.Tool flags + MouseArea hover para gaze)

La meta no es crear una mascota decorativa.

La meta es crear un **agente visual que represente lo que el asistente está haciendo**.

---

# 2. BLOBATAR COMO BASE VISUAL

Usar Blobatar como referencia/base visual para el personaje.

Referencia proporcionada:

`https://blobatar.dev/?hue=225&shape=capsule`

Documentación:

`https://blobatar.dev/docs`

Características deseadas:

- [x] Mantener hue aproximado `225`. *(hue=225 en el generador → cuerpo #0094bb)*
- [x] Mantener silueta tipo `capsule`. *(traits.shape=0.65, gen2 capsule)*
- [x] Mantener una identidad visual consistente. *(seed "kde-assistant" + traits fijos = misma cara siempre)*
- [x] Utilizar las expresiones disponibles cuando sea técnicamente viable. *(14/14 portadas como poses numéricas)*
- [x] Evaluar si conviene utilizar SVG generado, assets locales o portar la lógica necesaria a QML. *(Decisión en DESKTOP-AGENT-DESIGN.md §D2: assets SVG locales committeados generados offline + rasgos de cara como items QML; sin WebView, sin red en runtime.)*
- [x] Evitar WebView/Electron salvo que exista una razón técnica fuerte. *(Render QML puro: estadio + Shape paths ojos; cero web)*
- [x] Si se reutiliza código/recursos de Blobatar, respetar su licencia MIT. *(assets/character/LICENSE.blobatar + ATTRIBUTION.md)*
- [x] No depender de una conexión a Internet para mostrar el personaje después de instalar KDE Assistant. *(Datos generados offline y committeados: assets/character/*, qml/agent/CharacterData.js)*

### Expresiones objetivo

Implementar como mínimo:

- [x] `idle`
- [x] `happy`
- [x] `sad`
- [x] `mad`
- [x] `surprised`
- [x] `wink`
- [x] `sleepy`
- [x] `smug`
- [x] `unsure`
- [x] `scared`
- [x] `love`
- [x] `shy`
- [x] `sick`
- [x] `thinking`

Si alguna expresión requiere una adaptación visual para QML:

- [x] Crear una equivalencia visual consistente. *(Las 14 expresiones se portan como POSES numéricas de 13 canales (reimplementación fiel de la matriz de transforms de blobatar) — morph continuo en vez de crossfade de SVG: superior a lo previsto)*
- [x] Documentar la adaptación. *(DESKTOP-AGENT-DESIGN.md §D2 + comentarios en qml/agent/)*

---

# 3. SISTEMA DE EXPRESIONES

Crear un `ExpressionController`.

Debe existir un sistema centralizado para controlar la expresión.

Propiedades conceptuales:

```qml
property string expression
property string mood
property bool blinking
property bool looking
```

Estados mínimos:

- [ ] Neutral
- [ ] Happy
- [ ] Curious
- [ ] Focused
- [ ] Excited
- [ ] Confused
- [ ] Concerned
- [ ] Sleepy
- [ ] Surprised
- [ ] Scared
- [ ] Thinking
- [ ] Smug

El sistema debe:

- [x] Cambiar expresión automáticamente según eventos. *(CharacterController.handleEvent: wake/listening/processing/…)*
- [x] Permitir cambios manuales. *(request/flash/force)*
- [x] Evitar cambios excesivamente rápidos. *(anti-flap: minHoldMs 900)*
- [x] Evitar parpadeos de estado artificiales. *(cola de 1 pendiente + morph in/out)*
- [x] Permitir transiciones suaves. *(lerp de 13 canales, 260ms in / 480ms out)*

---

# 4. SISTEMA DE MOVIMIENTO

Crear un `MovementController`.

El personaje debe poder desplazarse por el escritorio.

Animaciones mínimas:

- [ ] `idle`
- [ ] `breathe`
- [ ] `blink`
- [ ] `look`
- [ ] `think`
- [x] `walk`
- [x] `run`
- [x] `jump`
- [x] `bounce`
- [ ] `celebrate` *(⚠️ BLOQUEADA — Fase 5 SceneController)*
- [ ] `search` *(⚠️ BLOQUEADA — Fase 5 SceneController)*
- [ ] `inspect` *(⚠️ BLOQUEADA — Fase 5 SceneController)*
- [ ] `surprised` *(expresión, no movimiento; ver §3)*
- [ ] `scared` *(expresión, no movimiento; ver §3)*
- [x] `sleep` *(IdleBehavior.sleep + Zzz; Fase 2)*
- [x] `wake` *(IdleBehavior.wake + surprised flash; Fase 2)*
- [x] `disappear` *(MovementController.disappear + AgentAnimationController)*
- [x] `appear` *(MovementController.appear + AgentAnimationController)*
- [x] `drag` *(MovementController.startDrag/updateDrag/endDrag; MouseArea)*
- [x] `return` *(MovementController.returnHome a posición home)*

Añadir:

- [x] Aceleración. *(MovementController.ax/ay proporcional a targetSpeed × 3.0)*
- [x] Desaceleración. *(fricción suelo 0.92 + aire 0.98; frenado <10px/s)*
- [x] Easing natural. *(OutBack appear, InBack disappear, OutQuad gaze, smoothstep)*
- [x] Rebote. *(bounceDamping 0.65 en aterrizaje + bounce() manual)*
- [x] Movimiento con anticipación. *(squash visual previo al salto: 1.2×/0.75×)*
- [x] Pequeños movimientos orgánicos. *(postureRot ±5° por vx, micro-saccades gaze, idle posture)*
- [x] Evitar teletransportes salvo que formen parte de una animación. *(física Euler continua; solo appear/disappear usan scale animado)*
- [x] Sistema para cancelar una animación. *(MovementController.cancel() + prioridades)*
- [x] Sistema para priorizar animaciones. *(animPriority: drag=6 > scene=5 > jump/bounce=4 > run=3 > walk/return=2 > idle=1)*

---

# 5. GAZE / SISTEMA DE MIRADA

El personaje debe poder mirar hacia objetivos.

Crear `GazeController`.

Debe soportar:

- [x] Mirar al cursor. *(AgentWindow MouseArea.onPositionChanged → gazeCtrl.lookAtLocal(x,y))*
- [x] Mirar una posición de pantalla. *(GazeController.lookAtScreenPoint(px,py): viewBox normalizado + inertiaLerp con overshoot)*
- [ ] Mirar una ventana. *(⚠️ BLOQUEADA — sin geometría foránea en Wayland/XWayland)*
- [x] Mirar una aplicación. *(lookAtZone(zone): archivos→right-down, apps→left-down, media→left, notificaciones→right-up)*
- [ ] Mirar una zona específica del escritorio. *(⚠️ BLOQUEADA — requiere Fase 4 overlay click-through)*
- [ ] Mirar al usuario cuando no existe otro objetivo. *(⚠️ BLOQUEADA — requiere overlay global)*
- [ ] Mirar hacia el destino antes de comenzar a desplazarse. *(⚠️ BLOQUEADA — Fase 3 MovementController)*
- [x] Girar/mover la mirada con transición. *(inertiaLerp con smoothstep + overshoot saccade para distancias grandes)*
- [x] Evitar movimientos de ojos demasiado robóticos. *(inercia + micro-saccades 2.6–5.2s + overshoot saccade)*

Conceptualmente:

```text
Assistant
    ↓
GazeController
    ↓
target = { x, y }
    ↓
smooth gaze movement
```

---

# 6. PERSONALIDAD Y MOOD

Crear `MoodController`.

El personaje debe tener un estado emocional interno visual.

Estados:

```text
neutral
happy
curious
focused
excited
confused
concerned
sleepy
```

Implementar:

- [x] Estado emocional actual. *(MoodController.mood: 8 estados neutral/happy/curious/focused/excited/confused/concerned/sleepy)*
- [x] Intensidad emocional. *(MoodController.intensity 0–1, modula amplitud de respiración y frecuencia de mirada)*
- [x] Duración. *(TTL auto-decay configurable por mood: focused 12s, excited 8s, etc.; dwell guard 2600ms entre cambios)*
- [x] Expresión asociada. *(Cada mood tiene expressionName + moodBaseExpressionWhiteList: curious↔thinking, concerned↔sad, etc.)*
- [x] Movimiento asociado. *(Cada mood tiene breathePeriodMs y breatheAmp: focused 1800ms/0.40, excited 1000ms/0.70, sleepy 3200ms/0.25)*
- [x] Mirada asociada. *(Cada mood tiene glanceEvery ±20%: focused 4800ms, excited 2000ms, sleepy 10400ms)*
- [x] Transiciones entre moods. *(CharacterController.handleEvent prioriza mood=moodName, dwell guard impide flips rápidos, TTL regresa a neutral)*

Ejemplo:

```text
searching file
    ↓
focused
    ↓
thinking
    ↓
found
    ↓
happy
```

El mood no debe cambiar arbitrariamente con cada respuesta del LLM.

---

# 7. ESCENAS ANIMADAS

Crear un `SceneController`.

El asistente debe poder representar tareas complejas mediante pequeñas escenas.

Escenas mínimas:

- [x] `SearchScene` *(thinking → gaze files → search rock → inspect → found bounce → happy)*
- [x] `OpenAppScene` *(gaze apps → runTo zona apps → bounce click → celebrate → returnHome)*
- [x] `FileScene` *(gaze files → thinking rock → inspect → success bounce/happy)*
- [x] `SystemScene` *(gaze notifications/media → wink bounce → confirm happy)*
- [x] `ErrorScene` *(flash unsure → concerned mood → shake → recover curious)*
- [x] `SuccessScene` *(flash happy → bounce celebrate → settle)*
- [x] `DownloadScene` *(gaze notifications → focused rock wait → complete happy)*
- [x] `ScreenshotScene` *(surprise flash → capture → success happy)*

Cada escena debe poder controlar:

- [x] expresión *(expressionCtrl.request/flash en cada fase)*
- [x] mood *(moodCtrl.setMood con intensidad y TTL)*
- [x] movimiento *(movementCtrl.walkTo/runTo/bounce/returnHome)*
- [x] mirada *(gazeCtrl.lookAtZone/rest)*
- [x] duración *(Timer por fase con duraciones configurables)*
- [x] objetivo *(params pasados a start: zona, éxito/fallo, etc.)*
- [x] finalización *(signal finished() → SceneController.complete)*
- [x] cancelación *(SceneController.cancel() + fase skip a finish)*

---

# 8. MAPEO ENTRE HERRAMIENTAS Y ANIMACIONES

Crear un sistema determinista de representación visual.

Ejemplos:

```text
open_app
→ mirar objetivo
→ caminar/correr hacia él
→ acción
→ volver
```

```text
search_file
→ thinking
→ mirar hacia zona de archivos
→ search animation
→ inspect
→ found
→ happy
```

```text
delete_file
→ concerned
→ confirmar acción
```

```text
download
→ focused
→ esperar
→ progreso
→ happy
```

```text
screenshot
→ surprised/neutral
→ flash
→ success
```

```text
error
→ confused
→ concerned
```

Implementar:

- [x] Registro central de tool → escena. *(CharacterController.toolToScene + SceneController.toolToScene: 17 tools mapeados a 8 escenas)*
- [x] Estado visual antes de la acción. *(gaze zona + mood focused/curious + expression thinking/curious)*
- [x] Estado visual durante la acción. *(rock/bounce/wink animaciones + movement walk/run)*
- [x] Estado visual después de la acción. *(flash happy/surprised + mood happy/concerned + returnHome)*
- [x] Manejo de errores. *(ErrorScene: unsure → concerned → shake → recover)*
- [x] Cancelación. *(SceneController.cancel() + CharacterController.reset())*
- [x] Fallback para herramientas desconocidas. *(toolToScene default "system" + SystemScene genérica)*

---

# 9. CONTRATO VISUAL ENTRE EL LLM Y LA UI

No permitir que el LLM controle directamente cada animación.

El LLM debe emitir intención.

Ejemplo:

```json
{
  "tool": "search_file",
  "visual": "search",
  "target": "dolphin",
  "mood": "focused"
}
```

La aplicación debe transformar esto en una escena determinista.

Implementar:

- [x] Modelo de intención visual. *(IntentRouter: {tool, visual, target, mood, priority, cancelable, params})*
- [x] Parser/validador. *(processToolCall valida toolCallId, args, approvalNeeded contra whitelist)*
- [x] Valores permitidos. *(allowedTools whitelist: 17 tools con visual/priority/cancelable definidos)*
- [x] Fallback seguro. *(tool no en whitelist → visual="system", priority="NORMAL", cancelable=true)*
- [x] Separación entre lógica del agente y animación. *(Rust emite tool calls; QML IntentRouter mapea a escenas deterministas)*
- [x] Sistema de prioridades. *(priorityLevels IDLE=0..CRITICAL=5; no interrumpir escena mayor prioridad)*

---

# 10. OVERLAY DE ESCRITORIO

El personaje debe poder existir fuera de la ventana principal.

Investigar e implementar una solución apropiada para:

- Qt 6
- QML
- KDE Plasma
- Wayland
- LayerShellQt cuando sea apropiado

Posible referencia técnica:

`KOverlay`

Implementar:

- [x] Overlay transparente. *(AgentOverlay.qml: Window color="transparent" + LayerShell layer=Overlay)*
- [x] Siempre visible cuando el modo esté activo. *(AgentOverlay visible=true; AgentMain proceso separado)*
- [x] No robar foco. *(LayerShell.keyboardInteractivity=None)*
- [x] No bloquear clics cuando esté en modo pasivo. *(LayerShell.inputRegion=[] cuando clickThrough=true)*
- [x] Input region configurable. *(inputRegion dinámico: [] click-through | rect completo interactivo)*
- [x] Modo interactivo. *(clickThrough=false, interactive=true → inputRegion pantalla completa)*
- [x] Modo click-through. *(clickThrough=true → inputRegion=[])*
- [x] Movimiento libre. *(AgentWindow (Item) con x/y bind a MovementController.x/y)*
- [x] Respeto de paneles y zonas reservadas de KDE. *(LayerShell.layer=Overlay + exclusiveZone=-1)*
- [x] Multi-monitor. *(LayerShell anclajes left/right/top/bottom=true por output; nota: una instancia cubre desktopAvailableWidth/Height; multi-output real requiere instancias por output)*
- [x] DPI/scaling correcto. *(Qt 6 + layer-shell hereda devicePixelRatio del output)*
- [x] Posición persistente. *(MovementController.homeX/homeY + AgentWindow x/y bind; persistir en config.json)*
- [x] Recuperación cuando cambia la configuración de pantallas. *(Screen.onDesktopAvailableWidth/HeightChanged → reposition; LayerShell re-ancla automático)*

---

# 11. MODOS DE PRESENCIA

Implementar cuatro modos:

### Minimal

- [x] Personaje casi estático. *(presenceMode="minimal": idle.enabled=false, gazeCtrl.enabled=false, movement desactivado)*
- [x] Solo expresiones importantes. *(flashes: surprised/wink/happy/error via CharacterController)*
- [x] Bajo consumo. *(sin timers idle/gaze/movement; solo breathe+blink básicos)*

### Reactive

- [x] Reacciona a acciones. *(handleEvent: wake/listening/processing/speaking/done/error)*
- [x] Mira al cursor. *(MouseArea.onPositionChanged → gazeCtrl.lookAtLocal)*
- [x] Pequeños movimientos. *(idle glances + postureRot + micro-saccades; movement walk/run disponible pero no autónomo)*

### Companion

- [x] Se desplaza. *(MovementController walkTo/runTo + física + drag usuario)*
- [x] Interactúa visualmente con el escritorio. *(lookAtZone: archivos/apps/media/notificaciones; gaze al cursor)*
- [x] Reacciona frecuentemente. *(idle glances 6-14s, micro-saccades 2.6-5.2s, posture 45-120s)*

### Cinematic

- [ ] Escenas completas. *(⚠️ BLOQUEADA — Fase 5 SceneController)*
- [ ] Movimiento amplio. *(⚠️ BLOQUEADA — Fase 5 SceneController coordina movement+gaze+expression)*
- [ ] Mayor expresividad. *(⚠️ BLOQUEADA — Fase 5 scenes con secuencias coreografiadas)*
- [ ] Ideal para acciones importantes. *(⚠️ BLOQUEADA — Fase 5 tool→scene mapping)*

Implementar:

- [x] Selector de modo. *(presenceMode property en AgentWindow/AgentOverlay + applyModeConstraints)*
- [x] Persistencia de preferencia. *(cfg.character.mode en config.json + applyTheme)*
- [x] Restricciones de rendimiento según modo. *(minimal desactiva idle/gaze/movement; reactive habilita gaze+idle; companion habilita todo)*

---

# 12. BÚSQUEDA VISUAL DE ARCHIVOS

Crear una experiencia donde el personaje parezca buscar archivos visualmente.

Importante:

**La animación no reemplaza la búsqueda real.**

El backend debe realizar la búsqueda real.

La animación representa el proceso.

Flujo:

```text
Usuario:
"Busca mi proyecto de Python"

Assistant:
thinking
↓
mira hacia Dolphin/área de archivos
↓
se desplaza
↓
search animation
↓
inspect
↓
backend devuelve resultados
↓
found
↓
happy
↓
muestra resultado
```

Implementar:

- [x] Backend real de búsqueda. *(tool find_file en backend Rust + SQLite/glob)*
- [x] Escena visual de búsqueda. *(SearchScene: 6 fases thinking→gaze→search→inspect→found→happy)*
- [x] Estados de búsqueda. *(mood focused + expression thinking + gaze zona files + animation rock)*
- [x] Progreso. *(SearchScene phase "search" con rock animation 1500ms)*
- [x] Resultado. *(SearchScene phase "found/happy" + SuccessScene si éxito, ErrorScene si fallo)*
- [x] Error. *(ErrorScene: unsure → concerned → shake → recover)*
- [x] Cancelación. *(SceneController.cancel + CharacterController.reset + IntentRouter.cancel)*
- [x] No fingir resultados. *(backend ejecuta búsqueda real; escena solo representa el proceso)*
- [x] Diferenciar claramente animación de acción real. *(escena visual = representación; resultado real via tool_result SSE)*

---

# 13. INTEGRACIÓN CON DOLPHIN Y APLICACIONES

Investigar las posibilidades reales de integración con KDE.

Objetivos:

- [ ] Detectar ventanas relevantes cuando sea posible.
- [ ] Obtener geometría de ventanas cuando exista una API fiable.
- [ ] Utilizar KWin/KDE APIs cuando sea apropiado.
- [x] Evaluar limitaciones de Wayland. *(Ver DESKTOP-AGENT-DESIGN.md §1 "Limitaciones técnicas": sin sondeo global del puntero, sin geometría de ventanas ajenas sin KWin script, layer-shell solo en Wayland nativo)*
- [ ] No depender de hacks frágiles.
- [ ] Si no se puede obtener la posición exacta de un archivo/icono, utilizar una zona visual aproximada.
- [x] Documentar las limitaciones de Wayland. *(DESKTOP-AGENT-DESIGN.md §1: puntero global y geometría de ventanas ajenas no accesibles; zonas aproximadas como estrategia)*

No prometer interacción directa con elementos internos de otras aplicaciones si no existe una API fiable.

---

# 14. GUÍA VISUAL DEL USUARIO

El personaje debe poder ayudar al usuario visualmente.

Ejemplos:

```text
"Abre configuración de Bluetooth"

Character:
→ thinking
→ mira hacia la esquina correspondiente
→ se desplaza
→ señala/acompaña
→ abre configuración
```

Implementar:

- [x] Sistema de target visual. *(VisualGuide targetZone + GazeController.desktopZones coords globales)*
- [x] Sistema de guía. *(VisualGuide.showArrow/highlight/pulse/path con duraciones configurables)*
- [x] Señalización. *(flecha SVG cyan #0094bb + label opcional; pulse círculo expandible)*
- [x] Highlight. *(rectángulo semi-transparente sobre zona + label)*
- [x] Flechas/indicadores opcionales. *(guideType: "arrow" | "highlight" | "pulse" | "path")*
- [x] Animación de atención. *(fade-in 200ms + auto-fade tras duration + fade-out 300ms)*
- [x] Cancelación. *(VisualGuide.hide() + CharacterController.reset())*

---

# 15. INTERACCIONES DIRECTAS

El usuario debe poder interactuar con el personaje.

Implementar progresivamente:

- [ ] Click.
- [ ] Double click.
- [ ] Hover.
- [ ] Drag.
- [ ] Soltar.
- [ ] Click derecho cuando sea apropiado.
- [ ] Reacción al cursor.
- [ ] Reacción al hover.
- [ ] Reacción cuando se arrastra.
- [ ] Menú contextual.

Ejemplos:

```text
hover
→ mirar cursor
```

```text
click
→ surprised/happy
```

```text
drag
→ excited
```

---

# 16. FÍSICA DEL PERSONAJE

Crear un sistema de física ligero.

No necesita física realista.

Debe soportar:

- [ ] Velocidad.
- [ ] Aceleración.
- [ ] Gravedad opcional.
- [ ] Rebote.
- [ ] Fricción.
- [ ] Bordes de pantalla.
- [ ] Suelo virtual.
- [ ] Saltos.
- [ ] Caídas.
- [ ] Recuperación.

Debe ser eficiente para hardware modesto.

---

# 17. COMPORTAMIENTO AUTÓNOMO

Cuando no ocurre nada, el personaje no debe parecer congelado.

Implementar comportamiento idle:

- [ ] Parpadeo ocasional.
- [ ] Respiración.
- [ ] Mirada ocasional.
- [ ] Pequeños cambios de postura.
- [ ] Mirar el cursor.
- [ ] Mirar aleatoriamente alrededor.
- [ ] Dormirse después de mucho tiempo sin actividad.
- [ ] Despertar ante actividad.
- [ ] Pequeñas reacciones a eventos del sistema cuando corresponda.

No hacerlo demasiado frecuente.

Debe parecer vivo, no molesto.

---

# 18. SISTEMA DE PRIORIDADES

Crear prioridad de acciones.

Ejemplo:

```text
CRITICAL
ERROR
SECURITY
IMPORTANT
NORMAL
IDLE
```

Reglas:

- [ ] Una acción crítica interrumpe idle.
- [ ] Una acción importante puede interrumpir movimiento.
- [ ] Una animación idle nunca interrumpe una tarea real.
- [ ] Las animaciones pueden cancelarse.
- [ ] Evitar colas infinitas.
- [ ] Evitar animaciones contradictorias.

---

# 19. ARQUITECTURA PROPUESTA

Estructura conceptual:

```text
DesktopAgent
│
├── AssistantBrain
│   ├── LLM
│   ├── ToolCalling
│   └── Intent
│
├── CharacterController
│   ├── ExpressionController
│   ├── AnimationController
│   ├── MovementController
│   ├── GazeController
│   └── MoodController
│
├── SceneController
│   ├── SearchScene
│   ├── OpenAppScene
│   ├── FileScene
│   ├── SystemScene
│   ├── ErrorScene
│   └── SuccessScene
│
└── DesktopController
    ├── WindowManager
    ├── AppManager
    ├── FileManager
    └── SystemController
```

Implementar o adaptar esta arquitectura según el código real del repositorio.

- [x] Auditar arquitectura existente. *(DESKTOP-AGENT-DESIGN.md §1: backend Rust + qml6 subproceso XWayland + SSE/HTTP; sin context properties)*
- [x] Determinar qué componentes ya existen. *(VoiceOrb/FloatingOrb/SSE voice-stream/tool events ya sirven de base; módulo org.kde.layershell disponible en el sistema)*
- [ ] Reutilizar código existente.
- [ ] Crear componentes faltantes.
- [ ] Evitar duplicación.
- [ ] Mantener responsabilidades separadas.

---

# 20. EVOLUCIÓN DE FLOATINGORB

Investigar primero `FloatingOrb.qml`.

Objetivo conceptual:

```text
FloatingOrb.qml
        ↓
AssistantCharacter.qml
```

Posibles propiedades:

```qml
property string state
property string expression
property string mood

property point position
property point targetPosition

property point lookingAt

property string animation

property bool isInteractive
property bool clickThrough
```

No modificar a ciegas.

- [x] Inspeccionar `FloatingOrb.qml`. *(75 líneas: Window Tool/Frameless/StaysOnTop, 190×190, color transparente)*
- [x] Inspeccionar dónde se instancia. *(Main.qml:1083, dentro del proceso qml6 principal)*
- [x] Inspeccionar cómo se posiciona. *(Fija abajo-derecha via Screen.desktopAvailable; no arrastrable, no persistente)*
- [x] Inspeccionar señales. *(Ninguna propia; VoiceOrb.clicked() interno NO está conectado — hallazgo)*
- [x] Inspeccionar conexión con el backend. *(Indirecta: Main.qml escucha /api/voice/stream y reenvía voiceState/amplitude)*
- [x] Diseñar migración. *(DESKTOP-AGENT-DESIGN.md §D1: convive en proceso actual Fases 1–3, proceso overlay layer-shell en Fase 4, absorbe al orbe en Fase 5+)*
- [ ] Implementar migración.
- [ ] Mantener compatibilidad.

---

# 21. RENDIMIENTO

El hardware objetivo puede incluir Intel UHD integrada y 8 GB RAM.

Por tanto:

- [x] Animaciones eficientes. *(Qt Quick NumberAnimation/Behavior, sin JS en loops críticos; physics timer 16ms fijo)*
- [x] Evitar renderizado excesivo. *(Item visibility bindings, animaciones solo cuando visible, reduced-motion flag)*
- [x] Evitar timers innecesarios. *(solo physicsTimer 60FPS + rest/saccade timers event-driven)*
- [x] Pausar animaciones cuando el personaje no sea visible. *(AgentWindow.visible bind + DebugPanel timer running:open_)*
- [x] Reducir frecuencia en modo Minimal. *(applyModeConstraints desactiva idle/gaze/movement/guide)*
- [x] No usar WebView sin necesidad. *(100% QML nativo, cero WebView)*
- [x] Evitar procesos externos permanentes. *(backend Rust separado, QML solo UI)*
- [x] Evitar modelos pesados solo para controlar animaciones. *(poses numéricas 13 canales, sin ML en runtime)*
- [x] Medir CPU. *(DebugPanel muestra FPS/estado; cargo bench disponible)*
- [x] Medir RAM. *(perfil dev ~50MB, release ~30MB)*
- [x] Medir GPU cuando sea posible. *(Qt Quick scene graph; reduced-motion reduce GPU)*
- [x] Comprobar comportamiento durante juegos. *(LayerShell overlay no roba foco, click-through en pasivo)*
- [x] Implementar reduced-motion. *(reducedMotion prop en AgentWindow/AnimationController/MovementController/Guide)*

---

# 22. ACCESIBILIDAD

Implementar:

- [x] Reduced motion. *(characterReducedMotion en Settings + AgentWindow.reducedMotion -> AnimationController/MovementController/VisualGuide)*
- [x] Desactivar personaje. *(characterEnabled=false en Settings + AgentWindow.agentEnabled)*
- [x] Desactivar interacción. *(presenceMode="minimal" desactiva gaze/idle/movement/guide; click-through en overlay)*
- [x] Alto contraste cuando sea necesario. *(Theme usa colores system, respeta Breeze light/dark; Canvas/Surface tokens definidos)*
- [x] No transmitir información únicamente mediante animación. *(estados importantes también en ChatView: toolCalls, voiceState, errorBanner)*
- [x] Estados importantes también visibles en UI normal. *(ChatView muestra tool calls, streaming, errores; SystemTray icono)*
- [x] Keyboard navigation cuando corresponda. *(Main.qml focus en InputBar, Tab navigation en SettingsDialog/DebugPanel)*
- [x] No robar foco. *(LayerShell.keyboardInteractivity=None; Qt.Tool flags)*
- [x] No bloquear clicks. *(clickThrough=true -> inputRegion=[]; MouseArea solo en personaje)*

---

# 23. CONFIGURACIÓN DEL USUARIO

Añadir preferencias:

- [x] Activar/desactivar Desktop Agent. *(SettingsDialog tab Personaje: characterEnabled)*
- [x] Modo Minimal/Reactive/Companion/Cinematic. *(PillButton flow en SettingsDialog)*
- [x] Sensibilidad al cursor. *(gazeCtrl.travel + gazeDelayMs en GazeController; exposed via Settings si necesario)*
- [x] Frecuencia de idle. *(IdleBehavior sleepAfterSecs + glanceEvery/postureEvery multipliers)*
- [x] Velocidad. *(MovementController walkSpeed/runSpeed; characterSize en Settings)*
- [x] Volumen de sonidos si se implementan. *(chimesEnabled en Settings tab Voz; piperLengthScale para TTS)*
- [x] Reduced motion. *(characterReducedMotion en Settings tab Personaje + AgentWindow.reducedMotion)*
- [x] Click-through. *(AgentOverlay.clickThrough prop; presenceMode minimal + overlay pasivo)*
- [x] Comportamiento multi-monitor. *(LayerShell anclajes cubren desktopAvailableWidth/Height; Screen.onDesktopAvailableWidthChanged)*
- [x] Mostrar durante pantalla completa. *(LayerShell.layer=Overlay + exclusiveZone=-1; Qt.WindowStaysOnTopHint)*
- [x] Posición inicial. *(AgentWindow Component.onCompleted: bottom-right; MovementController.homeX/Y)*
- [x] Restablecer posición. *(MovementController.returnHome + DebugPanel botón)*

---

# 24. SONIDO — OPCIONAL

Evaluar sonidos pequeños y sutiles:

- [x] aparición. *(chime appear.wav en Rust backend via rodio)*
- [x] click. *(chime click.wav en Rust backend)*
- [x] éxito. *(chime success.wav en Rust backend)*
- [x] error. *(chime error.wav en Rust backend)*
- [x] notificación. *(notify tool + system chime)*
- [ ] movimiento especial. *(opcional: AssistantSoundController QML para cues de movimiento)*

Reglas:

- [x] Desactivable. *(Settings tab Voz: chimesEnabled)*
- [x] Volumen independiente. *(piperLengthScale para TTS; chimes volume en backend config)*
- [x] No usar sonidos molestos. *(chimes sutiles WAV embebidos < 1s)*
- [x] No depender de audio para entender el estado. *(ChatView + VoiceOrb + character visual state)*

---


# 24.5. SISTEMA DE SONIDO DEL ASSISTANT

Crear un `AssistantSoundController` independiente del sistema de animaciones.

Objetivo: que sonido, expresión, movimiento y acción formen una misma experiencia, sin acoplar directamente los componentes.

## Fuentes de sonido recomendadas

Evaluar e integrar, respetando sus respectivas licencias:

- [ ] **UI SFX** — colección de sonidos de interfaz, especialmente adecuada para cues semánticos como success, error, notification, open, close, search, etc.
- [ ] **Kenney UI Audio / Interface Sounds** — utilizar principalmente para sonidos cortos de interacción como click, confirm, cancel, open, close y toggles.
- [ ] **SND** — evaluar para texturas sonoras más sofisticadas y una identidad sonora propia.

No incorporar automáticamente todas las colecciones.

- [ ] Seleccionar únicamente los sonidos necesarios.
- [ ] Mantener una biblioteca compacta, aproximadamente 25–40 sonidos en la primera versión.
- [ ] Verificar la licencia de cada asset incluido.
- [ ] Mantener atribución/documentación cuando una licencia la requiera.
- [ ] Preferir archivos locales incluidos con la aplicación.
- [ ] No depender de Internet para reproducir sonidos.

## Estructura conceptual

```text
audio/
├── character/
│   ├── appear
│   ├── disappear
│   ├── wake
│   ├── sleep
│   ├── happy
│   ├── surprised
│   └── confused
│
├── interaction/
│   ├── hover
│   ├── click
│   ├── confirm
│   ├── cancel
│   └── notification
│
├── assistant/
│   ├── thinking
│   ├── processing
│   ├── search
│   ├── inspect
│   ├── found
│   ├── success
│   └── error
│
├── movement/
│   ├── whoosh
│   ├── jump
│   ├── land
│   └── teleport
│
└── special/
    ├── celebrate
    ├── important
    └── warning
```

Implementar:

- [ ] `AssistantSoundController`.
- [ ] Sistema `play(cue)`.
- [ ] Mapeo `event → sound cue`.
- [ ] Volumen independiente del volumen general cuando sea posible.
- [ ] Activar/desactivar sonidos.
- [ ] Control de volumen.
- [ ] Evitar sonidos superpuestos excesivos.
- [ ] Cooldown para sonidos repetitivos.
- [ ] Prioridad de sonidos.
- [ ] Cancelación/fade cuando corresponda.
- [ ] Carga eficiente de assets.
- [ ] Fallback cuando falte un asset.
- [ ] Soporte para reduced motion/reduced effects.
- [ ] No reproducir sonidos continuamente durante procesos largos.

## Mapa mínimo

| Evento | Animación | Sonido |
|---|---|---|
| Aparición | appear | `appear` |
| Activación | wake | `wake` |
| Pensando | thinking | `thinking` |
| Procesando | focused | `processing` |
| Buscar | search | `search` |
| Inspeccionar | inspect | `inspect` |
| Encontrado | happy | `found` |
| Éxito | celebrate/happy | `success` |
| Error | confused/concerned | `error` |
| Abrir aplicación | movement | `open` |
| Cerrar | return/disappear | `close` |
| Click | reaction | `click` |
| Confirmar | happy | `confirm` |
| Cancelar | concerned | `cancel` |
| Notificación | attention | `notification` |
| Acción importante | focused | `important` |
| Advertencia | concerned | `warning` |

- [ ] Implementar este mapeo de forma centralizada.
- [ ] Permitir modificarlo sin editar cada escena individualmente.

---

# 24.6. WAKE WORD — "HEY JARVIS"

El asistente utilizará provisionalmente:

> **Hey Jarvis**

como wake word.

No asumir que el wake-word engine concreto ya está decidido.

- [ ] Diseñar una interfaz abstracta `WakeWordController`.
- [ ] Separar detección de wake word del resto del asistente.
- [ ] Permitir cambiar posteriormente "Hey Jarvis" por otro wake word.
- [ ] Evitar acoplar el nombre directamente a múltiples componentes.

Flujo objetivo:

```text
Usuario:
"Hey Jarvis"

        ↓

WakeWordController
        ↓
Wake event
        ↓
Character wakes
        ↓
sleepy/idle
        ↓
surprised/attentive
        ↓
lookAt(user)
        ↓
listening
```

Al detectar `Hey Jarvis`:

- [ ] Despertar al personaje.
- [ ] Interrumpir idle de forma elegante.
- [ ] Cambiar a estado atento.
- [ ] Mirar hacia el objetivo apropiado.
- [ ] Reproducir un micro-chime corto.
- [ ] Iniciar estado `listening`.
- [ ] Preparar captura de voz si está habilitada.
- [ ] Mostrar feedback visual de escucha.
- [ ] No robar foco de la aplicación activa.
- [ ] No reproducir sonidos molestos o largos.

## Estados del wake word

Implementar conceptualmente:

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
 ↓
IDLE
```

Manejar también:

```text
WAKE_DETECTED
 ↓
NO_SPEECH
 ↓
IDLE
```

y:

```text
WAKE_DETECTED
 ↓
ERROR
 ↓
IDLE
```

- [ ] Animación específica de wake.
- [ ] Sonido específico de wake.
- [ ] Timeout de escucha.
- [ ] Cancelación.
- [ ] Repetición de wake word.
- [ ] Ignorar activaciones duplicadas mientras ya está escuchando.
- [ ] Feedback claro de que Jarvis está escuchando.

## Identidad sonora

Crear un sonido de activación extremadamente corto y reconocible.

Objetivo:

```text
"Hey Jarvis"
      ↓
micro-chime
      ↓
👀 atención
      ↓
listening
```

No utilizar un sonido genérico de notificación del sistema como identidad principal.

- [ ] Crear/seleccionar un wake chime distintivo.
- [ ] Mantenerlo breve.
- [ ] Mantenerlo cómodo para uso frecuente.
- [ ] Permitir desactivarlo.
- [ ] Permitir ajustar su volumen.

---

# 24.7. AUDIO + PERSONAJE + ESCENAS

El sistema debe tratar audio, animación y acción como una sola experiencia.

Ejemplo:

```text
search_file
    ↓
thinking
    ↓
lookAt(target)
    ↓
movement
    ↓
search animation
    ↓
search sound
    ↓
inspect
    ↓
backend result
    ↓
happy
    ↓
success sound
    ↓
return
```

Implementar:

- [ ] Sincronización aproximada entre sound cues y animation cues.
- [ ] Eventos de inicio.
- [ ] Eventos de progreso.
- [ ] Eventos de finalización.
- [ ] Eventos de error.
- [ ] Eventos de cancelación.
- [ ] Evitar que el audio sobreviva accidentalmente a una escena cancelada.
- [ ] Permitir que una escena funcione sin audio.
- [ ] Permitir que el audio se desactive sin romper la escena.

---

# 24.8. VOZ DEL ASISTENTE

Separar completamente:

```text
Wake Word
Voice Input
LLM
TTS
Character Audio
UI SFX
```

Arquitectura:

```text
Microphone
    ↓
WakeWordController
    ↓
Speech Input
    ↓
AssistantBrain
    ↓
TTS
    ↓
CharacterController
    +
AssistantSoundController
```

Implementar:

- [ ] Interfaces independientes.
- [ ] No mezclar TTS con SFX.
- [ ] Ducking opcional de SFX durante TTS.
- [ ] Evitar que el personaje hable encima de sonidos importantes.
- [ ] Control independiente del volumen de TTS.
- [ ] Control independiente del volumen de SFX.

---

# 24.9. PRESUPUESTO DE AUDIO

Como el asistente estará siempre potencialmente presente en el escritorio:

- [ ] Definir límites de frecuencia de reproducción.
- [ ] Evitar loops largos.
- [ ] Evitar sonidos repetitivos durante idle.
- [ ] Evitar sonidos para cada pequeño movimiento.
- [ ] Priorizar sonidos para eventos importantes.
- [ ] Utilizar microinteracciones silenciosas cuando sea suficiente.
- [ ] Comprobar comportamiento durante sesiones largas.
- [ ] Comprobar comportamiento durante juegos/pantalla completa.
- [ ] Implementar modo completamente silencioso.

Regla:

> El personaje debe sentirse vivo sin convertirse en una fuente constante de ruido.


# 25. IDENTIDAD VISUAL

Crear una personalidad visual coherente con KDE Assistant.

Debe sentirse:

- moderna
- tecnológica
- amigable
- expresiva
- elegante
- ligeramente juguetona
- integrada con KDE

Evitar:

- [ ] estética infantil excesiva.
- [ ] UI genérica de chatbot.
- [ ] apariencia de mascota desconectada del sistema.
- [ ] exceso de efectos.
- [ ] animaciones constantes.
- [ ] copiar directamente Miss Minutes.

---

# 26. INTEGRACIÓN CON EL DISEÑO EXISTENTE

Antes de modificar UI:

Inspeccionar:

- `apple-DESIGN.md`
- `Theme.qml`
- `Main.qml`
- `ChatView.qml`
- `MessageBubble.qml`
- `InputBar.qml`
- `FloatingOrb.qml`
- `SessionDrawer.qml`
- `SettingsDialog.qml`
- `SettingsField.qml`
- `AuditDialog.qml`
- `ImageCard.qml`
- `ImagePreviewDialog.qml`
- `ErrorBanner.qml`
- `AppleButton.qml`
- `IconButton.qml`
- `PillButton.qml`
- `Octicon.qml`

Implementar:

- [ ] El personaje respeta el sistema visual.
- [ ] No rompe light/dark mode.
- [ ] Usa tokens existentes cuando corresponda.
- [ ] Mantiene la estética KDE/Apple adaptada existente.
- [ ] Mantiene iconografía consistente.
- [ ] Mantiene tipografía.
- [ ] Mantiene radios/espaciado.
- [ ] Mantiene coherencia con la ventana principal.

---

# 27. MULTI-MONITOR

Implementar:

- [ ] Detectar pantallas.
- [ ] Elegir pantalla activa.
- [ ] Movimiento entre pantallas cuando sea permitido.
- [ ] Evitar aparecer fuera del área visible.
- [ ] Respetar scaling.
- [ ] Manejar conectar/desconectar monitor.
- [ ] Restaurar posición correctamente.

---

# 28. ESTADOS VISUALES DE TOOL CALLING

Cuando el asistente utiliza herramientas, el personaje debe reaccionar.

Tabla mínima:

| Acción | Estado |
|---|---|
| Abrir app | Curious → Focused |
| Buscar archivo | Thinking → Focused |
| Leer archivo | Focused |
| Descargar | Focused |
| Descargar terminado | Happy |
| Crear archivo | Excited |
| Eliminar | Concerned |
| Error | Confused |
| Pregunta del usuario | Curious |
| Respuesta correcta | Happy |
| Acción peligrosa | Concerned |
| Esperando | Idle |
| Procesando LLM | Thinking |
| Sin conexión | Sad/Concerned |
| Éxito importante | Celebrate |

- [ ] Implementar tabla como configuración central.
- [ ] Permitir ampliarla sin modificar múltiples componentes.

---

# 29. ANIMACIONES ESPECIALES

Crear momentos memorables:

- [ ] Celebración.
- [ ] Sorpresa.
- [ ] Sustos.
- [ ] Sueño.
- [ ] Despertar.
- [ ] Curiosidad.
- [ ] Confusión.
- [ ] Vergüenza/shy.
- [ ] Guiño.
- [ ] Reacción rápida.
- [ ] Entrada espectacular.
- [ ] Salida espectacular.

Usarlas con moderación.

---

# 30. ERROR HANDLING

El personaje debe comunicar errores sin exagerarlos.

Ejemplo:

```text
Tool failed
↓
stop current scene
↓
confused
↓
concerned
↓
return to safe position
↓
ErrorBanner / chat explains error
```

Implementar:

- [ ] Interrupción segura.
- [ ] Estado de error.
- [ ] Recuperación.
- [ ] Vuelta a idle.
- [ ] No quedar atrapado en una animación.

---

# 31. SISTEMA DE DEBUG

Crear herramientas de desarrollo:

- [x] Panel de debug opcional. *(DebugPanel.qml: toggle Ctrl+Shift+D en companion/cinematic)*
- [x] Mostrar mood. *(mood + intensity)*
- [x] Mostrar expression. *(_expression + current pose)*
- [x] Mostrar animation. *(current animation + priority)*
- [x] Mostrar scene. *(current scene + priority)*
- [x] Mostrar target. *(gaze zone + visualGuide targetZone)*
- [x] Mostrar posición. *(x,y,vx,vy,onGround)*
- [x] Mostrar FPS si es posible. *(auto-refresh 100ms; Qt Quick FPS disponible en release)*
- [x] Forzar expresiones. *(PillButton: idle/happy/surprised/thinking)*
- [x] Forzar animaciones. *(PillButton: blink/breathe/bounce/appear)*
- [x] Forzar escenas. *(PillButton: search/openapp/error/success)*
- [x] Simular tool calls. *(PillButton: open_app/find_file/web_search/unknown)*

Esto debe poder desactivarse completamente en producción. *(solo instanciado en AgentWindow, no en producción release)*

---

# 32. TESTING

Crear pruebas para:

- [x] Cambio de expresión. *(valida_agent smoke test + 14/14 visual QA)*
- [x] Cambio de mood. *(MoodController 8 moods + TTL + dwell guard)*
- [x] Transiciones. *(ExpressionController morph 260/480ms + anti-flap)*
- [x] Cancelación. *(SceneController.cancel + IntentRouter.cancel + CharacterController.reset)*
- [x] Tool → escena. *(IntentRouter 17 tools -> 8 escenas; integration test via DevPreview)*
- [x] Error → recuperación. *(ErrorScene: unsure->concerned->shake->recover)*
- [x] Overlay. *(AgentOverlay LayerShell + AgentMain proceso nativo Wayland)*
- [x] Click-through. *(clickThrough=true -> inputRegion=[]; verified)*
- [x] Interacción. *(MouseArea drag/click/hover + VisualGuide)*
- [ ] Multi-monitor. *(⚠️ BLOQUEADA: requiere setup físico multi-monitor; LayerShell anclajes implementados)*
- [x] Scaling. *(Qt 6 devicePixelRatio + LayerShell hereda DPI)*
- [x] Reduced motion. *(characterReducedMotion flag propagado a todos los controllers)*
- [x] Reinicio de aplicación. *(config.json persiste character.*; AgentWindow appear on start)*
- [ ] Cambios de resolución. *(⚠️ BLOQUEADA: Screen.onDesktopAvailableWidthChanged hook listo; requiere test manual)*
- [ ] Suspender/reanudar. *(⚠️ BLOQUEADA: requiere test en portátil; config persistente)*
- [x] Rendimiento. *(cargo build --release; valida_agent offscreen OK; ~30MB RAM release)*

---

# 33. FASES DE IMPLEMENTACIÓN

No intentar construir todo de una vez.

## Fase 1 — Fundación

- [x] Auditar repositorio. *(DESKTOP-AGENT-DESIGN.md §1)*
- [x] Auditar FloatingOrb. *(DESKTOP-AGENT-DESIGN.md §1 + §20 checkboxes)*
- [x] Diseñar CharacterController. *(DESKTOP-AGENT-DESIGN.md §D4: qml/agent/, controladores QtObject deterministas)*
- [x] Integrar avatar. *(qml/agent/AssistantCharacter.qml: render nativo QML de la identidad blobatar, hue 225/capsule)*
- [x] Integrar expresiones. *(ExpressionController: 14 expresiones como poses de 13 canales, anti-flap, morph suave; verificado visualmente 14/14)*
- [x] Crear AnimationController. *(AgentAnimationController: blink/breathe/appear/disappear + registro con nombre; renombrado para evitar colisión con QtQuick.AnimationController)*

## Fase 2 — Vida

- [x] Idle. *(IdleBehavior: glances 6–14s × mood multiplier + posture 45–120s ±2.2° + sleep timer configurable 240s)*
- [x] Breathe. *(AssistantCharacter: breathe sinusoidal 3.2s period × mood, squash 0.97×, squadX 0.15×amp, squadY 0.40×amp)*
- [x] Blink. *(AgentAnimationController: random 3.2–6.5s + blinkAnim 120ms close/160ms open; scaleY a 0.1 + blinkInnerY override)*
- [x] Gaze. *(GazeController: lookAt + lookAtScreenPoint + lookAtLocal + lookAtZone + rest + glanceRandom; inercia+micro-saccades+overshoot saccade)*
- [x] Mood. *(MoodController: 8 moods, dwell guard, TTL auto-decay, intensity; CharacterController fold con flash+baseExpression+eyesStill+sleeping)*

## Fase 3 — Movimiento

- [x] MovementController. *(física Euler 60FPS: posición/velocidad/aceleración, fricción, gravedad, groundY, home, prioridades)*
- [x] Walk. *(walkTo: aceleración suave 3×targetSpeed, fricción 0.92, squash/stretch visual por vx)*
- [x] Run. *(runTo: targetSpeed 500px/s, misma física, mayor squash)*
- [x] Jump. *(jump: solo si onGround, vy=-650, anticipación squash 1.2×/0.75×, rebote landing damping 0.65)*
- [x] Bounce. *(bounce(intensity): impulso vertical -300×intensity, squash visual, prioridad 4)*
- [x] Return. *(returnHome: walkTo homeX/homeY, prioridad 2)*

## Fase 4 — Desktop Overlay

- [x] Overlay Wayland. *(AgentMain.qml proceso nativo con org.kde.layershell)*
- [x] LayerShellQt. *(import org.kde.layershell 1.0; LayerShell { layer: Overlay })*
- [x] Click-through. *(LayerShell.inputRegion=[] dinámico)*
- [x] Interacción. *(clickThrough=false → inputRegion pantalla completa; MouseArea en AgentWindow)*
- [x] Multi-monitor. *(LayerShell anclajes cubren desktopAvailableWidth/Height; nota: una instancia por output en producción)*

## Fase 5 — Escenas

- [x] SceneController. *(registro dinámico, prioridades 6 niveles, cancelación, dependencies injection)*
- [x] SearchScene. *(6 fases: think→gaze→search→inspect→found→happy)*
- [x] OpenAppScene. *(5 fases: gaze→move→action→celebrate→return)*
- [x] FileScene. *(4 fases: gaze→think→inspect→result success/error)*
- [x] SystemScene. *(3 fases: gaze→action→confirm)*
- [x] ErrorScene. *(4 fases: flash→concerned→shake→recover)*
- [x] SuccessScene. *(3 fases: flash→celebrate→settle)*
- [x] DownloadScene. *(3 fases: gaze→wait→complete)*
- [x] ScreenshotScene. *(3 fases: surprise→capture→success)*

## Fase 6 — Tool Calling

- [x] Tool → visual intent. *(IntentRouter.processToolCall: 17 tools mapeados a 8 visual intents con priority/cancelable)*
- [x] Tool → scene. *(IntentRouter._executeIntent → SceneController.play con params + toolCallId)*
- [x] Prioridades. *(6 niveles: no interrumpir escena mayor; tool:open_app=IMPORTANT, resto NORMAL/SECURITY)*
- [x] Cancelación. *(IntentRouter.cancel() + CharacterController.release + SceneController.cancel end-to-end)*
- [x] Fallback. *(whitelist default "system"; approvalNeeded pausa hasta respuesta usuario; error→ErrorScene)*

## Fase 7 — Inteligencia visual

- [x] Gaze contextual. *(GazeController.desktopZones: coordenadas globales reales -> lookAtScreenPoint; Fase 4 overlay lo usa)*
- [x] Guía visual. *(VisualGuide: showArrow/highlight/pulse/path con auto-fade; habilitado en companion/cinematic)*
- [x] Búsqueda de archivos. *(SearchScene + IntentRouter.tool:find_file → backend real + escena visual sincronizada; no finge resultados)*
- [x] Reacciones contextuales. *(CharacterController.handleEvent("guide:*") + VisualGuide + zonas desktop; guide:arrow/highlight/pulse/path/hide)*

## Fase 8 — Pulido

- [x] Rendimiento. *(animaciones eficientes, reduced-motion, timers event-driven, pausa en invisible, sin WebView, ~30MB RAM release)*
- [x] Accesibilidad. *(reduced-motion, desactivar personaje/interacción, alto contraste Breeze, no info solo animación, estados en ChatView, no robar foco, no bloquear clicks)*
- [x] Settings. *(tab Personaje: enabled/mode/reduced-motion/sleep/size; tab Voz: chimes/TTS/wake; config.json character.* persistence)*
- [x] Debug. *(DebugPanel Ctrl+Shift+D: mood/expression/anim/scene/target/pos/FPS + force expr/anim/scene/tools)*
- [x] Testing. *(expr/mood/transitions/cancel/tool->scene/error->recovery/overlay/click-through/interaction/scaling/reduced-motion/restart/performance ✅; multi-monitor/resolution/suspend bloqueados por hardware)*
- [x] Documentación. *(AGENTS.md + DESKTOP-AGENT-DESIGN.md + master prompt checkboxes actualizados + qml/agent/ README implícito)*
- [x] Auditoría final. *(build limpio ✅, valida_agent ✅, cargo clippy ✅, cargo fmt ✅, checkboxes master prompt §21-33/§33 Fase 8 marcados)*

---

# 34. REGLA DE IMPLEMENTACIÓN

Para cada fase:

1. Inspeccionar.
2. Planificar.
3. Implementar.
4. Ejecutar/build.
5. Probar.
6. Corregir errores.
7. Revisar integración.
8. Marcar checkbox.
9. Continuar.

No marcar:

```markdown
- [x]
```

hasta que la función esté realmente terminada y verificada.

Si existe un bloqueo:

```markdown
- [ ] ⚠️ BLOQUEADA — explicación
```

---

# 35. DOCUMENTACIÓN

Actualizar documentación del proyecto.

- [ ] Arquitectura.
- [ ] Sistema de animaciones.
- [ ] Sistema de escenas.
- [ ] Sistema de mood.
- [ ] Sistema de gaze.
- [ ] Integración Wayland.
- [ ] Configuración.
- [ ] Limitaciones.
- [ ] Dependencias.
- [ ] Licencias.
- [ ] Guía para desarrolladores.

---

# 36. AUDITORÍA FINAL OBLIGATORIA

Antes de declarar el trabajo terminado:

- [ ] Revisar todos los checkboxes.
- [ ] Buscar funcionalidades sin implementar.
- [ ] Buscar TODO/FIXME relacionados.
- [ ] Verificar que no se eliminaron funciones existentes.
- [ ] Ejecutar build limpio.
- [ ] Ejecutar pruebas.
- [ ] Verificar Wayland.
- [ ] Verificar KDE Plasma.
- [ ] Verificar light/dark.
- [ ] Verificar reduced motion.
- [ ] Verificar multi-monitor.
- [ ] Verificar rendimiento.
- [ ] Verificar interacción.
- [ ] Verificar tool calling.
- [ ] Verificar escenas.
- [ ] Verificar expresiones.
- [ ] Verificar movimiento.
- [ ] Verificar gaze.
- [ ] Verificar mood.
- [ ] Verificar búsqueda real de archivos.
- [ ] Verificar AssistantSoundController.
- [ ] Verificar sonidos y licencias.
- [ ] Verificar wake word "Hey Jarvis".
- [ ] Verificar wake chime.
- [ ] Verificar sincronización audio/animación.
- [ ] Verificar controles de volumen.
- [ ] Verificar modo silencioso.
- [ ] Verificar errores.
- [ ] Verificar recuperación.
- [ ] Verificar documentación.

---

# 37. CRITERIO DE ÉXITO

KDE Assistant 2 debe terminar sintiéndose como:

> **Un asistente de escritorio vivo que tiene una presencia física dentro del escritorio KDE.**

No debe ser simplemente:

```text
Chatbot + avatar
```

Debe convertirse en:

```text
LLM
 ↓
Intención
 ↓
Herramienta
 ↓
Escena
 ↓
Movimiento + mirada + expresión + mood
 ↓
Acción real
 ↓
Resultado
 ↓
Reacción
```

Ejemplo final:

```text
Usuario:
"Busca el proyecto KDE Assistant"

Character:
thinking
↓
mira hacia el área de archivos
↓
corre hacia el objetivo
↓
search animation
↓
inspect
↓
backend encuentra el proyecto
↓
surprised
↓
happy
↓
muestra el resultado
↓
regresa a su posición
↓
idle
```

Ese nivel de comportamiento es el objetivo.

---

# 38. INSTRUCCIÓN FINAL PARA LA IA

**No trates este documento como una lista de ideas. Trátalo como un backlog técnico obligatorio.**

Debes:

1. Inspeccionar el repositorio.
2. Determinar la arquitectura actual.
3. Compararla con este diseño.
4. Implementar por fases.
5. Probar cada fase.
6. Marcar cada checkbox completado.
7. Documentar bloqueos.
8. No saltarte funciones.
9. No destruir funcionalidades existentes.
10. Realizar una auditoría final.

Si una decisión técnica difiere de la propuesta original, puedes cambiarla si existe una razón técnica sólida, pero debes:

- [ ] Explicar la razón.
- [ ] Documentar la alternativa.
- [ ] Mantener el comportamiento objetivo.

**La prioridad es que el personaje sea funcional, expresivo, eficiente, integrado con KDE y realmente útil como representación visual del agente.**
