use eframe::egui;
use microtermi_core::{load_env, scan_projects, run_script_captured, Environment, Project};
use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::sync::mpsc;

/// Pestaña principal de la aplicación.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum MainTab {
    Settings,
    Projects,
    Git,
    MultiRun,
    Coverage,
}

/// Una pestaña de terminal: nombre, líneas de salida y proceso (si sigue corriendo).
/// Si pending_project/pending_script están presentes y no hay child, es un panel placeholder.
pub(crate) struct TerminalSession {
    pub(crate) name: String,
    pub(crate) lines: Vec<String>,
    pub(crate) child: Option<std::process::Child>,
    pub(crate) receiver: Option<mpsc::Receiver<String>>,
    /// Índice de proyecto para placeholder (panel vacío donde elegir proyecto + script y ejecutar).
    pub(crate) pending_project: Option<usize>,
    /// Script elegido para placeholder.
    pub(crate) pending_script: Option<String>,
}

pub struct MicrotermiApp {
    pub(crate) root_path: Option<PathBuf>,
    pub(crate) projects: Vec<Project>,
    pub(crate) selected_project: Option<usize>,
    pub(crate) run_all_script: String,
    pub(crate) run_mode_parallel: bool,
    pub(crate) environment: Environment,
    pub(crate) message: String,
    pub(crate) git_branch: Option<String>,
    pub(crate) git_clean: Option<bool>,
    pub(crate) git_modified: Vec<String>,
    pub(crate) pending_run: Option<(usize, String)>,
    /// Env vars for current environment (editable)
    pub(crate) env_vars: HashMap<String, String>,
    /// New env key (for add row)
    pub(crate) env_new_key: String,
    pub(crate) env_new_val: String,
    pub(crate) commit_message: String,
    pub(crate) env_needs_refresh: bool,
    /// Una pestaña por proceso; cada una con su salida y su proceso (si sigue corriendo).
    pub(crate) terminal_sessions: Vec<TerminalSession>,
    /// Índice de la pestaña de terminal seleccionada.
    pub(crate) selected_terminal_tab: usize,
    /// GitLab: URL y token (guardados en config).
    pub(crate) gitlab_url: String,
    pub(crate) gitlab_token: String,
    /// Proyectos listados desde GitLab API.
    pub(crate) gitlab_projects: Vec<microtermi_core::GitLabProject>,
    /// Ramas del proyecto GitLab seleccionado.
    pub(crate) gitlab_branches: Vec<microtermi_core::GitLabBranch>,
    /// Índice del proyecto GitLab seleccionado (para ver ramas / clonar).
    pub(crate) selected_gitlab_project: Option<usize>,
    /// Mensaje de error o estado de GitLab (ej. "Conectando...").
    pub(crate) gitlab_status: String,
    /// Ramas locales del repo abierto (para selector en panel Git).
    pub(crate) git_local_branches: Vec<String>,
    /// Rama seleccionada en el dropdown (para cambiar rama).
    pub(crate) selected_git_branch: String,
    /// Pestaña principal seleccionada.
    pub(crate) main_tab: MainTab,
    /// Texto que el usuario escribe en el filtro (repos GitLab).
    pub(crate) gitlab_repo_filter: String,
    /// Filtro aplicado: se actualiza al pulsar Enter o «Buscar».
    pub(crate) gitlab_filter_applied: String,
    /// True mientras se está listando proyectos (petición en segundo plano).
    pub(crate) gitlab_loading: bool,
    /// Receptor del resultado de list_projects en segundo plano.
    pub(crate) gitlab_receiver: Option<mpsc::Receiver<Result<Vec<microtermi_core::GitLabProject>, microtermi_core::GitLabError>>>,
    /// Historial de commits del repo local (log).
    pub(crate) git_log: Vec<microtermi_core::CommitInfo>,
    /// Índice del commit seleccionado en el historial (para ver detalle).
    pub(crate) git_log_selected: Option<usize>,
    /// Archivos cambiados en el commit seleccionado.
    pub(crate) git_commit_detail: Vec<microtermi_core::CommitFileChange>,
    /// Carpeta usada para Git (Pull/Push/Commit). Si es None, se usa la carpeta raíz.
    pub(crate) git_repo_path: Option<PathBuf>,
    /// Git del proyecto seleccionado en Projects (rama, modificados, ramas locales, log).
    pub(crate) project_git_branch: Option<String>,
    pub(crate) project_git_clean: Option<bool>,
    pub(crate) project_git_modified: Vec<String>,
    pub(crate) project_git_local_branches: Vec<String>,
    pub(crate) project_git_remote_branches: Vec<String>,
    pub(crate) project_git_selected_branch: String,
    pub(crate) project_git_selected_remote_branch: String,
    pub(crate) project_git_log: Vec<microtermi_core::CommitInfo>,
    pub(crate) project_git_log_selected: Option<usize>,
    pub(crate) project_git_commit_detail: Vec<microtermi_core::CommitFileChange>,
    /// Índice del proyecto para el que se cargó project_git_* (para refrescar al cambiar de proyecto).
    pub(crate) project_git_refreshed_for: Option<usize>,
    /// Multi-run: proyectos seleccionados para "Ejecutar en seleccionados".
    pub(crate) multi_run_selected: HashSet<usize>,
    /// Multi-run: script/comando a ejecutar.
    pub(crate) multi_run_script: String,
    #[allow(dead_code)]
    pub(crate) multi_run_columns: u32,
    /// Coverage: proyecto seleccionado para ver reporte / ejecutar tests.
    pub(crate) coverage_selected_project: Option<usize>,
}

