---
name: kde-integration
description: Integración con KDE Plasma: DBus, system tray, global hotkeys, KWin blur, y automatización de escritorio
metadata:
  audience: developers
  platform: kde-plasma
  module: src/backend/kde_integration.rs
---

## Qué hago

Proporciono guías y patrones para integrar KDE Assistant v2 con KDE Plasma 6: DBus, system tray, global hotkeys, KWin blur, y automatización de escritorio via MCPs.

## Cuándo usarme

Usa esta skill cuando:
- Modifiques la integración con el system tray
- Agregues o cambies global hotkeys
- Trabajes con DBus para comunicarse con KDE
- Implementes KWin blur o transparencia
- Uses kwin-mcp para automatizar el escritorio
- Depures problemas de integración con el escritorio

## Integración KDE Plasma

### System Tray (QSystemTrayIcon)
```rust
// En kde_integration.rs
use system_tray::SystemTray;

// El tray debe tener un Menu registrado
// Sin menu, Plasma 6 puede descartar el click derecho
let menu = Menu::new();
menu.add_item(MenuItem::new("Abrir"));
menu.add_item(MenuItem::new("Salir"));

let tray = SystemTray::new()
    .with_icon("kde-assistant")
    .with_menu(menu);

// IMPORTANTE: visible: false en el Menu QML
// Sin esto, Plasma muestra un popup en 0,0
```

### DBus
```rust
// Servicio DBus del asistente
// Nombre: org.kde.assistant
// Ruta: /

// Ejemplo de method call
use dbus::blocking::Connection;

let conn = Connection::new_session()?;
let proxy = proxy(
    &conn,
    "org.kde.assistant",
    "/",
    "org.kde.assistant",
);

// Para broadcasting
proxy.signal_method("MessageReceived", &[])?
```

### Global Hotkeys (rdev)
```rust
// En hotkey_listener.rs
// Usar rdev para escuchar atajos globales
// IMPORTANTE: En Wayland, rdev puede necesitar permisos especiales

use rdev::{listen, Event, EventType};

listen(move |event| {
    match event.event_type {
        EventType::KeyPress(Key::F5) => {
            // Toggle visibility
        }
        EventType::KeyPress(Key::F6) => {
            // Push-to-talk
        }
        _ => {}
    }
}).unwrap();
```

### KWin Blur
```qml
// En QML para transparencia
Window {
    flags: Qt.FramelessWindowHint
    color: "transparent"

    // KWin blur property
    property bool blurEnabled: true

    // Para KDE Plasma 6
    KWin.SceneBlur {
        enabled: true
        region: Qt.rect(0, 0, width, height)
    }
}

// O via DBus a KWin
// org.kde.KWin /Scripting eval
```

### Theme Detection
```rust
// Detectar tema de KDE (dark/light)
// Via DBus a Plasma
fn detect_theme() -> String {
    // org.kde.KWin /KWin get "Theme"
    // O usar KDE_PLASHA_THEME environment
    // O consultar config de Plasma
}
```

## Automatización de Escritorio (kwin-mcp)

### Instalación
```bash
pipx install kwin-mcp-server
```

### Herramientas Principales
```bash
# Verificar dependencias
kwin-mcp --check

# Diagnosticar estado
kwin-mcp --doctor

# Ejecutar servidor MCP (stdio)
kwin-mcp

# Ejecutar servidor HTTP
kwin-mcp --http 7575
```

### Uso desde Agentes
```
# Conectar a sesión existente
kwin-mcp session_connect

# Capturar screenshot
kwin-mcp screenshot

# Obtener árbol de accesibilidad
kwin-mcp accessibility_tree

# Hacer click
kwin-mcp mouse_click x=100 y=200

# Escribir texto
kwin-mcp keyboard_type text="hello"
```

### Casos de Uso para KDE Assistant
1. **Testing de UI**: Capturar screenshots y verificar layout
2. **Testing de voice**: Simular interacciones de voz
3. **Testing de tray**: Verificar que el ícono aparece correctamente
4. **Testing de hotkeys**: Verificar que los atajos funcionan

## Permisos en Wayland

### /dev/uinput
```bash
# Para input injection (kwin-mcp)
sudo usermod -aG input $USER
# O configurar udev rule
echo 'KERNEL=="uinput", MODE="0660", GROUP="input"' | \
  sudo tee /etc/udev/rules.d/99-uinput.rules
```

### DBus
```bash
# Verificar conexión
busctl --user list | grep kde
busctl --user call :1.X /org/kde/StatusNotifierItem org.kde.StatusNotifierItem Menu
```

## Troubleshooting

### System Tray no aparece
- Verificar que `menu: Menu {...}` está registrado
- Verificar que `visible: false` está en el Menu
- Verificar con `busctl` que el menú se expone via DBusMenu

### Hotkeys no funcionan
- Verificar que rdev tiene permisos en Wayland
- Probar con `evtest` si los eventos llegan
- Verificar que KGlobalAccel no está usando el mismo atajo

### KWin Blur no funciona
- Verificar que el compositor KWin está activo
- Verificar flags de la ventana (Qt.FramelessWindowHint)
- En Wayland, verificar soporte de blur en el compositor
