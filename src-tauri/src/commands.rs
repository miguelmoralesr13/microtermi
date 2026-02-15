use crate::config;
use microtermi_core::{
    branches, branches_remote, checkout_branch, checkout_remote_branch, clone_repo,
    clone_url_with_token, commit, commit_changes, fetch, list_branches, list_projects, log,
    open_repo, push, pull, stash, stash_pop, status,
    run_script_captured, Environment, Project,
};
use std::collections::{HashMap, HashSet};
use std::path::Path;
use std::thread;
use tauri::{AppHandle, Emitter, State};
use uuid::Uuid;

#[derive(serde::Serialize)]
pub struct ConfigPayload {
    pub root_path: Option<String>,
    pub gitlab_url: Option<String>,
    pub gitlab_token: Option<String>,
}

#[tauri::command]
pub fn load_config() -> ConfigPayload {
    let (root, url, token) = config::load_config();
    ConfigPayload {
        root_path: root.map(|p| p.to_string_lossy().into_owned()),
        gitlab_url: url,
        gitlab_token: token,
    }
}

#[tauri::command]
pub fn save_config_root(root: String) {
    config::save_config_root(Path::new(&root));
}

#[tauri::command]
pub fn save_config_gitlab(url: String, token: String) {
    config::save_config_gitlab(&url, &token);
}

#[tauri::command]
pub fn pick_folder() -> Option<String> {
    rfd::FileDialog::new().pick_folder().map(|p| p.to_string_lossy().into_owned())
}

#[tauri::command]
pub fn scan_projects(root: String) -> Result<Vec<Project>, String> {
    microtermi_core::scan_projects(Path::new(&root)).map_err(|e| e.to_string())
}

fn env_from_str(s: &str) -> Environment {
    match s.to_lowercase().as_str() {
        "staging" => Environment::Staging,
        "prod" => Environment::Prod,
        _ => Environment::Dev,
    }
}

#[tauri::command]
pub fn load_env(root: String, environment: String) -> Result<HashMap<String, String>, String> {
    microtermi_core::load_env(Path::new(&root), env_from_str(&environment)).map_err(|e| e.to_string())
}

#[tauri::command]
pub fn save_env(
    root: String,
    environment: String,
    vars: HashMap<String, String>,
) -> Result<(), String> {
    microtermi_core::save_env(Path::new(&root), env_from_str(&environment), &vars).map_err(|e| e.to_string())
}

#[tauri::command]
pub fn common_script_names(root: String) -> Result<Vec<String>, String> {
    let projects = microtermi_core::scan_projects(Path::new(&root)).map_err(|e| e.to_string())?;
    if projects.is_empty() {
        return Ok(Vec::new());
    }
    let mut sets: Vec<HashSet<String>> = projects
        .iter()
        .map(|p| p.scripts.iter().map(|(s, _)| s.clone()).collect())
        .collect();
    let mut common = sets.pop().unwrap_or_default();
    for set in sets {
        common.retain(|s| set.contains(s));
    }
    let mut names: Vec<String> = common.into_iter().collect();
    names.sort();
    Ok(names)
}

#[tauri::command]
pub fn run_script_start(
    app: AppHandle,
    project_path: String,
    script_name: String,
    env_vars: HashMap<String, String>,
    state: State<'_, crate::TerminalState>,
) -> Result<String, String> {
    let project = Project {
        name: script_name.clone(),
        path: std::path::PathBuf::from(&project_path),
        scripts: vec![(script_name.clone(), format!("run {}", script_name))],
    };
    let (child, receiver) = run_script_captured(&project, &script_name, &env_vars)
        .map_err(|e| e.to_string())?;
    let session_id = Uuid::new_v4().to_string();
    {
        let mut procs = state.processes.lock().unwrap();
        procs.insert(session_id.clone(), child);
    }
    let app_emit = app.clone();
    let sid = session_id.clone();
    thread::spawn(move || {
        while let Ok(line) = receiver.recv() {
            let _ = app_emit.emit("terminal-line", (sid.clone(), line));
        }
        let _ = app_emit.emit("terminal-end", sid);
    });
    Ok(session_id)
}

#[tauri::command]
pub fn terminal_stop(session_id: String, state: State<'_, crate::TerminalState>) -> Result<(), String> {
    let mut procs = state.processes.lock().unwrap();
    if let Some(mut child) = procs.remove(&session_id) {
        let _ = child.kill();
    }
    Ok(())
}