impl Default for MicrotermiApp {
    fn default() -> Self {
        Self {
            root_path: None,
            projects: Vec::new(),
            selected_project: None,
            run_all_script: "dev".to_string(),
            run_mode_parallel: true,
            environment: Environment::Dev,
            message: String::new(),
            git_branch: None,
            git_clean: None,
            git_modified: Vec::new(),
            pending_run: None,
            env_vars: HashMap::new(),
            env_new_key: String::new(),
            env_new_val: String::new(),
            commit_message: String::new(),
            env_needs_refresh: false,
            terminal_sessions: Vec::new(),
            selected_terminal_tab: 0,
            gitlab_url: String::new(),
            gitlab_token: String::new(),
            gitlab_projects: Vec::new(),
            gitlab_branches: Vec::new(),
            selected_gitlab_project: None,
            gitlab_status: String::new(),
            git_local_branches: Vec::new(),
            selected_git_branch: String::new(),
            main_tab: MainTab::Projects,
            gitlab_repo_filter: String::new(),
            gitlab_filter_applied: String::new(),
            gitlab_loading: false,
            gitlab_receiver: None,
            git_log: Vec::new(),
            git_log_selected: None,
            git_commit_detail: Vec::new(),
            git_repo_path: None,
            project_git_branch: None,
            project_git_clean: None,
            project_git_modified: Vec::new(),
            project_git_local_branches: Vec::new(),
            project_git_remote_branches: Vec::new(),
            project_git_selected_branch: String::new(),
            project_git_selected_remote_branch: String::new(),
            project_git_log: Vec::new(),
            project_git_log_selected: None,
            project_git_commit_detail: Vec::new(),
            project_git_refreshed_for: None,
            multi_run_selected: HashSet::new(),
            multi_run_script: "dev".to_string(),
            multi_run_columns: 2,
            coverage_selected_project: None,
        }
    }
}

impl MicrotermiApp {
    pub fn new(_cc: &eframe::CreationContext<'_>) -> Self {
        let mut app = Self::default();
        let config = crate::config::load_config_json();
        if let Some(root) = config.get("last_root").and_then(|v| v.as_str()).map(PathBuf::from) {
            if root.exists() && root.is_dir() {
                app.root_path = Some(root);
                app.refresh_projects();
                app.refresh_git();
                app.refresh_env();
                app.refresh_git_branches();
            }
        }
        if let Some(url) = config.get("gitlab_url").and_then(|v| v.as_str()) {
            app.gitlab_url = url.to_string();
        }
        if let Some(t) = config.get("gitlab_token").and_then(|v| v.as_str()) {
            app.gitlab_token = t.to_string();
        }
        if let Some(s) = config.get("multi_run_script").and_then(|v| v.as_str()) {
            app.multi_run_script = s.to_string();
        }
        if let Some(arr) = config.get("multi_run_selected_paths").and_then(|v| v.as_array()) {
            let paths: std::collections::HashSet<String> = arr
                .iter()
                .filter_map(|v| v.as_str())
                .map(|s| s.to_string())
                .collect();
            for (i, p) in app.projects.iter().enumerate() {
                if paths.contains(&p.path.to_string_lossy().to_string()) {
                    app.multi_run_selected.insert(i);
                }
            }
        }
        if let Some(s) = config.get("run_all_script").and_then(|v| v.as_str()) {
            app.run_all_script = s.to_string();
        }
        if let Some(b) = config.get("run_mode_parallel").and_then(|v| v.as_bool()) {
            app.run_mode_parallel = b;
        }
        if let Some(s) = config.get("environment").and_then(|v| v.as_str()) {
            match s {
                "staging" => app.environment = Environment::Staging,
                "prod" => app.environment = Environment::Prod,
                _ => app.environment = Environment::Dev,
            }
        }
        if let Some(s) = config.get("gitlab_repo_filter").and_then(|v| v.as_str()) {
            app.gitlab_repo_filter = s.to_string();
        }
        if let Some(s) = config.get("main_tab").and_then(|v| v.as_str()) {
            app.main_tab = match s {
                "settings" => MainTab::Settings,
                "git" => MainTab::Git,
                "multi_run" => MainTab::MultiRun,
                "coverage" => MainTab::Coverage,
                _ => MainTab::Projects,
            };
        }
        app
    }

    /// Guarda en disco toda la configuración actual (raíz, GitLab, Multi-run, terminal, pestaña, etc.).
    pub fn persist_app_config(&self) {
        let multi_run_paths: Vec<String> = self
            .projects
            .iter()
            .enumerate()
            .filter(|(i, _)| self.multi_run_selected.contains(i))
            .map(|(_, p)| p.path.to_string_lossy().to_string())
            .collect();
        let main_tab_str = match self.main_tab {
            MainTab::Settings => "settings",
            MainTab::Projects => "projects",
            MainTab::Git => "git",
            MainTab::MultiRun => "multi_run",
            MainTab::Coverage => "coverage",
        };
        let json = serde_json::json!({
            "last_root": self.root_path.as_ref().map(|p| p.to_string_lossy().to_string()).unwrap_or_default(),
            "gitlab_url": self.gitlab_url,
            "gitlab_token": self.gitlab_token,
            "multi_run_script": self.multi_run_script,
            "multi_run_selected_paths": multi_run_paths,
            "run_all_script": self.run_all_script,
            "run_mode_parallel": self.run_mode_parallel,
            "environment": self.environment.as_str(),
            "gitlab_repo_filter": self.gitlab_repo_filter,
            "main_tab": main_tab_str,
        });
        crate::config::save_config_write(&json);
    }

