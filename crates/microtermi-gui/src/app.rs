use eframe::egui;
use microtermi_core::{load_env, save_env, scan_projects, run_script_captured, Environment, Project};
use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::sync::mpsc;

fn config_path() -> Option<PathBuf> {
    dirs::config_dir().map(|d| d.join("microtermi").join("config.json"))
}

/// Carga el JSON completo de configuración. Si no existe o falla, devuelve un objeto vacío.
fn load_config_json() -> serde_json::Value {
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

fn save_config_write(json: &serde_json::Value) {
    if let Some(path) = config_path() {
        if let Some(parent) = path.parent() {
            let _ = std::fs::create_dir_all(parent);
        }
        let _ = std::fs::write(path, json.to_string());
    }
}

/// Elimina códigos de escape ANSI sin corromper UTF-8: trabaja en bytes y mantiene secuencias multibyte.
fn strip_ansi(s: &str) -> String {
    let mut out: Vec<u8> = Vec::with_capacity(s.len());
    let mut bytes = s.bytes().peekable();
    while let Some(b) = bytes.next() {
        if b != 0x1b && b != 0x9b {
            out.push(b);
            continue;
        }
        if b == 0x9b {
            // CSI (ESC [)
        } else if bytes.peek() != Some(&b'[') && bytes.peek() != Some(&b']') && bytes.peek() != Some(&b'?') {
            out.push(b);
            continue;
        } else {
            let _ = bytes.next();
        }
        while let Some(&n) = bytes.peek() {
            if (0x30..=0x3f).contains(&n) || (0x20..=0x2f).contains(&n) {
                let _ = bytes.next();
            } else if (0x40..=0x7e).contains(&n) {
                let _ = bytes.next();
                break;
            } else {
                break;
            }
        }
    }
    String::from_utf8_lossy(&out).to_string()
}

/// Segmento de una línea con posible color ANSI aplicado.
struct AnsiSegment {
    text: String,
    color: Option<egui::Color32>,
    bold: bool,
}

/// Parsea una línea que puede contener códigos ANSI SGR y devuelve segmentos para pintar con color.
fn parse_ansi_line(s: &str) -> Vec<AnsiSegment> {
    let mut out: Vec<AnsiSegment> = Vec::new();
    let mut current: Vec<u8> = Vec::new();
    let mut color: Option<egui::Color32> = None;
    let mut bold = false;
    let mut bytes = s.bytes().peekable();

    let flush = |cur: &mut Vec<u8>, segs: &mut Vec<AnsiSegment>, col: Option<egui::Color32>, b: bool| {
        if !cur.is_empty() {
            segs.push(AnsiSegment {
                text: String::from_utf8_lossy(cur).to_string(),
                color: col,
                bold: b,
            });
            cur.clear();
        }
    };

    while let Some(b) = bytes.next() {
        if b != 0x1b && b != 0x9b {
            current.push(b);
            continue;
        }
        if b == 0x9b {
            // CSI
        } else if bytes.peek() != Some(&b'[') {
            current.push(b);
            continue;
        } else {
            let _ = bytes.next();
        }
        let mut params: Vec<u8> = Vec::new();
        while let Some(&n) = bytes.peek() {
            if (0x30..=0x3f).contains(&n) {
                let _ = bytes.next();
                params.push(n);
            } else if (0x20..=0x2f).contains(&n) {
                let _ = bytes.next();
            } else if (0x40..=0x7e).contains(&n) {
                let _ = bytes.next();
                let c = n as char;
                if c == 'm' {
                    flush(&mut current, &mut out, color, bold);
                    let s = String::from_utf8_lossy(&params);
                    for part in s.split(';') {
                        let n: u8 = part.trim().parse().unwrap_or(0);
                        match n {
                            0 => {
                                color = None;
                                bold = false;
                            }
                            1 => bold = true,
                            30 => color = Some(egui::Color32::from_rgb(0, 0, 0)),
                            31 => color = Some(egui::Color32::from_rgb(205, 49, 49)),
                            32 => color = Some(egui::Color32::from_rgb(13, 188, 121)),
                            33 => color = Some(egui::Color32::from_rgb(229, 229, 16)),
                            34 => color = Some(egui::Color32::from_rgb(36, 114, 200)),
                            35 => color = Some(egui::Color32::from_rgb(188, 63, 188)),
                            36 => color = Some(egui::Color32::from_rgb(17, 168, 205)),
                            37 => color = Some(egui::Color32::from_rgb(229, 229, 229)),
                            90..=97 => {
                                let i = (n - 90) as usize;
                                let bright: [egui::Color32; 8] = [
                                    egui::Color32::from_rgb(102, 102, 102),
                                    egui::Color32::from_rgb(241, 76, 76),
                                    egui::Color32::from_rgb(35, 209, 139),
                                    egui::Color32::from_rgb(245, 245, 67),
                                    egui::Color32::from_rgb(59, 142, 234),
                                    egui::Color32::from_rgb(214, 112, 214),
                                    egui::Color32::from_rgb(41, 184, 219),
                                    egui::Color32::from_rgb(255, 255, 255),
                                ];
                                color = Some(bright[i]);
                            }
                            _ => {}
                        }
                    }
                }
                break;
            } else {
                break;
            }
        }
    }
    flush(&mut current, &mut out, color, bold);
    out
}

/// Pestaña principal de la aplicación.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum MainTab {
    Settings,
    Projects,
    Git,
    MultiRun,
    Coverage,
}

