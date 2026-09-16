use crate::config::{AppConfig, LANGUAGES};
use crate::translate::{TranslateResult, Translator};
use egui::TextStyle;

/// 当前页面
#[derive(Default, PartialEq, Clone, Copy)]
pub enum Page {
    #[default]
    Translate,
    Settings,
}

impl Page {
    fn label(&self) -> &'static str {
        match self {
            Page::Translate => "🌐 翻译",
            Page::Settings => "⚙ 设置",
        }
    }
}

/// 切换到翻译页（供系统托盘调用）
pub fn switch_to_translate() {
    STATE.lock().unwrap().page = Page::Translate;
}

/// 切换到设置页（供系统托盘调用）
pub fn switch_to_settings() {
    STATE.lock().unwrap().page = Page::Settings;
}

/// 应用状态
#[derive(Default)]
pub struct AppState {
    pub page: Page,
    /// 输入翻译 - 源文本
    pub input_text: String,
    /// 输入翻译 - 译文结果
    pub result: Option<Result<TranslateResult, String>>,
    /// 当前选择的引擎 index
    pub engine_index: usize,
    /// 源语言
    pub from_lang: String,
    /// 目标语言
    pub to_lang: String,
    /// 是否正在翻译
    pub translating: bool,
    /// 保存配置后的提示消息
    pub toast: Option<String>,
    /// toast 消失时间戳
    pub toast_time: f64,
    /// 配置的可变副本（用于设置页编辑）
    pub config_edit: AppConfig,
    /// 交换语言动画
    pub swap_anim: f32,
}

/// 全局状态
static STATE: std::sync::LazyLock<std::sync::Mutex<AppState>> =
    std::sync::LazyLock::new(|| std::sync::Mutex::new(AppState::default()));

/// 主绘制入口
pub fn draw(ctx: &egui::Context, config: std::sync::Arc<AppConfig>) {
    // 首次运行时用传入的 config 初始化状态
    {
        let mut state = STATE.lock().unwrap();
        if state.config_edit.engines.engines.is_empty() {
            state.config_edit = (*config).clone();
            state.from_lang = state.config_edit.default_from.clone();
            state.to_lang = state.config_edit.default_to.clone();
        }
        // toast 自动消失（3 秒）
        if let Some(toast_time) = state.toast.as_ref().map(|_| state.toast_time) {
            if ctx.input(|i| i.time) - toast_time > 3.0 {
                state.toast = None;
            }
        }
    }

    // 顶部导航栏
    egui::TopBottomPanel::top("nav").show(ctx, |ui| {
        ui.add_space(4.0);
        ui.horizontal(|ui| {
            for page in [Page::Translate, Page::Settings] {
                let state = STATE.lock().unwrap();
                let selected = state.page == page;
                drop(state);
                if ui.selectable_label(selected, page.label()).clicked() {
                    STATE.lock().unwrap().page = page;
                }
                ui.separator();
            }

            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                let state = STATE.lock().unwrap();
                let engine_name = state
                    .config_edit
                    .engines
                    .engines
                    .get(state.engine_index)
                    .map(|e| e.name.as_str())
                    .unwrap_or("未配置引擎");
                ui.label(format!("当前引擎: {}", engine_name));
                if let Some(ref toast) = state.toast {
                    ui.separator();
                    ui.colored_label(egui::Color32::from_rgb(120, 200, 120), toast);
                }
            });
        });
        ui.add_space(2.0);
    });

    // 中央内容区
    egui::CentralPanel::default().show(ctx, |ui| {
        let page = STATE.lock().unwrap().page;
        match page {
            Page::Translate => draw_translate_page(ui, ctx),
            Page::Settings => draw_settings_page(ui, ctx),
        }
    });
}

// ════════════════════════════════════════════════════
// 翻译页 - 左右结构
// ════════════════════════════════════════════════════

