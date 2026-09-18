//! 翻译历史弹窗 UI 模块（独立 OS 窗口）。

use crate::ui::state::STATE;
use std::sync::atomic::{AtomicBool, Ordering};

/// 列宽配置：时间、源文本、译文、引擎、语言
const COL_WIDTHS: [f32; 5] = [52.0, 200.0, 200.0, 60.0, 64.0];

/// 标记历史窗口已首帧渲染完毕（用于消除闪屏）
static HISTORY_SHOWN: AtomicBool = AtomicBool::new(false);

/// 重置首帧标记（打开窗口时调用）
pub fn reset_shown() {
    HISTORY_SHOWN.store(false, Ordering::SeqCst);
}

/// 绘制翻译历史窗口（独立视口）
pub fn draw_history(ctx: &egui::Context) {
    let (win_w, win_h) = (720.0, 480.0);
    let (screen_w, screen_h) = crate::window::get_screen_size();
    let center_pos = egui::pos2(
        ((screen_w as f32 - win_w) / 2.0).max(0.0),
        ((screen_h as f32 - win_h) / 2.0).max(0.0),
    );

    ctx.show_viewport_immediate(
        egui::ViewportId::from_hash_of("history"),
        egui::ViewportBuilder::default()
            .with_title("翻译历史")
            .with_inner_size([win_w, win_h])
            .with_min_inner_size([420.0, 300.0])
            .with_position(center_pos)
            .with_visible(false),
        |ui, _class| {
            // 关闭按钮 → 隐藏历史窗口
            if ui.ctx().input(|i| i.viewport().close_requested()) {
                ui.ctx().send_viewport_cmd(egui::ViewportCommand::CancelClose);
                crate::ui::state::hide_history_window();
                return;
            }

            egui::CentralPanel::default().show(ui, |ui| {
                let mut clicked_entry: Option<(String, String, String)> = None;
                let mut clear_all = false;

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
                        .max_width(ui.available_width())
                        .show(ui, |ui| {
                            // 表头
                            ui.horizontal(|ui| {
                                let headers = ["时间", "源文本", "译文", "引擎", "语言"];
                                for (i, h) in headers.iter().enumerate() {
                                    ui.add_sized(
                                        [COL_WIDTHS[i], 0.0],
                                        egui::Label::new(
                                            egui::RichText::new(*h)
                                                .small()
                                                .color(crate::theme::history_meta()),
                                        ),
                                    );
                                }
                            });
                            ui.separator();

                            // 数据行
                            for (row_i, entry) in history.iter().enumerate() {
                                let frame = if row_i % 2 == 1 {
                                    egui::Frame::NONE
                                        .inner_margin(egui::Margin::symmetric(4, 2))
                                        .fill(crate::theme::history_row_alt())
                                } else {
                                    egui::Frame::NONE
                                        .inner_margin(egui::Margin::symmetric(4, 2))
                                };

                                let row = frame.show(ui, |ui| {
                                    ui.horizontal(|ui| {
                                        let r_time = ui.add_sized(
                                            [COL_WIDTHS[0], 0.0],
                                            egui::Label::new(
                                                egui::RichText::new(format_timestamp(entry.timestamp))
                                                    .small()
                                                    .color(crate::theme::history_meta()),
                                            )
                                            .sense(egui::Sense::click()),
                                        );
                                        let r_src = ui.add_sized(
                                            [COL_WIDTHS[1], 0.0],
                                            egui::Label::new(
                                                egui::RichText::new(truncate_str(&entry.source, 28))
                                                    .color(crate::theme::text_source()),
                                            )
                                            .sense(egui::Sense::click()),
                                        );
                                        let r_tr = ui.add_sized(
                                            [COL_WIDTHS[2], 0.0],
                                            egui::Label::new(
                                                egui::RichText::new(truncate_str(&entry.translated, 28))
                                                    .color(crate::theme::text_translated())
                                                    .strong(),
                                            )
                                            .sense(egui::Sense::click()),
                                        );
                                        let r_engine = ui.add_sized(
                                            [COL_WIDTHS[3], 0.0],
                                            egui::Label::new(
                                                egui::RichText::new(&entry.engine)
                                                    .small()
                                                    .color(crate::theme::history_meta()),
                                            )
                                            .sense(egui::Sense::click()),
                                        );
                                        let r_lang = ui.add_sized(
                                            [COL_WIDTHS[4], 0.0],
                                            egui::Label::new(
                                                egui::RichText::new(format!("{}→{}", entry.from, entry.to))
                                                    .small()
                                                    .color(crate::theme::history_meta()),
                                            )
                                            .sense(egui::Sense::click()),
                                        );

                                        if r_time.clicked()
                                            || r_src.clicked()
                                            || r_tr.clicked()
                                            || r_engine.clicked()
                                            || r_lang.clicked()
                                        {
                                            clicked_entry = Some((
                                                entry.source.clone(),
                                                entry.from.clone(),
                                                entry.to.clone(),
                                            ));
                                        }
                                    });
                                });

                                // hover 高亮
                                if row.response.hovered() {
                                    let rect = row.response.rect;
                                    ui.painter().rect_filled(
                                        rect,
                                        0.0,
                                        egui::Color32::from_rgba_unmultiplied(100, 149, 237, 30),
                                    );
                                }
                            }
                        });
                }

                // 底栏：清空按钮居中
                ui.separator();
                ui.vertical_centered(|ui| {
                    if ui.button("🗑 清空历史").clicked() {
                        clear_all = true;
                    }
                });

                if clear_all {
                    crate::history::clear();
                }

                // 点击复用：只填入输入框和语言对，关闭弹窗（不自动翻译，避免阻塞）
                if let Some((source, from, to)) = clicked_entry {
                    let mut s = STATE.lock().unwrap();
                    s.input_text = source;
                    s.from_lang = from;
                    s.to_lang = to;
                    s.show_history = false;
                }
            });

            // 首帧渲染完毕，显示窗口（消除闪屏）
            if !HISTORY_SHOWN.swap(true, Ordering::SeqCst) {
                ui.ctx().send_viewport_cmd(egui::ViewportCommand::Visible(true));
            }
        },
    );
}

/// 按 char 数截断字符串，超出部分用省略号替代
fn truncate_str(s: &str, max_chars: usize) -> String {
    let chars: Vec<char> = s.chars().collect();
    if chars.len() <= max_chars {
        s.to_string()
    } else {
        let mut t: String = chars[..max_chars].iter().collect();
        t.push('…');
        t
    }
}

/// 将 Unix 秒格式化为 HH:MM 显示
fn format_timestamp(ts: u64) -> String {
    let dt = chrono::DateTime::from_timestamp(ts as i64, 0)
        .map(|t| t.with_timezone(&chrono::Local))
        .map(|t| t.format("%H:%M").to_string());
    dt.unwrap_or_else(|| "--:--".into())
}
