/// 加载系统中文字体并注入 egui
pub fn setup_fonts(ctx: &egui::Context) {
    use std::sync::Once;
    static INIT: Once = Once::new();
    INIT.call_once(|| {
        let mut fonts = egui::FontDefinitions::default();

        let candidates: &[&str] = &[
            "C:\\Windows\\Fonts\\simhei.ttf",
            "C:\\Windows\\Fonts\\Deng.ttf",
            "C:\\Windows\\Fonts\\msyh.ttc",
            "C:\\Windows\\Fonts\\simsun.ttc",
            "/Library/Fonts/Arial Unicode.ttf",
            "/System/Library/Fonts/PingFang.ttc",
            "/System/Library/Fonts/STHeiti Medium.ttc",
            "/usr/share/fonts/truetype/wqy/wqy-microhei.ttc",
            "/usr/share/fonts/truetype/noto/NotoSansCJK-Regular.ttc",
            "/usr/share/fonts/opentype/noto/NotoSansCJK-Regular.ttc",
            "/usr/share/fonts/wqy-microhei/wqy-microhei.ttc",
        ];

        for path in candidates {
            if let Ok(data) = std::fs::read(path) {
                if !is_valid_font(&data) {
                    continue;
                }
                fonts
                    .font_data
                    .insert("chinese".to_owned(), egui::FontData::from_owned(data));
                fonts
                    .families
                    .entry(egui::FontFamily::Proportional)
                    .or_default()
                    .push("chinese".to_owned());
                fonts
                    .families
                    .entry(egui::FontFamily::Monospace)
                    .or_default()
                    .push("chinese".to_owned());
                break;
            }
        }

        // 加载系统符号字体，用于显示箭头等 Unicode 符号（如 ⇄）
        let symbol_candidates: &[&str] = &[
            "C:\\Windows\\Fonts\\seguisym.ttf",
            "C:\\Windows\\Fonts\\segoeui.ttf",
        ];
        for path in symbol_candidates {
            if let Ok(data) = std::fs::read(path) {
                if !is_valid_font(&data) {
                    continue;
                }
                fonts
                    .font_data
                    .insert("symbols".to_owned(), egui::FontData::from_owned(data));
                fonts
                    .families
                    .entry(egui::FontFamily::Proportional)
                    .or_default()
                    .push("symbols".to_owned());
                fonts
                    .families
                    .entry(egui::FontFamily::Monospace)
                    .or_default()
                    .push("symbols".to_owned());
                break;
            }
        }

        ctx.set_fonts(fonts);
    });
}

fn is_valid_font(data: &[u8]) -> bool {
    if data.len() < 4 {
        return false;
    }
    let magic = u32::from_be_bytes([data[0], data[1], data[2], data[3]]);
    matches!(
        magic,
        0x00010000 | 0x4F54544F | 0x74727565 | 0x74797031
    )
}

/// 设置 egui 深色主题配色
pub fn setup_style(ctx: &egui::Context) {
    use std::sync::Once;
    static INIT: Once = Once::new();
    INIT.call_once(|| {
        let mut style = (*ctx.style()).clone();
        style.spacing.item_spacing = egui::vec2(6.0, 6.0);
        style.spacing.button_padding = egui::vec2(10.0, 4.0);
        style.spacing.window_margin = egui::Margin::same(0.0);
        ctx.set_style(style);

        let mut vis = egui::Visuals::dark();
        vis.panel_fill = egui::Color32::from_rgb(43, 43, 43);
        vis.window_fill = egui::Color32::from_rgb(30, 30, 30);
        vis.widgets.noninteractive.bg_fill = egui::Color32::from_rgb(43, 43, 43);
        vis.widgets.inactive.bg_fill = egui::Color32::from_rgb(60, 60, 60);
        vis.widgets.hovered.bg_fill = egui::Color32::from_rgb(76, 76, 76);
        vis.widgets.active.bg_fill = egui::Color32::from_rgb(80, 80, 80);
        vis.widgets.noninteractive.fg_stroke =
            egui::Stroke::new(1.0_f32, egui::Color32::from_rgb(224, 224, 224));
        vis.widgets.inactive.fg_stroke =
            egui::Stroke::new(1.0_f32, egui::Color32::from_rgb(224, 224, 224));
        vis.widgets.noninteractive.bg_stroke = egui::Stroke::new(0.0_f32, egui::Color32::TRANSPARENT);
        vis.widgets.inactive.bg_stroke = egui::Stroke::new(0.0_f32, egui::Color32::TRANSPARENT);
        vis.selection.bg_fill = egui::Color32::from_rgb(137, 180, 250);
        vis.selection.stroke = egui::Stroke::new(1.0_f32, egui::Color32::from_rgb(137, 180, 250));
        vis.hyperlink_color = egui::Color32::from_rgb(137, 180, 250);
        ctx.set_visuals(vis);
    });
}