fn draw_translate_page(ui: &mut egui::Ui, ctx: &egui::Context) {
    // 顶部工具栏：引擎选择 + 语言选择 + 翻译按钮
    egui::TopBottomPanel::top("translate_toolbar")
        .exact_height(40.0)
        .show_inside(ui, |ui| {
            ui.add_space(4.0);
            ui.horizontal(|ui| {
                // 引擎选择
                let engines: Vec<String> = {
                    let s = STATE.lock().unwrap();
                    s.config_edit
                        .engines
                        .engines
                        .iter()
                        .filter(|e| e.enabled)
                        .map(|e| e.name.clone())
                        .collect()
                };

                let mut idx = {
                    STATE.lock().unwrap().engine_index
                };
                let combo = egui::ComboBox::from_id_salt("engine_combo")
                    .selected_text(engines.get(idx).cloned().unwrap_or_else(|| "无引擎".into()))
                    .show_ui(ui, |ui| {
                        for (i, name) in engines.iter().enumerate() {
                            ui.selectable_value(&mut idx, i, name);
                        }
                    });
                if combo.response.changed() {
                    STATE.lock().unwrap().engine_index = idx;
                }

                ui.separator();

                // 源语言
                lang_combo(ui, "from_lang_combo", false);
                ui.label("→");
                lang_combo(ui, "to_lang_combo", false);

                // 交换语言按钮
                if ui.button("⇄").clicked() {
                    let mut s = STATE.lock().unwrap();
                    if s.from_lang != "auto" {
                        let tmp = s.from_lang.clone();
                        s.from_lang = s.to_lang.clone();
                        s.to_lang = tmp;
                    }
                }

                ui.separator();

                // 从剪贴板翻译（划词翻译入口）
                if ui.button("📋 从剪贴板翻译").clicked() {
                    translate_clipboard(ctx);
                }

                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    let translating = STATE.lock().unwrap().translating;
                    let btn = egui::Button::new(if translating { "翻译中..." } else { "翻译" });
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
                });
            });
            ui.add_space(2.0);
        });

    // 左右两栏
    egui::CentralPanel::default().show_inside(ui, |ui| {
        ui.horizontal(|ui| {
            // 左侧：源文本输入
            ui.vertical(|ui| {
                ui.label(
                    egui::RichText::new("原文")
                        .strong()
                        .color(egui::Color32::from_rgb(137, 180, 250)),
                );
                let text = STATE.lock().unwrap().input_text.clone();
                let mut text_buf = text;
                let resp = ui.add_sized(
                    [ui.available_width(), ui.available_height()],
                    egui::TextEdit::multiline(&mut text_buf)
                        .desired_width(f32::INFINITY)
                        .lock_focus(false),
                );
                STATE.lock().unwrap().input_text = text_buf.clone();

                // Ctrl+Enter 翻译
                if resp.lost_focus()
                    && ui.input(|i| i.key_pressed(egui::Key::Enter) && i.modifiers.ctrl)
                {
                    let (from, to, engine_idx) = {
                        let s = STATE.lock().unwrap();
                        (s.from_lang.clone(), s.to_lang.clone(), s.engine_index)
                    };
                    do_translate(&text_buf, &from, &to, engine_idx, ctx);
                }
            });

            // 分隔线
            ui.separator();

            // 右侧：翻译结果
            ui.vertical(|ui| {
                ui.horizontal(|ui| {
                    ui.label(
                        egui::RichText::new("译文")
                            .strong()
                            .color(egui::Color32::from_rgb(137, 180, 250)),
                    );

                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        // 复制结果按钮
                        let has_result = STATE
                            .lock()
                            .unwrap()
                            .result
                            .as_ref()
                            .map(|r| r.is_ok())
                            .unwrap_or(false);
                        if ui.add_enabled(has_result, egui::Button::new("📋 复制")).clicked() {
                            let result = STATE.lock().unwrap().result.clone();
                            if let Some(Ok(r)) = result {
                                if let Ok(mut clip) = arboard::Clipboard::new() {
                                    let _ = clip.set_text(r.text);
                                    show_toast(ctx, "已复制到剪贴板");
                                }
                            }
                        }
                        // 清空按钮
                        if ui.button("🗑 清空").clicked() {
                            let mut s = STATE.lock().unwrap();
                            s.input_text.clear();
                            s.result = None;
                        }
                    });
                });

                let result = STATE.lock().unwrap().result.clone();
                let translating = STATE.lock().unwrap().translating;

                if translating {
                    ui.add_space(20.0);
                    ui.vertical_centered(|ui| {
                        ui.spinner();
                        ui.label("正在翻译...");
                    });
                } else {
                    match &result {
                        Some(Ok(r)) => {
                            ui.add_space(2.0);
                            // 使用 ScrollArea 包裹长文本
                            egui::ScrollArea::vertical()
                                .auto_shrink([false; 2])
                                .show(ui, |ui| {
                                    ui.label(
                                        egui::RichText::new(&r.text)
                                            .text_style(TextStyle::Body)
                                            .color(egui::Color32::from_gray(230)),
                                    );
                                });
                        }
                        Some(Err(e)) => {
                            ui.add_space(8.0);
                            ui.colored_label(
                                egui::Color32::from_rgb(230, 120, 120),
                                format!("❌ {}", e),
                            );
                        }
                        None => {
                            ui.add_space(20.0);
                            ui.vertical_centered(|ui| {
                                ui.label(
                                    egui::RichText::new("译文将显示在这里")
                                        .color(egui::Color32::from_gray(100))
                                        .italics(),
                                );
                                ui.add_space(8.0);
                                ui.label(
                                    egui::RichText::new("输入文本后点击「翻译」\n或按 Ctrl+Enter")
                                        .color(egui::Color32::from_gray(80))
                                        .small(),
                                );
                            });
                        }
                    }
                }
            });
        });
    });
}

