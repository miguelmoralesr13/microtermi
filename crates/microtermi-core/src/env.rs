use serde::Serialize;
use std::collections::HashMap;
use std::path::Path;
use thiserror::Error;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub enum Environment {
    Dev,
    Staging,
    Prod,
}

impl Environment {
    pub const ALL: [Environment; 3] = [Environment::Dev, Environment::Staging, Environment::Prod];

    pub fn as_str(self) -> &'static str {
        match self {
            Environment::Dev => "dev",
            Environment::Staging => "staging",
            Environment::Prod => "prod",
        }
    }

    pub fn env_file_name(self) -> &'static str {
        match self {
            Environment::Dev => ".env.dev",
            Environment::Staging => ".env.staging",
            Environment::Prod => ".env.prod",
        }
    }
}

#[derive(Debug, Error)]
pub enum EnvError {
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
}

/// Load env vars from root/.env.<env> or root/.env
pub fn load_env(root: &Path, env: Environment) -> Result<HashMap<String, String>, EnvError> {
    let mut vars = HashMap::new();
    let env_path = root.join(env.env_file_name());
    if env_path.exists() {
        let content = std::fs::read_to_string(&env_path)?;
        for line in content.lines() {
            let line = line.trim();
            if line.is_empty() || line.starts_with('#') {
                continue;
            }
            if let Some((k, v)) = parse_env_line(line) {
                vars.insert(k, v);
            }
        }
    }
    let fallback = root.join(".env");
    if fallback.exists() && vars.is_empty() {
        let content = std::fs::read_to_string(&fallback)?;
        for line in content.lines() {
            let line = line.trim();
            if line.is_empty() || line.starts_with('#') {
                continue;
            }
            if let Some((k, v)) = parse_env_line(line) {
                vars.insert(k, v);
            }
        }
    }
    Ok(vars)
}

fn parse_env_line(line: &str) -> Option<(String, String)> {
    let eq = line.find('=')?;
    let (k, v) = line.split_at(eq);
    let k = k.trim().to_string();
    let v = v[1..].trim().trim_matches('"').trim_matches('\'').to_string();
    Some((k, v))
}

/// Save env vars to root/.env.<env>
pub fn save_env(
    root: &Path,
    env: Environment,
    vars: &HashMap<String, String>,
) -> Result<(), EnvError> {
    let path = root.join(env.env_file_name());
    let mut lines: Vec<String> = vars
        .iter()
        .map(|(k, v)| format!("{}={}", k, v))
        .collect();
    lines.sort();
    std::fs::write(path, lines.join("\n"))?;
    Ok(())
}