    /// Ruta usada para operaciones Git (repo local). Prioridad: git_repo_path, luego root_path.
    pub(crate) fn git_root(&self) -> Option<PathBuf> {
        self.git_repo_path.clone().or_else(|| self.root_path.clone())
    }

    pub(crate) fn refresh_projects(&mut self) {
        if let Some(ref root) = self.root_path {
            match scan_projects(root) {
                Ok(p) => self.projects = p,
                Err(e) => self.message = format!("Error scanning: {}", e),
            }
        }
    }

    pub(crate) fn refresh_git(&mut self) {
        let root = match self.git_root() {
            Some(p) => p,
            None => return,
        };
        match microtermi_core::open_repo(&root) {
            Ok(repo) => {
                if let Ok(st) = microtermi_core::status(&repo) {
                    self.git_branch = Some(st.branch.clone());
                    self.git_clean = Some(st.is_clean);
                    self.git_modified = st.modified;
                    self.git_modified.extend(st.untracked);
                }
                self.refresh_git_branches();
                self.git_log = microtermi_core::log(&repo, 200).unwrap_or_default();
                self.git_log_selected = None;
                self.git_commit_detail.clear();
            }
            Err(_) => {
                self.git_branch = None;
                self.git_clean = None;
                self.git_modified.clear();
                self.git_local_branches.clear();
                self.git_log.clear();
                self.git_log_selected = None;
                self.git_commit_detail.clear();
            }
        }
    }

    /// Carga el estado Git del proyecto (rama, modificados, ramas locales, log) para la ruta del proyecto.
    pub(crate) fn refresh_project_git(&mut self, project_path: &Path) {
        match microtermi_core::open_repo(project_path) {
            Ok(repo) => {
                if let Ok(st) = microtermi_core::status(&repo) {
                    self.project_git_branch = Some(st.branch.clone());
                    self.project_git_clean = Some(st.is_clean);
                    self.project_git_modified = st.modified;
                    self.project_git_modified.extend(st.untracked);
                } else {
                    self.project_git_branch = None;
                    self.project_git_clean = None;
                    self.project_git_modified.clear();
                }
                self.project_git_local_branches = microtermi_core::branches(&repo).unwrap_or_default();
                self.project_git_remote_branches = microtermi_core::branches_remote(&repo).unwrap_or_default();
                let current = self.project_git_branch.clone().unwrap_or_default();
                if self.project_git_local_branches.contains(&current) {
                    self.project_git_selected_branch = current.clone();
                } else if !self.project_git_local_branches.is_empty() {
                    self.project_git_selected_branch = self.project_git_local_branches[0].clone();
                }
                if self.project_git_remote_branches.contains(&current) {
                    self.project_git_selected_remote_branch = current;
                } else if !self.project_git_remote_branches.is_empty() {
                    self.project_git_selected_remote_branch = self.project_git_remote_branches[0].clone();
                }
                self.project_git_log = microtermi_core::log(&repo, 100).unwrap_or_default();
                self.project_git_log_selected = None;
                self.project_git_commit_detail.clear();
            }
            Err(_) => {
                self.project_git_branch = None;
                self.project_git_clean = None;
                self.project_git_modified.clear();
                self.project_git_local_branches.clear();
                self.project_git_remote_branches.clear();
                self.project_git_log.clear();
                self.project_git_log_selected = None;
                self.project_git_commit_detail.clear();
            }
        }
    }

    pub(crate) fn refresh_git_branches(&mut self) {
        self.git_local_branches.clear();
        let root = match self.git_root() {
            Some(p) => p,
            None => {
                self.selected_git_branch.clear();
                return;
            }
        };
        if let Ok(repo) = microtermi_core::open_repo(&root) {
            if let Ok(branches) = microtermi_core::branches(&repo) {
                self.git_local_branches = branches;
                let current = self.git_branch.clone().unwrap_or_default();
                if self.git_local_branches.contains(&current) {
                    self.selected_git_branch = current;
                } else if !self.git_local_branches.is_empty() {
                    self.selected_git_branch = self.git_local_branches[0].clone();
                }
            }
        }
    }

    pub(crate) fn refresh_env(&mut self) {
        if let Some(ref root) = self.root_path {
            match load_env(root, self.environment) {
                Ok(vars) => self.env_vars = vars,
                Err(_) => self.env_vars.clear(),
            }
        }
    }

    /// Nombres de scripts que tienen en común todos los proyectos (intersección).
    pub(crate) fn common_script_names(&self) -> Vec<String> {
        if self.projects.is_empty() {
            return Vec::new();
        }
        let mut common: HashSet<String> = self.projects[0]
            .scripts
            .iter()
            .map(|(s, _)| s.clone())
            .collect();
        for p in self.projects.iter().skip(1) {
            let set: HashSet<String> = p.scripts.iter().map(|(s, _)| s.clone()).collect();
            common.retain(|s| set.contains(s));
        }
        let mut list: Vec<String> = common.into_iter().collect();
        list.sort();
        list
    }