#[tauri::command]
pub fn terminal_stop_all(state: State<'_, crate::TerminalState>) -> Result<(), String> {
    let mut procs = state.processes.lock().unwrap();
    for (_, mut child) in procs.drain() {
        let _ = child.kill();
    }
    Ok(())
}

#[tauri::command]
pub fn git_status(path: String) -> Result<microtermi_core::GitStatus, String> {
    let repo = open_repo(Path::new(&path)).map_err(|e| e.to_string())?;
    status(&repo).map_err(|e| e.to_string())
}

#[tauri::command]
pub fn git_branches(path: String) -> Result<Vec<String>, String> {
    let repo = open_repo(Path::new(&path)).map_err(|e| e.to_string())?;
    branches(&repo).map_err(|e| e.to_string())
}

#[tauri::command]
pub fn git_branches_remote(path: String) -> Result<Vec<String>, String> {
    let repo = open_repo(Path::new(&path)).map_err(|e| e.to_string())?;
    branches_remote(&repo).map_err(|e| e.to_string())
}

#[tauri::command]
pub fn git_fetch(path: String) -> Result<(), String> {
    let repo = open_repo(Path::new(&path)).map_err(|e| e.to_string())?;
    fetch(&repo).map_err(|e| e.to_string())
}

#[tauri::command]
pub fn git_checkout_branch(path: String, branch: String) -> Result<(), String> {
    let repo = open_repo(Path::new(&path)).map_err(|e| e.to_string())?;
    checkout_branch(&repo, &branch).map_err(|e| e.to_string())
}

#[tauri::command]
pub fn git_checkout_remote_branch(path: String, branch: String) -> Result<(), String> {
    let repo = open_repo(Path::new(&path)).map_err(|e| e.to_string())?;
    checkout_remote_branch(&repo, &branch).map_err(|e| e.to_string())
}

#[tauri::command]
pub fn git_pull(path: String) -> Result<String, String> {
    let repo = open_repo(Path::new(&path)).map_err(|e| e.to_string())?;
    pull(&repo).map_err(|e| e.to_string())
}

#[tauri::command]
pub fn git_push(path: String) -> Result<String, String> {
    let repo = open_repo(Path::new(&path)).map_err(|e| e.to_string())?;
    push(&repo).map_err(|e| e.to_string())
}

#[tauri::command]
pub fn git_commit(path: String, message: String, paths: Vec<String>) -> Result<(), String> {
    let repo = open_repo(Path::new(&path)).map_err(|e| e.to_string())?;
    let path_refs: Vec<&Path> = paths.iter().map(|s| Path::new(s)).collect();
    commit(&repo, &message, &path_refs).map_err(|e| e.to_string())
}

#[tauri::command]
pub fn git_log(path: String, max_count: usize) -> Result<Vec<microtermi_core::CommitInfo>, String> {
    let repo = open_repo(Path::new(&path)).map_err(|e| e.to_string())?;
    log(&repo, max_count).map_err(|e| e.to_string())
}

#[tauri::command]
pub fn git_commit_changes(
    path: String,
    commit_id: String,
) -> Result<Vec<microtermi_core::CommitFileChange>, String> {
    let repo = open_repo(Path::new(&path)).map_err(|e| e.to_string())?;
    commit_changes(&repo, &commit_id).map_err(|e| e.to_string())
}

#[tauri::command]
pub fn git_stash(path: String) -> Result<(), String> {
    let mut repo = open_repo(Path::new(&path)).map_err(|e| e.to_string())?;
    stash(&mut repo).map_err(|e| e.to_string())
}

#[tauri::command]
pub fn git_stash_pop(path: String) -> Result<(), String> {
    let mut repo = open_repo(Path::new(&path)).map_err(|e| e.to_string())?;
    stash_pop(&mut repo).map_err(|e| e.to_string())
}

#[tauri::command]
pub fn gitlab_list_projects(
    url: String,
    token: String,
    search: Option<String>,
) -> Result<Vec<microtermi_core::GitLabProject>, String> {
    list_projects(&url, &token, search.as_deref()).map_err(|e| e.to_string())
}

#[tauri::command]
pub fn gitlab_list_branches(
    url: String,
    token: String,
    project_id: u64,
) -> Result<Vec<microtermi_core::GitLabBranch>, String> {
    list_branches(&url, &token, project_id).map_err(|e| e.to_string())
}

#[tauri::command]
pub fn gitlab_clone(http_url: String, token: String, dest: String) -> Result<(), String> {
    let url = clone_url_with_token(&http_url, &token);
    let path = std::path::Path::new(&dest);
    clone_repo(&url, path).map_err(|e| e.to_string())?;
    Ok(())
}
