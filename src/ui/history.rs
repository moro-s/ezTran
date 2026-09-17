//! 翻译历史弹窗 UI 模块。

use crate::ui::state::STATE;

/// 绘制翻译历史弹窗
pub fn draw_history(ctx: &egui::Context) {
    log::info!("[history] draw_history 开始");
    let mut open = true;
    let mut clicked_entry: Option<(String, String, String)> = None;
    let mut clear_all = false;

    log::info!("[history] 准备创建 Window");
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
            log::info!("[history] Window show 闭包开始");
            let count = crate::history::get_all().len();
            log::info!("[history] history count={}", count);

            // 顶栏
            ui.horizontal(|ui| {
                ui.label(
                    egui::RichText::new(format!("共 {} 条记录（最多 50 条）", count))
                        .small()
                        .color(egui::Color32::from_gray(120)),
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
                            .color(egui::Color32::from_gray(100)),
                    );
                });
            } else {
                egui::ScrollArea::vertical()
                    .auto_shrink([false; 2])
                    .show(ui, |ui| {
                        for entry in &history {
                            let frame = egui::Frame::group(ui.style())
                                .inner_margin(8.0)
                                .stroke(egui::Stroke::new(0.5_f32, egui::Color32::from_gray(70)));
                            let resp = frame.show(ui, |ui| {
                                ui.horizontal_wrapped(|ui| {
                                    ui.label(
                                        egui::RichText::new(&entry.source)
                                            .color(egui::Color32::from_gray(200)),
                                    );
                                    ui.label(
                                        egui::RichText::new("→")
                                            .color(egui::Color32::from_gray(120)),
                                    );
                                    ui.label(
                                        egui::RichText::new(&entry.translated)
                                            .color(egui::Color32::from_gray(230))
                                            .strong(),
                                    );
                                });
                                ui.add_space(2.0);
                                ui.label(
                                    egui::RichText::new(format!(
                                        "{} → {} · {}",
                                        entry.from, entry.to, entry.engine
                                    ))
                                    .small()
                                    .color(egui::Color32::from_gray(110)),
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
            log::info!("[history] Window show 闭包结束");
        });

    log::info!("[history] Window show 返回, open={}", open);

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
    log::info!("[history] draw_history 结束");
}