    fn run_script_click(&mut self, project: &Project, script_name: &str) {
        self.terminal_sessions.clear();
        self.selected_terminal_tab = 0;
        let header = crate::ansi::strip_ansi(&format!(
            "> {} » {}",
            project.name,
            match microtermi_core::detect_package_manager(&project.path) {
                microtermi_core::PackageManager::Npm => format!("npm run {}", script_name),
                microtermi_core::PackageManager::Yarn => format!("yarn {}", script_name),
                microtermi_core::PackageManager::Pnpm => format!("pnpm {}", script_name),
            }
        ));
        match run_script_captured(project, script_name, &self.env_vars) {
            Ok((child, receiver)) => {
                self.terminal_sessions.push(TerminalSession {
                    name: format!("{} » {}", project.name, script_name),
                    lines: vec![header],
                    child: Some(child),
                    receiver: Some(receiver),
                    pending_project: None,
                    pending_script: None,
                });
                self.message = format!("Ejecutando {} en {}", script_name, project.name);
            }
            Err(e) => {
                self.terminal_sessions.push(TerminalSession {
                    name: format!("{} » {}", project.name, script_name),
                    lines: vec![header, crate::ansi::strip_ansi(&format!("[error] {}", e))],
                    child: None,
                    receiver: None,
                    pending_project: None,
                    pending_script: None,
                });
                self.message = format!("Error: {}", e);
            }
        }
    }

    fn terminal_drain(&mut self) {
        for session in self.terminal_sessions.iter_mut() {
            if let Some(ref rx) = session.receiver {
                while let Ok(line) = rx.try_recv() {
                    session.lines.push(line);
                }
            }
            if let Some(ref mut child) = session.child {
                if child.try_wait().ok().flatten().is_some() {
                    session.lines.push("[proceso terminado]".to_string());
                    session.child = None;
                    session.receiver = None;
                }
            }
        }
    }

    /// Detiene el proceso de la pestaña actual (si tiene uno en ejecución).
    fn terminal_stop_current(&mut self) {
        if self.selected_terminal_tab < self.terminal_sessions.len() {
            self.terminal_stop_at(self.selected_terminal_tab);
        }
    }

    /// Detiene el proceso de la sesión en el índice dado.
    pub(crate) fn terminal_stop_at(&mut self, index: usize) {
        if index >= self.terminal_sessions.len() {
            return;
        }
        let session = &mut self.terminal_sessions[index];
        if let Some(mut child) = session.child.take() {
            session.receiver = None;
            let _ = child.kill();
            session.lines.push("[proceso detenido]".to_string());
        }
    }

    /// Detiene todos los procesos de todas las pestañas.
    pub(crate) fn terminal_stop_all(&mut self) {
        for session in self.terminal_sessions.iter_mut() {
            if let Some(mut child) = session.child.take() {
                session.receiver = None;
                let _ = child.kill();
                session.lines.push("[proceso detenido]".to_string());
            }
        }
    }

    /// Cierra la pestaña de terminal en el índice dado.
    fn terminal_close_tab(&mut self, index: usize) {
        if index < self.terminal_sessions.len() {
            let mut session = self.terminal_sessions.remove(index);
            if let Some(mut child) = session.child.take() {
                let _ = child.kill();
            }
            if self.selected_terminal_tab >= self.terminal_sessions.len() && !self.terminal_sessions.is_empty() {
                self.selected_terminal_tab = self.terminal_sessions.len() - 1;
            } else if self.selected_terminal_tab > index {
                self.selected_terminal_tab -= 1;
            }
        }
    }

    pub(crate) fn run_all_click(&mut self) {
        let script = self.run_all_script.trim();
        if script.is_empty() {
            self.message = "Escribe el nombre del script (ej. dev, start).".to_string();
            return;
        }
        let projects: Vec<_> = self
            .projects
            .iter()
            .filter(|p| p.scripts.iter().any(|(s, _)| s == script))
            .cloned()
            .collect();
        if projects.is_empty() {
            self.message = format!("Ningún proyecto tiene el script \"{}\".", script);
            return;
        }
        self.terminal_sessions.clear();
        self.selected_terminal_tab = 0;
        let script_owned = script.to_string();
        let mut started = 0;
        for project in &projects {
            let proj_idx = self.projects.iter().position(|p| p.path == project.path).unwrap_or(0);
            let header = format!(
                "> {} » {}",
                project.name,
                match microtermi_core::detect_package_manager(&project.path) {
                    microtermi_core::PackageManager::Npm => format!("npm run {}", script),
                    microtermi_core::PackageManager::Yarn => format!("yarn {}", script),
                    microtermi_core::PackageManager::Pnpm => format!("pnpm {}", script),
                }
            );
            match run_script_captured(project, script, &self.env_vars) {
                Ok((child, receiver)) => {
                    self.terminal_sessions.push(TerminalSession {
                        name: format!("{} » {}", project.name, script),
                        lines: vec![header],
                        child: Some(child),
                        receiver: Some(receiver),
                        pending_project: Some(proj_idx),
                        pending_script: Some(script_owned.clone()),
                    });
                    started += 1;
                }
                Err(e) => {
                    self.terminal_sessions.push(TerminalSession {
                        name: format!("{} » {}", project.name, script),
                        lines: vec![header, crate::ansi::strip_ansi(&format!("[error] {}", e))],
                        child: None,
                        receiver: None,
                        pending_project: Some(proj_idx),
                        pending_script: Some(script_owned.clone()),
                    });
                }
            }
        }
        self.message = format!("Ejecutando {} en {} proyecto(s)", script, started);
    }

