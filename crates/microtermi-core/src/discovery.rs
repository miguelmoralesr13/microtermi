use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::path::Path;
use thiserror::Error;
use walkdir::WalkDir;

fn path_serialize<S>(path: &std::path::PathBuf, s: S) -> Result<S::Ok, S::Error>
where
    S: serde::Serializer,
{
    s.serialize_str(&path.to_string_lossy())
}

fn path_deserialize<'de, D>(d: D) -> Result<std::path::PathBuf, D::Error>
where
    D: serde::Deserializer<'de>,
{
    let s = String::deserialize(d)?;
    Ok(std::path::PathBuf::from(s))
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Project {
    pub name: String,
    #[serde(serialize_with = "path_serialize", deserialize_with = "path_deserialize")]
    pub path: std::path::PathBuf,
    pub scripts: Vec<(String, String)>,
}

#[derive(Debug, Deserialize)]
struct PackageJson {
    name: Option<String>,
    scripts: Option<BTreeMap<String, String>>,
}

#[derive(Debug, Error)]
pub enum DiscoveryError {
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
    #[error("JSON error: {0}")]
    Json(#[from] serde_json::Error),
}

/// Scan root directory for all package.json files and return projects with name and scripts.
pub fn scan_projects(root: &Path) -> Result<Vec<Project>, DiscoveryError> {
    let mut projects = Vec::new();
    for entry in WalkDir::new(root)
        .max_depth(8)
        .follow_links(false)
        .into_iter()
        .filter_entry(|e| {
            let name = e.file_name().to_string_lossy();
            !name.starts_with('.') && name != "node_modules"
        })
    {
        let entry = entry.map_err(|e| std::io::Error::new(std::io::ErrorKind::Other, e))?;
        if !entry.file_type().is_file() {
            continue;
        }
        if entry.file_name() != "package.json" {
            continue;
        }
        let path = entry.path();
        let content = std::fs::read_to_string(path)?;
        let pkg: PackageJson = serde_json::from_str(&content)?;
        let dir = path
            .parent()
            .map(|p| p.to_path_buf())
            .unwrap_or_else(|| root.to_path_buf());
        let name = pkg
            .name
            .unwrap_or_else(|| dir.file_name().unwrap_or_default().to_string_lossy().to_string());
        let scripts: Vec<(String, String)> = pkg
            .scripts
            .unwrap_or_default()
            .into_iter()
            .collect();
        projects.push(Project {
            name,
            path: dir,
            scripts,
        });
    }
    Ok(projects)
}
