// No abrir ventana de consola al ejecutar el .exe en Windows (app de escritorio)
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

fn main() -> eframe::Result<()> {
    eframe::run_native(
        "Microtermi",
        eframe::NativeOptions {
            viewport: egui::ViewportBuilder::default()
                .with_inner_size(egui::vec2(1000.0, 700.0))
                .with_max_inner_size(egui::vec2(1400.0, 900.0)),
            ..Default::default()
        },
        Box::new(|cc| Ok(Box::new(microtermi_gui::MicrotermiApp::new(cc)))),
    )
}