// ════════════════════════════════════════════════════
// 设置页
// ════════════════════════════════════════════════════

fn draw_settings_page(ui: &mut egui::Ui, _ctx: &egui::Context) {
    egui::ScrollArea::vertical()
        .auto_shrink([false; 2])
        .show(ui, |ui| {
            ui.add_space(4.0);

            // ── 引擎配置 ──
            ui.heading("翻译引擎配置");
            ui.add_space(2.0);
            ui.label(
                egui::RichText::new("配置一个或多个翻译引擎，勾选「启用」后可在翻译页选择使用")
                    .color(egui::Color32::from_gray(140))
                    .small(),
            );
            ui.add_space(8.0);

            let engine_count = STATE.lock().unwrap().config_edit.engines.engines.len();
            for i in 0..engine_count {
                let (kind, name, enabled, api_key, api_secret, endpoint) = {
                    let s = STATE.lock().unwrap();
                    let e = &s.config_edit.engines.engines[i];
                    (
                        e.kind.clone(),
                        e.name.clone(),
                        e.enabled,
                        e.api_key.clone(),
                        e.api_secret.clone(),
                        e.endpoint.clone(),
                    )
                };

                egui::Frame::group(ui.style())
                    .inner_margin(8.0)
                    .show(ui, |ui| {
                        ui.horizontal(|ui| {
                            ui.label("名称:");
                            let mut name_buf = name;
                            ui.text_edit_singleline(&mut name_buf);
                            STATE.lock().unwrap().config_edit.engines.engines[i].name = name_buf;

                            let mut en = enabled;
                            ui.checkbox(&mut en, "启用");
                            STATE.lock().unwrap().config_edit.engines.engines[i].enabled = en;

                            ui.separator();

                            ui.label("类型:");
                            let combo = egui::ComboBox::from_id_salt(format!("kind_{}", i))
                                .selected_text(kind.label())
                                .show_ui(ui, |ui| {
                                    for k in crate::translate::EngineKind::all() {
                                        let mut k_str = format!("{:?}", k);
                                        ui.selectable_value(
                                            &mut k_str,
                                            format!("{:?}", k),
                                            k.label(),
                                        );
                                    }
                                });
                            let _ = combo;
                            // 从 combo 文本更新 kind
                            let kind_label = kind.label();
                            {
                                let mut s = STATE.lock().unwrap();
                                s.config_edit.engines.engines[i].kind = match kind_label {
                                    "有道翻译" => crate::translate::EngineKind::Youdao,
                                    "百度翻译" => crate::translate::EngineKind::Baidu,
                                    "DeepL" => crate::translate::EngineKind::DeepL,
                                    _ => crate::translate::EngineKind::Custom,
                                };
                            }
                        });

                        ui.horizontal(|ui| {
                            ui.label("API Key:");
                            let mut key_buf = api_key;
                            ui.add(
                                egui::TextEdit::singleline(&mut key_buf)
                                    .password(true)
                                    .desired_width(220.0),
                            );
                            STATE.lock().unwrap().config_edit.engines.engines[i].api_key = key_buf;
                        });

                        ui.horizontal(|ui| {
                            ui.label("Secret:");
                            let mut secret_buf = api_secret;
                            ui.add(
                                egui::TextEdit::singleline(&mut secret_buf)
                                    .password(true)
                                    .desired_width(220.0),
                            );
                            STATE.lock().unwrap().config_edit.engines.engines[i].api_secret =
                                secret_buf;
                        });

                        ui.horizontal(|ui| {
                            ui.label("Endpoint:");
                            let mut ep_buf = endpoint;
                            ui.add(
                                egui::TextEdit::singleline(&mut ep_buf).desired_width(220.0),
                            );
                            STATE.lock().unwrap().config_edit.engines.engines[i].endpoint = ep_buf;

                            ui.separator();

                            if ui.button("🗑 删除此引擎").clicked() {
                                STATE.lock().unwrap().config_edit.engines.engines.remove(i);
                            }
                        });
                    });
                ui.add_space(4.0);
            }

            if ui.button("➕ 添加引擎").clicked() {
                STATE
                    .lock()
                    .unwrap()
                    .config_edit
                    .engines
                    .engines
                    .push(crate::translate::engine::EngineConfig::default());
            }

            ui.add_space(12.0);
            ui.separator();
            ui.add_space(8.0);

            // ── 通用设置 ──
            ui.heading("通用设置");
            ui.add_space(4.0);

            ui.horizontal(|ui| {
                ui.label("默认源语言:");
                lang_combo_settings(ui, "default_from_combo", true);
            });

            ui.horizontal(|ui| {
                ui.label("默认目标语言:");
                lang_combo_settings(ui, "default_to_combo", false);
            });

            ui.horizontal(|ui| {
                ui.label("划词翻译快捷键:");
                let mut hk = STATE.lock().unwrap().config_edit.selection_hotkey.clone();
                ui.add(egui::TextEdit::singleline(&mut hk).desired_width(160.0));
                STATE.lock().unwrap().config_edit.selection_hotkey = hk;
            });

            ui.add_space(12.0);
            ui.separator();
            ui.add_space(8.0);

            // ── 保存 / 恢复 ──
            ui.horizontal(|ui| {
                if ui.button("💾 保存配置").clicked() {
                    let mut s = STATE.lock().unwrap();
                    match s.config_edit.save() {
                        Ok(()) => {
                            s.toast = Some("配置已保存".into());
                            s.toast_time = ui.input(|i| i.time);
                        }
                        Err(e) => {
                            s.toast = Some(format!("保存失败: {}", e));
                            s.toast_time = ui.input(|i| i.time);
                        }
                    }
                }
                if ui.button("↩ 恢复默认").clicked() {
                    STATE.lock().unwrap().config_edit = AppConfig::default();
                }
            });
        });
}

