use eframe::egui;
use std::path::{Path, PathBuf};

use crate::MicrotermiApp;

pub fn draw(app: &mut MicrotermiApp, ctx: &egui::Context) {
    egui::SidePanel::left("git_repos")
        .resizable(true)
        .default_width(320.0)
        .show(ctx, |ui| {
            ui.heading("Repositorios GitLab");
            ui.horizontal(|ui| {
                ui.label("Filtro:");
                let resp = ui.text_edit_singleline(&mut app.gitlab_repo_filter);
                if resp.has_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter)) && !app.gitlab_loading {
                    app.gitlab_list_projects();
                }
            });
            ui.horizontal(|ui| {
                let buscar_clicked = ui.button("Buscar").on_hover_text("Lista proyectos en GitLab con el filtro actual").clicked();
                if buscar_clicked && !app.gitlab_loading {
                    app.gitlab_list_projects();
                }
                let listar_enabled = !app.gitlab_loading;
                let listar_btn = ui.add_enabled(listar_enabled, egui::Button::new("Listar proyectos"));
                if listar_btn.clicked() {
                    app.gitlab_list_projects();
                }
                if app.gitlab_loading {
                    ui.spinner();
                    ui.label("Buscando…");
                    ui.ctx().request_repaint();
                }
            });
            if !app.gitlab_filter_applied.is_empty() {
                ui.label(egui::RichText::new(format!("Filtro aplicado: \"{}\"", app.gitlab_filter_applied)).color(ui.visuals().weak_text_color()));
            }
            if !app.gitlab_status.is_empty() {
                ui.label(egui::RichText::new(&app.gitlab_status).color(ui.visuals().weak_text_color()));
            }
            ui.add_space(4.0);
            let grouped = app.gitlab_projects_grouped();
            let mut click_project: Option<usize> = None;
            let selected = app.selected_gitlab_project;
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
            if grouped.is_empty() && !app.gitlab_projects.is_empty() {
                ui.label("(ningún proyecto coincide con el filtro)");
            } else if grouped.is_empty() {
                ui.label("Pulsa «Listar proyectos» para cargar repos.");
            }
            if let Some(i) = click_project {
                app.gitlab_select_project(i);
            }
        });

    egui::CentralPanel::default().show(ctx, |ui| {
        egui::ScrollArea::both().show(ui, |ui| {
            ui.heading("Repositorio local");
            let root_clone: Option<PathBuf> = app.git_root();
            if let Some(ref root) = root_clone {
                if microtermi_core::open_repo(root).is_ok() {
                    if app.git_repo_path.is_some() {
                        ui.horizontal(|ui| {
                            ui.label(egui::RichText::new("Repo:").weak());
                            ui.label(egui::RichText::new(root.display().to_string()).small());
                            if ui.small_button("Usar carpeta raíz").clicked() {
                                app.git_repo_path = None;
                                app.refresh_git();
                            }
                        });
                    }
                    ui.horizontal(|ui| {
                        ui.strong("Acciones:");
                        if ui.button("Pull").clicked() {
                            if let Ok(repo) = microtermi_core::open_repo(root) {
                                match microtermi_core::pull(&repo) {
                                    Ok(msg) => app.message = msg,
                                    Err(e) => app.message = format!("Error: {}", e),
                                }
                                app.refresh_git();
                            }
                        }
                        if ui.button("Push").clicked() {
                            if let Ok(repo) = microtermi_core::open_repo(root) {
                                match microtermi_core::push(&repo) {
                                    Ok(msg) => app.message = msg,
                                    Err(e) => app.message = format!("Error: {}", e),
                                }
                                app.refresh_git();
                            }
                        }
                        if ui.button("Refrescar").clicked() {
                            app.refresh_git();
                        }
                    });
                    ui.add_space(4.0);
                    ui.horizontal(|ui| {
                        ui.label("Rama:");
                        ui.label(egui::RichText::new(app.git_branch.as_deref().unwrap_or("—")).strong());
                        if !app.git_local_branches.is_empty() {
                            egui::ComboBox::from_id_salt("git_branch_tab")
                                .selected_text(if app.selected_git_branch.is_empty() { "—" } else { &app.selected_git_branch })
                                .show_ui(ui, |ui| {
                                    for name in &app.git_local_branches {
                                        if ui.selectable_label(app.selected_git_branch == *name, name).clicked() {
                                            app.selected_git_branch = name.clone();
                                        }
                                    }
                                });
                            if ui.button("Cambiar rama").clicked() {
                                app.git_checkout_branch();
                            }
                        }
                    });
                    if let Some(clean) = app.git_clean {
                        if clean {
                            ui.label(egui::RichText::new("Estado: limpio").color(ui.visuals().weak_text_color()));
                        } else {
                            ui.label("Archivos modificados:");
                            for f in &app.git_modified {
                                ui.label(egui::RichText::new(f).font(egui::FontId::monospace(12.0)));
                            }
                        }
                    }
                    ui.add_space(4.0);
                    ui.horizontal(|ui| {
                        ui.label("Mensaje:");
                        ui.text_edit_singleline(&mut app.commit_message);
                        if ui.button("Commit").clicked() {
                            if let Ok(repo) = microtermi_core::open_repo(root) {
                                let path_refs: Vec<&Path> = app.git_modified.iter().map(|s| Path::new(s)).collect();
                                if let Err(e) = microtermi_core::commit(&repo, &app.commit_message, &path_refs) {
                                    app.message = format!("Error: {}", e);
                                } else {
                                    app.message = "Commit realizado.".to_string();
                                    app.commit_message.clear();
                                    app.refresh_git();
                                }
                            }
                        }
                    });
                    ui.add_space(12.0);
                    ui.separator();
                    ui.add_space(8.0);
                    ui.heading("Historial de commits");
                    let mut click_log: Option<usize> = None;
                    egui::ScrollArea::vertical().max_height(220.0).show(ui, |ui| {
                        for (i, c) in app.git_log.iter().enumerate() {
                            let selected = app.git_log_selected == Some(i);
                            let line = format!("{}  {}  {}  {}", c.id_short, c.date, c.author, c.message);
                            if ui.selectable_label(selected, line).clicked() {
                                click_log = Some(i);
                            }
                        }
                    });
                    if let Some(idx) = click_log {
                        app.git_log_selected = Some(idx);
                        if idx < app.git_log.len() {
                            if let Ok(repo) = microtermi_core::open_repo(root) {
                                app.git_commit_detail = microtermi_core::commit_changes(&repo, &app.git_log[idx].id_short).unwrap_or_default();
                            }
                        }
                    }
                    if let Some(idx) = app.git_log_selected {
                        if idx < app.git_log.len() && !app.git_commit_detail.is_empty() {
                            ui.add_space(4.0);
                            ui.label("Archivos en este commit:");
                            for f in &app.git_commit_detail {
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
            if !app.message.is_empty() {
                ui.add_space(8.0);
                ui.label(egui::RichText::new(&app.message).color(ui.visuals().weak_text_color()));
            }

            ui.add_space(16.0);
            ui.separator();
            ui.add_space(8.0);
            if let Some(idx) = app.selected_gitlab_project {
                if idx < app.gitlab_projects.len() {
                    let proj = &app.gitlab_projects[idx];
                    ui.heading("Proyecto GitLab seleccionado");
                    ui.label(egui::RichText::new(&proj.path_with_namespace).strong());
                    ui.label(egui::RichText::new(proj.web_url.as_str()).color(ui.visuals().weak_text_color()));
                    ui.horizontal(|ui| {
                        if ui.button("Clonar en carpeta raíz").clicked() {
                            app.gitlab_clone_to_root(idx);
                        }
                        if ui.button("Clonar en otra carpeta…").clicked() {
                            app.gitlab_clone(idx);
                        }
                    });
                    ui.add_space(4.0);
                    ui.label("Ramas (GitLab):");
                    egui::ScrollArea::vertical().max_height(180.0).show(ui, |ui| {
                        for b in &app.gitlab_branches {
                            ui.label(&b.name);
                        }
                    });
                }
            } else {
                ui.label(egui::RichText::new("Selecciona un proyecto en la lista de la izquierda para ver ramas y clonar.").color(ui.visuals().weak_text_color()));
            }
        });
    });
}
