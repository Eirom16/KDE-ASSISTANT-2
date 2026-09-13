---
name: qml-frontend
description: Convenciones y patrones para desarrollo QML/Qt6 en el frontend de KDE Assistant v2
metadata:
  audience: developers
  stack: qml
  module: qml/
---

## Qué hago

Proporciono convenciones, patrones y reglas específicas para escribir código QML/Qt6 en el frontend de KDE Assistant v2. Incluye componentes, diseño, y integración con el backend Rust.

## Cuándo usarme

Usa esta skill cuando:
- Erites o modifiques archivos `.qml`
- Crees nuevos componentes en `qml/components/`
- Modifiques `Main.qml` o `Theme.qml`
- Agregues iconos Octicons
- Trabajes con el sistema de diseño Apple

## Estructura QML

```
qml/
├── Main.qml              # Ventana principal (~1400+ líneas)
├── Theme.qml             # Tokens de diseño Apple (163 líneas)
├── Octicon.qml           # Componente de iconos SVG (76 líneas)
├── qmldir                # Definición del módulo
├── auth/
│   ├── AuthToken.qml     # Token singleton fallback
│   └── qmldir
└── components/           # 22 componentes reutilizables
    ├── AppleButton.qml
    ├── ChatView.qml
    ├── InputBar.qml
    ├── MessageBubble.qml
    ├── ToolCallBadge.qml
    ├── VoiceOrb.qml
    └── ... (ver qmldir para lista completa)
```

## Convenciones QML del Proyecto

### Indentación
- **4 espacios** (nunca tabs)

### Naming
- Tipos: `PascalCase` (`MessageBubble`, `InputBar`)
- Propiedades: `camelCase` (`isStreaming`, `toolCallId`)
- Signals: prefijo `on` (`onMessageReceived`, `onTokenReceived`)
- Functions: `camelCase` (`formatTimestamp()`)
- IDs: `camelCase` corto (`root`, `container`, `label`)

### Propiedades
```qml
// SIEMPRE con tipo explícito
property bool isActive: false
property string messageText: ""
property int messageCount: 0
property var toolCall: ({})

// NUNCA sin tipo
property isActive: false  // INCORRECTO
```

### Componentes
```qml
// Un componente = una responsabilidad
// Reutilizables via importación
import "../components" as Components

Components.AppleButton {
    text: "Enviar"
    onClicked: console.log("clicked")
}
```

### Tema (Theme.qml)
```qml
// SIEMPRE usar tokens de Theme
Rectangle {
    color: Theme.surface          // CORRECTO
    color: "#2a2a2c"              // INCORRECTO - hardcoded
}

Text {
    color: Theme.ink
    font.family: Theme.fontPrimary
    font.pixelSize: Theme.bodySize
}

// Tokens disponibles:
// Theme.primary, Theme.surface, Theme.ink, Theme.inkMuted
// Theme.canvas, Theme.hairline, Theme.error, Theme.success
// Theme.radiusLg, Theme.radiusMd, Theme.radiusSm, Theme.radiusPill
// Theme.bodySize, Theme.captionSize, Theme.heroSize
// Theme.spacingMd, Theme.spacingLg, Theme.spacingXl
```

### Iconos (Octicon.qml)
```qml
// SIEMPRE usar Octicons, nunca Breeze o SF Symbols
import "../assets/octicons/" as Octicons

Octicon.Octicon {
    icon: "rocket-16"  // Nombre del SVG sin extensión
    size: 16
    color: Theme.primary
}

// Iconos disponibles en assets/octicons/
// Buscar con: ls assets/octicons/*.svg | wc -l
```

### Señales y Conexiones
```qml
// Definir señales con nombre descriptivo
signal messageSent(string text)
signal sessionChanged(string sessionId)

// Conectar usando Connections
Connections {
    target: backend
    function onMessageReceived(msg) {
        // manejar mensaje
    }
}
```

### Propiedades Owner
```qml
// Definir quién posee cada propiedad
property QtObject backend: null  // Dueño explícito
property var parentRef: ({})     // Referencia al padre
```

### Estilos de Fuentes
```qml
// Usar Theme para consistencia
Text {
    font.family: Theme.fontPrimary
    font.pixelSize: Theme.bodySize
    font.weight: Font.Normal
    color: Theme.ink
    lineHeight: Theme.bodyLineHeight
    // Letter spacing negativo para sensación premium
}
```

### Transiciones y Animaciones
```qml
// Suaves, no mecánicas
Behavior on opacity {
    NumberAnimation { duration: 200; easing.type: Easing.OutCubic }
}

// Para layouts
Behavior on height {
    NumberAnimation { duration: 300; easing.type: Easing.InOutQuad }
}
```

### Anti-Patterns a Evitar
```qml
// NO hardcodear colores
color: "#ffffff"          // INCORRECTO
color: Theme.canvas       // CORRECTO

// NO hardcodear tamaños
font.pixelSize: 14        // INCORRECTO
font.pixelSize: Theme.captionSize  // CORRECTO

// NO usar IDs externos
// En componentes, nunca referenciar IDs de otros componentes

// NO crear dependencias circulares
// Un componente no puede importar a su padre
```

### Integración con Rust
```qml
// El backend se expone via Q_INVOKABLE
// Acceder a propiedades del backend
Connections {
    target: backend
    function onNewMessage(message) { /* ... */ }
}

// Llamar métodos del backend
Backend.sendMessage(text)
```

### Después de Cada Cambio
1. Verificar que no hay errores en consola QML
2. Probar en dark y light mode
3. Verificar que los iconos se cargan correctamente
4. Probar con la ventana redimensionada