// ════════════════════════════════════════════════════
// 辅助函数
// ════════════════════════════════════════════════════

/// 翻译页的语言选择下拉框（读写 STATE.from_lang / to_lang）
fn lang_combo(ui: &mut egui::Ui, id: &str, _include_auto: bool) {
    let (current, is_from) = match id {
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

    let display = LANGUAGES
        .iter()
        .find(|(code, _)| *code == current)
        .map(|(_, name)| *name)
        .unwrap_or(&current);

    egui::ComboBox::from_id_salt(id)
        .selected_text(display)
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

/// 设置页的语言选择下拉框（读写 config_edit.default_from / default_to）
fn lang_combo_settings(ui: &mut egui::Ui, id: &str, include_auto: bool) {
    let (current, is_from) = match id {
        "default_from_combo" => {
            let s = STATE.lock().unwrap();
            (s.config_edit.default_from.clone(), true)
        }
        "default_to_combo" => {
            let s = STATE.lock().unwrap();
            (s.config_edit.default_to.clone(), false)
        }
        _ => (String::new(), true),
    };

    let display = LANGUAGES
        .iter()
        .find(|(code, _)| *code == current)
        .map(|(_, name)| *name)
        .unwrap_or(&current);

    egui::ComboBox::from_id_salt(id)
        .selected_text(display)
        .show_ui(ui, |ui| {
            for (code, name) in LANGUAGES.iter() {
                if !include_auto && *code == "auto" {
                    continue;
                }
                if ui.selectable_label(*code == current, *name).clicked() {
                    let mut s = STATE.lock().unwrap();
                    if is_from {
                        s.config_edit.default_from = code.to_string();
                    } else {
                        s.config_edit.default_to = code.to_string();
                    }
                }
            }
        });
}

fn do_translate(text: &str, from: &str, to: &str, engine_index: usize, ctx: &egui::Context) {
    let text = text.trim().to_string();
    if text.is_empty() {
        return;
    }

    let engine = {
        let s = STATE.lock().unwrap();
        s.config_edit.engines.engines.get(engine_index).cloned()
    };

    let engine = match engine {
        Some(e) => e,
        None => {
            STATE.lock().unwrap().result = Some(Err("未选择引擎".into()));
            return;
        }
    };

    STATE.lock().unwrap().translating = true;
    ctx.request_repaint();

    let result = Translator::translate(&engine, &text, from, to);

    let mut s = STATE.lock().unwrap();
    s.translating = false;
    s.result = Some(result.map_err(|e| e.to_string()));
}

fn translate_clipboard(ctx: &egui::Context) {
    let mut clipboard = match arboard::Clipboard::new() {
        Ok(c) => c,
        Err(e) => {
            STATE.lock().unwrap().result = Some(Err(format!("无法访问剪贴板: {}", e)));
            return;
        }
    };

    let text = match clipboard.get_text() {
        Ok(t) => t,
        Err(_) => {
            STATE.lock().unwrap().result = Some(Err("剪贴板中没有文本".into()));
            return;
        }
    };

    let (from, to, engine_index) = {
        let s = STATE.lock().unwrap();
        (
            s.from_lang.clone(),
            s.to_lang.clone(),
            s.engine_index,
        )
    };

    // 把剪贴板文本填入输入框
    STATE.lock().unwrap().input_text = text.clone();

    do_translate(&text, &from, &to, engine_index, ctx);
}

fn show_toast(ctx: &egui::Context, msg: &str) {
    let mut s = STATE.lock().unwrap();
    s.toast = Some(msg.into());
    s.toast_time = ctx.input(|i| i.time);
}
