//! Registro de herramientas expuestas al LLM.
//!
//! Define los schemas JSON (formato OpenAI function calling) para las 11 herramientas
//! del agente: open_app, create_file, edit_file, read_file, web_search, show_image,
//! find_file, open_file, open_url, system_info, notify.

use crate::models::{Config, Tool, ToolFunction, ToolParameters, ToolProperty};
use std::collections::HashMap;

/// Nivel de permiso de una herramienta (F4-1).
/// 🟢 Green: solo lectura o inocuas → se ejecutan solas.
/// 🟡 Yellow: cambian estado visible → piden confirmación (salvo modo potencia).
/// 🔴 Red: destructivas o disruptivas → siempre piden confirmación explícita.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Permission {
    Green,
    Yellow,
    Red,
}

impl Permission {
    pub fn as_str(self) -> &'static str {
        match self {
            Permission::Green => "green",
            Permission::Yellow => "yellow",
            Permission::Red => "red",
        }
    }
}

/// Nivel de cada herramienta.
pub fn permission(name: &str) -> Permission {
    match name {
        "read_file" | "web_search" | "show_image" | "find_file" | "system_info"
        | "network_status" => Permission::Green,
        "edit_file" => Permission::Red,
        _ => Permission::Yellow,
    }
}

pub fn all_tools() -> Vec<Tool> {
    vec![
        open_app_tool(),
        create_file_tool(),
        edit_file_tool(),
        read_file_tool(),
        web_search_tool(),
        show_image_tool(),
        find_file_tool(),
        open_file_tool(),
        open_url_tool(),
        system_info_tool(),
        notify_tool(),
        media_tool(),
        volume_tool(),
        brightness_tool(),
        network_status_tool(),
        remind_in_tool(),
        kdeconnect_tool(),
    ]
}

fn open_app_tool() -> Tool {
    let mut properties = HashMap::new();
    properties.insert(
        "name".to_string(),
        ToolProperty {
            prop_type: "string".to_string(),
            description: Some(
                "Nombre o alias de la aplicacion (ej: 'firefox', 'dolphin', 'terminal', 'spotify')"
                    .to_string(),
            ),
            r#enum: None,
            items: None,
        },
    );

    Tool {
        tool_type: "function".to_string(),
        function: ToolFunction {
            name: "open_app".to_string(),
            description: "Abre una aplicacion del sistema. Acepta el nombre del menú (Name del .desktop, ej: 'Firefox', 'Dolphin') o alias (ej: 'firefox', 'terminal', 'spotify')."
                .to_string(),
            parameters: ToolParameters {
                param_type: "object".to_string(),
                properties,
                required: vec!["name".to_string()],
            },
        },
    }
}

fn create_file_tool() -> Tool {
    let mut properties = HashMap::new();
    properties.insert(
        "path".to_string(),
        ToolProperty {
            prop_type: "string".to_string(),
            description: Some(
                "Ruta absoluta del archivo a crear (ej: '/home/user/notas.md')".to_string(),
            ),
            r#enum: None,
            items: None,
        },
    );
    properties.insert(
        "content".to_string(),
        ToolProperty {
            prop_type: "string".to_string(),
            description: Some("Contenido completo del archivo".to_string()),
            r#enum: None,
            items: None,
        },
    );

    Tool {
        tool_type: "function".to_string(),
        function: ToolFunction {
            name: "create_file".to_string(),
            description: "Crea un archivo nuevo con el contenido dado. Falla si el archivo ya existe o si la ruta no esta permitida."
                .to_string(),
            parameters: ToolParameters {
                param_type: "object".to_string(),
                properties,
                required: vec!["path".to_string(), "content".to_string()],
            },
        },
    }
}

