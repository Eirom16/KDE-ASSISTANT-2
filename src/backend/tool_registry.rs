//! Registro de herramientas expuestas al LLM.
//!
//! Define los schemas JSON (formato OpenAI function calling) para las 6 herramientas
//! del agente: open_app, create_file, edit_file, read_file, web_search, show_image.

use crate::models::{Config, Tool, ToolFunction, ToolParameters, ToolProperty};
use std::collections::HashMap;

pub fn all_tools() -> Vec<Tool> {
    vec![
        open_app_tool(),
        create_file_tool(),
        edit_file_tool(),
        read_file_tool(),
        web_search_tool(),
        show_image_tool(),
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

/// Herramientas filtradas según la configuración del usuario.
///
/// Respeta `ai.enable_tool_calling` (si es false → ninguna) y cada
/// flag `tools.{open_app,create_file,edit_file,read_file,web_search,show_image}`.
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
        let tools = filtered_tools(&cfg);
        assert_eq!(tools.len(), 1);
        assert_eq!(tools[0].function.name, "open_app");
    }

    #[test]
    fn all_enabled_by_default() {
        let cfg = Config::default();
        assert_eq!(filtered_tools(&cfg).len(), 6);
    }
}