    /// Ejecuta el script en los proyectos seleccionados (Multi-run). Crea una sesión por proyecto.
    pub(crate) fn multi_run_click(&mut self) {
        let script = self.multi_run_script.trim();
        if script.is_empty() {
            self.message = "Escribe el nombre del script (ej. dev, start).".to_string();
            return;
        }
        let selected: Vec<_> = self
            .multi_run_selected
            .iter()
            .copied()
            .filter(|&i| {
                self.projects.get(i).map_or(false, |p| {
                    p.scripts.iter().any(|(s, _)| s == script)
                })
            })
            .collect();
        if selected.is_empty() {
            self.message = "Selecciona al menos un proyecto que tenga ese script.".to_string();
            return;
        }
        let first_new_tab = self.terminal_sessions.len();
        let mut started = 0;
        for &idx in &selected {
            let project = match self.projects.get(idx) {
                Some(p) => p.clone(),
                None => continue,
            };
            let header = format!(
                "> {} » {}",
                project.name,
                match microtermi_core::detect_package_manager(&project.path) {
                    microtermi_core::PackageManager::Npm => format!("npm run {}", script),
                    microtermi_core::PackageManager::Yarn => format!("yarn {}", script),
                    microtermi_core::PackageManager::Pnpm => format!("pnpm {}", script),
                }
            );
            let script_owned = script.to_string();
            match run_script_captured(&project, script, &self.env_vars) {
                Ok((child, receiver)) => {
                    self.terminal_sessions.push(TerminalSession {
                        name: format!("{} » {}", project.name, script),
                        lines: vec![header],
                        child: Some(child),
                        receiver: Some(receiver),
                        pending_project: Some(idx),
                        pending_script: Some(script_owned),
                    });
                    started += 1;
                }
                Err(e) => {
                    self.terminal_sessions.push(TerminalSession {
                        name: format!("{} » {}", project.name, script),
                        lines: vec![header, crate::ansi::strip_ansi(&format!("[error] {}", e))],
                        child: None,
                        receiver: None,
                        pending_project: Some(idx),
                        pending_script: Some(script_owned),
                    });
                }
            }
        }
        if started > 0 {
            self.selected_terminal_tab = first_new_tab;
        }
        self.message = format!("Ejecutando {} en {} proyecto(s)", script, started);
    }

    /// Añade un panel placeholder en Multi-run (para luego elegir proyecto + script y ejecutar).
    pub(crate) fn multi_run_add_placeholder(&mut self) {
        self.terminal_sessions.push(TerminalSession {
            name: "Nuevo…".to_string(),
            lines: Vec::new(),
            child: None,
            receiver: None,
            pending_project: None,
            pending_script: None,
        });
        self.selected_terminal_tab = self.terminal_sessions.len() - 1;
    }

    /// Ejecuta el script en el placeholder en el índice dado. Requiere pending_project y pending_script.
    pub(crate) fn multi_run_placeholder_execute(&mut self, index: usize) {
        if index >= self.terminal_sessions.len() {
            return;
        }
        let (proj_idx, script_name) = match (
            self.terminal_sessions[index].pending_project,
            self.terminal_sessions[index].pending_script.clone(),
        ) {
            (Some(pi), Some(s)) if !s.trim().is_empty() => (pi, s.trim().to_string()),
            _ => {
                self.message = "Elige proyecto y script en el panel.".to_string();
                return;
            }
        };
        let project = match self.projects.get(proj_idx) {
            Some(p) => p.clone(),
            None => {
                self.message = "Proyecto no encontrado.".to_string();
                return;
            }
        };
        if !project.scripts.iter().any(|(s, _)| s == &script_name) {
            self.message = format!("El proyecto no tiene el script \"{}\".", script_name);
            return;
        }
        let header = format!(
            "> {} » {}",
            project.name,
            match microtermi_core::detect_package_manager(&project.path) {
                microtermi_core::PackageManager::Npm => format!("npm run {}", script_name),
                microtermi_core::PackageManager::Yarn => format!("yarn {}", script_name),
                microtermi_core::PackageManager::Pnpm => format!("pnpm {}", script_name),
            }
        );
        match run_script_captured(&project, &script_name, &self.env_vars) {
            Ok((child, receiver)) => {
                self.terminal_sessions[index] = TerminalSession {
                    name: format!("{} » {}", project.name, script_name),
                    lines: vec![header],
                    child: Some(child),
                    receiver: Some(receiver),
                    pending_project: Some(proj_idx),
                    pending_script: Some(script_name.clone()),
                };
                self.message = format!("Ejecutando {} en {}", script_name, project.name);
            }
            Err(e) => {
                self.terminal_sessions[index].lines = vec![header, crate::ansi::strip_ansi(&format!("[error] {}", e))];
                self.terminal_sessions[index].pending_project = Some(proj_idx);
                self.terminal_sessions[index].pending_script = Some(script_name);
                self.message = format!("Error: {}", e);
            }
        }
    }

    /// Cierra el panel de Multi-run en el índice de sesión dado y actualiza el árbol.
    pub(crate) fn multi_run_close_pane(&mut self, session_idx: usize) {
        if session_idx >= self.terminal_sessions.len() {
            return;
        }
        let session = &mut self.terminal_sessions[session_idx];
        if let Some(mut child) = session.child.take() {
            session.receiver = None;
            let _ = child.kill();
        }
        self.terminal_sessions.remove(session_idx);
        if self.selected_terminal_tab >= self.terminal_sessions.len() && !self.terminal_sessions.is_empty() {
            self.selected_terminal_tab = self.terminal_sessions.len() - 1;
        } else if self.selected_terminal_tab > session_idx {
            self.selected_terminal_tab = self.selected_terminal_tab.saturating_sub(1);
        }
    }

