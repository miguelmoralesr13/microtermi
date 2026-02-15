use eframe::egui;

use crate::MicrotermiApp;

pub fn draw(app: &mut MicrotermiApp, ctx: &egui::Context) {
    egui::SidePanel::left("coverage_sidebar")
        .resizable(true)
        .default_width(260.0)
        .show(ctx, |ui| {
            ui.heading("Coverage");
            ui.label(
                egui::RichText::new("Elige un proyecto. Ejecuta tests (npm run test) y abre el reporte HTML en coverage/lcov-report/.")
                    .small()
                    .color(ui.visuals().weak_text_color()),
            );
            ui.add_space(8.0);
            if app.projects.is_empty() && app.root_path.is_some() {
                ui.label("No se encontraron proyectos.");
            } else if app.projects.is_empty() {
                ui.label("Selecciona una carpeta raíz primero.");
            } else {
                for (i, p) in app.projects.iter().enumerate() {
                    let selected = app.coverage_selected_project == Some(i);
                    if ui.selectable_label(selected, &p.name).clicked() {
                        app.coverage_selected_project = Some(i);
                    }
                }
            }
        });

    egui::CentralPanel::default().show(ctx, |ui| {
        egui::ScrollArea::both().show(ui, |ui| {
            ui.heading("Reporte de cobertura");
            if let Some(idx) = app.coverage_selected_project {
                if let Some(project) = app.projects.get(idx) {
                    let report_path = project
                        .path
                        .join("coverage")
                        .join("lcov-report")
                        .join("index.html");
                    let has_test_script = project.scripts.iter().any(|(s, _)| s == "test");
                    let report_exists = report_path.exists();

                    ui.horizontal(|ui| {
                        ui.label(format!("Proyecto: {}", project.name));
                    });
                    ui.add_space(6.0);
                    ui.label(
                        egui::RichText::new(format!(
                            "Ruta del reporte: {}",
                            report_path.display()
                        ))
                        .small()
                        .color(ui.visuals().weak_text_color()),
                    );
                    ui.add_space(8.0);

                    if has_test_script {
                        if ui.button("Ejecutar tests (npm run test)").clicked() {
                            app.coverage_run_tests_for_project(idx);
                        }
                        ui.label(
                            egui::RichText::new("Los tests se ejecutan en una terminal; al terminar podrás abrir el reporte.")
                                .small()
                                .color(ui.visuals().weak_text_color()),
                        );
                    } else {
                        ui.label(
                            egui::RichText::new("Este proyecto no tiene script \"test\" en package.json.")
                                .color(ui.visuals().weak_text_color()),
                        );
                    }

                    ui.add_space(12.0);
                    if report_exists {
                        if ui.button("Abrir en navegador").clicked() {
                            if let Err(e) = opener::open(&report_path) {
                                app.message = format!("Error al abrir: {}", e);
                            }
                        }
                    } else {
                        ui.label(
                            egui::RichText::new("No hay reporte aún. Ejecuta los tests primero (npm run test con cobertura).")
                                .color(ui.visuals().weak_text_color()),
                        );
                    }
                } else {
                    ui.label("Proyecto no encontrado.");
                }
            } else {
                ui.label(
                    egui::RichText::new("Selecciona un proyecto en la lista de la izquierda.")
                        .color(ui.visuals().weak_text_color()),
                );
            }
        });
    });
}
