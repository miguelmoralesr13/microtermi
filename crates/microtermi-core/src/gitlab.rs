//! Integración con GitLab API v4: listar proyectos y ramas.

use serde::{Deserialize, Serialize};
use thiserror::Error;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GitLabProject {
    pub id: u64,
    pub name: String,
    pub path_with_namespace: String,
    pub web_url: String,
    pub http_url_to_repo: String,
    pub default_branch: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GitLabBranch {
    pub name: String,
}

#[derive(Debug, Deserialize)]
struct ProjectJson {
    id: u64,
    name: String,
    path_with_namespace: String,
    web_url: String,
    http_url_to_repo: String,
    default_branch: Option<String>,
}

#[derive(Debug, Deserialize)]
struct BranchJson {
    name: String,
}

#[derive(Debug, Error)]
pub enum GitLabError {
    #[error("HTTP/reqwest: {0}")]
    Http(#[from] reqwest::Error),
    #[error("GitLab API error: {0}")]
    Api(String),
}

/// Normaliza la URL base (sin / al final, sin /api/v4).
fn normalize_base_url(url: &str) -> String {
    let s = url.trim().trim_end_matches('/');
    let s = s.strip_suffix("/api/v4").unwrap_or(s);
    s.to_string()
}

/// Lista proyectos a los que el usuario tiene acceso (membership o visibles).
/// Si `search` es `Some(s)` y no está vacío, la API de GitLab filtra por nombre/path (servidor).
pub fn list_projects(
    base_url: &str,
    token: &str,
    search: Option<&str>,
) -> Result<Vec<GitLabProject>, GitLabError> {
    let base = normalize_base_url(base_url);
    let search_trim = search.map(|s| s.trim()).and_then(|s| if s.is_empty() { None } else { Some(s) });
    let url = match search_trim {
        Some(s) => {
            let encoded = urlencoding::encode(s);
            format!("{}/api/v4/projects?membership=true&per_page=100&search={}", base, encoded)
        }
        None => format!("{}/api/v4/projects?membership=true&per_page=100", base),
    };
    let client = reqwest::blocking::Client::builder()
        .timeout(std::time::Duration::from_secs(30))
        .build()?;
    let resp = client
        .get(&url)
        .header("PRIVATE-TOKEN", token.trim())
        .send()?;
    if !resp.status().is_success() {
        let status = resp.status();
        let body = resp.text().unwrap_or_default();
        return Err(GitLabError::Api(format!("{}: {}", status, body)));
    }
    let list: Vec<ProjectJson> = resp.json()?;
    Ok(list
        .into_iter()
        .map(|p| GitLabProject {
            id: p.id,
            name: p.name,
            path_with_namespace: p.path_with_namespace,
            web_url: p.web_url,
            http_url_to_repo: p.http_url_to_repo,
            default_branch: p.default_branch,
        })
        .collect())
}

/// Lista ramas de un proyecto por ID.
pub fn list_branches(base_url: &str, token: &str, project_id: u64) -> Result<Vec<GitLabBranch>, GitLabError> {
    let base = normalize_base_url(base_url);
    let url = format!("{}/api/v4/projects/{}/repository/branches?per_page=100", base, project_id);
    let client = reqwest::blocking::Client::builder()
        .timeout(std::time::Duration::from_secs(30))
        .build()?;
    let resp = client
        .get(&url)
        .header("PRIVATE-TOKEN", token.trim())
        .send()?;
    if !resp.status().is_success() {
        let status = resp.status();
        let body = resp.text().unwrap_or_default();
        return Err(GitLabError::Api(format!("{}: {}", status, body)));
    }
    let list: Vec<BranchJson> = resp.json()?;
    Ok(list
        .into_iter()
        .map(|b| GitLabBranch { name: b.name })
        .collect())
}

/// URL de clonación HTTPS con token incrustado (para git clone).
pub fn clone_url_with_token(http_url: &str, token: &str) -> String {
    let token = token.trim();
    if token.is_empty() {
        return http_url.to_string();
    }
    if let Some(rest) = http_url.strip_prefix("https://") {
        return format!("https://oauth2:{}@{}", token, rest);
    }
    if let Some(rest) = http_url.strip_prefix("http://") {
        return format!("http://oauth2:{}@{}", token, rest);
    }
    http_url.to_string()
}