    /// Ejecuta npm run test para el proyecto indicado (pestaña Coverage). Añade una sesión de terminal.
    pub(crate) fn coverage_run_tests_for_project(&mut self, project_idx: usize) {
        let project = match self.projects.get(project_idx) {
            Some(p) => p.clone(),
            None => return,
        };
        if !project.scripts.iter().any(|(s, _)| s == "test") {
            self.message = "El proyecto no tiene script \"test\".".to_string();
            return;
        }
        let header = format!(
            "> {} » {}",
            project.name,
            match microtermi_core::detect_package_manager(&project.path) {
                microtermi_core::PackageManager::Npm => "npm run test".to_string(),
                microtermi_core::PackageManager::Yarn => "yarn test".to_string(),
                microtermi_core::PackageManager::Pnpm => "pnpm test".to_string(),
            }
        );
        match run_script_captured(&project, "test", &self.env_vars) {
            Ok((child, receiver)) => {
                self.terminal_sessions.push(TerminalSession {
                    name: format!("{} » test", project.name),
                    lines: vec![header],
                    child: Some(child),
                    receiver: Some(receiver),
                    pending_project: Some(project_idx),
                    pending_script: Some("test".to_string()),
                });
                self.selected_terminal_tab = self.terminal_sessions.len() - 1;
                self.main_tab = MainTab::MultiRun;
                self.message = format!("Tests de {} en ejecución. Ve a Multi-run para ver la salida.", project.name);
            }
            Err(e) => {
                self.message = format!("Error al ejecutar tests: {}", e);
            }
        }
    }

    /// Dibuja el contenido de una sesión (pestaña) de Multi-run: placeholder o terminal. Devuelve acciones.
    pub(crate) fn draw_multi_run_session_content(
        &mut self,
        ui: &mut egui::Ui,
        idx: usize,
        font_id: &egui::FontId,
    ) -> (Option<usize>, Option<usize>, Option<usize>) {
        let mut close_tab = None;
        let mut run_placeholder = None;
        let mut stop_at = None;
        if idx >= self.terminal_sessions.len() {
            return (close_tab, run_placeholder, stop_at);
        }
        let session = &mut self.terminal_sessions[idx];
        let no_process = session.child.is_none() && session.receiver.is_none();
        let has_pending = session.pending_project.is_some()
            && session.pending_script.as_ref().map_or(false, |s| !s.trim().is_empty());
        let is_placeholder = no_process && !has_pending;
        let can_run_again = no_process && has_pending;
        egui::Frame::group(ui.style()).inner_margin(6.0).show(ui, |ui| {
            if is_placeholder {
                ui.label(egui::RichText::new("Nuevo terminal").small());
                let proj_count = self.projects.len();
                let sel_proj = session.pending_project.unwrap_or(0).min(proj_count.saturating_sub(1));
                egui::ComboBox::from_id_salt(("multi_proj", idx))
                    .selected_text(self.projects.get(sel_proj).map(|p| p.name.as_str()).unwrap_or("—"))
                    .show_ui(ui, |ui| {
                        for (i, p) in self.projects.iter().enumerate() {
                            if ui.selectable_label(session.pending_project == Some(i), &p.name).clicked() {
                                session.pending_project = Some(i);
                            }
                        }
                    });
                if let Some(pi) = session.pending_project {
                    if let Some(proj) = self.projects.get(pi) {
                        let script_names: Vec<String> = proj.scripts.iter().map(|(s, _)| s.clone()).collect();
                        let cur = session.pending_script.as_deref().unwrap_or("").to_string();
                        let sel = script_names.iter().position(|s| s.as_str() == cur).unwrap_or(0);
                        egui::ComboBox::from_id_salt(("multi_script", idx))
                            .selected_text(script_names.get(sel).map(String::as_str).unwrap_or("—"))
                            .show_ui(ui, |ui| {
                                for (_i, name) in script_names.iter().enumerate() {
                                    if ui.selectable_label(session.pending_script.as_deref() == Some(name.as_str()), name).clicked() {
                                        session.pending_script = Some(name.clone());
                                    }
                                }
                            });
                    }
                }
                if ui.button("Ejecutar").clicked() {
                    run_placeholder = Some(idx);
                }
            } else {
                ui.horizontal(|ui| {
                    ui.label(egui::RichText::new(&session.name).small().strong());
                    if session.child.is_some() && ui.small_button("Detener").clicked() {
                        stop_at = Some(idx);
                    }
                    if can_run_again && ui.small_button("Ejecutar de nuevo").clicked() {
                        run_placeholder = Some(idx);
                    }
                    if ui.small_button("Limpiar").clicked() {
                        session.lines.clear();
                    }
                    if ui.small_button("✕").clicked() {
                        close_tab = Some(idx);
                    }
                });
                egui::ScrollArea::vertical()
                    .stick_to_bottom(true)
                    .auto_shrink([false; 2])
                    .show(ui, |ui| {
                        let default_color = ui.visuals().text_color();
                        for line in &session.lines {
                            ui.horizontal_wrapped(|ui| {
                                for seg in crate::ansi::parse_ansi_line(line) {
                                    let color = seg.color.unwrap_or(default_color);
                                    let mut rt = egui::RichText::new(seg.text)
                                        .font(font_id.clone())
                                        .color(color);
                                    if seg.bold {
                                        rt = rt.strong();
                                    }
                                    ui.label(rt);
                                }
                            });
                        }
                    });
            }
        });
        (close_tab, run_placeholder, stop_at)
    }