/// Una pestaña de terminal: nombre, líneas de salida y proceso (si sigue corriendo).
/// Si pending_project/pending_script están presentes y no hay child, es un panel placeholder.
struct TerminalSession {
    name: String,
    lines: Vec<String>,
    child: Option<std::process::Child>,
    receiver: Option<mpsc::Receiver<String>>,
    /// Índice de proyecto para placeholder (panel vacío donde elegir proyecto + script y ejecutar).
    pending_project: Option<usize>,
    /// Script elegido para placeholder.
    pending_script: Option<String>,
}

pub struct MicrotermiApp {
    root_path: Option<PathBuf>,
    projects: Vec<Project>,
    selected_project: Option<usize>,
    run_all_script: String,
    run_mode_parallel: bool,
    environment: Environment,
    message: String,
    git_branch: Option<String>,
    git_clean: Option<bool>,
    git_modified: Vec<String>,
    pending_run: Option<(usize, String)>,
    /// Env vars for current environment (editable)
    env_vars: HashMap<String, String>,
    /// New env key (for add row)
    env_new_key: String,
    env_new_val: String,
    commit_message: String,
    env_needs_refresh: bool,
    /// Una pestaña por proceso; cada una con su salida y su proceso (si sigue corriendo).
    terminal_sessions: Vec<TerminalSession>,
    /// Índice de la pestaña de terminal seleccionada.
    selected_terminal_tab: usize,
    /// GitLab: URL y token (guardados en config).
    gitlab_url: String,
    gitlab_token: String,
    /// Proyectos listados desde GitLab API.
    gitlab_projects: Vec<microtermi_core::GitLabProject>,
    /// Ramas del proyecto GitLab seleccionado.
    gitlab_branches: Vec<microtermi_core::GitLabBranch>,
    /// Índice del proyecto GitLab seleccionado (para ver ramas / clonar).
    selected_gitlab_project: Option<usize>,
    /// Mensaje de error o estado de GitLab (ej. "Conectando...").
    gitlab_status: String,
    /// Ramas locales del repo abierto (para selector en panel Git).
    git_local_branches: Vec<String>,
    /// Rama seleccionada en el dropdown (para cambiar rama).
    selected_git_branch: String,
    /// Pestaña principal seleccionada.
    main_tab: MainTab,
    /// Texto que el usuario escribe en el filtro (repos GitLab).
    gitlab_repo_filter: String,
    /// Filtro aplicado: se actualiza al pulsar Enter o «Buscar».
    gitlab_filter_applied: String,
    /// True mientras se está listando proyectos (petición en segundo plano).
    gitlab_loading: bool,
    /// Receptor del resultado de list_projects en segundo plano.
    gitlab_receiver: Option<mpsc::Receiver<Result<Vec<microtermi_core::GitLabProject>, microtermi_core::GitLabError>>>,
    /// Historial de commits del repo local (log).
    git_log: Vec<microtermi_core::CommitInfo>,
    /// Índice del commit seleccionado en el historial (para ver detalle).
    git_log_selected: Option<usize>,
    /// Archivos cambiados en el commit seleccionado.
    git_commit_detail: Vec<microtermi_core::CommitFileChange>,
    /// Carpeta usada para Git (Pull/Push/Commit). Si es None, se usa la carpeta raíz.
    /// Se establece al clonar "en carpeta raíz" para que el repo clonado sea el activo.
    git_repo_path: Option<PathBuf>,
    /// Git del proyecto seleccionado en Projects (rama, modificados, ramas locales, log).
    project_git_branch: Option<String>,
    project_git_clean: Option<bool>,
    project_git_modified: Vec<String>,
    project_git_local_branches: Vec<String>,
    project_git_remote_branches: Vec<String>,
    project_git_selected_branch: String,
    project_git_selected_remote_branch: String,
    project_git_log: Vec<microtermi_core::CommitInfo>,
    project_git_log_selected: Option<usize>,
    project_git_commit_detail: Vec<microtermi_core::CommitFileChange>,
    /// Índice del proyecto para el que se cargó project_git_* (para refrescar al cambiar de proyecto).
    project_git_refreshed_for: Option<usize>,
    /// Multi-run: proyectos seleccionados para "Ejecutar en seleccionados".
    multi_run_selected: HashSet<usize>,
    /// Multi-run: script/comando a ejecutar.
    multi_run_script: String,
    #[allow(dead_code)]
    multi_run_columns: u32,
    /// Coverage: proyecto seleccionado para ver reporte / ejecutar tests.
    coverage_selected_project: Option<usize>,
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
        let config = load_config_json();
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
    fn persist_app_config(&self) {
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
        save_config_write(&json);
    }

    /// Ruta usada para operaciones Git (repo local). Prioridad: git_repo_path, luego root_path.
    fn git_root(&self) -> Option<PathBuf> {
        self.git_repo_path.clone().or_else(|| self.root_path.clone())
    }

    fn refresh_projects(&mut self) {
        if let Some(ref root) = self.root_path {
            match scan_projects(root) {
                Ok(p) => self.projects = p,
                Err(e) => self.message = format!("Error scanning: {}", e),
            }
        }
    }

