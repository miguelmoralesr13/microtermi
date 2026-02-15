use std::path::{Path, PathBuf};

fn config_path() -> Option<PathBuf> {
    dirs::config_dir().map(|d| d.join("microtermi").join("config.json"))
}

pub fn load_config() -> (Option<PathBuf>, Option<String>, Option<String>) {
    let path = match config_path() {
        Some(p) => p,
        None => return (None, None, None),
    };
    let content = match std::fs::read_to_string(&path) {
        Ok(c) => c,
        Err(_) => return (None, None, None),
    };
    let json: serde_json::Value = match serde_json::from_str(&content) {
        Ok(j) => j,
        Err(_) => return (None, None, None),
    };
    let root = json
        .get("last_root")
        .and_then(|v| v.as_str())
        .map(PathBuf::from);
    let gitlab_url = json.get("gitlab_url").and_then(|v| v.as_str()).map(String::from);
    let gitlab_token = json.get("gitlab_token").and_then(|v| v.as_str()).map(String::from);
    (root, gitlab_url, gitlab_token)
}

pub fn save_config_root(root: &Path) {
    if let Some(path) = config_path() {
        if let Some(parent) = path.parent() {
            let _ = std::fs::create_dir_all(parent);
        }
        let (_, url, token) = load_config();
        let json = serde_json::json!({
            "last_root": root.to_string_lossy(),
            "gitlab_url": url.unwrap_or_default(),
            "gitlab_token": token.unwrap_or_default(),
        });
        let _ = std::fs::write(path, json.to_string());
    }
}

pub fn save_config_gitlab(url: &str, token: &str) {
    if let Some(path) = config_path() {
        if let Some(parent) = path.parent() {
            let _ = std::fs::create_dir_all(parent);
        }
        let (root, _, _) = load_config();
        let json = serde_json::json!({
            "last_root": root.map(|p| p.to_string_lossy().to_string()).unwrap_or_default(),
            "gitlab_url": url,
            "gitlab_token": token,
        });
        let _ = std::fs::write(path, json.to_string());
    }
}