    pub(crate) fn gitlab_list_projects(&mut self) {
        let url = self.gitlab_url.trim().to_string();
        let token = self.gitlab_token.clone();
        if url.is_empty() || token.is_empty() {
            self.gitlab_status = "Indica URL y token.".to_string();
            return;
        }
        let search = self.gitlab_repo_filter.trim();
        let search_opt = if search.is_empty() { None } else { Some(search.to_string()) };
        let (tx, rx) = mpsc::channel();
        std::thread::spawn(move || {
            let result = microtermi_core::list_projects(&url, &token, search_opt.as_deref());
            let _ = tx.send(result);
        });
        self.gitlab_loading = true;
        self.gitlab_filter_applied = self.gitlab_repo_filter.trim().to_string();
        self.gitlab_status = "Buscando…".to_string();
        self.gitlab_receiver = Some(rx);
        self.persist_app_config();
    }

    pub(crate) fn gitlab_select_project(&mut self, index: usize) {
        self.selected_gitlab_project = Some(index);
        self.gitlab_branches.clear();
        let url = self.gitlab_url.trim().to_string();
        let token = self.gitlab_token.clone();
        if let Some(proj) = self.gitlab_projects.get(index) {
            match microtermi_core::list_branches(&url, &token, proj.id) {
                Ok(branches) => self.gitlab_branches = branches,
                Err(e) => self.gitlab_status = format!("Ramas: {}", e),
            }
        }
    }

    pub(crate) fn gitlab_clone(&mut self, index: usize) {
        if index >= self.gitlab_projects.len() {
            return;
        }
        let proj = &self.gitlab_projects[index];
        let url = microtermi_core::clone_url_with_token(&proj.http_url_to_repo, &self.gitlab_token);
        if let Some(dir) = rfd::FileDialog::new().set_title("Carpeta donde clonar").pick_folder() {
            let dest = dir.join(proj.path_with_namespace.replace('/', std::path::MAIN_SEPARATOR_STR));
            self.gitlab_status = "Clonando…".to_string();
            match microtermi_core::clone_repo(&url, &dest) {
                Ok(_) => {
                    self.gitlab_status = format!("Clonado en {}", dest.display());
                    self.message = format!("Repositorio clonado en {}", dest.display());
                }
                Err(e) => {
                    self.gitlab_status = format!("Error al clonar: {}", e);
                    self.message = format!("Error: {}", e);
                }
            }
        }
    }

    /// Clona el proyecto GitLab seleccionado en la carpeta raíz (como subcarpeta).
    pub(crate) fn gitlab_clone_to_root(&mut self, index: usize) {
        if index >= self.gitlab_projects.len() {
            return;
        }
        let root = match &self.root_path {
            Some(p) => p.clone(),
            None => {
                self.gitlab_status = "Selecciona primero la carpeta raíz en Settings o Projects.".to_string();
                return;
            }
        };
        let proj = &self.gitlab_projects[index];
        let url = microtermi_core::clone_url_with_token(&proj.http_url_to_repo, &self.gitlab_token);
        let dest = root.join(proj.path_with_namespace.replace('/', std::path::MAIN_SEPARATOR_STR));
        self.gitlab_status = "Clonando…".to_string();
        match microtermi_core::clone_repo(&url, &dest) {
            Ok(_) => {
                self.git_repo_path = Some(dest.clone());
                self.gitlab_status = format!("Clonado en {}", dest.display());
                self.message = format!("Repositorio clonado. Pull/Push/Commit usarán esta carpeta.");
                self.refresh_projects();
                self.refresh_git();
            }
            Err(e) => {
                self.gitlab_status = format!("Error al clonar: {}", e);
                self.message = format!("Error: {}", e);
            }
        }
    }

    /// Agrupa proyectos GitLab por el primer segmento del path (grupo). La lista ya viene filtrada por la API si se usó búsqueda.
    pub(crate) fn gitlab_projects_grouped(&self) -> std::collections::BTreeMap<String, Vec<(usize, &microtermi_core::GitLabProject)>> {
        let mut map: std::collections::BTreeMap<String, Vec<(usize, &microtermi_core::GitLabProject)>> = std::collections::BTreeMap::new();
        for (i, proj) in self.gitlab_projects.iter().enumerate() {
            let group = proj.path_with_namespace.split('/').next().unwrap_or("").to_string();
            map.entry(group).or_default().push((i, proj));
        }
        map
    }

    pub(crate) fn git_checkout_branch(&mut self) {
        if self.selected_git_branch.is_empty() {
            return;
        }
        let root = match self.git_root() {
            Some(p) => p,
            None => return,
        };
        if let Ok(repo) = microtermi_core::open_repo(&root) {
            match microtermi_core::checkout_branch(&repo, &self.selected_git_branch) {
                Ok(()) => {
                    self.message = format!("Rama cambiada a {}", self.selected_git_branch);
                    self.refresh_git();
                }
                Err(e) => self.message = format!("Error: {}", e),
            }
        }
    }
}

