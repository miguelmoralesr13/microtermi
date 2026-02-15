// No abrir ventana de consola al ejecutar el .exe en Windows (app de escritorio)
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

fn main() -> eframe::Result<()> {
    eframe::run_native(
        "Microtermi",
        eframe::NativeOptions::default(),
        Box::new(|cc| Ok(Box::new(microtermi_gui::MicrotermiApp::new(cc)))),
    )
}
