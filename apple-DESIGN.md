# Apple — Design System (Adaptado para KDE Assistant v2)

> Sistema de diseño Cupertino adaptado a un asistente de escritorio KDE Plasma con estética edge-to-edge, tipografía con tracking negativo, un solo color interactivo (Action Blue) y chrome translucido frosted. La UI debe permitir que el producto (las burbujas de chat, el orbe de voz y las cards) sea el protagonista.

**Theme:** system (auto-detect dark/light desde KDE Breeze)
**Source website:** [https://www.apple.com/](https://www.apple.com/)
**Aplicación:** KDE Assistant v2 — ventana flotante translucida con QML

## Filosofía para KDE Assistant

1. **Photography-first / Content-first:** Las burbujas de chat, los tool call badges y el VoiceOrb son los protagonistas. El chrome (window frame, drawer) se difumina.
2. **Frosted acrylic:** La ventana usa blur nativo de KWin (X11/Wayland) con `Qt.WA_TranslucentBackground`.
3. **Un solo color interactivo:** Action Blue (`#0066cc` light / `#2997ff` dark). Todo lo interactivo usa este color; el resto son neutros.
4. **Tracking negativo:** Titulares y body con letter-spacing sutilmente negativo para sensación tipográfica premium.
5. **Octicons sobre SF Symbols:** En lugar de SF Symbols (macOS), usamos GitHub Primer Octicons para mantener coherencia multiplataforma.
6. **Sin sombras en chrome, solo en cards/imagenes:** El chrome recede, las cards tienen una sombra sutil de 1-2 niveles.

---

## Tokens — Colors

| Name | Value | Token | Role |
|---|---|---|---|
| primary | `#0066cc` | `--color-primary` | Botón primario, link, foco |
| primary focus | `#0071e3` | `--color-primary-focus` | Estado focus/hover de primary |
| primary on dark | `#2997ff` | `--color-primary-on-dark` | Primary en temas oscuros |
| ink | `#1d1d1f` | `--color-ink` | Texto principal (light) |
| body | `#1d1d1f` | `--color-body` | Texto de cuerpo (light) |
| body on dark | `#ffffff` | `--color-body-on-dark` | Texto en temas oscuros |
| body muted | `#cccccc` | `--color-body-muted` | Texto secundario (dark) |
| ink muted 80 | `#333333` | `--color-ink-muted-80` | Texto muted 80 (light) |
| ink muted 48 | `#7a7a7a` | `--color-ink-muted-48` | Texto muted 48 (dark) |
| divider soft | `#f0f0f0` | `--color-divider-soft` | Divisores suaves (light) |
| hairline | `#e0e0e0` | `--color-hairline` | Hairline 1px (light) |
| hairline dark | `#3a3a3c` | `--color-hairline-dark` | Hairline 1px (dark) |
| canvas | `#ffffff` | `--color-canvas` | Fondo (light) |
| canvas parchment | `#f5f5f7` | `--color-canvas-parchment` | Fondo principal ventana light |
| surface pearl | `#fafafc` | `--color-surface-pearl` | Cards sutiles (light) |
| surface tile 1 | `#272729` | `--color-surface-tile-1` | Tile oscuro 1 |
| surface tile 2 | `#2a2a2c` | `--color-surface-tile-2` | Burbuja asistente (dark) |
| surface tile 3 | `#252527` | `--color-surface-tile-3` | Tile oscuro 3 |
| surface black | `#000000` | `--color-surface-black` | Global nav (referencia) |
| surface chip translucent | `#d2d2d7` | `--color-surface-chip-translucent` | Botón circular mic (light) |
| surface chip dark | `#3a3a3c` | `--color-surface-chip-dark` | Botón circular mic (dark) |
| on primary | `#ffffff` | `--color-on-primary` | Texto sobre primary |
| on dark | `#ffffff` | `--color-on-dark` | Texto sobre dark |
| error | `#ff3b30` | `--color-error` | Light error |
| error dark | `#ff453a` | `--color-error-dark` | Dark error |
| success | `#34c759` | `--color-success` | Light success |
| success dark | `#30d158` | `--color-success-dark` | Dark success |
| warn | `#ff9500` | `--color-warn` | Light warning |
| warn dark | `#ff9f0a` | `--color-warn-dark` | Dark warning |
| voice orb grad start | `#2997ff` | `--color-orb-start` | Gradient Action Blue |
| voice orb grad mid | `#5e5ce6` | `--color-orb-mid` | Gradient Indigo |
| voice orb grad end | `#bf5af2` | `--color-orb-end` | Gradient Violet |

---

## Tokens — Typography

### SF Pro Display, system-ui, -apple-system, sans-serif · `--font-primary`
- **Sustituto en Linux:** Inter, Noto Sans, system-ui, sans-serif
- **Weights:** 600 (DemiBold), 400 (Normal)
- **Sizes para KDE Assistant:** 28px (hero), 21px (tagline), 17px (body strong)
- **Line height:** 1.14, 1.19, 1.24
- **Letter spacing:** -0.28px, 0.231px, -0.374px

### SF Pro Text, system-ui, -apple-system, sans-serif · `--font-family-2`
- **Sustituto en Linux:** Inter, Noto Sans, system-ui, sans-serif
- **Weights:** 600, 400, 300
- **Sizes para KDE Assistant:** 17px (body), 15px (body small), 14px (caption), 13px (caption small), 12px (fine print), 11px (micro), 18px (button large)
- **Line height:** 1.47, 1.43, 1.29, 1
- **Letter spacing:** -0.374px, -0.224px, -0.12px, -0.08px

### Type Scale (Adaptado para KDE Assistant v2)

| Role | Size | Line Height | Letter Spacing | Token | Uso |
|---|---|---|---|---|---|
| hero | 28px | 1.14 | -0.28px | `--text-hero` | Welcome screen title |
| tagline | 21px | 1.19 | 0.231px | `--text-tagline` | SettingsDialog headers |
| body-strong | 17px | 1.24 | -0.374px | `--text-body-strong` | Burbuja asistente |
| body | 15px | 1.47 | -0.224px | `--text-body` | Texto general, InputBar |
| body-small | 13px | 1.43 | -0.12px | `--text-body-small` | Texto secundario |
| caption | 13px | 1.43 | -0.12px | `--text-caption` | Hints, labels |
| button-large | 18px | 1 | 0 | `--text-button-large` | Botones primarios grandes |
| button-utility | 14px | 1.29 | -0.224px | `--text-button-utility` | Botones secundarios |
| fine-print | 12px | 1 | -0.12px | `--text-fine-print` | Footers, legal |
| micro | 11px | 1.3 | -0.08px | `--text-micro` | Role labels (Tu / KDE Assistant) |
| nav-link | 12px | 1 | -0.12px | `--text-nav-link` | SessionDrawer items |

---

## Tokens — Spacing & Shapes

**Density:** comfortable

### Spacing Scale

| Name | Value | Token | Uso |
|---|---|---|---|
| xxs | 4px | `--spacing-xxs` | Gaps internos icon-button |
| xs | 8px | `--spacing-xs` | Padding input-icon |
| sm | 12px | `--spacing-sm` | Padding burbuja usuario |
| md | 17px | `--spacing-md` | Element gap |
| lg | 24px | `--spacing-lg` | Card padding |
| xl | 32px | `--spacing-xl` | Section gap |
| xxl | 48px | `--spacing-xxl` | Hero padding |
| section | 80px | `--spacing-section` | WelcomeScreen vertical |

### Border Radius

| Name | Value | Token | Uso |
|---|---|---|---|
| none | 0px | `--radius-none` | Hairlines, divs |
| xs | 5px | `--radius-xs` | Tags pequeños |
| sm | 8px | `--radius-sm` | Chips, small buttons |
| md | 11px | `--radius-md` | Input fields |
| lg | 18px | `--radius-lg` | **Cards, burbujas, modales (default)** |
| pill | 9999px | `--radius-pill` | **InputBar capsule, botones primarios** |
| full | 9999px | `--radius-full` | **Botón mic circular, VoiceOrb** |

### Layout

- **Section gap:** 80px (WelcomeScreen vertical rhythm)
- **Card padding:** 24px (`--spacing-lg`)
- **Element gap:** 17px (`--spacing-md`)
- **Max content width:** 720px (chat area, centrado en pantallas grandes)
- **Ventana min:** 360x500
- **Ventana max:** 800x900
- **Ventana default:** 420x680

---

## Componentes

### button primary
**Role:** Botón de acción principal (enviar, confirmar)
- **backgroundColor:** `{colors.primary}`
- **textColor:** `{colors.on-primary}`
- **typography:** `{typography.body-strong}`
- **rounded:** `{rounded.pill}`
- **padding:** `11px 22px`
- **press effect:** scale 0.95, 100ms
- **disabled:** opacity 0.4

### button primary focus
- **backgroundColor:** `{colors.primary-focus}`
- **textColor:** `{colors.on-primary}`

### button secondary pill
**Role:** Botón de acción secundaria (cancelar, reintentar)
- **backgroundColor:** `{colors.canvas}` o `{colors.surface-tile-2}` (dark)
- **textColor:** `{colors.primary}`
- **rounded:** `{rounded.pill}`
- **padding:** `11px 22px`

### button dark utility
- **backgroundColor:** `{colors.ink}` (light) / `{colors.surface-tile-1}` (dark)
- **textColor:** `{colors.on-dark}`
- **typography:** `{typography.button-utility}`
- **rounded:** `{rounded.sm}`
- **padding:** `8px 15px`

### button pearl capsule
- **backgroundColor:** `{colors.surface-pearl}`
- **textColor:** `{colors.ink-muted-80}`
- **typography:** `{typography.caption}`
- **rounded:** `{rounded.md}`
- **padding:** `8px 14px`

### button icon circular
**Role:** Botón micrófono, botones de acción en InputBar
- **backgroundColor:** `{colors.surface-chip-translucent}` (light) / `{colors.surface-chip-dark}` (dark)
- **textColor:** `{colors.ink}` (light) / `{colors.on-dark}` (dark)
- **rounded:** `{rounded.full}`
- **size:** `44px x 44px`
- **icon size:** 20px
- **press effect:** scale 0.92, 100ms

### text link
- **backgroundColor:** `transparent`
- **textColor:** `{colors.primary}`
- **typography:** `{typography.body}`

### text link on dark
- **backgroundColor:** `transparent`
- **textColor:** `{colors.primary-on-dark}`
- **typography:** `{typography.body}`

### session drawer item (chip pill)
**Role:** Items de la lista de sesiones
- **backgroundColor:** `transparent` (default) / `{colors.primary}` (selected)
- **textColor:** `{colors.ink}` (default) / `{colors.on-primary}` (selected)
- **typography:** `{typography.nav-link}`
- **rounded:** `{rounded.pill}`
- **padding:** `10px 16px`
- **height:** `36px`

### input bar (floating capsule)
**Role:** Input principal del chat
- **backgroundColor:** `{colors.surface}` con 80% opacity (frosted)
- **textColor:** `{colors.ink}`
- **placeholderColor:** `{colors.ink-muted-48}`
- **typography:** `{typography.body}`
- **rounded:** `{rounded.pill}` (9999px)
- **padding:** `12px 16px 12px 20px`
- **height:** `56px`
- **max height:** `140px` (auto-resize hasta 4 lineas)
- **border:** `1px {colors.hairline}` (sutil)

### message bubble user
- **backgroundColor:** `{colors.primary}`
- **textColor:** `{colors.on-primary}`
- **typography:** `{typography.body}`
- **rounded:** `{rounded.lg}` (18px uniforme)
- **padding:** `12px 16px`
- **max width:** `75%`
- **align:** right
- **shadow:** none

### message bubble assistant
- **backgroundColor:** `{colors.surface}` (light: white, dark: surface-tile-2)
- **textColor:** `{colors.ink}` (light) / `{colors.on-dark}` (dark)
- **typography:** `{typography.body}`
- **rounded:** `{rounded.lg}` (18px uniforme)
- **padding:** `12px 16px`
- **max width:** `75%`
- **align:** left
- **border:** `1px {colors.hairline}` (light) / `1px {colors.hairline-dark}` (dark)
- **shadow:** `0 1px 2px rgba(0,0,0,0.04)` (light) / `0 1px 2px rgba(0,0,0,0.2)` (dark)

### tool call badge (pill)
**Role:** Pill informativa de accion ejecutada (open_app, search, etc.)
- **backgroundColor:** `{colors.surface-pearl}` (light) / `{colors.surface-tile-1}` (dark)
- **textColor:** `{colors.ink-muted-80}` (light) / `{colors.body-muted}` (dark)
- **typography:** `{typography.caption}`
- **rounded:** `{rounded.pill}`
- **padding:** `6px 12px`
- **icon:** 14px Octicon a la izquierda
- **states:**
  - **running:** RotationAnimation en icono, fondo surface
  - **success:** fondo `success` con 15% opacity, icono `check-16`
  - **error:** fondo `error` con 15% opacity, icono `x-16`

### image card
**Role:** Tarjeta de imagen en chat (show_image)
- **backgroundColor:** `{colors.surface}`
- **rounded:** `{rounded.lg}` (18px)
- **border:** `1px {colors.hairline}`
- **shadow:** `0 4px 12px rgba(0,0,0,0.08)` (light) / `0 4px 12px rgba(0,0,0,0.3)` (dark)
- **max width:** `100%` (dentro de bubble)
- **max height:** `320px`
- **caption:** debajo, fontSize 12, inkMuted
- **on click:** abrir en visor nativo (xdg-open) o zoom modal

### voice orb (siri-like)
**Role:** Orbe luminoso cuando el asistente escucha o habla
- **size:** `120px x 120px` (puede crecer con amplitud)
- **rounded:** `{rounded.full}`
- **gradient:** radial de `voice-orb-grad-start` -> `voice-orb-grad-mid` -> `voice-orb-grad-end`
- **shadow:** `0 0 40px rgba(41,151,255,0.6)` (glow azul)
- **states:**
  - **idle:** anillo sutil, radius pequeño, opacity 0.4
  - **listening:** orbe completo, scale reactivo a amplitud del mic
  - **processing:** orbe contraido (60%) con RotationAnimation lenta
  - **speaking:** ondas expansivas concentricas (3-4 circulos con opacity fade)
- **animation:** SpringAnimation con damping 0.4

### typing indicator (3 dots)
- **size:** 8px diameter por dot
- **color:** `{colors.ink-muted-48}`
- **spacing:** 6px entre dots
- **animation:** Scale 1.0 -> 1.2 con timing escalonado (0ms, 150ms, 300ms)
- **cycle:** 400ms

### session drawer
- **width:** 220px (fijo)
- **backgroundColor:** `{colors.canvas}` con 70% opacity (frosted)
- **border-right:** `1px {colors.hairline}` (sutil)
- **padding:** 16px
- **collapsible:** con boton sidebar

### floating action bar (input area)
- **backgroundColor:** `transparent` (flota sobre el chat)
- **padding:** 16px horizontal
- **position:** bottom, fixed

### global nav (system tray menu)
- **backgroundColor:** native (KDE Plasma)
- **textColor:** native
- **height:** 44px (QSystemTrayIcon menu)

### sub nav frosted
- **backgroundColor:** `{colors.canvas-parchment}` con 80% opacity
- **rounded:** `none` (edge-to-edge)

### product tile light (welcome screen)
- **backgroundColor:** `{colors.canvas-parchment}`
- **textColor:** `{colors.ink}`
- **rounded:** `{rounded.lg}` (18px, en lugar de 0 de Apple, para integrarse con QML)
- **padding:** 80px
- **shadow:** `0 1px 3px rgba(0,0,0,0.04)`

### footer (about/legal)
- **backgroundColor:** `{colors.canvas-parchment}` con 50% opacity
- **textColor:** `{colors.ink-muted-80}`
- **typography:** `{typography.fine-print}`
- **padding:** 32px

---

## Do's and Don'ts (para KDE Assistant)

### Do

- Usar `--color-primary` para TODA interaccion (botones, links, burbuja usuario, foco).
- Mantener superficies ancladas a `--color-canvas` o `--color-canvas-parchment` con translucidez.
- Preservar cada estilo tipografico documentado (size, line-height, letter-spacing).
- Usar `--radius-lg` (18px) por defecto para cards/burbujas.
- Usar `--radius-pill` (9999px) para InputBar y botones primarios.
- Usar `--radius-full` para botones circulares (microfono) y VoiceOrb.
- Cargar iconos desde `assets/octicons/*.svg` embebidos en `resources.qrc`.
- Usar el componente `Octicon.qml` para todo icono, nunca iconos sueltos.
- Permitir auto-detect del tema (Breeze Dark/Light) en `SettingsDialog`.
- Aplicar blur nativo KWin a la ventana principal (`Qt.WA_TranslucentBackground` + KWin blur effect via DBus).
- Usar `SpringAnimation` (no `NumberAnimation` rigido) para el VoiceOrb y microinteracciones.
- Validar contra el sitio oficial de Apple (https://www.apple.com/) para nuevas decisiones visuales.

### Don't

- No introducir colores fuera del set documentado de tokens.
- No reemplazar `--color-ink` con un neutral arbitrario.
- No aplanar estados de componentes documentados (pressed, focus, disabled).
- No usar SF Symbols (no son multiplataforma), solo Octicons.
- No usar `NumberAnimation` para VoiceOrb (debe ser spring).
- No tratar este snapshot extraido como mas nuevo que el sitio web oficial vivo.
- No usar bordes decorativos o gradientes en chrome (solo en VoiceOrb y cards destacadas).
- No poner sombras en la ventana principal (solo en cards flotantes como ImageCard y menu del tray).

---

## Iconografía: GitHub Primer Octicons (Reemplazo de SF Symbols)

KDE Assistant v2 usa el set oficial de **GitHub Primer Octicons** (https://github.com/primer/octicons) en lugar de SF Symbols de Apple, porque:
- SF Symbols son propietarios de Apple (solo disponibles en macOS/iOS).
- Octicons son open source (MIT) y multiplataforma.
- Estilo geometrico consistente que se integra bien con el diseno Apple.

**Tamaños usados:**
- 16px (iconos estandar en botones y pills)
- 24px (iconos en cards y headers)
- 64px (iconos hero en WelcomeScreen)
- 20px (iconos en botones circulares de 44px)

**Color:**
- Heredar del contexto (`currentColor` en SVG fill).
- En botones primary: `onPrimary` (blanco).
- En botones secondary: `primary` (azul) o `ink` (negro/blanco segun tema).
- En badges: `inkMuted` por defecto, cambia a success/error segun estado.

---

## Layout

- Usar el spacing scale y geometria documentada como baseline de implementacion.
- Validar composicion responsiva y ritmo de pagina contra el sitio web oficial vivo.
- Tres breakpoints:
  - **Compacto** (< 360px): SessionDrawer oculto
  - **Normal** (360-600px): SessionDrawer colapsado (icono)
  - **Amplio** (> 600px): SessionDrawer visible (220px) + chat max-width 720px centrado
