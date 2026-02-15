use eframe::egui;
use microtermi_core::{save_env, Environment};
use std::path::Path;

use crate::MicrotermiApp;

pub fn draw(app: &mut MicrotermiApp, ctx: &egui::Context) {
    egui::SidePanel::left("projects")
        .resizable(true)
        .default_width(280.0)
        .show(ctx, |ui| {
            ui.heading("Proyectos");
            if app.projects.is_empty() && app.root_path.is_some() {
                ui.label("No se encontraron package.json");
            }
            for (i, p) in app.projects.iter().enumerate() {
                let selected = app.selected_project == Some(i);
                if ui.selectable_label(selected, &p.name).clicked() {
                    app.selected_project = Some(i);
                }
            }
        });

    let mut pending_run_script: Option<(usize, String)> = None;
    let selected_project = app.selected_project;
    let project_clone = selected_project.and_then(|idx| app.projects.get(idx).cloned());
    egui::CentralPanel::default().show(ctx, |ui| {
        egui::ScrollArea::both().show(ui, |ui| {
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
                        if ui.selectable_label(app.environment == env, env.as_str()).clicked() {
                            app.environment = env;
                            app.env_needs_refresh = true;
                            app.persist_app_config();
                        }
                    }
                });
                ui.label(egui::RichText::new("Configuración, scripts y Git en los menús de abajo. La terminal está siempre visible más abajo.").small().color(ui.visuals().weak_text_color()));
                ui.add_space(4.0);
                egui::CollapsingHeader::new("⚙ Configuración")
                    .default_open(false)
                    .show(ui, |ui| {
                        let mut to_remove = None;
                        let mut keys: Vec<_> = app.env_vars.keys().cloned().collect();
                        keys.sort();
                        for k in keys {
                            ui.horizontal(|ui| {
                                ui.label(format!("{}:", k));
                                if let Some(v) = app.env_vars.get_mut(&k) {
                                    ui.text_edit_singleline(v);
                                }
                                if ui.button("Eliminar").clicked() {
                                    to_remove = Some(k);
                                }
                            });
                        }
                        if let Some(k) = to_remove {
                            app.env_vars.remove(&k);
                        }
                        ui.horizontal(|ui| {
                            ui.label("Nueva:");
                            ui.text_edit_singleline(&mut app.env_new_key);
                            ui.text_edit_singleline(&mut app.env_new_val);
                            if ui.button("Añadir").clicked() {
                                if !app.env_new_key.is_empty() {
                                    app.env_vars.insert(app.env_new_key.clone(), app.env_new_val.clone());
                                    app.env_new_key.clear();
                                    app.env_new_val.clear();
                                }
                            }
                        });
                        if ui.button("Guardar en disco").clicked() {
                            if let Some(ref root) = app.root_path {
                                if let Err(e) = save_env(root, app.environment, &app.env_vars) {
                                    app.message = format!("Error guardando env: {}", e);
                                } else {
                                    app.message = "Variables guardadas.".to_string();
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
                        let common_scripts = app.common_script_names();
                        let can_run_all = !common_scripts.is_empty();
                        if !can_run_all && !app.projects.is_empty() {
                            ui.label(egui::RichText::new("No hay scripts en común.").color(ui.visuals().weak_text_color()));
                        }
                        ui.horizontal(|ui| {
                            ui.label("Script:");
                            if can_run_all {
                                if !common_scripts.contains(&app.run_all_script) {
                                    app.run_all_script = common_scripts.first().cloned().unwrap_or_default();
                                }
                                let idx_script = common_scripts.iter().position(|s| s == &app.run_all_script).unwrap_or(0);
                                egui::ComboBox::from_id_salt("run_all_script")
                                    .selected_text(&app.run_all_script)
                                    .show_ui(ui, |ui| {
                                        for (i, name) in common_scripts.iter().enumerate() {
                                            if ui.selectable_label(idx_script == i, name).clicked() {
                                                app.run_all_script = name.clone();
                                                app.persist_app_config();
                                            }
                                        }
                                    });
                            } else {
                                ui.label("—");
                            }
                            if ui.checkbox(&mut app.run_mode_parallel, "Paralelo").changed() {
                                app.persist_app_config();
                            }
                            if ui.button("Ejecutar todos").clicked() && can_run_all {
                                app.run_all_click();
                            }
                        });
                    });
                if app.project_git_refreshed_for != Some(idx) {
                    app.refresh_project_git(&project.path);
                    app.project_git_refreshed_for = Some(idx);
                }
                egui::CollapsingHeader::new("🔀 Git")
                    .default_open(false)
                    .show(ui, |ui| {
                        if app.project_git_branch.is_some() {
                            let path = project.path.clone();
                            ui.horizontal(|ui| {
                                if ui.button("Refrescar").clicked() {
                                    app.refresh_project_git(&path);
                                }
                                if ui.button("Fetch").clicked() {
                                    if let Ok(repo) = microtermi_core::open_repo(&path) {
                                        match microtermi_core::fetch(&repo) {
                                            Ok(()) => {
                                                app.message = "Fetch completado.".to_string();
                                                app.refresh_project_git(&path);
                                            }
                                            Err(e) => app.message = format!("Error: {}", e),
                                        }
                                    }
                                }
                                if ui.button("Pull").clicked() {
                                    if let Ok(repo) = microtermi_core::open_repo(&path) {
                                        match microtermi_core::pull(&repo) {
                                            Ok(msg) => app.message = msg,
                                            Err(e) => app.message = format!("Error: {}", e),
                                        }
                                        app.refresh_project_git(&path);
                                    }
                                }
                                if ui.button("Push").clicked() {
                                    if let Ok(repo) = microtermi_core::open_repo(&path) {
                                        match microtermi_core::push(&repo) {
                                            Ok(msg) => app.message = msg,
                                            Err(e) => app.message = format!("Error: {}", e),
                                        }
                                        app.refresh_project_git(&path);
                                    }
                                }
                                if ui.button("Stash").clicked() {
                                    if let Ok(mut repo) = microtermi_core::open_repo(&path) {
                                        match microtermi_core::stash(&mut repo) {
                                            Ok(()) => {
                                                app.message = "Cambios guardados en stash.".to_string();
                                                app.refresh_project_git(&path);
                                            }
                                            Err(e) => app.message = format!("Error: {}", e),
                                        }
                                    }
                                }
                                if ui.button("Stash pop").clicked() {
                                    if let Ok(mut repo) = microtermi_core::open_repo(&path) {
                                        match microtermi_core::stash_pop(&mut repo) {
                                            Ok(()) => {
                                                app.message = "Stash aplicado y eliminado.".to_string();
                                                app.refresh_project_git(&path);
                                            }
                                            Err(e) => app.message = format!("Error: {}", e),
                                        }
                                    }
                                }
                            });
                            ui.horizontal(|ui| {
                                ui.label("Rama local:");
                                ui.label(egui::RichText::new(app.project_git_branch.as_deref().unwrap_or("—")).strong());
                                if !app.project_git_local_branches.is_empty() {
                                    egui::ComboBox::from_id_salt("project_git_branch")
                                        .selected_text(if app.project_git_selected_branch.is_empty() { "—" } else { &app.project_git_selected_branch })
                                        .show_ui(ui, |ui| {
                                            for name in &app.project_git_local_branches {
                                                if ui.selectable_label(app.project_git_selected_branch == *name, name).clicked() {
                                                    app.project_git_selected_branch = name.clone();
                                                }
                                            }
                                        });
                                    if ui.button("Cambiar rama").clicked() && !app.project_git_selected_branch.is_empty() {
                                        if let Ok(repo) = microtermi_core::open_repo(&path) {
                                            if microtermi_core::checkout_branch(&repo, &app.project_git_selected_branch).is_ok() {
                                                app.message = format!("Rama cambiada a {}", app.project_git_selected_branch);
                                                app.refresh_project_git(&path);
                                            } else {
                                                app.message = "Error al cambiar rama.".to_string();
                                            }
                                        }
                                    }
                                }
                            });
                            ui.horizontal(|ui| {
                                ui.label("Rama remota (origin):");
                                if !app.project_git_remote_branches.is_empty() {
                                    egui::ComboBox::from_id_salt("project_git_remote_branch")
                                        .selected_text(if app.project_git_selected_remote_branch.is_empty() { "—" } else { &app.project_git_selected_remote_branch })
                                        .show_ui(ui, |ui| {
                                            for name in &app.project_git_remote_branches {
                                                if ui.selectable_label(app.project_git_selected_remote_branch == *name, name).clicked() {
                                                    app.project_git_selected_remote_branch = name.clone();
                                                }
                                            }
                                        });
                                    if ui.button("Cambiar a rama remota").clicked() && !app.project_git_selected_remote_branch.is_empty() {
                                        if let Ok(repo) = microtermi_core::open_repo(&path) {
                                            match microtermi_core::checkout_remote_branch(&repo, &app.project_git_selected_remote_branch) {
                                                Ok(()) => {
                                                    app.message = format!("Cambiado a rama remota {}", app.project_git_selected_remote_branch);
                                                    app.refresh_project_git(&path);
                                                }
                                                Err(e) => app.message = format!("Error: {}", e),
                                            }
                                        }
                                    }
                                } else {
                                    ui.label(egui::RichText::new("Pulsa Fetch para cargar ramas remotas.").color(ui.visuals().weak_text_color()));
                                }
                            });
                            if let Some(clean) = app.project_git_clean {
                                if clean {
                                    ui.label(egui::RichText::new("Estado: limpio").color(ui.visuals().weak_text_color()));
                                } else {
                                    ui.label("Archivos modificados:");
                                    for f in &app.project_git_modified {
                                        ui.label(egui::RichText::new(f).font(egui::FontId::monospace(12.0)));
                                    }
                                }
                            }
                            ui.horizontal(|ui| {
                                ui.label("Mensaje:");
                                ui.text_edit_singleline(&mut app.commit_message);
                                if ui.button("Commit").clicked() {
                                    if let Ok(repo) = microtermi_core::open_repo(&path) {
                                        let path_refs: Vec<&Path> = app.project_git_modified.iter().map(|s| Path::new(s)).collect();
                                        if let Err(e) = microtermi_core::commit(&repo, &app.commit_message, &path_refs) {
                                            app.message = format!("Error: {}", e);
                                        } else {
                                            app.message = "Commit realizado.".to_string();
                                            app.commit_message.clear();
                                            app.refresh_project_git(&path);
                                        }
                                    }
                                }
                            });
                            ui.label("Historial de commits:");
                            let mut click_log: Option<usize> = None;
                            egui::ScrollArea::vertical().max_height(180.0).show(ui, |ui| {
                                for (i, c) in app.project_git_log.iter().enumerate() {
                                    let selected = app.project_git_log_selected == Some(i);
                                    let line = format!("{}  {}  {}  {}", c.id_short, c.date, c.author, c.message);
                                    if ui.selectable_label(selected, line).clicked() {
                                        click_log = Some(i);
                                    }
                                }
                            });
                            if let Some(i) = click_log {
                                app.project_git_log_selected = Some(i);
                                if i < app.project_git_log.len() {
                                    if let Ok(repo) = microtermi_core::open_repo(&path) {
                                        app.project_git_commit_detail = microtermi_core::commit_changes(&repo, &app.project_git_log[i].id_short).unwrap_or_default();
                                    }
                                }
                            }
                            if let Some(i) = app.project_git_log_selected {
                                if i < app.project_git_log.len() && !app.project_git_commit_detail.is_empty() {
                                    ui.label("Archivos en este commit:");
                                    for f in &app.project_git_commit_detail {
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
            if !app.message.is_empty() {
                ui.add_space(8.0);
                crate::shared::message_label(ui, &app.message);
            }
        });
    });
    if let Some(p) = pending_run_script {
        app.pending_run = Some(p);
    }
}
