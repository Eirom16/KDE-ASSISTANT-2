# MCP-GUIDE.md — Guia de MCPs para KDE Assistant v2

> Documentación de los servidores MCP (Model Context Protocol) disponibles en este proyecto.
> Cada modelo AI debe consultar esta guía antes de usar herramientas MCP.

---

## Resumen de MCPs Disponibles

| MCP | Tipo | Lenguaje | Herramientas | Uso principal |
|-----|------|----------|--------------|---------------|
| **kwin-mcp** | local | Python | 30 | Automatización KDE Plasma 6 Wayland |
| **rust-analyzer** | local | Rust | 10 | Análisis semántico código Rust |
| **filesystem** | local | Rust | 15+ | Operaciones filesystem de alto rendimiento |
| **git** | local | Rust | 52 | Gestión completa de repositorio Git |
| **memory** | local | Node.js | 11 | Memoria persistente (knowledge graph) |
| **sqlite** | local | Node.js | 13 | Consultas y gestión SQLite |
| **context7** | remote | - | 2 | Documentación frameworks en tiempo real |

---

## 1. kwin-mcp — Automatización KDE Plasma Wayland

**Cuándo usar:**
- Testing de la UI QML del asistente
- Capturar screenshots del escritorio KDE
- Inspeccionar el árbol de accesibilidad AT-SPI
- Automatizar interacciones (clicks, teclado, ventanas)
- Conectar a sesiones KWin virtuales o reales
- Verificar protocolos Wayland disponibles

**Herramientas principales:**

| Herramienta | Descripción | Cuándo usarla |
|-------------|-------------|---------------|
| `session_start` | Iniciar sesión KWin virtual aislada | Testing aislado sin afectar escritorio |
| `session_connect` | Conectar a sesión KWin existente | Automatizar el escritorio real |
| `screenshot` | Capturar pantalla o ventana | Ver estado visual de la UI |
| `accessibility_tree` | Obtener árbol AT-SPI | Encontrar elementos UI por rol/nombre |
| `find_ui_elements` | Buscar elementos por texto/tipo | Localizar botones, campos, etc. |
| `mouse_click` | Hacer click en coordenadas | Interactuar con elementos UI |
| `keyboard_type` | Escribir texto | Llenar campos de entrada |
| `list_windows` | Listar ventanas abiertas | Ver qué apps están corriendo |
| `focus_window` | Enfocar una ventana | Traer ventana al frente |
| `launch_app` | Lanzar aplicación | Abrir apps para testing |
| `dbus_call` | Llamar a D-Bus | Interactuar con servicios KDE |
| `wayland_info` | Info de protocolos Wayland | Verificar soporte de protocolos |

**Ejemplo de prompt:**
```
Usa kwin-mcp para capturar un screenshot del escritorio y verificar que el ícono del
asistente aparece correctamente en el system tray de KDE.
```

---

## 2. rust-analyzer — Análisis Semántico Rust

**Cuándo usar:**
- Encontrar definiciones de tipos, funciones, traits
- Buscar todas las referencias a un símbolo
- Obtener documentación y tipos de un elemento
- Diagnosticar errores del compilador
- Obtener sugerencias de código (code actions)
- Formatear código con rustfmt
- Validar lifetimes y borrow checker
- Refactorizar código (rename, extract function)

**Herramientas principales:**

| Herramienta | Descripción | Cuándo usarla |
|-------------|-------------|---------------|
| `rust_analyzer_hover` | Infohover (tipos, docs) | Entender qué hace un símbolo |
| `rust_analyzer_definition` | Ir a definición | Navegar al código fuente |
| `rust_analyzer_references` | Encontrar referencias | Ver dónde se usa un símbolo |
| `rust_analyzer_diagnostics` | Errores/warnings del compilador | Verificar si el código compila |
| `rust_analyzer_code_actions` | Quick fixes y refactorizaciones | Mejorar código automáticamente |
| `rust_analyzer_format` | Formatear con rustfmt | Mantener estilo consistente |
| `rust_analyzer_symbols` | Listar símbolos del archivo | Ver estructura de un archivo |
| `rust_analyzer_completion` | Sugerencias de código | Obtener autocompletado |
| `rust_analyzer_workspace_diagnostics` | Diagnósticos del workspace | Ver errores globales |
| `rust_analyzer_set_workspace` | Cambiar workspace root | Proyectos con múltiples crates |

**Ejemplo de prompt:**
```
Usa rust-analyzer para encontrar todas las referencias a la función `chat()` en
src/backend/ai_service.rs y explicarme qué hace cada una.
```

---

## 3. filesystem — Operaciones de Archivo

