//! 翻译历史弹窗 UI 模块。

use crate::ui::state::STATE;

/// 绘制翻译历史弹窗
pub fn draw_history(ctx: &egui::Context) {
    let mut open = true;
    let mut clicked_entry: Option<(String, String, String)> = None;
    let mut clear_all = false;

    egui::Window::new("翻译历史")
        .id(egui::Id::new("history_window"))
        .open(&mut open)
        .resizable(true)
        .collapsible(false)
        .default_width(560.0)
        .default_height(380.0)
        .min_width(360.0)
        .min_height(240.0)
        .anchor(egui::Align2::CENTER_CENTER, [0.0, 0.0])
        .show(ctx, |ui| {
            let count = crate::history::get_all().len();

            // 顶栏
            ui.horizontal(|ui| {
                ui.label(
                    egui::RichText::new(format!("共 {} 条记录（最多 50 条）", count))
                        .small()
                        .color(crate::theme::history_meta()),
                );
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    if ui.button("🗑 清空").clicked() {
                        clear_all = true;
                    }
                });
            });
            ui.separator();

            if clear_all {
                crate::history::clear();
            }

            let history = crate::history::get_all();
            if history.is_empty() {
                ui.add_space(40.0);
                ui.vertical_centered(|ui| {
                    ui.label(
                        egui::RichText::new("暂无翻译历史")
                            .color(crate::theme::history_empty()),
                    );
                });
            } else {
                egui::ScrollArea::vertical()
                    .auto_shrink([false; 2])
                    .show(ui, |ui| {
                        for entry in &history {
                            let frame = egui::Frame::group(ui.style())
                                .inner_margin(8.0)
                                .stroke(egui::Stroke::new(0.5_f32, crate::theme::history_border()));
                            let resp = frame.show(ui, |ui| {
                                ui.horizontal_wrapped(|ui| {
                                    ui.label(
                                        egui::RichText::new(&entry.source)
                                            .color(crate::theme::text_source()),
                                    );
                                    ui.label(
                                        egui::RichText::new("→")
                                            .color(crate::theme::arrow_color()),
                                    );
                                    ui.label(
                                        egui::RichText::new(&entry.translated)
                                            .color(crate::theme::text_translated())
                                            .strong(),
                                    );
                                });
                                ui.add_space(2.0);
                                ui.label(
                                    egui::RichText::new(format!(
                                        "{} → {} · {} · {}",
                                        entry.from, entry.to, entry.engine,
                                        format_timestamp(entry.timestamp)
                                    ))
                                    .small()
                                    .color(crate::theme::history_meta()),
                                );
                            });
                            if resp.response.clicked() {
                                clicked_entry = Some((
                                    entry.source.clone(),
                                    entry.from.clone(),
                                    entry.to.clone(),
                                ));
                            }
                            ui.add_space(2.0);
                        }
                    });
            }
        });

    // 窗口关闭
    if !open {
        STATE.lock().unwrap().show_history = false;
    }

    // 点击复用：只填入输入框和语言对，关闭弹窗（不自动翻译，避免阻塞）
    if let Some((source, from, to)) = clicked_entry {
        let mut s = STATE.lock().unwrap();
        s.input_text = source;
        s.from_lang = from;
        s.to_lang = to;
        s.show_history = false;
    }
}

/// 将 Unix 秒格式化为 HH:MM 显示
fn format_timestamp(ts: u64) -> String {
    let dt = chrono::DateTime::from_timestamp(ts as i64, 0)
        .map(|t| t.with_timezone(&chrono::Local))
        .map(|t| t.format("%H:%M").to_string());
    dt.unwrap_or_else(|| "--:--".into())
}
