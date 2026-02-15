mod config;
mod commands;

use std::collections::HashMap;
use std::sync::Mutex;

#[derive(Default)]
pub struct TerminalState {
    pub processes: Mutex<HashMap<String, std::process::Child>>,
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_shell::init())
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_fs::init())
        .plugin(tauri_plugin_http::init())
        .manage(TerminalState::default())
        .invoke_handler(tauri::generate_handler![
            commands::load_config,
            commands::save_config_root,
            commands::save_config_gitlab,
            commands::pick_folder,
            commands::scan_projects,
            commands::load_env,
            commands::save_env,
            commands::run_script_start,
            commands::terminal_stop,
            commands::terminal_stop_all,
            commands::git_status,
            commands::git_branches,
            commands::git_branches_remote,
            commands::git_fetch,
            commands::git_checkout_branch,
            commands::git_checkout_remote_branch,
            commands::git_pull,
            commands::git_push,
            commands::git_commit,
            commands::git_log,
            commands::git_commit_changes,
            commands::git_stash,
            commands::git_stash_pop,
            commands::gitlab_list_projects,
            commands::gitlab_list_branches,
            commands::gitlab_clone,
            commands::common_script_names,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