**Cuándo usar:**
- Leer contenido de archivos del proyecto
- Escribir o modificar archivos
- Buscar archivos por patrón glob
- Explorar la estructura de directorios
- Buscar contenido en archivos

**Herramientas principales:**

| Herramienta | Descripción | Cuándo usarla |
|-------------|-------------|---------------|
| `read_file` | Leer archivo completo | Ver contenido de código |
| `write_file` | Crear/escribir archivo | Crear nuevos archivos |
| `list_directory` | Listar directorio | Explorar estructura |
| `search_files` | Buscar por patrón glob | Encontrar archivos *.rs, *.qml |
| `search_content` | Buscar contenido regex | Encontrar código específico |
| `directory_tree` | Árbol visual de directorios | Ver estructura completa |
| `get_file_info` | Metadata de archivo | Ver tamaño, fechas, tipo |

**Ejemplo de prompt:**
```
Usa filesystem para listar todos los archivos .qml en el directorio qml/components/
y leer el contenido de ToolCallBadge.qml.
```

---

## 4. git — Gestión de Repositorio

**Cuándo usar:**
- Ver estado del working tree (cambios pendientes)
- Ver diff de cambios
- Crear commits con mensajes descriptivos
- Gestionar branches (crear, cambiar, eliminar)
- Ver historial de commits
- Hacer push/pull a remotos
- Stash de cambios temporales
- Ver blame de archivos

**Herramientas principales:**

| Herramienta | Descripción | Cuándo usarla |
|-------------|-------------|---------------|
| `git_status` | Estado del working tree | Ver qué archivos cambiaron |
| `git_diff` | Ver diferencias | Revisar cambios antes de commit |
| `git_diff_staged` | Diff de cambios staged | Ver qué se va a commitear |
| `git_commit` | Crear commit | Guardar cambios |
| `git_add` | Stage archivos | Preparar archivos para commit |
| `git_log` | Historial de commits | Ver recientes commits |
| `git_branch` | Listar/crear branches | Gestionar ramas |
| `git_checkout` | Cambiar branch | Navegar entre ramas |
| `git_push` | Push a remoto | Subir cambios |
| `git_pull` | Pull de remoto | Bajar cambios |
| `git_stash` | Stash cambios | Guardar cambios temporalmente |
| `git_blame` | Ver autor por línea | Entender quién escribió qué |

**Ejemplo de prompt:**
```
Usa git para ver el estado actual del repositorio y los cambios staged que hay.
Si hay cambios sin stage, muéstrame el diff.
```

---

## 5. memory — Memoria Persistente

**Cuándo usar:**
- Recordar decisiones de diseño del proyecto
- Guardar preferencias del usuario
- Mantener contexto entre sesiones de desarrollo
- Documentar patrones arquitectónicos
- Guardar información de debugging
- Registrar bugs conocidos y soluciones

**Herramientas principales:**

| Herramienta | Descripción | Cuándo usarla |
|-------------|-------------|---------------|
| `create_entities` | Crear entidades | Guardar conceptos nuevos |
| `create_relations` | Crear relaciones | Conectar conceptos |
| `add_observations` | Agregar observaciones | Guardar hechos/detalles |
| `read_graph` | Leer grafo completo | Ver todo el conocimiento |
| `search_nodes` | Buscar en el grafo | Encontrar información guardada |
| `open_nodes` | Abrir entidades específicas | Ver detalles de un concepto |
| `delete_entities` | Eliminar entidades | Limpiar información obsoleta |

**Ejemplo de prompt:**
```
Usa memory para guardar que decidimos usar Axum como HTTP server en lugar de
Actix-web, y que la razón fue la mejor integración con Tokio y SSE streaming.
```

---

## 6. sqlite — Base de Datos SQLite

**Cuándo usar:**
- Inspeccionar el esquema de la BD del proyecto
- Consultar datos de sesiones o mensajes
- Diagnosticar problemas de rendimiento
- Ejecutar migraciones de esquema
- Verificar integridad de datos
- Hacer backup de la BD

**Herramientas principales:**

| Herramienta | Descripción | Cuándo usarla |
|-------------|-------------|---------------|
| `execute` | Ejecutar SQL | Consultas SELECT/INSERT/UPDATE/DELETE |
| `backup` | Crear backup | Respaldo antes de cambios grandes |
| `database_info` | Info de la BD | Ver tamaño, versión, pages |
| `search_fts` | Búsqueda full-text | Buscar en contenido de mensajes |
| `create_fts_index` | Crear índice FTS | Optimizar búsquedas de texto |

**Ejemplo de prompt:**
```
Usa sqlite para consultar la estructura de las tablas de la BD kde-assistant.db
y verificar que la tabla sessions tiene los campos correctos según el esquema
definido en KDE-ASSISTANT-V2-PLAN.md.
```