fn edit_file_tool() -> Tool {
    let mut properties = HashMap::new();
    properties.insert(
        "path".to_string(),
        ToolProperty {
            prop_type: "string".to_string(),
            description: Some("Ruta absoluta del archivo a editar".to_string()),
            r#enum: None,
            items: None,
        },
    );
    properties.insert(
        "mode".to_string(),
        ToolProperty {
            prop_type: "string".to_string(),
            description: Some("Modo de edicion".to_string()),
            r#enum: Some(vec!["overwrite".to_string(), "append".to_string()]),
            items: None,
        },
    );
    properties.insert(
        "content".to_string(),
        ToolProperty {
            prop_type: "string".to_string(),
            description: Some(
                "Contenido a escribir (reemplaza todo o se anade al final segun mode)".to_string(),
            ),
            r#enum: None,
            items: None,
        },
    );

    Tool {
        tool_type: "function".to_string(),
        function: ToolFunction {
            name: "edit_file".to_string(),
            description: "Edita un archivo existente. Si mode='overwrite' reemplaza el contenido; si mode='append' lo anade al final."
                .to_string(),
            parameters: ToolParameters {
                param_type: "object".to_string(),
                properties,
                required: vec![
                    "path".to_string(),
                    "mode".to_string(),
                    "content".to_string(),
                ],
            },
        },
    }
}

fn read_file_tool() -> Tool {
    let mut properties = HashMap::new();
    properties.insert(
        "path".to_string(),
        ToolProperty {
            prop_type: "string".to_string(),
            description: Some("Ruta absoluta del archivo a leer".to_string()),
            r#enum: None,
            items: None,
        },
    );

    Tool {
        tool_type: "function".to_string(),
        function: ToolFunction {
            name: "read_file".to_string(),
            description: "Lee el contenido de un archivo de texto. Solo funciona con rutas permitidas por el usuario."
                .to_string(),
            parameters: ToolParameters {
                param_type: "object".to_string(),
                properties,
                required: vec!["path".to_string()],
            },
        },
    }
}

fn web_search_tool() -> Tool {
    let mut properties = HashMap::new();
    properties.insert(
        "query".to_string(),
        ToolProperty {
            prop_type: "string".to_string(),
            description: Some(
                "Terminos de busqueda (ej: 'noticias linux kernel 6.10')".to_string(),
            ),
            r#enum: None,
            items: None,
        },
    );

    Tool {
        tool_type: "function".to_string(),
        function: ToolFunction {
            name: "web_search".to_string(),
            description: "Busca en la web usando DuckDuckGo y devuelve los primeros resultados con titulo, URL y extracto."
                .to_string(),
            parameters: ToolParameters {
                param_type: "object".to_string(),
                properties,
                required: vec!["query".to_string()],
            },
        },
    }
}

fn show_image_tool() -> Tool {
    let mut properties = HashMap::new();
    properties.insert(
        "source".to_string(),
        ToolProperty {
            prop_type: "string".to_string(),
            description: Some(
                "URL (https://...) o ruta local (file:///...) de la imagen".to_string(),
            ),
            r#enum: None,
            items: None,
        },
    );
    properties.insert(
        "caption".to_string(),
        ToolProperty {
            prop_type: "string".to_string(),
            description: Some("Pie de foto opcional".to_string()),
            r#enum: None,
            items: None,
        },
    );

    Tool {
        tool_type: "function".to_string(),
        function: ToolFunction {
            name: "show_image".to_string(),
            description: "Muestra una imagen en el chat. Acepta URLs publicas o rutas locales. Si es una URL remota, se descarga y se almacena en cache."
                .to_string(),
            parameters: ToolParameters {
                param_type: "object".to_string(),
                properties,
                required: vec!["source".to_string()],
            },
        },
    }
}

fn str_prop(desc: &str) -> ToolProperty {
    ToolProperty {
        prop_type: "string".to_string(),
        description: Some(desc.to_string()),
        r#enum: None,
        items: None,
    }
}

