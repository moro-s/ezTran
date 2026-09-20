use crate::config::LANGUAGES;
use crate::translate::{EngineConfig, TranslateResult, Translator};
use crate::ui::components::{show_toast, update_toast};
use crate::ui::state::STATE;
use egui::TextStyle;
use std::sync::mpsc::{self, Sender};
use std::sync::LazyLock;

/// 翻译任务（发送给常驻 worker 线程）
struct TranslateJob {
    engine: EngineConfig,
    text: String,
    from: String,
    to: String,
}

/// 翻译完成消息（回传给 UI 线程）
enum TranslateDone {
    Ok(TranslateResult, String, String, String, String),
    Err(String),
}

/// 全局翻译任务 channel sender（worker 线程持有 receiver）
static JOB_TX: LazyLock<Sender<TranslateJob>> = LazyLock::new(|| {
    let (tx, rx) = mpsc::channel::<TranslateJob>();
    std::thread::spawn(move || {
        for job in rx {
            let engine_name = job.engine.name.clone();
            let from = job.from.clone();
            let to = job.to.clone();
            let text_for_history = job.text.clone();
            let result = Translator::translate(&job.engine, &job.text, &job.from, &job.to);

            // 兜底：无论成功/失败，都复位 translating 并写入结果
            let done = match result {
                Ok(r) => TranslateDone::Ok(r, text_for_history, from, to, engine_name),
                Err(e) => TranslateDone::Err(e.to_string()),
            };

            {
                let mut s = STATE.lock().unwrap();
                s.translating = false;
                match done {
                    TranslateDone::Ok(r, ref text, ref from, ref to, ref engine_name) => {
                        crate::history::add_entry(text, &r.text, from, to, engine_name);
                        s.result = Some(Ok(r));
                    }
                    TranslateDone::Err(ref msg) => {
                        s.result = Some(Err(msg.to_string()));
                    }
                }
            }

            // 通知 UI 重绘以显示结果
            if let Some(ctx) = crate::window::try_ctx() {
                ctx.request_repaint();
            }
        }
        log::debug!("[translate] worker 线程已退出");
    });
    tx
});

