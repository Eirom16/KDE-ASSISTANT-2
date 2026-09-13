---
name: tool-calling
description: Implementación de tool calling: bucle ReAct, formato OpenAI, ejecución de herramientas, y confirmación de usuario
metadata:
  audience: developers
  module: src/backend/
  ai: true
---

## Qué hago

Proporciono guías y patrones para implementar tool calling en KDE Assistant: formato OpenAI, bucle ReAct, ejecución segura de herramientas, y sistema de confirmación.

## Cuándo usarme

Usa esta skill cuando:
- Modifiques `ai_service.rs` (bucle de tool calling)
- Agregues nuevas herramientas en `tool_executor.rs`
- Cambies el esquema de herramientas en `tool_registry.rs`
- Modifiques el sistema de confirmación en `approvals.rs`
- Depures problemas de tool calling

## Arquitectura

```
┌─────────────┐     ┌──────────────┐     ┌─────────────┐
│  OpenRouter │────>│  ai_service  │────>│  UI (QML)   │
│  (LLM API)  │<────│  (ReAct loop)│     │  ToolBadge  │
└─────────────┘     └──────────────┘     └─────────────┘
                           │
                           ▼
                    ┌──────────────┐
                    │tool_executor │
                    │  (17 tools)  │
                    └──────────────┘
                           │
                           ▼
                    ┌──────────────┐
                    │  Sistema     │
                    │  Archivo/DB  │
                    └──────────────┘
```

## Formato OpenAI

### Definición de Herramientas
```json
{
  "type": "function",
  "function": {
    "name": "open_app",
    "description": "Lanza una aplicación del sistema",
    "parameters": {
      "type": "object",
      "properties": {
        "name": {
          "type": "string",
          "description": "Nombre de la aplicación"
        }
      },
      "required": ["name"]
    }
  }
}
```

### Request con Tool Calling
```json
{
  "model": "openrouter/auto",
  "messages": [...],
  "tools": [
    {
      "type": "function",
      "function": {
        "name": "open_app",
        "description": "...",
        "parameters": {...}
      }
    }
  ],
  "tool_choice": "auto"
}
```

### Response con Tool Call
```json
{
  "choices": [{
    "message": {
      "role": "assistant",
      "content": null,
      "tool_calls": [{
        "id": "call_abc123",
        "type": "function",
        "function": {
          "name": "open_app",
          "arguments": "{\"name\": \"firefox\"}"
        }
      }]
    }
  }]
}
```

## Bucle ReAct (en ai_service.rs)

```rust
// Pseudo-código del bucle
const MAX_ITERATIONS: usize = 8;

loop {
    let response = ai_service.chat(messages, tools).await?;

    if let Some(tool_calls) = response.tool_calls {
        for tool_call in tool_calls {
            // 1. Mostrar ToolCallBadge en UI
            ui::show_tool_call(tool_call.clone());

            // 2. Verificar permisos
            let permission = tool_registry.get_permission(&tool_call.name);
            if permission == Permission::Confirm {
                let confirmed = ui::ask_confirmation(&tool_call).await;
                if !confirmed {
                    continue;
                }
            }

            // 3. Ejecutar herramienta
            let result = tool_executor.execute(&tool_call).await?;

            // 4. Mostrar resultado
            ui::show_tool_result(&result);

            // 5. Agregar al historial
            messages.push(Message::tool(tool_call.id, result));
        }

        if iterations >= MAX_ITERATIONS {
            break;
        }
        iterations += 1;
        continue;
    }

    // Respuesta final sin tool calls
    return Ok(response.content);
}
```

## Herramientas Disponibles (17)

### 🟢 Auto (sin confirmación)
| Nombre | Descripción | Ejemplo |
|--------|-------------|---------|
| `read_file` | Lee un archivo | `{"path": "/etc/passwd"}` |
| `web_search` | Busca en web | `{"query": "rust async"}` |
| `show_image` | Inyecta imagen en chat | `{"source": "/tmp/img.png"}` |
| `find_file` | Busca archivos por nombre | `{"query": "*.rs"}` |
| `system_info` | Info del sistema | `{}` |
| `network_status` | Estado de red | `{}` |

### 🟡 Confirma (en modo normal)
| Nombre | Descripción | Ejemplo |
|--------|-------------|---------|
| `open_app` | Lanza app del sistema | `{"name": "firefox"}` |
| `create_file` | Crea archivo | `{"path": "...", "content": "..."}` |
| `open_file` | Abre archivo con app | `{"path": "..."}` |
| `open_url` | Abre URL en navegador | `{"url": "https://..."}` |
| `notify` | Notificación KDE | `{"title": "...", "body": "..."}` |
| `media` | Control multimedia | `{"action": "play"}` |
| `volume` | Control de volumen | `{"action": "set", "level": 80}` |
| `brightness` | Control de brillo | `{"action": "set", "level": 50}` |
| `remind_in` | Recordatorio | `{"minutes": 30, "text": "..."}` |
| `kdeconnect` | Móvil KDE Connect | `{"action": "devices"}` |

### 🔴 Siempre confirma
| Nombre | Descripción | Ejemplo |
|--------|-------------|---------|
| `edit_file` | Edita archivo | `{"path": "...", "content": "..."}` |

## Sistema de Permisos (approvals.rs)

```rust
pub enum Permission {
    Auto,       // Ejecuta sin preguntar
    Confirm,    // Pregunta al usuario
    AlwaysDeny, // Nunca ejecuta
}

// En modo manos-libres (voz):
// 🟡 Auto → se ejecuta automáticamente
// 🔴 AlwaysDeny → se deniega al momento
```

## Registrando Nueva Herramienta

### 1. Definir en tool_registry.rs
```rust
Tool {
    name: "mi_herramienta".to_string(),
    description: "Descripción de la herramienta".to_string(),
    parameters: serde_json::json!({
        "type": "object",
        "properties": {
            "param1": {
                "type": "string",
                "description": "Descripción del parámetro"
            }
        },
        "required": ["param1"]
    }),
    permission: Permission::Confirm,
}
```

### 2. Implementar en tool_executor.rs
```rust
async fn execute_mi_herramienta(args: &Value) -> Result<String> {
    let param1 = args["param1"]
        .as_str()
        .ok_or_else(|| anyhow!("Missing param1"))?;

    // Lógica de ejecución
    Ok(format!("Resultado: {}", param1))
}
```

### 3. Agregar al match en execute()
```rust
pub async fn execute(&self, tool_call: &ToolCall) -> Result<String> {
    match tool_call.function.name.as_str() {
        "open_app" => self.execute_open_app(&args).await,
        "mi_herramienta" => self.execute_mi_herramienta(&args).await,
        _ => Err(anyhow!("Unknown tool: {}", name)),
    }
}
```

## Troubleshooting

### LLM no llama herramientas
- Verificar que `tools` está en el request
- Verificar que el modelo soporta tool calling
- Verificar que los esquemas son válidos JSON Schema

### Herramienta falla silenciosamente
- Verificar logs del backend
- Verificar que el executor tiene la herramienta registrada
- Verificar que los parámetros son correctos

### Bucle infinito de tool calls
- Verificar MAX_ITERATIONS (debe ser 8)
- Verificar que el LLM recibe el resultado de la herramienta
- Verificar que el historial se actualiza correctamente