fn find_file_tool() -> Tool {
    let mut properties = HashMap::new();
    properties.insert(
        "query".to_string(),
        str_prop("Subcadena del nombre a buscar (ej: 'factura', 'notas.md')"),
    );
    properties.insert(
        "dir".to_string(),
        str_prop("Directorio base opcional (dentro de allowed_paths). Sin el, busca en todos."),
    );

    Tool {
        tool_type: "function".to_string(),
        function: ToolFunction {
            name: "find_file".to_string(),
            description: "Busca archivos por nombre dentro de las carpetas permitidas. Devuelve hasta 20 rutas."
                .to_string(),
            parameters: ToolParameters {
                param_type: "object".to_string(),
                properties,
                required: vec!["query".to_string()],
            },
        },
    }
}

fn open_file_tool() -> Tool {
    let mut properties = HashMap::new();
    properties.insert(
        "path".to_string(),
        str_prop("Ruta del archivo o carpeta a abrir (dentro de allowed_paths)"),
    );
    properties.insert(
        "reveal".to_string(),
        ToolProperty {
            prop_type: "boolean".to_string(),
            description: Some(
                "Si true, lo muestra seleccionado en Dolphin en vez de abrirlo".to_string(),
            ),
            r#enum: None,
            items: None,
        },
    );

    Tool {
        tool_type: "function".to_string(),
        function: ToolFunction {
            name: "open_file".to_string(),
            description: "Abre un archivo o carpeta con la app por defecto. Con reveal=true lo muestra en Dolphin."
                .to_string(),
            parameters: ToolParameters {
                param_type: "object".to_string(),
                properties,
                required: vec!["path".to_string()],
            },
        },
    }
}

fn open_url_tool() -> Tool {
    let mut properties = HashMap::new();
    properties.insert(
        "url".to_string(),
        str_prop("URL http(s) a abrir en el navegador"),
    );

    Tool {
        tool_type: "function".to_string(),
        function: ToolFunction {
            name: "open_url".to_string(),
            description: "Abre una URL http(s) en el navegador por defecto.".to_string(),
            parameters: ToolParameters {
                param_type: "object".to_string(),
                properties,
                required: vec!["url".to_string()],
            },
        },
    }
}

fn system_info_tool() -> Tool {
    Tool {
        tool_type: "function".to_string(),
        function: ToolFunction {
            name: "system_info".to_string(),
            description: "Muestra información del sistema: OS, kernel, uptime, CPU, RAM, disco y batería. Solo lectura."
                .to_string(),
            parameters: ToolParameters {
                param_type: "object".to_string(),
                properties: HashMap::new(),
                required: vec![],
            },
        },
    }
}

fn notify_tool() -> Tool {
    let mut properties = HashMap::new();
    properties.insert(
        "title".to_string(),
        str_prop("Título corto de la notificación (máx 100 chars)"),
    );
    properties.insert(
        "body".to_string(),
        str_prop("Cuerpo de la notificación (máx 500 chars)"),
    );

    Tool {
        tool_type: "function".to_string(),
        function: ToolFunction {
            name: "notify".to_string(),
            description: "Envía una notificación nativa de KDE Plasma al usuario.".to_string(),
            parameters: ToolParameters {
                param_type: "object".to_string(),
                properties,
                required: vec!["title".to_string(), "body".to_string()],
            },
        },
    }
}

fn enum_prop(desc: &str, values: &[&str]) -> ToolProperty {
    ToolProperty {
        prop_type: "string".to_string(),
        description: Some(desc.to_string()),
        r#enum: Some(values.iter().map(|s| s.to_string()).collect()),
        items: None,
    }
}

fn media_tool() -> Tool {
    let mut properties = HashMap::new();
    properties.insert(
        "action".to_string(),
        enum_prop(
            "Acción multimedia",
            &["play", "pause", "play-pause", "next", "previous", "status"],
        ),
    );

    Tool {
        tool_type: "function".to_string(),
        function: ToolFunction {
            name: "media".to_string(),
            description: "Controla la reproducción multimedia (playerctl): play, pause, next, previous o status."
                .to_string(),
            parameters: ToolParameters {
                param_type: "object".to_string(),
                properties,
                required: vec!["action".to_string()],
            },
        },
    }
}

