use eframe::egui;

use crate::MicrotermiApp;

pub fn draw(app: &mut MicrotermiApp, ctx: &egui::Context) {
    egui::CentralPanel::default().show(ctx, |ui| {
        egui::ScrollArea::both().show(ui, |ui| {
            ui.heading("Configuración");
            ui.add_space(8.0);
            ui.collapsing("GitLab", |ui| {
                ui.horizontal(|ui| {
                    ui.label("URL:");
                    ui.text_edit_singleline(&mut app.gitlab_url);
                });
                ui.horizontal(|ui| {
                    ui.label("Token:");
                    ui.add(
                        egui::TextEdit::singleline(&mut app.gitlab_token)
                            .password(true)
                            .desired_width(280.0),
                    );
                });
                if ui.button("Guardar").clicked() {
                    app.persist_app_config();
                    app.message = "GitLab guardado.".to_string();
                }
            });
            ui.add_space(12.0);
            ui.collapsing("Carpeta raíz", |ui| {
                if let Some(p) = app.root_path.as_ref() {
                    ui.label(p.display().to_string());
                } else {
                    ui.label("(no seleccionada)");
                }
                if ui.button("Cambiar carpeta raíz").clicked() {
                    if let Some(path) = rfd::FileDialog::new().pick_folder() {
                        app.root_path = Some(path.clone());
                        app.persist_app_config();
                        app.refresh_projects();
                        app.refresh_git();
                        app.refresh_env();
                        app.refresh_git_branches();
                    }
                }
            });
            crate::shared::message_label(ui, &app.message);
        });
    });
}
