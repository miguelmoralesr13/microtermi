//! Utilidades para salida de terminal con códigos ANSI (colores, negrita).

use eframe::egui;

/// Segmento de una línea con posible color ANSI aplicado.
pub struct AnsiSegment {
    pub text: String,
    pub color: Option<egui::Color32>,
    pub bold: bool,
}

/// Elimina códigos de escape ANSI sin corromper UTF-8.
pub fn strip_ansi(s: &str) -> String {
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

/// Parsea una línea que puede contener códigos ANSI SGR y devuelve segmentos para pintar con color.
pub fn parse_ansi_line(s: &str) -> Vec<AnsiSegment> {
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
