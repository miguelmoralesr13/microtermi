//! Configuración persistente (config.json).

use std::path::PathBuf;

pub fn config_path() -> Option<PathBuf> {
    dirs::config_dir().map(|d| d.join("microtermi").join("config.json"))
}

/// Carga el JSON completo de configuración. Si no existe o falla, devuelve un objeto vacío.
pub fn load_config_json() -> serde_json::Value {
    let path = match config_path() {
        Some(p) => p,
        None => return serde_json::json!({}),
    };
    let content = match std::fs::read_to_string(&path) {
        Ok(c) => c,
        Err(_) => return serde_json::json!({}),
    };
    serde_json::from_str(&content).unwrap_or_else(|_| serde_json::json!({}))
}

pub fn save_config_write(json: &serde_json::Value) {
    if let Some(path) = config_path() {
        if let Some(parent) = path.parent() {
            let _ = std::fs::create_dir_all(parent);
        }
        let _ = std::fs::write(path, json.to_string());
    }
}
