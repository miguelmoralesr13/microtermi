use eframe::egui;

use crate::MicrotermiApp;

pub fn draw(app: &mut MicrotermiApp, ctx: &egui::Context) {
    egui::SidePanel::left("multi_run_sidebar")
        .resizable(true)
        .default_width(280.0)
        .show(ctx, |ui| {
            ui.heading("Multi-run");
            ui.label(egui::RichText::new("Selecciona proyectos y un script; luego «Ejecutar en seleccionados». Or añade terminales y elige proyecto+script en cada una.").small().color(ui.visuals().weak_text_color()));
            ui.add_space(6.0);
            if app.projects.is_empty() && app.root_path.is_some() {
                ui.label("No se encontraron package.json");
            }
            for (i, p) in app.projects.iter().enumerate() {
                let selected = app.multi_run_selected.contains(&i);
                let mut sel = selected;
                if ui.checkbox(&mut sel, &p.name).changed() {
                    if sel {
                        app.multi_run_selected.insert(i);
                    } else {
                        app.multi_run_selected.remove(&i);
                    }
                    app.persist_app_config();
                }
            }
            ui.add_space(6.0);
            ui.label("Script:");
            let common = app.common_script_names();
            if !common.is_empty() {
                if !common.contains(&app.multi_run_script) {
                    app.multi_run_script = common.first().cloned().unwrap_or_default();
                }
                let idx_script = common.iter().position(|s| s == &app.multi_run_script).unwrap_or(0);
                egui::ComboBox::from_id_salt("multi_run_script")
                    .selected_text(&app.multi_run_script)
                    .show_ui(ui, |ui| {
                        for (i, name) in common.iter().enumerate() {
                            if ui.selectable_label(idx_script == i, name).clicked() {
                                app.multi_run_script = name.clone();
                                app.persist_app_config();
                            }
                        }
                    });
            } else {
                if ui.text_edit_singleline(&mut app.multi_run_script).lost_focus() {
                    app.persist_app_config();
                }
            }
            if ui.button("Ejecutar en seleccionados").clicked() {
                app.multi_run_click();
            }
            if ui.button("Añadir terminal").clicked() {
                app.multi_run_add_placeholder();
            }
            if !app.message.is_empty() {
                ui.add_space(4.0);
                ui.label(egui::RichText::new(&app.message).small().color(ui.visuals().weak_text_color()));
            }
        });

    let sessions = app.terminal_sessions.len();
    egui::CentralPanel::default().show(ctx, |ui| {
        egui::ScrollArea::both().show(ui, |ui| {
            ui.horizontal(|ui| {
                ui.strong("Terminales");
                let any_running = app.terminal_sessions.iter().any(|s| s.child.is_some());
                if any_running && ui.button("Detener todos").clicked() {
                    app.terminal_stop_all();
                }
            });
            if sessions == 0 {
                ui.label(egui::RichText::new("Selecciona proyectos a la izquierda y pulsa «Ejecutar en seleccionados», o «Añadir terminal» y elige proyecto + script en cada pestaña.").color(ui.visuals().weak_text_color()));
            } else {
                ui.separator();
                ui.horizontal(|ui| {
                    let mut close_tab = None;
                    let mut run_again_tab = None;
                    for (i, session) in app.terminal_sessions.iter().enumerate() {
                        let running = session.child.is_some();
                        let label = if running {
                            format!("● {}", session.name)
                        } else {
                            session.name.clone()
                        };
                        let selected = app.selected_terminal_tab == i;
                        if ui.selectable_label(selected, &label).clicked() {
                            app.selected_terminal_tab = i;
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
                        app.multi_run_placeholder_execute(i);
                    }
                    if let Some(i) = close_tab {
                        app.multi_run_close_pane(i);
                    }
                });
                ui.add_space(4.0);
                let font_id = egui::FontId::monospace(12.0);
                let idx = app.selected_terminal_tab.min(app.terminal_sessions.len().saturating_sub(1));
                let (close_tab, run_placeholder, stop_at) =
                    app.draw_multi_run_session_content(ui, idx, &font_id);
                if let Some(i) = close_tab {
                    app.multi_run_close_pane(i);
                }
                if let Some(i) = stop_at {
                    app.terminal_stop_at(i);
                }
                if let Some(i) = run_placeholder {
                    app.multi_run_placeholder_execute(i);
                }
            }
        });
    });
}
