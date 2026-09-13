---
name: design-system
description: Sistema de diseño Apple (Cupertino) adaptado para KDE Assistant: tokens de color, tipografía, espaciado, radios, y componentes
metadata:
  audience: developers
  design: apple
  module: qml/Theme.qml
---

## Qué hago

Proporciono tokens de diseño, patrones de UI, y guías de implementación para el sistema de diseño Apple (Cupertino) adaptado a KDE Assistant v2.

## Cuándo usarme

Usa esta skill cuando:
- Modifiques `Theme.qml` o tokens de diseño
- Crees o modifiques componentes QML
- Trabajes con colores, tipografía o espaciado
- Implementes modos dark/light
- Diseñes nuevas vistas o componentes
- Verifices consistencia visual

## Tokens de Color

### Paleta Principal
| Token | Dark | Light | Uso |
|-------|------|-------|-----|
| `primary` | `#2997ff` | `#0066cc` | Botones, links, acentos |
| `canvas` | `#1d1d1f` | `#f5f5f7` | Fondo ventana |
| `surface` | `#2a2a2c` | `#ffffff` | Cards, burbujas asistente |
| `ink` | `#ffffff` | `#1d1d1f` | Texto principal |
| `inkMuted` | `#7a7a7a` | `#6d6f72` | Texto secundario |
| `hairline` | `#3a3a3c` | `#e0e0e0` | Bordes 1px |

### Estados
| Token | Dark | Light | Uso |
|-------|------|-------|-----|
| `error` | `#ff453a` | `#ff3b30` | Errores |
| `success` | `#30d158` | `#34c759` | Éxito |
| `warn` | `#ff9f0a` | `#ff9500` | Advertencias |

### Gradientes (VoiceOrb)
```qml
// Gradiente del orbe de voz
Gradient {
    GradientStop { position: 0.0; color: "#2997ff" }  // Action Blue
    GradientStop { position: 0.5; color: "#5e5ce6" }  // Indigo
    GradientStop { position: 1.0; color: "#bf5af2" }  // Violet
}
```

## Tokens de Tipografía

### Fuentes
```qml
// Principal (SF Pro Display en macOS)
property string fontPrimary: "Inter, Noto Sans, system-ui, sans-serif"

// Secundaria (SF Pro Text en macOS)
property string fontSecondary: "Inter, Noto Sans, system-ui, sans-serif"
```

### Tamaños
| Token | Size | Line Height | Letter Spacing | Uso |
|-------|------|-------------|----------------|-----|
| `heroSize` | 28px | 1.14 | -0.28px | Welcome screen |
| `taglineSize` | 21px | 1.19 | 0.231px | Headers settings |
| `bodyStrongSize` | 17px | 1.24 | -0.374px | Burbuja asistente |
| `bodySize` | 15px | 1.47 | -0.224px | Texto general |
| `bodySmallSize` | 13px | 1.43 | -0.12px | Texto secundario |
| `captionSize` | 13px | 1.43 | -0.12px | Hints, labels |
| `buttonLargeSize` | 18px | 1 | 0 | Botones grandes |
| `buttonUtilitySize` | 14px | 1.29 | -0.224px | Botones secundarios |
| `finePrintSize` | 12px | 1 | -0.12px | Footers |
| `microSize` | 11px | 1.3 | -0.08px | Role labels |

### Letter Spacing Negativo
```qml
// La sensación premium viene del tracking negativo
// En headlines: -0.28px a -0.374px
// En body: -0.224px
// En captions: -0.12px
// Nunca positivo excepto en tagline
```

## Tokens de Espaciado

| Token | Valor | Uso |
|-------|-------|-----|
| `spacingXxs` | 2px | Gap mínimo |
| `spacingXs` | 4px | Gap entre elementos cercanos |
| `spacingSm` | 8px | Padding interno cards |
| `spacingMd` | 12px | Gap entre elementos |
| `spacingLg` | 16px | Padding cards |
| `spacingXl` | 24px | Secciones |
| `spacingXxl` | 32px | Margen externo |

## Tokens de Forma (Radios)

| Token | Valor | Uso |
|-------|-------|-----|
| `radiusSm` | 8px | Chips, badges |
| `radiusMd` | 12px | Cards pequeñas |
| `radiusLg` | 18px | Cards, burbujas chat |
| `radiusXl` | 24px | Modales |
| `radiusPill` | 9999px | InputBar, botones primarios |

## Componentes Principales

### AppleButton.qml
```qml
// Botón primario con radio pill
AppleButton {
    text: "Enviar"
    primary: true  // Azul con texto blanco
    // secondary: true  // Gris transparente
}
```

### MessageBubble.qml
```qml
// Burbuja de chat
// - Radio radiusLg (18px)
// - Surface color
// - Padding spacingLg (16px)
// - roleLabel: "Tú" / "KDE Assistant" en microSize
```

### InputBar.qml
```qml
// Barra de entrada
// - Radio radiusPill (9999px)
// - Hairline border 1px
// - Padding spacingMd horizontal
```

### VoiceOrb.qml
```qml
// Orbe de voz luminoso
// - Gradiente azul→indigo→violeta
// - Animación de pulso
// - Glow sutil
// - Tamaño: 64px idle, 96px activo
```

### ToolCallBadge.qml
```qml
// Badge de tool call
// - Chip con radio radiusSm
// - Icono Octicon + texto
// - Color depende de estado
```

## Filosofía de Diseño

1. **Content-first**: Las burbujas de chat y el VoiceOrb son protagonistas
2. **Frosted acrylic**: Ventana translucida con KWin blur
3. **Un solo color interactivo**: Action Blue para todo lo interactivo
4. **Tracking negativo**: Sensación tipográfica premium
5. **Sin sombras en chrome**: Solo en cards/imagenes
6. **Octicons sobre SF Symbols**: Consistencia multiplataforma

## Anti-Patterns

```qml
// NO hardcodear colores
color: "#ffffff"           // INCORRECTO
color: Theme.canvas        // CORRECTO

// NO hardcodear tamaños
font.pixelSize: 14         // INCORRECTO
font.pixelSize: Theme.captionSize  // CORRECTO

// NO hardcodear radios
radius: 18                 // INCORRECTO
radius: Theme.radiusLg     // CORRECTO

// NO usar sombras excesivas
// El chrome no tiene sombras, solo cards
```

## Referencia Completa

Ver `apple-DESIGN.md` para tokens completos, componentes detallados, y guías de implementación.