    fn refresh_git(&mut self) {
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
    fn refresh_project_git(&mut self, project_path: &Path) {
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

    fn refresh_git_branches(&mut self) {
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

    fn refresh_env(&mut self) {
        if let Some(ref root) = self.root_path {
            match load_env(root, self.environment) {
                Ok(vars) => self.env_vars = vars,
                Err(_) => self.env_vars.clear(),
            }
        }
    }

    /// Nombres de scripts que tienen en común todos los proyectos (intersección).
    fn common_script_names(&self) -> Vec<String> {
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
        let header = strip_ansi(&format!(
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
                    lines: vec![header, strip_ansi(&format!("[error] {}", e))],
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
    fn terminal_stop_at(&mut self, index: usize) {
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
    fn terminal_stop_all(&mut self) {
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

    fn run_all_click(&mut self) {
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
                        lines: vec![header, strip_ansi(&format!("[error] {}", e))],
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
    fn multi_run_click(&mut self) {
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
                        lines: vec![header, strip_ansi(&format!("[error] {}", e))],
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
    fn multi_run_add_placeholder(&mut self) {
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
    fn multi_run_placeholder_execute(&mut self, index: usize) {
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
                self.terminal_sessions[index].lines = vec![header, strip_ansi(&format!("[error] {}", e))];
                self.terminal_sessions[index].pending_project = Some(proj_idx);
                self.terminal_sessions[index].pending_script = Some(script_name);
                self.message = format!("Error: {}", e);
            }
        }
    }

    /// Cierra el panel de Multi-run en el índice de sesión dado y actualiza el árbol.
    fn multi_run_close_pane(&mut self, session_idx: usize) {
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
    fn coverage_run_tests_for_project(&mut self, project_idx: usize) {
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
    fn draw_multi_run_session_content(
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
                                for seg in parse_ansi_line(line) {
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

    fn gitlab_list_projects(&mut self) {
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

    fn gitlab_select_project(&mut self, index: usize) {
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

    fn gitlab_clone(&mut self, index: usize) {
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
    fn gitlab_clone_to_root(&mut self, index: usize) {
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
    fn gitlab_projects_grouped(&self) -> std::collections::BTreeMap<String, Vec<(usize, &microtermi_core::GitLabProject)>> {
        let mut map: std::collections::BTreeMap<String, Vec<(usize, &microtermi_core::GitLabProject)>> = std::collections::BTreeMap::new();
        for (i, proj) in self.gitlab_projects.iter().enumerate() {
            let group = proj.path_with_namespace.split('/').next().unwrap_or("").to_string();
            map.entry(group).or_default().push((i, proj));
        }
        map
    }

    fn git_checkout_branch(&mut self) {
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
            MainTab::Settings => {
                egui::CentralPanel::default().show(ctx, |ui| {
                    ui.heading("Configuración");
                    ui.add_space(8.0);
                    ui.collapsing("GitLab", |ui| {
                        ui.horizontal(|ui| {
                            ui.label("URL:");
                            ui.text_edit_singleline(&mut self.gitlab_url);
                        });
                        ui.horizontal(|ui| {
                            ui.label("Token:");
                            ui.add(egui::TextEdit::singleline(&mut self.gitlab_token).password(true).desired_width(280.0));
                        });
                        if ui.button("Guardar").clicked() {
                            self.persist_app_config();
                            self.message = "GitLab guardado.".to_string();
                        }
                    });
                    ui.add_space(12.0);
                    ui.collapsing("Carpeta raíz", |ui| {
                        if let Some(ref p) = self.root_path {
                            ui.label(p.display().to_string());
                        } else {
                            ui.label("(no seleccionada)");
                        }
                        if ui.button("Cambiar carpeta raíz").clicked() {
                            if let Some(path) = rfd::FileDialog::new().pick_folder() {
                                self.root_path = Some(path.clone());
                                self.persist_app_config();
                                self.refresh_projects();
                                self.refresh_git();
                                self.refresh_env();
                                self.refresh_git_branches();
                            }
                        }
                    });
                    if !self.message.is_empty() {
                        ui.add_space(8.0);
                        ui.label(&self.message);
                    }
                });
            }
            MainTab::Projects => {
                egui::SidePanel::left("projects")
                    .resizable(true)
                    .default_width(280.0)
                    .show(ctx, |ui| {
                        ui.heading("Proyectos");
                        if self.projects.is_empty() && self.root_path.is_some() {
                            ui.label("No se encontraron package.json");
                        }
                        for (i, p) in self.projects.iter().enumerate() {
                            let selected = self.selected_project == Some(i);
                            if ui.selectable_label(selected, &p.name).clicked() {
                                self.selected_project = Some(i);
                            }
                        }
                    });

                let mut pending_run_script: Option<(usize, String)> = None;
                let selected_project = self.selected_project;
                let project_clone = selected_project.and_then(|idx| self.projects.get(idx).cloned());
                egui::CentralPanel::default().show(ctx, |ui| {
                    if let Some(ref project) = project_clone {
                        let idx = selected_project.unwrap();
                        ui.horizontal(|ui| {
                            ui.strong(&project.name);
                            ui.label(egui::RichText::new("·").color(ui.visuals().weak_text_color()));
                            ui.label(egui::RichText::new(project.path.display().to_string()).small().color(ui.visuals().weak_text_color()));
                        });
                        ui.horizontal(|ui| {
                            ui.label("Ambiente:");
                            for env in Environment::ALL {
                                if ui.selectable_label(self.environment == env, env.as_str()).clicked() {
                                    self.environment = env;
                                    self.env_needs_refresh = true;
                                    self.persist_app_config();
                                }
                            }
                        });
                        ui.label(egui::RichText::new("Configuración, scripts y Git en los menús de abajo. La terminal está siempre visible más abajo.").small().color(ui.visuals().weak_text_color()));
                        ui.add_space(4.0);
                        egui::CollapsingHeader::new("⚙ Configuración")
                            .default_open(false)
                            .show(ui, |ui| {
                            let mut to_remove = None;
                            let mut keys: Vec<_> = self.env_vars.keys().cloned().collect();
                            keys.sort();
                            for k in keys {
                                ui.horizontal(|ui| {
                                    ui.label(format!("{}:", k));
                                    if let Some(v) = self.env_vars.get_mut(&k) {
                                        ui.text_edit_singleline(v);
                                    }
                                    if ui.button("Eliminar").clicked() {
                                        to_remove = Some(k);
                                    }
                                });
                            }
                            if let Some(k) = to_remove {
                                self.env_vars.remove(&k);
                            }
                            ui.horizontal(|ui| {
                                ui.label("Nueva:");
                                ui.text_edit_singleline(&mut self.env_new_key);
                                ui.text_edit_singleline(&mut self.env_new_val);
                                if ui.button("Añadir").clicked() {
                                    if !self.env_new_key.is_empty() {
                                        self.env_vars.insert(self.env_new_key.clone(), self.env_new_val.clone());
                                        self.env_new_key.clear();
                                        self.env_new_val.clear();
                                    }
                                }
                            });
                            if ui.button("Guardar en disco").clicked() {
                                if let Some(ref root) = self.root_path {
                                    if let Err(e) = save_env(root, self.environment, &self.env_vars) {
                                        self.message = format!("Error guardando env: {}", e);
                                    } else {
                                        self.message = "Variables guardadas.".to_string();
                                    }
                                }
                            }
                        });
                        egui::CollapsingHeader::new("📜 Scripts")
                            .default_open(false)
                            .show(ui, |ui| {
                        for (name, cmd) in &project.scripts {
                            ui.horizontal(|ui| {
                                if ui.button("Ejecutar").clicked() {
                                    pending_run_script = Some((idx, name.clone()));
                                }
                                ui.label(format!("{} → {}", name, cmd));
                            });
                        }
                        ui.add_space(6.0);
                        ui.label("Ejecutar todos (scripts en común):");
                        let common_scripts = self.common_script_names();
                        let can_run_all = !common_scripts.is_empty();
                        if !can_run_all && !self.projects.is_empty() {
                            ui.label(egui::RichText::new("No hay scripts en común.").color(ui.visuals().weak_text_color()));
                        }
                        ui.horizontal(|ui| {
                            ui.label("Script:");
                            if can_run_all {
                                if !common_scripts.contains(&self.run_all_script) {
                                    self.run_all_script = common_scripts.first().cloned().unwrap_or_default();
                                }
                                let idx_script = common_scripts.iter().position(|s| s == &self.run_all_script).unwrap_or(0);
                                egui::ComboBox::from_id_salt("run_all_script")
                                    .selected_text(&self.run_all_script)
                                    .show_ui(ui, |ui| {
                                        for (i, name) in common_scripts.iter().enumerate() {
                                            if ui.selectable_label(idx_script == i, name).clicked() {
                                                self.run_all_script = name.clone();
                                                self.persist_app_config();
                                            }
                                        }
                                    });
                            } else {
                                ui.label("—");
                            }
                            if ui.checkbox(&mut self.run_mode_parallel, "Paralelo").changed() {
                                self.persist_app_config();
                            }
                            if ui.button("Ejecutar todos").clicked() && can_run_all {
                                self.run_all_click();
                            }
                        });
                        });
                        if self.project_git_refreshed_for != Some(idx) {
                            self.refresh_project_git(&project.path);
                            self.project_git_refreshed_for = Some(idx);
                        }
                        egui::CollapsingHeader::new("🔀 Git")
                            .default_open(false)
                            .show(ui, |ui| {
                        if self.project_git_branch.is_some() {
                            let path = project.path.clone();
                            ui.horizontal(|ui| {
                                if ui.button("Refrescar").clicked() {
                                    self.refresh_project_git(&path);
                                }
                                if ui.button("Fetch").clicked() {
                                    if let Ok(repo) = microtermi_core::open_repo(&path) {
                                        match microtermi_core::fetch(&repo) {
                                            Ok(()) => {
                                                self.message = "Fetch completado.".to_string();
                                                self.refresh_project_git(&path);
                                            }
                                            Err(e) => self.message = format!("Error: {}", e),
                                        }
                                    }
                                }
                                if ui.button("Pull").clicked() {
                                    if let Ok(repo) = microtermi_core::open_repo(&path) {
                                        match microtermi_core::pull(&repo) {
                                            Ok(msg) => self.message = msg,
                                            Err(e) => self.message = format!("Error: {}", e),
                                        }
                                        self.refresh_project_git(&path);
                                    }
                                }
                                if ui.button("Push").clicked() {
                                    if let Ok(repo) = microtermi_core::open_repo(&path) {
                                        match microtermi_core::push(&repo) {
                                            Ok(msg) => self.message = msg,
                                            Err(e) => self.message = format!("Error: {}", e),
                                        }
                                        self.refresh_project_git(&path);
                                    }
                                }
                                if ui.button("Stash").clicked() {
                                    if let Ok(mut repo) = microtermi_core::open_repo(&path) {
                                        match microtermi_core::stash(&mut repo) {
                                            Ok(()) => {
                                                self.message = "Cambios guardados en stash.".to_string();
                                                self.refresh_project_git(&path);
                                            }
                                            Err(e) => self.message = format!("Error: {}", e),
                                        }
                                    }
                                }
                                if ui.button("Stash pop").clicked() {
                                    if let Ok(mut repo) = microtermi_core::open_repo(&path) {
                                        match microtermi_core::stash_pop(&mut repo) {
                                            Ok(()) => {
                                                self.message = "Stash aplicado y eliminado.".to_string();
                                                self.refresh_project_git(&path);
                                            }
                                            Err(e) => self.message = format!("Error: {}", e),
                                        }
                                    }
                                }
                            });
                            ui.horizontal(|ui| {
                                ui.label("Rama local:");
                                ui.label(egui::RichText::new(self.project_git_branch.as_deref().unwrap_or("—")).strong());
                                if !self.project_git_local_branches.is_empty() {
                                    egui::ComboBox::from_id_salt("project_git_branch")
                                        .selected_text(if self.project_git_selected_branch.is_empty() { "—" } else { &self.project_git_selected_branch })
                                        .show_ui(ui, |ui| {
                                            for name in &self.project_git_local_branches {
                                                if ui.selectable_label(self.project_git_selected_branch == *name, name).clicked() {
                                                    self.project_git_selected_branch = name.clone();
                                                }
                                            }
                                        });
                                    if ui.button("Cambiar rama").clicked() && !self.project_git_selected_branch.is_empty() {
                                        if let Ok(repo) = microtermi_core::open_repo(&path) {
                                            if microtermi_core::checkout_branch(&repo, &self.project_git_selected_branch).is_ok() {
                                                self.message = format!("Rama cambiada a {}", self.project_git_selected_branch);
                                                self.refresh_project_git(&path);
                                            } else {
                                                self.message = "Error al cambiar rama.".to_string();
                                            }
                                        }
                                    }
                                }
                            });
                            ui.horizontal(|ui| {
                                ui.label("Rama remota (origin):");
                                if !self.project_git_remote_branches.is_empty() {
                                    egui::ComboBox::from_id_salt("project_git_remote_branch")
                                        .selected_text(if self.project_git_selected_remote_branch.is_empty() { "—" } else { &self.project_git_selected_remote_branch })
                                        .show_ui(ui, |ui| {
                                            for name in &self.project_git_remote_branches {
                                                if ui.selectable_label(self.project_git_selected_remote_branch == *name, name).clicked() {
                                                    self.project_git_selected_remote_branch = name.clone();
                                                }
                                            }
                                        });
                                    if ui.button("Cambiar a rama remota").clicked() && !self.project_git_selected_remote_branch.is_empty() {
                                        if let Ok(repo) = microtermi_core::open_repo(&path) {
                                            match microtermi_core::checkout_remote_branch(&repo, &self.project_git_selected_remote_branch) {
                                                Ok(()) => {
                                                    self.message = format!("Cambiado a rama remota {}", self.project_git_selected_remote_branch);
                                                    self.refresh_project_git(&path);
                                                }
                                                Err(e) => self.message = format!("Error: {}", e),
                                            }
                                        }
                                    }
                                } else {
                                    ui.label(egui::RichText::new("Pulsa Fetch para cargar ramas remotas.").color(ui.visuals().weak_text_color()));
                                }
                            });
                            if let Some(clean) = self.project_git_clean {
                                if clean {
                                    ui.label(egui::RichText::new("Estado: limpio").color(ui.visuals().weak_text_color()));
                                } else {
                                    ui.label("Archivos modificados:");
                                    for f in &self.project_git_modified {
                                        ui.label(egui::RichText::new(f).font(egui::FontId::monospace(12.0)));
                                    }
                                }
                            }
                            ui.horizontal(|ui| {
                                ui.label("Mensaje:");
                                ui.text_edit_singleline(&mut self.commit_message);
                                if ui.button("Commit").clicked() {
                                    if let Ok(repo) = microtermi_core::open_repo(&path) {
                                        let path_refs: Vec<&Path> = self.project_git_modified.iter().map(|s| Path::new(s)).collect();
                                        if let Err(e) = microtermi_core::commit(&repo, &self.commit_message, &path_refs) {
                                            self.message = format!("Error: {}", e);
                                        } else {
                                            self.message = "Commit realizado.".to_string();
                                            self.commit_message.clear();
                                            self.refresh_project_git(&path);
                                        }
                                    }
                                }
                            });
                            ui.label("Historial de commits:");
                            let mut click_log: Option<usize> = None;
                            egui::ScrollArea::vertical().max_height(180.0).show(ui, |ui| {
                                for (i, c) in self.project_git_log.iter().enumerate() {
                                    let selected = self.project_git_log_selected == Some(i);
                                    let line = format!("{}  {}  {}  {}", c.id_short, c.date, c.author, c.message);
                                    if ui.selectable_label(selected, line).clicked() {
                                        click_log = Some(i);
                                    }
                                }
                            });
                            if let Some(i) = click_log {
                                self.project_git_log_selected = Some(i);
                                if i < self.project_git_log.len() {
                                    if let Ok(repo) = microtermi_core::open_repo(&path) {
                                        self.project_git_commit_detail = microtermi_core::commit_changes(&repo, &self.project_git_log[i].id_short).unwrap_or_default();
                                    }
                                }
                            }
                            if let Some(i) = self.project_git_log_selected {
                                if i < self.project_git_log.len() && !self.project_git_commit_detail.is_empty() {
                                    ui.label("Archivos en este commit:");
                                    for f in &self.project_git_commit_detail {
                                        ui.label(format!("  [{}] {}", f.status, f.path));
                                    }
                                }
                            }
                        } else {
                            ui.label(egui::RichText::new("Este proyecto no es un repositorio Git (no hay .git en su carpeta).").color(ui.visuals().weak_text_color()));
                        }
                        });
                    } else {
                        ui.label("Selecciona una carpeta raíz (arriba) y luego un proyecto.");
                    }
                    if !self.message.is_empty() {
                        ui.add_space(8.0);
                        ui.label(&self.message);
                    }
                });
                if let Some(p) = pending_run_script {
                    self.pending_run = Some(p);
                }
            }
            MainTab::Git => {
                egui::SidePanel::left("git_repos")
                    .resizable(true)
                    .default_width(320.0)
                    .show(ctx, |ui| {
                        ui.heading("Repositorios GitLab");
                        ui.horizontal(|ui| {
                            ui.label("Filtro:");
                            let resp = ui.text_edit_singleline(&mut self.gitlab_repo_filter);
                            if resp.has_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter)) && !self.gitlab_loading {
                                self.gitlab_list_projects();
                            }
                        });
                        ui.horizontal(|ui| {
                            let buscar_clicked = ui.button("Buscar").on_hover_text("Lista proyectos en GitLab con el filtro actual").clicked();
                            if buscar_clicked && !self.gitlab_loading {
                                self.gitlab_list_projects();
                            }
                            let listar_enabled = !self.gitlab_loading;
                            let listar_btn = ui.add_enabled(listar_enabled, egui::Button::new("Listar proyectos"));
                            if listar_btn.clicked() {
                                self.gitlab_list_projects();
                            }
                            if self.gitlab_loading {
                                ui.spinner();
                                ui.label("Buscando…");
                                ui.ctx().request_repaint();
                            }
                        });
                        if !self.gitlab_filter_applied.is_empty() {
                            ui.label(egui::RichText::new(format!("Filtro aplicado: \"{}\"", self.gitlab_filter_applied)).color(ui.visuals().weak_text_color()));
                        }
                        if !self.gitlab_status.is_empty() {
                            ui.label(egui::RichText::new(&self.gitlab_status).color(ui.visuals().weak_text_color()));
                        }
                        ui.add_space(4.0);
                        let grouped = self.gitlab_projects_grouped();
                        let mut click_project: Option<usize> = None;
                        let selected = self.selected_gitlab_project;
                        egui::ScrollArea::vertical().show(ui, |ui| {
                            for (group_name, projects) in &grouped {
                                let group_label = if group_name.is_empty() { "(sin grupo)" } else { group_name.as_str() };
                                egui::CollapsingHeader::new(group_label)
                                    .default_open(true)
                                    .show(ui, |ui| {
                                        for (i, proj) in projects {
                                            if ui.selectable_label(selected == Some(*i), &proj.path_with_namespace).clicked() {
                                                click_project = Some(*i);
                                            }
                                        }
                                    });
                            }
                        });
                        if grouped.is_empty() && !self.gitlab_projects.is_empty() {
                            ui.label("(ningún proyecto coincide con el filtro)");
                        } else if grouped.is_empty() {
                            ui.label("Pulsa «Listar proyectos» para cargar repos.");
                        }
                        if let Some(i) = click_project {
                            self.gitlab_select_project(i);
                        }
                    });

                egui::CentralPanel::default().show(ctx, |ui| {
                    // PRIMERO: Repositorio local (siempre visible arriba)
                    ui.heading("Repositorio local");
                    let root_clone = self.git_root();
                    if let Some(ref root) = root_clone {
                        if microtermi_core::open_repo(root).is_ok() {
                            // Barra de acciones (estilo IntelliJ): Pull, Push, Refrescar
                            if self.git_repo_path.is_some() {
                                ui.horizontal(|ui| {
                                    ui.label(egui::RichText::new("Repo:").weak());
                                    ui.label(egui::RichText::new(root.display().to_string()).small());
                                    if ui.small_button("Usar carpeta raíz").clicked() {
                                        self.git_repo_path = None;
                                        self.refresh_git();
                                    }
                                });
                            }
                            ui.horizontal(|ui| {
                                ui.strong("Acciones:");
                                if ui.button("Pull").clicked() {
                                    if let Ok(repo) = microtermi_core::open_repo(root) {
                                        match microtermi_core::pull(&repo) {
                                            Ok(msg) => self.message = msg,
                                            Err(e) => self.message = format!("Error: {}", e),
                                        }
                                        self.refresh_git();
                                    }
                                }
                                if ui.button("Push").clicked() {
                                    if let Ok(repo) = microtermi_core::open_repo(root) {
                                        match microtermi_core::push(&repo) {
                                            Ok(msg) => self.message = msg,
                                            Err(e) => self.message = format!("Error: {}", e),
                                        }
                                        self.refresh_git();
                                    }
                                }
                                if ui.button("Refrescar").clicked() {
                                    self.refresh_git();
                                }
                            });
                            ui.add_space(4.0);
                            // Rama actual + selector para cambiar
                            ui.horizontal(|ui| {
                                ui.label("Rama:");
                                ui.label(egui::RichText::new(self.git_branch.as_deref().unwrap_or("—")).strong());
                                if !self.git_local_branches.is_empty() {
                                    egui::ComboBox::from_id_salt("git_branch_tab")
                                        .selected_text(if self.selected_git_branch.is_empty() { "—" } else { &self.selected_git_branch })
                                        .show_ui(ui, |ui| {
                                            for name in &self.git_local_branches {
                                                if ui.selectable_label(self.selected_git_branch == *name, name).clicked() {
                                                    self.selected_git_branch = name.clone();
                                                }
                                            }
                                        });
                                    if ui.button("Cambiar rama").clicked() {
                                        self.git_checkout_branch();
                                    }
                                }
                            });
                            // Archivos modificados
                            if let Some(clean) = self.git_clean {
                                if clean {
                                    ui.label(egui::RichText::new("Estado: limpio").color(ui.visuals().weak_text_color()));
                                } else {
                                    ui.label("Archivos modificados:");
                                    for f in &self.git_modified {
                                        ui.label(egui::RichText::new(f).font(egui::FontId::monospace(12.0)));
                                    }
                                }
                            }
                            ui.add_space(4.0);
                            // Commit: mensaje + botón
                            ui.horizontal(|ui| {
                                ui.label("Mensaje:");
                                ui.text_edit_singleline(&mut self.commit_message);
                                if ui.button("Commit").clicked() {
                                    if let Ok(repo) = microtermi_core::open_repo(root) {
                                        let path_refs: Vec<&Path> = self.git_modified.iter().map(|s| Path::new(s)).collect();
                                        if let Err(e) = microtermi_core::commit(&repo, &self.commit_message, &path_refs) {
                                            self.message = format!("Error: {}", e);
                                        } else {
                                            self.message = "Commit realizado.".to_string();
                                            self.commit_message.clear();
                                            self.refresh_git();
                                        }
                                    }
                                }
                            });
                            ui.add_space(12.0);
                            ui.separator();
                            ui.add_space(8.0);
                            // Historial de commits
                            ui.heading("Historial de commits");
                            let mut click_log: Option<usize> = None;
                            egui::ScrollArea::vertical().max_height(220.0).show(ui, |ui| {
                                for (i, c) in self.git_log.iter().enumerate() {
                                    let selected = self.git_log_selected == Some(i);
                                    let line = format!("{}  {}  {}  {}", c.id_short, c.date, c.author, c.message);
                                    if ui.selectable_label(selected, line).clicked() {
                                        click_log = Some(i);
                                    }
                                }
                            });
                            if let Some(idx) = click_log {
                                self.git_log_selected = Some(idx);
                                if idx < self.git_log.len() {
                                    if let Ok(repo) = microtermi_core::open_repo(root) {
                                        self.git_commit_detail = microtermi_core::commit_changes(&repo, &self.git_log[idx].id_short).unwrap_or_default();
                                    }
                                }
                            }
                            if let Some(idx) = self.git_log_selected {
                                if idx < self.git_log.len() && !self.git_commit_detail.is_empty() {
                                    ui.add_space(4.0);
                                    ui.label("Archivos en este commit:");
                                    for f in &self.git_commit_detail {
                                        let st = &f.status;
                                        ui.label(format!("  [{}] {}", st, f.path));
                                    }
                                }
                            }
                        } else {
                            ui.label("La carpeta actual no es un repositorio Git.");
                            ui.label(egui::RichText::new("Pulsa «Clonar en carpeta raíz» en el proyecto GitLab de abajo: el repo clonado se usará aquí para Pull, Push y Commit.").color(ui.visuals().weak_text_color()));
                        }
                    } else {
                        ui.label("Selecciona carpeta raíz en Settings o Projects y luego clona un proyecto (o elige una carpeta que ya sea un repo).");
                    }
                    if !self.message.is_empty() {
                        ui.add_space(8.0);
                        ui.label(egui::RichText::new(&self.message).color(ui.visuals().weak_text_color()));
                    }

                    // DESPUÉS: Detalle del proyecto GitLab seleccionado (abajo, con scroll en ramas)
                    ui.add_space(16.0);
                    ui.separator();
                    ui.add_space(8.0);
                    if let Some(idx) = self.selected_gitlab_project {
                        if idx < self.gitlab_projects.len() {
                            let proj = &self.gitlab_projects[idx];
                            ui.heading("Proyecto GitLab seleccionado");
                            ui.label(egui::RichText::new(&proj.path_with_namespace).strong());
                            ui.label(egui::RichText::new(proj.web_url.as_str()).color(ui.visuals().weak_text_color()));
                            ui.horizontal(|ui| {
                                if ui.button("Clonar en carpeta raíz").clicked() {
                                    self.gitlab_clone_to_root(idx);
                                }
                                if ui.button("Clonar en otra carpeta…").clicked() {
                                    self.gitlab_clone(idx);
                                }
                            });
                            ui.add_space(4.0);
                            ui.label("Ramas (GitLab):");
                            egui::ScrollArea::vertical().max_height(180.0).show(ui, |ui| {
                                for b in &self.gitlab_branches {
                                    ui.label(&b.name);
                                }
                            });
                        }
                    } else {
                        ui.label(egui::RichText::new("Selecciona un proyecto en la lista de la izquierda para ver ramas y clonar.").color(ui.visuals().weak_text_color()));
                    }
                });
            }
            MainTab::MultiRun => {
                egui::SidePanel::left("multi_run_sidebar")
                    .resizable(true)
                    .default_width(280.0)
                    .show(ctx, |ui| {
                        ui.heading("Multi-run");
                        ui.label(egui::RichText::new("Selecciona proyectos y un script; luego «Ejecutar en seleccionados». Or añade terminales y elige proyecto+script en cada una.").small().color(ui.visuals().weak_text_color()));
                        ui.add_space(6.0);
                        if self.projects.is_empty() && self.root_path.is_some() {
                            ui.label("No se encontraron package.json");
                        }
                        for (i, p) in self.projects.iter().enumerate() {
                            let selected = self.multi_run_selected.contains(&i);
                            let mut sel = selected;
                            if ui.checkbox(&mut sel, &p.name).changed() {
                                if sel {
                                    self.multi_run_selected.insert(i);
                                } else {
                                    self.multi_run_selected.remove(&i);
                                }
                                self.persist_app_config();
                            }
                        }
                        ui.add_space(6.0);
                        ui.label("Script:");
                        let common = self.common_script_names();
                        if !common.is_empty() {
                            if !common.contains(&self.multi_run_script) {
                                self.multi_run_script = common.first().cloned().unwrap_or_default();
                            }
                            let idx_script = common.iter().position(|s| s == &self.multi_run_script).unwrap_or(0);
                            egui::ComboBox::from_id_salt("multi_run_script")
                                .selected_text(&self.multi_run_script)
                                .show_ui(ui, |ui| {
                                    for (i, name) in common.iter().enumerate() {
                                        if ui.selectable_label(idx_script == i, name).clicked() {
                                            self.multi_run_script = name.clone();
                                            self.persist_app_config();
                                        }
                                    }
                                });
                        } else {
                            if ui.text_edit_singleline(&mut self.multi_run_script).lost_focus() {
                                self.persist_app_config();
                            }
                        }
                        if ui.button("Ejecutar en seleccionados").clicked() {
                            self.multi_run_click();
                        }
                        if ui.button("Añadir terminal").clicked() {
                            self.multi_run_add_placeholder();
                        }
                        if !self.message.is_empty() {
                            ui.add_space(4.0);
                            ui.label(egui::RichText::new(&self.message).small().color(ui.visuals().weak_text_color()));
                        }
                    });

                let sessions = self.terminal_sessions.len();
                egui::CentralPanel::default().show(ctx, |ui| {
                    ui.horizontal(|ui| {
                        ui.strong("Terminales");
                        let any_running = self.terminal_sessions.iter().any(|s| s.child.is_some());
                        if any_running && ui.button("Detener todos").clicked() {
                            self.terminal_stop_all();
                        }
                    });
                    if sessions == 0 {
                        ui.label(egui::RichText::new("Selecciona proyectos a la izquierda y pulsa «Ejecutar en seleccionados», o «Añadir terminal» y elige proyecto + script en cada pestaña.").color(ui.visuals().weak_text_color()));
                    } else {
                        ui.separator();
                        ui.horizontal(|ui| {
                            let mut close_tab = None;
                            let mut run_again_tab = None;
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
                                let can_run_again = session.child.is_none() && session.receiver.is_none()
                                    && session.pending_project.is_some()
                                    && session.pending_script.as_ref().map_or(false, |s| !s.trim().is_empty());
                                if can_run_again && ui.small_button("↻").on_hover_text("Ejecutar de nuevo").clicked() {
                                    run_again_tab = Some(i);
                                }
                                if ui.small_button("✕").clicked() {
                                    close_tab = Some(i);
                                }
                            }
                            if let Some(i) = run_again_tab {
                                self.multi_run_placeholder_execute(i);
                            }
                            if let Some(i) = close_tab {
                                self.multi_run_close_pane(i);
                            }
                        });
                        ui.add_space(4.0);
                        let font_id = egui::FontId::monospace(12.0);
                        let idx = self.selected_terminal_tab.min(self.terminal_sessions.len().saturating_sub(1));
                        let (close_tab, run_placeholder, stop_at) =
                            self.draw_multi_run_session_content(ui, idx, &font_id);
                        if let Some(i) = close_tab {
                            self.multi_run_close_pane(i);
                        }
                        if let Some(i) = stop_at {
                            self.terminal_stop_at(i);
                        }
                        if let Some(i) = run_placeholder {
                            self.multi_run_placeholder_execute(i);
                        }
                    }
                });
            }
            MainTab::Coverage => {
                egui::SidePanel::left("coverage_sidebar")
                    .resizable(true)
                    .default_width(260.0)
                    .show(ctx, |ui| {
                        ui.heading("Coverage");
                        ui.label(egui::RichText::new("Elige un proyecto. Ejecuta tests (npm run test) y abre el reporte HTML en coverage/lcov-report/.").small().color(ui.visuals().weak_text_color()));
                        ui.add_space(8.0);
                        if self.projects.is_empty() && self.root_path.is_some() {
                            ui.label("No se encontraron proyectos.");
                        } else if self.projects.is_empty() {
                            ui.label("Selecciona una carpeta raíz primero.");
                        } else {
                            for (i, p) in self.projects.iter().enumerate() {
                                let selected = self.coverage_selected_project == Some(i);
                                if ui.selectable_label(selected, &p.name).clicked() {
                                    self.coverage_selected_project = Some(i);
                                }
                            }
                        }
                    });

                egui::CentralPanel::default().show(ctx, |ui| {
                    ui.heading("Reporte de cobertura");
                    if let Some(idx) = self.coverage_selected_project {
                        if let Some(project) = self.projects.get(idx) {
                            let report_path = project.path.join("coverage").join("lcov-report").join("index.html");
                            let has_test_script = project.scripts.iter().any(|(s, _)| s == "test");
                            let report_exists = report_path.exists();

                            ui.horizontal(|ui| {
                                ui.label(format!("Proyecto: {}", project.name));
                            });
                            ui.add_space(6.0);
                            ui.label(egui::RichText::new(format!("Ruta del reporte: {}", report_path.display())).small().color(ui.visuals().weak_text_color()));
                            ui.add_space(8.0);

                            if has_test_script {
                                if ui.button("Ejecutar tests (npm run test)").clicked() {
                                    self.coverage_run_tests_for_project(idx);
                                }
                                ui.label(egui::RichText::new("Los tests se ejecutan en una terminal; al terminar podrás abrir el reporte.").small().color(ui.visuals().weak_text_color()));
                            } else {
                                ui.label(egui::RichText::new("Este proyecto no tiene script \"test\" en package.json.").color(ui.visuals().weak_text_color()));
                            }

                            ui.add_space(12.0);
                            if report_exists {
                                if ui.button("Abrir en navegador").clicked() {
                                    if let Err(e) = opener::open(&report_path) {
                                        self.message = format!("Error al abrir: {}", e);
                                    }
                                }
                            } else {
                                ui.label(egui::RichText::new("No hay reporte aún. Ejecuta los tests primero (npm run test con cobertura).").color(ui.visuals().weak_text_color()));
                            }
                        } else {
                            ui.label("Proyecto no encontrado.");
                        }
                    } else {
                        ui.label(egui::RichText::new("Selecciona un proyecto en la lista de la izquierda.").color(ui.visuals().weak_text_color()));
                    }
                });
            }
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
                                        for seg in parse_ansi_line(line) {
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
