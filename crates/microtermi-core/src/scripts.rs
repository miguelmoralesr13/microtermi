use std::collections::HashMap;
use std::io::{BufRead, BufReader};
use std::path::Path;
use std::process::{Command, Stdio};
use std::sync::mpsc;
use std::thread;
use thiserror::Error;

#[cfg(windows)]
use std::os::windows::process::CommandExt;

use crate::Project;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PackageManager {
    Npm,
    Yarn,
    Pnpm,
}

#[derive(Debug, Clone, Copy)]
pub enum ScriptRunMode {
    Parallel,
    Sequence,
}

#[derive(Debug, Error)]
pub enum ScriptError {
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
    #[error("Script failed: {0}")]
    Failed(String),
}

pub fn detect_package_manager(project_path: &Path) -> PackageManager {
    if project_path.join("pnpm-lock.yaml").exists() {
        PackageManager::Pnpm
    } else if project_path.join("yarn.lock").exists() {
        PackageManager::Yarn
    } else {
        PackageManager::Npm
    }
}

fn run_script_cmd(
    project_path: &Path,
    script_name: &str,
    env_vars: &HashMap<String, String>,
    package_manager: PackageManager,
) -> Result<std::process::Child, ScriptError> {
    #[cfg(windows)]
    {
        // On Windows npm/yarn/pnpm are .cmd scripts; the shell must run them so PATH/PATHEXT resolve.
        let shell_cmd = match package_manager {
            PackageManager::Npm => format!("npm run {}", script_name),
            PackageManager::Yarn => format!("yarn {}", script_name),
            PackageManager::Pnpm => format!("pnpm {}", script_name),
        };
        let mut cmd_builder = Command::new("cmd");
        cmd_builder
            .args(["/k", &shell_cmd])
            .current_dir(project_path)
            .envs(env_vars);
        cmd_builder.creation_flags(0x0000_0010); // CREATE_NEW_CONSOLE
        let child = cmd_builder.spawn()?;
        return Ok(child);
    }

    #[cfg(not(windows))]
    {
        let (cmd, args): (&str, Vec<&str>) = match package_manager {
            PackageManager::Npm => ("npm", vec!["run", script_name]),
            PackageManager::Yarn => ("yarn", vec![script_name]),
            PackageManager::Pnpm => ("pnpm", vec![script_name]),
        };
        let child = Command::new(cmd)
            .args(args)
            .current_dir(project_path)
            .envs(env_vars)
            .spawn()?;
        Ok(child)
    }
}

/// Run a single script with stdout/stderr captured. Returns the child process and a receiver
/// that receives output lines. Use this for the integrated terminal.
pub fn run_script_captured(
    project: &Project,
    script_name: &str,
    env_vars: &HashMap<String, String>,
) -> Result<(std::process::Child, mpsc::Receiver<String>), ScriptError> {
    let pm = detect_package_manager(&project.path);
    let (shell_cmd, _) = shell_cmd_and_args(pm, script_name);

    #[cfg(windows)]
    {
        let mut cmd_builder = Command::new("cmd");
        cmd_builder
            .args(["/c", &shell_cmd])
            .current_dir(&project.path)
            .envs(env_vars)
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());
        // Sin ventana de consola: la salida se muestra en la terminal integrada de la app.
        cmd_builder.creation_flags(0x0800_0000); // CREATE_NO_WINDOW
        let mut child = cmd_builder.spawn()?;
        let stdout = child.stdout.take().ok_or_else(|| {
            ScriptError::Io(std::io::Error::new(
                std::io::ErrorKind::Other,
                "no stdout",
            ))
        })?;
        let stderr = child.stderr.take().ok_or_else(|| {
            ScriptError::Io(std::io::Error::new(
                std::io::ErrorKind::Other,
                "no stderr",
            ))
        })?;
        let (tx, rx) = mpsc::channel();
        let tx2 = tx.clone();
        thread::spawn(move || {
            for line in BufReader::new(stdout).lines() {
                if let Ok(l) = line {
                    let _ = tx.send(l);
                }
            }
        });
        thread::spawn(move || {
            for line in BufReader::new(stderr).lines() {
                if let Ok(l) = line {
                    let _ = tx2.send(format!("[stderr] {}", l));
                }
            }
        });
        Ok((child, rx))
    }

    #[cfg(not(windows))]
    {
        let (_, args) = shell_cmd_and_args(pm, script_name);
        let mut child = Command::new(&args[0])
            .args(&args[1..])
            .current_dir(&project.path)
            .envs(env_vars)
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()?;
        let stdout = child.stdout.take().ok_or_else(|| {
            ScriptError::Io(std::io::Error::new(
                std::io::ErrorKind::Other,
                "no stdout",
            ))
        })?;
        let stderr = child.stderr.take().ok_or_else(|| {
            ScriptError::Io(std::io::Error::new(
                std::io::ErrorKind::Other,
                "no stderr",
            ))
        })?;
        let (tx, rx) = mpsc::channel();
        let tx2 = tx.clone();
        thread::spawn(move || {
            for line in BufReader::new(stdout).lines() {
                if let Ok(l) = line {
                    let _ = tx.send(l);
                }
            }
        });
        thread::spawn(move || {
            for line in BufReader::new(stderr).lines() {
                if let Ok(l) = line {
                    let _ = tx2.send(format!("[stderr] {}", l));
                }
            }
        });
        Ok((child, rx))
    }
}

fn shell_cmd_and_args(
    package_manager: PackageManager,
    script_name: &str,
) -> (String, Vec<String>) {
    match package_manager {
        PackageManager::Npm => (
            format!("npm run {}", script_name),
            vec!["npm".into(), "run".into(), script_name.into()],
        ),
        PackageManager::Yarn => (
            format!("yarn {}", script_name),
            vec!["yarn".into(), script_name.into()],
        ),
        PackageManager::Pnpm => (
            format!("pnpm {}", script_name),
            vec!["pnpm".into(), script_name.into()],
        ),
    }
}

/// Run a single script in the project directory. Spawns process and returns immediately
/// (process runs in background / external console on Windows we use creation flags to show console).
pub fn run_script(
    project: &Project,
    script_name: &str,
    env_vars: &HashMap<String, String>,
) -> Result<std::process::Child, ScriptError> {
    let pm = detect_package_manager(&project.path);
    run_script_cmd(&project.path, script_name, env_vars, pm)
}

/// Run the same script in multiple projects. Returns vec of child processes (or errors).
pub fn run_scripts(
    projects: &[Project],
    script_name: &str,
    env_vars: &HashMap<String, String>,
    mode: ScriptRunMode,
) -> Vec<Result<std::process::Child, ScriptError>> {
    let mut results = Vec::new();
    match mode {
        ScriptRunMode::Parallel => {
            for project in projects {
                if project.scripts.iter().any(|(s, _)| s == script_name) {
                    results.push(run_script(project, script_name, env_vars));
                }
            }
        }
        ScriptRunMode::Sequence => {
            for project in projects {
                if project.scripts.iter().any(|(s, _)| s == script_name) {
                    match run_script(project, script_name, env_vars) {
                        Ok(mut child) => {
                            let _ = child.wait();
                            results.push(Ok(child));
                        }
                        Err(e) => results.push(Err(e)),
                    }
                }
            }
        }
    }
    results
}