---

## 7. context7 — Documentación de Frameworks

**Cuándo usar:**
- Buscar documentación actualizada de Qt 6 / QML
- Consultar API de Tokio, reqwest, axum
- Ver ejemplos de uso de serde, cpal, rodio
- Documentación de whisper-rs, piper-tts
- Cualquier dependencia del proyecto

**Herramientas principales:**

| Herramienta | Descripción | Cuándo usarla |
|-------------|-------------|---------------|
| `resolve-library-id` | Buscar ID de librería | Encontrar la lib correcta |
| `get-library-docs` | Obtener documentación | Ver docs de una librería |

**Ejemplo de prompt:**
```
Usa context7 para buscar cómo configurar SSE streaming con axum en Rust,
específicamente el patrón correcto para enviar eventos de token atoken.
```

---

## Árbol de Decisión: Qué MCP usar

```
¿Qué necesitas hacer?
│
├─ 🖥️ Automatizar/interactuar con el escritorio KDE?
│  └─ → kwin-mcp
│
├─ 📝 Analizar/navegar código Rust?
│  └─ → rust-analyzer
│
├─ 📁 Leer/escribir/buscar archivos?
│  └─ → filesystem
│
├─ 🔀 Gestionar git (commits, branches, diffs)?
│  └─ → git
│
├─ 🧠 Guardar/recuperar información entre sesiones?
│  └─ → memory
│
├─ 🗄️ Consultar/modificar la base de datos SQLite?
│  └─ → sqlite
│
├─ 📚 Buscar documentación de un framework/librería?
│  └─ → context7
│
└─ 🤔 No sé qué herramienta usar
   └─ → Consultar esta guía o preguntar al usuario
```

---

## Reglas de Uso

### Prioridad
1. **Siempre preferir herramientas MCP** sobre comandos bash cuando existan
2. **No duplicar funcionalidad**: si filesystem puede leer un archivo, no usar `cat` via bash
3. **Usar memory** para guardar decisiones importantes que deben persistir

### Conflictos
- **filesystem vs git**: Para ver archivos usa `filesystem`, para ver cambios usa `git diff`
- **rust-analyzer vs cargo clippy**: Para errores usa `rust-analyzer diagnostics`, para linters usa bash con `cargo clippy`
- **sqlite vs código Rust**: Para inspeccionar la BD usa `sqlite`, para modificar el código de acceso usa `filesystem` + `rust-analyzer`

### Seguridad
- **Nunca ejecutar SQL destructivo** (DROP, DELETE masivo) sin confirmación del usuario
- **Nunca hacer git push --force** sin confirmación explícita
- **Nunca modificar archivos de configuración del sistema** solo del proyecto
- **kwin-mcp en modo virtual** para testing aislado por defecto

### Contexto
- Los MCPs agregan tokens al contexto. Si el contexto se llena, deshabilitar MCPs no esenciales
- Para tareas simples de lectura, `filesystem` es más ligero que `rust-analyzer`
- Para documentación, `context7` es más rápido que buscar en archivos .md del proyecto

---

## Instalación y Dependencias

### Binarios instalados
```bash
~/.cargo/bin/rust-analyzer-mcp      # v0.4.0 (compilado desde crates.io)
~/.cargo/bin/rust-mcp-filesystem    # v0.4.5 (binario pre-compilado)
~/.cargo/bin/mcp-server-git-rs      # v0.2.1 (binario pre-compilado)
```

### Dependencias runtime
```bash
# kwin-mcp (Python, instalado via pipx)
pipx install kwin-mcp-server

# memory (Node.js, ejecuta via npx)
npx -y @modelcontextprotocol/server-memory

# sqlite (Node.js, ejecuta via npx)
npx -y mcp-server-sqlite --db ./kde-assistant.db

# context7 (remoto, sin instalación)
# Se conecta a https://mcp.context7.com/mcp
```

### System deps para kwin-mcp
```bash
# Arch Linux
sudo pacman -S at-spi2-core python-gobject spectacle kdotool

# Verificar que KDE Plasma 6 Wayland está corriendo
echo $XDG_CURRENT_DESKTOP  # Debe mostrar KDE
echo $WAYLAND_DISPLAY      # Debe mostrar wayland-0
```

---

## Configuración en opencode.jsonc

La configuración de estos MCPs se encuentra en `opencode.jsonc` en la raíz del proyecto.
Cada MCP se puede habilitar/deshabilitar individualmente setting `enabled: true/false`.

Para deshabilitar temporalmente un MCP sin eliminarlo:
```jsonc
{
  "mcp": {
    "kwin-mcp": {
      "enabled": false  // Deshabilitado temporalmente
    }
  }
}
```