fn volume_tool() -> Tool {
    let mut properties = HashMap::new();
    properties.insert(
        "action".to_string(),
        enum_prop("Acción de volumen", &["get", "set", "mute", "unmute"]),
    );
    properties.insert(
        "level".to_string(),
        ToolProperty {
            prop_type: "number".to_string(),
            description: Some("Nivel 0.0-1.0 (solo para set)".to_string()),
            r#enum: None,
            items: None,
        },
    );

    Tool {
        tool_type: "function".to_string(),
        function: ToolFunction {
            name: "volume".to_string(),
            description: "Consulta o ajusta el volumen del sistema (wpctl/pactl). set necesita level 0.0-1.0."
                .to_string(),
            parameters: ToolParameters {
                param_type: "object".to_string(),
                properties,
                required: vec!["action".to_string()],
            },
        },
    }
}

fn brightness_tool() -> Tool {
    let mut properties = HashMap::new();
    properties.insert(
        "action".to_string(),
        enum_prop("Acción de brillo", &["get", "set"]),
    );
    properties.insert(
        "level".to_string(),
        ToolProperty {
            prop_type: "number".to_string(),
            description: Some("Nivel 1-100 (solo para set)".to_string()),
            r#enum: None,
            items: None,
        },
    );

    Tool {
        tool_type: "function".to_string(),
        function: ToolFunction {
            name: "brightness".to_string(),
            description:
                "Consulta o ajusta el brillo de pantalla (brightnessctl). set necesita level 1-100."
                    .to_string(),
            parameters: ToolParameters {
                param_type: "object".to_string(),
                properties,
                required: vec!["action".to_string()],
            },
        },
    }
}

fn network_status_tool() -> Tool {
    Tool {
        tool_type: "function".to_string(),
        function: ToolFunction {
            name: "network_status".to_string(),
            description: "Muestra el estado de red y bluetooth (nmcli/bluetoothctl). Solo lectura."
                .to_string(),
            parameters: ToolParameters {
                param_type: "object".to_string(),
                properties: HashMap::new(),
                required: vec![],
            },
        },
    }
}

fn remind_in_tool() -> Tool {
    let mut properties = HashMap::new();
    properties.insert(
        "minutes".to_string(),
        ToolProperty {
            prop_type: "number".to_string(),
            description: Some("Minutos a esperar (0.1-1440)".to_string()),
            r#enum: None,
            items: None,
        },
    );
    properties.insert(
        "text".to_string(),
        str_prop("Texto del recordatorio (máx 500 chars)"),
    );

    Tool {
        tool_type: "function".to_string(),
        function: ToolFunction {
            name: "remind_in".to_string(),
            description:
                "Crea un recordatorio persistente con notificación nativa. Sobrevive reinicios."
                    .to_string(),
            parameters: ToolParameters {
                param_type: "object".to_string(),
                properties,
                required: vec!["minutes".to_string(), "text".to_string()],
            },
        },
    }
}

fn kdeconnect_tool() -> Tool {
    let mut properties = HashMap::new();
    properties.insert(
        "action".to_string(),
        enum_prop(
            "Acción KDE Connect",
            &["devices", "ping", "ring", "share_url", "share_file", "sms"],
        ),
    );
    properties.insert(
        "device".to_string(),
        str_prop("Id del móvil (ver devices). Requerido salvo en devices."),
    );
    properties.insert("url".to_string(), str_prop("URL http(s) para share_url"));
    properties.insert(
        "path".to_string(),
        str_prop("Ruta local (allowed_paths) para share_file"),
    );
    properties.insert("number".to_string(), str_prop("Número destino para sms"));
    properties.insert(
        "text".to_string(),
        str_prop("Texto para sms (máx 500 chars)"),
    );

    Tool {
        tool_type: "function".to_string(),
        function: ToolFunction {
            name: "kdeconnect".to_string(),
            description: "KDE Connect: listar móviles, ping/ring, compartir URL/archivo, enviar SMS. Requiere kdeconnect-cli y móvil emparejado."
                .to_string(),
            parameters: ToolParameters {
                param_type: "object".to_string(),
                properties,
                required: vec!["action".to_string()],
            },
        },
    }
}