/// 绘制翻译工作台
pub fn draw_translate(ui: &mut egui::Ui) {
    let ctx = ui.ctx().clone();
    // toast 自动消失（3 秒）
    update_toast(&ctx);

    // 一次性取出本帧所需状态快照，避免绘制过程中反复加锁与克隆
    let (translating, result, input_text, from_lang, to_lang, engine_index, engines) = {
        let s = STATE.lock().unwrap();
        (
            s.translating,
            s.result.clone(),
            s.input_text.clone(),
            s.from_lang.clone(),
            s.to_lang.clone(),
            s.engine_index,
            s.config_edit
                .engines
                .engines
                .iter()
                .enumerate()
                .map(|(i, e)| (i, e.name.clone(), e.enabled))
                .collect::<Vec<(usize, String, bool)>>(),
        )
    };

    // Esc 键隐藏到托盘
    if ctx.input(|i| i.key_pressed(egui::Key::Escape)) {
        crate::window::hide_main();
    }

    // Ctrl+Enter 触发翻译（消费事件，防止 multiline 插入换行）
    if ctx.input(|i| i.key_pressed(egui::Key::Enter) && i.modifiers.ctrl) {
        ctx.input_mut(|i| i.consume_key(egui::Modifiers::CTRL, egui::Key::Enter));
        if !translating {
            do_translate(&input_text, &from_lang, &to_lang, engine_index, &ctx);
        }
    }

    // 面板背景色 #1E1E1E
    let panel_bg = crate::theme::panel_bg();
    let panel_frame = egui::Frame::default()
        .fill(panel_bg)
        .inner_margin(egui::Margin::same(10));

    // ── 顶部工具栏 ──
    egui::Panel::top("translate_toolbar")
        .exact_size(44.0)
        .show(ui, |ui| {
            ui.add_space(5.0);
            ui.horizontal(|ui| {
                ui.spacing_mut().item_spacing.x = 4.0;
                ui.spacing_mut().interact_size.y = 26.0;

                // 引擎选择（使用开头快照的引擎列表）
                let mut idx = engine_index;
                let selected_text = engines
                    .iter()
                    .find(|(i, _, _)| *i == idx)
                    .map(|(_, name, enabled)| {
                        if *enabled {
                            name.clone()
                        } else {
                            format!("{} (已禁用)", name)
                        }
                    })
                    .unwrap_or_else(|| "无引擎".into());
                let combo = egui::ComboBox::from_id_salt("engine_combo")
                    .selected_text(selected_text)
                    .height(ui.spacing().interact_size.y)
                    .show_ui(ui, |ui| {
                        for (i, name, enabled) in engines.iter() {
                            let label = if *enabled {
                                name.clone()
                            } else {
                                format!("{} (已禁用)", name)
                            };
                            if ui.selectable_label(*i == idx, label).clicked() {
                                idx = *i;
                            }
                        }
                    });
                if combo.response.changed() {
                    STATE.lock().unwrap().engine_index = idx;
                }

                ui.separator();

                // 源语言 → 目标语言 + 交换
                lang_combo(ui, "from_lang_combo", true);
                let swap_btn = egui::Button::new("\u{21C4}")
                    .min_size(egui::vec2(
                        ui.spacing().interact_size.x,
                        ui.spacing().interact_size.y,
                    ));
                if ui.add(swap_btn).clicked() {
                    let mut s = STATE.lock().unwrap();
                    if s.from_lang != "auto" {
                        let tmp = s.from_lang.clone();
                        s.from_lang = s.to_lang.clone();
                        s.to_lang = tmp;
                    }
                }
                lang_combo(ui, "to_lang_combo", false);

                ui.separator();

                // 右侧：历史 + 设置按钮
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    if ui.button("⚙ 设置").clicked() {
                        crate::ui::state::show_settings_window();
                    }
                    if ui.button("📜 历史").clicked() {
                        if crate::ui::state::is_history_visible() {
                            crate::ui::state::hide_history_window();
                        } else {
                            crate::ui::state::show_history_window();
                        }
                    }
                });
            });
            ui.add_space(2.0);
        });

    // ── 主体内容区：左右分栏 ──
    egui::CentralPanel::default()
        .frame(
            egui::Frame::default()
                .fill(crate::theme::central_bg())
                .inner_margin(egui::Margin::same(2)),
        )
        .show(ui, |ui| {
            let spacing = 2.0;
            let total = ui.available_width();
            let panel_w = ((total - spacing) / 2.0).max(100.0);
            let panel_h = ui.available_height();
            let panel_size = egui::vec2(panel_w, panel_h);

            ui.horizontal(|ui| {
                ui.spacing_mut().item_spacing.x = spacing;

                // ── 左侧面板：原文 ──
                ui.allocate_ui_with_layout(
                    panel_size,
                    egui::Layout::top_down(egui::Align::LEFT),
                    |ui| {
                        ui.set_max_size(panel_size);
                        panel_frame.show(ui, |ui| {
                            ui.vertical(|ui| {
                                ui.horizontal(|ui| {
                                    ui.label(
                                        egui::RichText::new("原文")
                                            .color(crate::theme::label_secondary()),
                                    );
                                    ui.with_layout(
                                        egui::Layout::right_to_left(egui::Align::Center),
                                        |ui| {
                                            let btn = egui::Button::new(if translating {
                                                "翻译中..."
                                            } else {
                                                "翻译"
                                            });
                                            if ui.add_enabled(!translating, btn).clicked() {
                                                let (text, from, to, engine_idx) = {
                                                    let s = STATE.lock().unwrap();
                                                    (
                                                        s.input_text.clone(),
                                                        s.from_lang.clone(),
                                                        s.to_lang.clone(),
                                                        s.engine_index,
                                                    )
                                                };
                                                do_translate(&text, &from, &to, engine_idx, &ctx);
                                            }

                                            if ui.button("📋 粘贴").clicked() {
                                                if let Ok(mut clipboard) =
                                                    arboard::Clipboard::new()
                                                {
                                                    if let Ok(text) = clipboard.get_text() {
                                                        STATE.lock().unwrap().input_text = text;
                                                        show_toast(&ctx, "已粘贴");
                                                    }
                                                }
                                            }
                                        },
                                    );
                                });

                                let mut text_buf = input_text.clone();
                                let te_resp = ui.add_sized(
                                    [ui.available_width(), ui.available_height()],
                                    egui::TextEdit::multiline(&mut text_buf)
                                        .desired_width(f32::INFINITY)
                                        .lock_focus(false),
                                );
                                if te_resp.changed() {
                                    STATE.lock().unwrap().input_text = text_buf;
                                }
                            });
                        });
                    },
                );

                // ── 右侧面板：译文 ──
                ui.allocate_ui_with_layout(
                    panel_size,
                    egui::Layout::top_down(egui::Align::LEFT),
                    |ui| {
                        ui.set_max_size(panel_size);
                        panel_frame.show(ui, |ui| {
                            ui.vertical(|ui| {
                                ui.horizontal(|ui| {
                                    ui.label(
                                        egui::RichText::new("译文")
                                            .color(crate::theme::label_secondary()),
                                    );
                                    ui.with_layout(
                                        egui::Layout::right_to_left(egui::Align::Center),
                                        |ui| {
                                            let has_result = result
                                                .as_ref()
                                                .map(|r| r.is_ok())
                                                .unwrap_or(false);
                                            if ui
                                                .add_enabled(
                                                    has_result,
                                                    egui::Button::new("📋 复制"),
                                                )
                                                .clicked()
                                            {
                                                let result =
                                                    STATE.lock().unwrap().result.clone();
                                                if let Some(Ok(r)) = result {
                                                    if let Ok(mut clip) =
                                                        arboard::Clipboard::new()
                                                    {
                                                        let _ = clip.set_text(r.text);
                                                        show_toast(&ctx, "已复制到剪贴板");
                                                    }
                                                }
                                            }
                                        },
                                    );
                                });

                                if translating {
                                    ui.add_space(30.0);
                                    ui.vertical_centered(|ui| {
                                        ui.spinner();
                                        ui.label(
                                            egui::RichText::new("正在翻译...")
                                                .color(crate::theme::text_loading()),
                                        );
                                    });
                                } else {
                                    match &result {
                                        Some(Ok(r)) => {
                                            egui::ScrollArea::vertical()
                                                .auto_shrink([false; 2])
                                                .max_width(ui.available_width())
                                                .show(ui, |ui| {
                                                    ui.label(
                                                        egui::RichText::new(&r.text)
                                                            .text_style(TextStyle::Body)
                                                            .color(crate::theme::text_translated()),
                                                    );
                                                });
                                        }
                                        Some(Err(e)) => {
                                            ui.add_space(12.0);
                                            ui.colored_label(
                                                crate::theme::error_text(),
                                                format!("❌ {}", e),
                                            );
                                        }
                                        None => {
                                            ui.add_space(30.0);
                                            ui.vertical_centered(|ui| {
                                                ui.label(
                                                    egui::RichText::new("译文将显示在这里")
                                                        .color(crate::theme::text_hint())
                                                        .italics(),
                                                );
                                                ui.add_space(6.0);
                                                ui.label(
                                                    egui::RichText::new(
                                                        "输入文本后点击「翻译」或按 Ctrl+Enter",
                                                    )
                                                    .color(crate::theme::text_hint_dim())
                                                    .small(),
                                                );
                                            });
                                        }
                                    }
                                }

                                ui.allocate_space(ui.available_size());
                            });
                        });
                    },
                );
            });
        });
}


