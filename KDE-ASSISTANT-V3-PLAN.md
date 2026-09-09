# KDE ASSISTANT v3 - Plan UI/UX + Habilitacion total

> Jornada de pulido: habilitar todas las funciones, terminar interfaces y
> optimizar. Precondicion: API key configurada (Groq) para probar respuestas.
> Estado de partida verificado: chat por texto funciona, tool calling con Groq
> funciona, STT/TTS/wake-word ML funcionan, persistencia funciona.

## Principios de esta fase

1. **Una sola casilla por trabajo** — nada de controles duplicados.
2. **Cero scroll horizontal** — todo envuelve o recorta; `ScrollBar.horizontal.policy: AlwaysOff` en todos los `ScrollView`.
3. **Regla voz/texto** — voz pregunta → voz responde (+ texto en chat); texto pregunta → solo texto.
4. **Sin `unwrap()` nuevo en produccion**; `cargo clippy` limpio; `cargo fmt` antes de cada commit.

---

## Problema 1 — Selector de modelos con doble casilla

**Sintoma:** campo de texto del modelo + ComboBox debajo hacen el mismo trabajo.

**Solucion:** un unico `ComboBox` **editable** (`qml/components/SettingsDialog.qml`):
- Muestra el modelo actual, permite escribir cualquiera a mano y desplegar la lista de la API.
- Quitar el `SettingsField` de modelo; conservar el boton refrescar (sync-16) y el hint de conteo/error.
- `editText` se fija de forma imperativa en `loadConfig()` y se lee en `saveConfig()` (igual que hoy, sin bindings).
- Ojo (bug conocido): NO personalizar `contentItem` — el estilo Breeze espera un TextInput con `positionToRectangle()` (ya nos rompio una vez).

**Criterio:** un solo control visible; elegir de la lista o escribir a mano; persiste tras Guardar.

## Problema 2 — Input con scroll interno

**Sintoma:** al escribir varias lineas aparece scrollbar y se oculta la linea de arriba.

**Solucion** (`qml/components/InputBar.qml` + altura en `Main.qml`):
- Altura del area de texto = `min(input.contentHeight, 4 lineas)`; crece con cada salto de linea.
- `ScrollBar.vertical.policy`: `AlwaysOff` hasta el tope, `AsNeeded` solo a partir de la 5ª linea.
- Quitar el `onTextChanged` muerto; el contenedor de `Main.qml` (`Layout.preferredHeight: 64` fijo hoy) debe seguir a `inputBar.implicitHeight`.
- Verificar que Enter envia / Shift+Enter salta (ya implementado, no romper).

**Criterio:** 1-4 lineas sin scroll visible; la burbuja crece; a partir de 5 lineas scroll interno.

## Problema 3a — Scrolls horizontales

**Regla:** `ScrollBar.horizontal.policy: ScrollBar.AlwaysOff` en ChatView, SessionDrawer, SettingsDialog, ImagePreviewDialog.
**Casos detectados en el barrido:**
- Bloques de codigo del markdown: hoy se emiten como `<pre>` (no envuelve). Cambiar el conversor (`markdownToHtml` en `Main.qml`) a bloque monoespaciado con `<br>` para que envuelva.
- `ImageCard`: `maxWidth` ya existe; verificar imagen panoramica (ancha) no empuja el layout — `clip: true` + limite de ancho.
- Tablas o URLs largas en respuestas: `wrapMode: Text.WrapAnywhere` donde aplique.

**Criterio:** con ventana a 360px de ancho no aparece ningun scroll horizontal en ningun dialogo.

## Problema 3b — Flujo de voz completo (wake word con app minimizada)

**Comportamiento objetivo:**
1. Digo "hey jarvis" (incluso minimizada) → la app **se abre** y el asistente dice **"Sí, dígame"** (saludo configurable).
2. Entra en escucha y graba todo lo que digo (orbe + burbuja flotante visibles).
3. Al terminar responde **en el chat con texto Y dictado**; si pregunte por texto, **solo texto**.
4. Cada respuesta del asistente lleva botones **copiar** y **reproducir**.

**Implementacion:**

- **Abrir al invocar** (`Main.qml`, polling ya existente): al transicionar `voiceState` a `listening`, ademas `root.show()` + `root.raise()`.
- **Saludo antes de grabar** (`src/main.rs` + `voice_pipeline.rs`): tras `HotwordEvent::Detected`, sintetizar el saludo (`speech.wake_greeting`, default `"Sí, dígame"`, nuevo campo en `SpeechConfig`), **esperarlo** y recien ahi `start_listening()`. Asi el micro no se traga nuestra propia voz.
- **Regla voz/texto**:
  - `process_utterance()` (origen voz) habla **siempre** la respuesta.
  - `POST /api/chat/complete` (origen texto) nunca habla, salvo `auto_speak=true` (el toggle pasa a significar "hablar tambien las respuestas escritas"; actualizar su descripcion).
  - `POST /api/speak {text}` (nuevo, fire-and-forget) para el boton reproducir.