/// Herramientas filtradas según la configuración del usuario.
///
/// Respeta `ai.enable_tool_calling` (si es false → ninguna) y cada
/// flag `tools.*` (open_app, create/edit/read_file, web_search, show_image,
/// find_file, open_file, open_url).
/// El executor también valida por si llega una llamada deshabilitada.
pub fn filtered_tools(cfg: &Config) -> Vec<Tool> {
    if !cfg.ai.enable_tool_calling {
        return Vec::new();
    }
    let mut out = Vec::new();
    if cfg.tools.open_app {
        out.push(open_app_tool());
    }
    if cfg.tools.create_file {
        out.push(create_file_tool());
    }
    if cfg.tools.edit_file {
        out.push(edit_file_tool());
    }
    if cfg.tools.read_file {
        out.push(read_file_tool());
    }
    if cfg.tools.web_search {
        out.push(web_search_tool());
    }
    if cfg.tools.show_image {
        out.push(show_image_tool());
    }
    if cfg.tools.find_file {
        out.push(find_file_tool());
    }
    if cfg.tools.open_file {
        out.push(open_file_tool());
    }
    if cfg.tools.open_url {
        out.push(open_url_tool());
    }
    if cfg.tools.system_info {
        out.push(system_info_tool());
    }
    if cfg.tools.notify {
        out.push(notify_tool());
    }
    if cfg.tools.media {
        out.push(media_tool());
    }
    if cfg.tools.volume {
        out.push(volume_tool());
    }
    if cfg.tools.brightness {
        out.push(brightness_tool());
    }
    if cfg.tools.network_status {
        out.push(network_status_tool());
    }
    if cfg.tools.remind_in {
        out.push(remind_in_tool());
    }
    if cfg.tools.kdeconnect {
        out.push(kdeconnect_tool());
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::Config;

    #[test]
    fn disabled_tool_calling_returns_empty() {
        let mut cfg = Config::default();
        cfg.ai.enable_tool_calling = false;
        assert!(filtered_tools(&cfg).is_empty());
    }

    #[test]
    fn per_tool_flags_filter() {
        let mut cfg = Config::default();
        cfg.ai.enable_tool_calling = true;
        cfg.tools.open_app = true;
        cfg.tools.create_file = false;
        cfg.tools.edit_file = false;
        cfg.tools.read_file = false;
        cfg.tools.web_search = false;
        cfg.tools.show_image = false;
        cfg.tools.find_file = false;
        cfg.tools.open_file = false;
        cfg.tools.open_url = false;
        cfg.tools.system_info = false;
        cfg.tools.notify = false;
        cfg.tools.media = false;
        cfg.tools.volume = false;
        cfg.tools.brightness = false;
        cfg.tools.network_status = false;
        cfg.tools.remind_in = false;
        cfg.tools.kdeconnect = false;
        let tools = filtered_tools(&cfg);
        assert_eq!(tools.len(), 1);
        assert_eq!(tools[0].function.name, "open_app");
    }

    #[test]
    fn all_enabled_by_default() {
        let cfg = Config::default();
        assert_eq!(filtered_tools(&cfg).len(), 17);
    }

    #[test]
    fn permission_levels() {
        use super::permission;
        assert_eq!(permission("read_file"), super::Permission::Green);
        assert_eq!(permission("network_status"), super::Permission::Green);
        assert_eq!(permission("open_app"), super::Permission::Yellow);
        assert_eq!(permission("media"), super::Permission::Yellow);
        assert_eq!(permission("kdeconnect"), super::Permission::Yellow);
        assert_eq!(permission("edit_file"), super::Permission::Red);
        assert_eq!(permission("whatever_unknown"), super::Permission::Yellow);
    }
}
