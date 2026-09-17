use crate::config::LANGUAGES;
use crate::translate::Translator;
use crate::ui::state::STATE;
use egui::TextStyle;

/// 绘制翻译工作台
pub fn draw_translate(ctx: &egui::Context) {
    // toast 自动消失（3 秒）
    update_toast(ctx);

    // Esc 键最小化翻译工作台
    if ctx.input(|i| i.key_pressed(egui::Key::Escape)) {
        crate::window::minimize_main();
    }

    // 面板背景色 #1E1E1E
    let panel_bg = crate::theme::panel_bg();
    let panel_frame = egui::Frame::default()
        .fill(panel_bg)
        .rounding(8.0)
        .inner_margin(egui::Margin::same(10.0));

    // ── 顶部工具栏 ──
    egui::TopBottomPanel::top("translate_toolbar")
        .exact_height(44.0)
        .show(ctx, |ui| {
            ui.add_space(5.0);
            ui.horizontal(|ui| {
                ui.spacing_mut().item_spacing.x = 4.0;
                ui.spacing_mut().interact_size.y = 26.0;

                // 引擎选择（从翻译服务配置中读取所有引擎）
                let engines: Vec<(usize, String, bool)> = {
                    let s = STATE.lock().unwrap();
                    s.config_edit
                        .engines
                        .engines
                        .iter()
                        .enumerate()
                        .map(|(i, e)| (i, e.name.clone(), e.enabled))
                        .collect()
                };
                let mut idx = STATE.lock().unwrap().engine_index;
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
                        let mut s = STATE.lock().unwrap();
                        s.show_history = !s.show_history;
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
                .inner_margin(egui::Margin::same(2.0)),
        )
        .show(ctx, |ui| {
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
                            ui.set_min_size(egui::vec2(panel_size.x - 20.0, panel_size.y - 20.0));
                            ui.vertical(|ui| {
                                ui.horizontal(|ui| {
                                    ui.label(
                                        egui::RichText::new("原文")
                                            .color(crate::theme::label_secondary()),
                                    );
                                    ui.with_layout(
                                        egui::Layout::right_to_left(egui::Align::Center),
                                        |ui| {
                                            let translating = STATE.lock().unwrap().translating;
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
                                                do_translate(&text, &from, &to, engine_idx, ctx);
                                            }

                                            if ui.button("📋 粘贴").clicked() {
                                                if let Ok(mut clipboard) =
                                                    arboard::Clipboard::new()
                                                {
                                                    if let Ok(text) = clipboard.get_text() {
                                                        STATE.lock().unwrap().input_text = text;
                                                        show_toast(ctx, "已粘贴");
                                                    }
                                                }
                                            }
                                        },
                                    );
                                });

                                let text = STATE.lock().unwrap().input_text.clone();
                                let mut text_buf = text;
                                let resp = ui.add_sized(
                                    [ui.available_width(), ui.available_height()],
                                    egui::TextEdit::multiline(&mut text_buf)
                                        .desired_width(f32::INFINITY)
                                        .lock_focus(false),
                                );
                                STATE.lock().unwrap().input_text = text_buf.clone();

                                if resp.lost_focus()
                                    && ui.input(|i| {
                                        i.key_pressed(egui::Key::Enter) && i.modifiers.ctrl
                                    })
                                {
                                    let (from, to, engine_idx) = {
                                        let s = STATE.lock().unwrap();
                                        (s.from_lang.clone(), s.to_lang.clone(), s.engine_index)
                                    };
                                    do_translate(&text_buf, &from, &to, engine_idx, ctx);
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
                            ui.set_min_size(egui::vec2(panel_size.x - 20.0, panel_size.y - 20.0));
                            ui.vertical(|ui| {
                                ui.horizontal(|ui| {
                                    ui.label(
                                        egui::RichText::new("译文")
                                            .color(crate::theme::label_secondary()),
                                    );
                                    ui.with_layout(
                                        egui::Layout::right_to_left(egui::Align::Center),
                                        |ui| {
                                            let has_result = STATE
                                                .lock()
                                                .unwrap()
                                                .result
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
                                                        show_toast(ctx, "已复制到剪贴板");
                                                    }
                                                }
                                            }
                                        },
                                    );
                                });

                                let result = STATE.lock().unwrap().result.clone();
                                let translating = STATE.lock().unwrap().translating;

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

    let result = Translator::translate(&engine, &text, from, to);

    let mut s = STATE.lock().unwrap();
    s.translating = false;
    match &result {
        Ok(r) => {
            crate::history::add_entry(&text, &r.text, from, to, &engine.name);
            s.result = Some(Ok(r.clone()));
        }
        Err(e) => {
            s.result = Some(Err(e.to_string()));
        }
    }
}

pub(crate) fn show_toast(ctx: &egui::Context, msg: &str) {
    let mut s = STATE.lock().unwrap();
    s.toast = Some(msg.into());
    s.toast_time = ctx.input(|i| i.time);
}

/// toast 自动消失检查（3 秒后清除），在每帧绘制开始时调用
pub(crate) fn update_toast(ctx: &egui::Context) {
    let mut state = STATE.lock().unwrap();
    if let Some(toast_time) = state.toast.as_ref().map(|_| state.toast_time) {
        if ctx.input(|i| i.time) - toast_time > 3.0 {
            state.toast = None;
        }
    }
}

/// 渲染 toast 提示（如果存在），在 UI 底部调用
pub(crate) fn render_toast(ui: &mut egui::Ui) {
    let toast = STATE.lock().unwrap().toast.clone();
    if let Some(ref t) = toast {
        ui.add_space(6.0);
        ui.colored_label(crate::theme::toast_color(), t);
    }
}