- **Botones por respuesta** (`MessageBubble.qml`, solo rol asistente, fila junto al timestamp):
  - **Copiar** (`copy-16.svg`, ya en assets): buffer `TextEdit` oculto en `Main.qml` → `text = raw; selectAll(); copy()`. Requiere guardar el texto **plano** (`plainContent`) ademas del HTML (el HTML con tags no sirve para TTS ni portapapeles).
  - **Reproducir** (`play-16.svg`): `stripMarkdownForSpeech()` en QML (quita ```, `, **, *, #, >, -, links) + `POST /api/speak`.
  - Plumbing de signals como `imageClicked`: `MessageBubble.copyRequested/playRequested` → `ChatView` → `Main`.
- **La respuesta de voz debe caer en el chat** (hoy `stop_and_process` solo hace `log::info`):
  - Nuevo `GET /api/voice/last` → `{transcript, response, timestamp}` del ultimo ciclo.
  - Nuevo `POST /api/voice/log {transcript, response, session_id?}` → persiste ambos mensajes (crea sesion "Conversacion por voz" si no hay) y devuelve `session_id`.
  - `Main.qml`: al detectar transicion a `idle` con stamp nuevo, GET last → append al chat + POST log → actualizar `currentSessionId` si cambio + `loadSessions()`.

**Criterios:** con la ventana minimizada, decir la wake word abre la app, suena el saludo, graba, y al terminar aparece mi frase + su respuesta en el chat y la escucho; por texto no suena nada (salvo toggle).

## Problema 4 — Esquinas exteriores cuadradas

**Sintoma:** el fondo redondeado (`bgRect radiusLg+4`) choca con la decoracion Breeze.

**Solucion** (`Main.qml`): `bgRect.radius = 0` (ventana cuadrada por fuera). No tocar nada mas: burbujas, cards, InputBar y pills conservan sus radios.

**Criterio:** con decoracion Breeze, los bordes de la ventana asientan sin halo; el interior sigue igual.

## Problema 5 — Imagenes de la IA directo en el chat

**Causa raiz (bug hunt):** `show_image` SI descarga a cache, pero el `image_url` se pierde por el camino: `StreamEvent::ToolResult` solo lleva `{tool_call_id, content}`, asi que el QML jamas recibe la imagen y al usuario solo le queda abrir la carpeta.

**Solucion (backend):**
- `StreamEvent::ToolResult` suma `image_url: Option<String>`; `run_agent` lo rellena desde `ToolResult.image_url`; el SSE lo incluye.
- `Message::Tool` suma `image_url: Option<String>` (serde default → filas viejas siguen leyendo).
- `session_manager`: persistirla en la columna `image_url` (ya existe) y restaurarla en `get_messages`.
- `MessageInfo` (endpoint `/api/messages`) la expone.
**Solucion (QML):**
- Streaming: mapear `imageUrl` del evento a la entrada del `toolArr` (por `id`).
- Historial (`loadMessages`): adjuntar cada tool con imagen al `toolCalls` del assistant anterior (`{name:"show_image", status:"success", result, imageUrl}`).
- `MessageBubble` ya renderiza `ImageCard` desde `toolCalls[].imageUrl` — sin cambios.

**Criterio:** pedir "muestrame una imagen del espacio" la incrusta en el chat; recargar la sesion la conserva.

---

## Bugs encontrados en el barrido (backlog de esta fase)

### Backend
- **B1 (critico, causa del P5):** `image_url` se pierde en `ToolResult → SSE → SQLite`. Fix arriba.
- **B2:** `process_utterance` ante transcript vacio retorna `("","")` en silencio → la UI no muestra nada. Fix: que el ciclo de voz emita un aviso ("No te escuche, repitelo") en vez de nada.
- **B3:** `Mutex::lock().unwrap()` en hot paths de audio/voz (`hotword.rs`, `voice_pipeline.rs`, `stt.rs`). Riesgo bajo (solo panics envenenan), pero pasar a `lock().unwrap_or_else(|e| e.into_inner())` donde sea barato.
- **B4:** PTT y wake word pueden pisarse el buffer (`start_recording` hace `clear`). Aceptable; documentar que el ultimo en empezar gana.
- **B5:** `GET /api/sessions` hace N+1 queries (`count_messages` por sesion). Con <100 sesiones es irrelevante; dejar constancia, no tocar.

### Visual
- **V1 (hecho):** `x-16`/`alert-16` faltantes, `Octicon` con nombre vacio, `pulseAnim.opacity`, binding loop de `ImagePreviewDialog`. Ya commiteados.
- **V2:** `SessionDrawer` esperaba `{updatedAt, count}` y el backend manda `{updated_at, message_count}` — hoy solo se lee `title`/`id`, asi que no rompe, pero unificar nombres al tocar el drawer (delete ya agregado).
- **V3:** `Qt.platform.environment["HOME"]` puede ser null en algunos entornos (ya hay fallback a `/root`, que es incorrecto si el usuario no es root). Mejora: resolver el home real; si falla, desactivar el polling con aviso en consola en vez de leer una ruta ajena.
- **V4 (cosmetico, no tocar):** warning Wayland `Failed to create grabbing popup` del menu del tray — bug conocido de Qt, el menu funciona igual.
- **V5:** `FloatingOrb` usa `Screen.desktopAvailableWidth` — verificado que carga sin errores en offscreen, pero confirmar en la sesion real que aparece abajo-derecha.

### Hallazgos de la captura 08/09/26 (tu screenshot, 3 errores marcados)

**HC1 — "Tu" y burbuja de usuario pegados al borde derecho / scrollbar (mala colocación):**
- La columna del chat usa `width: scroll.width` con `rightPadding: spacingMd (17px)`, pero el `ScrollBar.vertical` es overlay y se superpone. El label "Tu" y su punto quedan contra el borde y el timestamp `21:45` se corta contra la barra (flecha superior).
- Fix: `ChatView.qml` → `rightPadding: Theme.spacingMd + 8` + `ScrollBar.vertical.policy: AsNeeded` dejando 4px de gutter, o envolver `MessageBubble` con `Layout.rightMargin` que considere `scroll.ScrollBar.vertical.width`.

**HC2 — Botones de micro/enviar dentro de la cápsula desalineados (mala colocación):**
- En la captura la cápsula muestra el micrófono y el avión en el borde inferior, no centrados con el texto placeholder. Venía del `RowLayout` anclado `bottom` con botones `AlignBottom` (parche anterior lo dejó así).
- Fix ya aplicado en `InputBar.qml` (RowLayout con `Layout.alignment: AlignVCenter` y `bar.implicitHeight = inputBox.implicitHeight + 24`) pero el contenedor en `Main.qml` seguía con `Layout.preferredHeight: 64` fijo que recortaba. Fix: `Main.qml` → `InputBar { Layout.preferredHeight: implicitHeight }` y `inputBox` centrado verticalmente; `hintText` con `anchors.top: bar.bottom + 6` y `elide`.

**HC3 — Espacio horizontal desperdiciado por burbujas estrechas:**
- El rectángulo verde marca el vacío a la derecha del asistente. `MessageBubble.maxWidth = 480` fijo deja ~40% vacío en ventana ancha.
- Fix: `MessageBubble.maxWidth: Math.min(600, parent.width * 0.78)` y `ChatView` centrado con `maxContentWidth: 720` (contenedor interior `width: Math.min(parent.width, Theme.maxContentWidth)` centrado). Así la burbuja usa hasta 78% del ancho disponible.

**Confirmación visual post-fix:** captura Xvfb tras el parche de InputBar muestra el placeholder centrado y el hint visible (commit `793797e`). Los 3 hallazgos de esta captura se corrigen juntos en el siguiente bloque.

### Fuera de alcance (siguientes jornadas)
- Barge-in (interrumpir TTS al hablar).
- `ort` con `download-binaries` (seguimos con `load-dynamic` + auto-descarga, funciona).
- Whisper `tiny` como opcion y release como modo de uso (ya recomendado en README).

---

## Orden de ejecucion propuesto

1. P5 imagenes (backend: evento+DB+endpoint; QML: streaming+historial) + B1.
2. P3b flujo de voz (saludo, abrir ventana, endpoints voice/last+log, botones copiar/reproducir, regla voz/texto) + B2.
3. P1 combo unico + P2 input auto-creciente + P4 esquinas + V2/V3/V5.
4. P3a auditoria horizontal (incluye fix `<pre>`).
5. Verificacion completa: build 0 warnings, 27+ tests, `valida_qml`, prueba viva con Groq (texto, voz, imagen, error de key).
6. Commit por bloque.

## Que necesito de ti

- La **API key** (Groq) puesta para probar respuestas, tools e imagenes de verdad.
- Probar cada bloque en tu sesion real (sobre todo burbuja flotante posicionada y menu del tray en Wayland) y reportar lo que veas.