impl eframe::App for MicrotermiApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        if let Some((idx, script_name)) = self.pending_run.take() {
            if let Some(project) = self.projects.get(idx).cloned() {
                self.run_script_click(&project, &script_name);
            }
        }
        if self.env_needs_refresh {
            self.refresh_env();
            self.env_needs_refresh = false;
        }
        self.terminal_drain();

        if let Some(rx) = &mut self.gitlab_receiver {
            if let Ok(result) = rx.try_recv() {
                match result {
                    Ok(projects) => {
                        self.gitlab_projects = projects;
                        self.gitlab_status = format!("{} proyecto(s) listados.", self.gitlab_projects.len());
                        self.persist_app_config();
                    }
                    Err(e) => self.gitlab_status = format!("Error: {}", e),
                }
                self.gitlab_loading = false;
                self.gitlab_receiver = None;
            }
        }

        // Barra superior: pestañas + en Projects el selector de carpeta
        egui::TopBottomPanel::top("top").show(ctx, |ui| {
            ui.horizontal(|ui| {
                ui.heading("Microtermi");
                ui.separator();
                if ui.selectable_label(self.main_tab == MainTab::Settings, "Settings").clicked() {
                    self.main_tab = MainTab::Settings;
                    self.persist_app_config();
                }
                if ui.selectable_label(self.main_tab == MainTab::Projects, "Projects").clicked() {
                    self.main_tab = MainTab::Projects;
                    self.persist_app_config();
                }
                if ui.selectable_label(self.main_tab == MainTab::Git, "Git").clicked() {
                    self.main_tab = MainTab::Git;
                    self.persist_app_config();
                }
                if ui.selectable_label(self.main_tab == MainTab::MultiRun, "Multi-run").clicked() {
                    self.main_tab = MainTab::MultiRun;
                    self.persist_app_config();
                    let common = self.common_script_names();
                    if !common.contains(&self.multi_run_script) && !common.is_empty() {
                        self.multi_run_script = common.first().cloned().unwrap_or_default();
                    }
                }
                if ui.selectable_label(self.main_tab == MainTab::Coverage, "Coverage").clicked() {
                    self.main_tab = MainTab::Coverage;
                    self.persist_app_config();
                }
                ui.separator();
                if self.main_tab == MainTab::Projects || self.main_tab == MainTab::MultiRun {
                    if ui.button("Seleccionar carpeta raíz").clicked() {
                        if let Some(path) = rfd::FileDialog::new().pick_folder() {
                            self.root_path = Some(path.clone());
                            self.persist_app_config();
                            self.refresh_git_branches();
                            self.refresh_projects();
                            self.refresh_git();
                            self.refresh_env();
                        }
                    }
                    if let Some(ref p) = self.root_path {
                        ui.label(egui::RichText::new(p.display().to_string()).color(ui.visuals().weak_text_color()));
                    }
                }
            });
        });

        match self.main_tab {
            MainTab::Settings => crate::tabs::draw_settings(self, ctx),
            MainTab::Projects => crate::tabs::draw_projects(self, ctx),
            MainTab::Git => crate::tabs::draw_git(self, ctx),
            MainTab::MultiRun => crate::tabs::draw_multi_run(self, ctx),
            MainTab::Coverage => crate::tabs::draw_coverage(self, ctx),
        }

        if self.main_tab == MainTab::Projects {
        egui::TopBottomPanel::bottom("terminal")
            .min_height(160.0)
            .resizable(true)
            .default_height(320.0)
            .show(ctx, |ui| {
                ui.horizontal(|ui| {
                    ui.label(egui::RichText::new("Terminal").strong());
                    if self.terminal_sessions.is_empty() {
                        ui.label(egui::RichText::new("(sin pestañas)").color(ui.visuals().weak_text_color()));
                    } else {
                        let n = self.terminal_sessions.len();
                        if self.selected_terminal_tab < n && self.terminal_sessions[self.selected_terminal_tab].child.is_some() {
                            if ui.button("Detener pestaña").clicked() {
                                self.terminal_stop_current();
                            }
                        }
                        if n > 1 {
                            let any_running = self.terminal_sessions.iter().any(|s| s.child.is_some());
                            if any_running && ui.button("Detener todos").clicked() {
                                self.terminal_stop_all();
                            }
                        }
                        if self.selected_terminal_tab < n && ui.button("Limpiar").clicked() {
                            self.terminal_sessions[self.selected_terminal_tab].lines.clear();
                        }
                    }
                });
                if !self.terminal_sessions.is_empty() {
                    ui.separator();
                    // Pestañas
                    ui.horizontal(|ui| {
                        let mut close_tab = None;
                        for (i, session) in self.terminal_sessions.iter().enumerate() {
                            let running = session.child.is_some();
                            let label = if running {
                                format!("● {}", session.name)
                            } else {
                                session.name.clone()
                            };
                            let selected = self.selected_terminal_tab == i;
                            if ui.selectable_label(selected, &label).clicked() {
                                self.selected_terminal_tab = i;
                            }
                            if ui.small_button("✕").clicked() {
                                close_tab = Some(i);
                            }
                        }
                        if let Some(i) = close_tab {
                            self.terminal_close_tab(i);
                        }
                    });
                    ui.add_space(2.0);
                    if !self.terminal_sessions.is_empty() {
                        let font_id = egui::FontId::monospace(12.0);
                        let idx = self.selected_terminal_tab.min(self.terminal_sessions.len() - 1);
                        let lines = &self.terminal_sessions[idx].lines;
                        egui::ScrollArea::vertical()
                            .stick_to_bottom(true)
                            .auto_shrink([false; 2])
                            .show(ui, |ui| {
                                let default_color = ui.visuals().text_color();
                                for line in lines {
                                    ui.horizontal_wrapped(|ui| {
                                        for seg in crate::ansi::parse_ansi_line(line) {
                                            let color = seg.color.unwrap_or(default_color);
                                            let mut rt = egui::RichText::new(seg.text)
                                                .font(font_id.clone())
                                                .color(color);
                                            if seg.bold {
                                                rt = rt.strong();
                                            }
                                            ui.label(rt);
                                        }
                                    });
                                }
                            });
                    }
                }
            });
        }
    }
}