fn lang_combo(ui: &mut egui::Ui, id: &str, is_from: bool) {
    let (current, is_from_confirmed) = match id {
        "from_lang_combo" => {
            let s = STATE.lock().unwrap();
            (s.from_lang.clone(), true)
        }
        "to_lang_combo" => {
            let s = STATE.lock().unwrap();
            (s.to_lang.clone(), false)
        }
        _ => (String::new(), true),
    };
    let is_from = is_from_confirmed || is_from;

    let display = LANGUAGES
        .iter()
        .find(|(code, _)| *code == current)
        .map(|(_, name)| *name)
        .unwrap_or(&current);

    egui::ComboBox::from_id_salt(id)
        .selected_text(display)
        .height(ui.spacing().interact_size.y)
        .show_ui(ui, |ui| {
            for (code, name) in LANGUAGES.iter() {
                if !is_from && *code == "auto" {
                    continue;
                }
                if ui.selectable_label(*code == current, *name).clicked() {
                    let mut s = STATE.lock().unwrap();
                    if is_from {
                        s.from_lang = code.to_string();
                    } else {
                        s.to_lang = code.to_string();
                    }
                }
            }
        });
}

pub(crate) fn do_translate(text: &str, from: &str, to: &str, engine_index: usize, ctx: &egui::Context) {
    let text = text.trim().to_string();
    if text.is_empty() {
        return;
    }

    let engine = {
        let s = STATE.lock().unwrap();
        s.config_edit.engines.engines.get(engine_index).cloned()
    };

    let engine = match engine {
        Some(e) if e.enabled => e,
        Some(_) => {
            STATE.lock().unwrap().result =
                Some(Err("当前引擎已禁用，请在设置中启用".into()));
            return;
        }
        None => {
            STATE.lock().unwrap().result =
                Some(Err("未选择引擎，请在翻译服务中添加".into()));
            return;
        }
    };

    STATE.lock().unwrap().translating = true;
    ctx.request_repaint();

    // 发送给常驻 worker 线程执行（避免每次翻译新建线程）
    let job = TranslateJob {
        engine,
        text,
        from: from.to_string(),
        to: to.to_string(),
    };
    if JOB_TX.send(job).is_err() {
        // worker 线程已退出，复位 translating 并报错
        let mut s = STATE.lock().unwrap();
        s.translating = false;
        s.result = Some(Err("翻译服务不可用".into()));
    }
}
