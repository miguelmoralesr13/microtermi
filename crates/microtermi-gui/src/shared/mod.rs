//! Helpers y tipos compartidos entre las pestañas de la GUI.

use eframe::egui;

/// Pinta un mensaje de estado en color débil (para no repetir el mismo bloque en varias tabs).
pub fn message_label(ui: &mut egui::Ui, message: &str) {
    if !message.is_empty() {
        ui.add_space(8.0);
        ui.label(egui::RichText::new(message).color(ui.visuals().weak_text_color()));
    }
}
