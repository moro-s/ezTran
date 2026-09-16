use crate::config::{AppConfig, LANGUAGES};
use crate::translate::{TranslateResult, Translator};
use egui::TextStyle;

/// 设置页面标签
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum SettingsTab {
    General,
    Engines,
    About,
}

impl Default for SettingsTab {
    fn default() -> Self {
        SettingsTab::General
    }
}

/// 应用状态
#[derive(Default)]
pub struct AppState {
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
    /// 是否显示设置窗口
    pub show_settings: bool,
    /// 保存配置后的提示消息
    pub toast: Option<String>,
    /// toast 消失时间戳
    pub toast_time: f64,
    /// 配置的可变副本（用于设置页编辑）
    pub config_edit: AppConfig,
    /// 窗口是否置顶
    pub pinned: bool,
    /// 设置窗口当前选中的标签页
    pub settings_tab: SettingsTab,
}

/// 全局状态
static STATE: std::sync::LazyLock<std::sync::Mutex<AppState>> =
    std::sync::LazyLock::new(|| std::sync::Mutex::new(AppState::default()));

/// 初始化全局状态（程序启动时调用一次）
pub fn init_state(config: AppConfig) {
    let mut state = STATE.lock().unwrap();
    state.config_edit = config.clone();
    state.from_lang = state.config_edit.default_from.clone();
    state.to_lang = state.config_edit.default_to.clone();
}

/// 打开设置窗口
pub fn show_settings_window() {
    STATE.lock().unwrap().show_settings = true;
}

/// 关闭设置窗口
pub fn hide_settings_window() {
    STATE.lock().unwrap().show_settings = false;
}

/// 设置窗口是否应该显示
pub fn is_settings_visible() -> bool {
    STATE.lock().unwrap().show_settings
}

// ════════════════════════════════════════════════════
// 翻译工作台（主窗口）
// ════════════════════════════════════════════════════

/// 绘制翻译工作台
pub fn draw_translate(ctx: &egui::Context) {
    // toast 自动消失（3 秒）
    {
        let mut state = STATE.lock().unwrap();
        if let Some(toast_time) = state.toast.as_ref().map(|_| state.toast_time) {
            if ctx.input(|i| i.time) - toast_time > 3.0 {
                state.toast = None;
            }
        }
    }

    // 面板背景色 #1E1E1E
    let panel_bg = egui::Color32::from_rgb(30, 30, 30);
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
                // 统一所有交互控件的高度，避免高低不齐
                ui.spacing_mut().interact_size.y = 26.0;

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
                let mut idx = STATE.lock().unwrap().engine_index;
                let combo = egui::ComboBox::from_id_salt("engine_combo")
                    .selected_text(
                        engines
                            .get(idx)
                            .cloned()
                            .unwrap_or_else(|| "无引擎".into()),
                    )
                    .height(ui.spacing().interact_size.y)
                    .show_ui(ui, |ui| {
                        for (i, name) in engines.iter().enumerate() {
                            ui.selectable_value(&mut idx, i, name);
                        }
                    });
                if combo.response.changed() {
                    STATE.lock().unwrap().engine_index = idx;
                }

                ui.separator();

                // 源语言 → 目标语言 + 交换
                lang_combo(ui, "from_lang_combo", true);
                let swap_btn = egui::Button::new("\u{21C4}")
                    .min_size(egui::vec2(ui.spacing().interact_size.x, ui.spacing().interact_size.y));
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

                // 右侧：钉住 + 设置按钮
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    if ui.button("⚙ 设置").clicked() {
                        show_settings_window();
                    }
                    let pinned = STATE.lock().unwrap().pinned;
                    let btn_text = if pinned { "📌 取消置顶" } else { "📌 置顶" };
                    if ui.button(btn_text).clicked() {
                        let mut s = STATE.lock().unwrap();
                        s.pinned = !s.pinned;
                        let level = if s.pinned {
                            egui::WindowLevel::AlwaysOnTop
                        } else {
                            egui::WindowLevel::Normal
                        };
                        ui.ctx().send_viewport_cmd(egui::ViewportCommand::WindowLevel(level));
                    }
                });
            });
            ui.add_space(2.0);
        });

    // ── 主体内容区：左右分栏 ──
    egui::CentralPanel::default()
        .frame(
            egui::Frame::default()
                .fill(egui::Color32::from_rgb(43, 43, 43))
                .inner_margin(egui::Margin::same(2.0)),
        )
        .show(ctx, |ui| {
            // 计算左右面板各自的可用宽度（各占一半，减去间距）
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
                            ui.set_min_size(egui::vec2(
                                panel_size.x - 20.0,
                                panel_size.y - 20.0,
                            ));
                            ui.vertical(|ui| {
                                // 头部：左标签 + 右翻译按钮
                                ui.horizontal(|ui| {
                                    ui.label(
                                        egui::RichText::new("原文")
                                            .color(egui::Color32::from_rgb(170, 170, 170)),
                                    );
                                    ui.with_layout(
                                        egui::Layout::right_to_left(egui::Align::Center),
                                        |ui| {
                                            let translating = STATE.lock().unwrap().translating;
                                            let btn = egui::Button::new(
                                                if translating { "翻译中..." } else { "翻译" },
                                            );
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
                                                if let Ok(mut clipboard) = arboard::Clipboard::new()
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

                                // 输入区
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
                            ui.set_min_size(egui::vec2(
                                panel_size.x - 20.0,
                                panel_size.y - 20.0,
                            ));
                            ui.vertical(|ui| {
                                // 头部
                                ui.horizontal(|ui| {
                                    ui.label(
                                        egui::RichText::new("译文")
                                            .color(egui::Color32::from_rgb(170, 170, 170)),
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
                                                .color(egui::Color32::from_gray(120)),
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
                                                            .color(
                                                egui::Color32::from_gray(230),
                                            ),
                                                    );
                                                });
                                        }
                                        Some(Err(e)) => {
                                            ui.add_space(12.0);
                                            ui.colored_label(
                                                egui::Color32::from_rgb(230, 120, 120),
                                                format!("❌ {}", e),
                                            );
                                        }
                                        None => {
                                            ui.add_space(30.0);
                                            ui.vertical_centered(|ui| {
                                                ui.label(
                                                    egui::RichText::new("译文将显示在这里")
                                                        .color(egui::Color32::from_gray(90))
                                                        .italics(),
                                                );
                                                ui.add_space(6.0);
                                                ui.label(
                                                    egui::RichText::new(
                                                        "输入文本后点击「翻译」或按 Ctrl+Enter",
                                                    )
                                                    .color(egui::Color32::from_gray(70))
                                                    .small(),
                                                );
                                            });
                                        }
                                    }
                                }

                                // 填充剩余空间，使面板高度与左侧一致
                                ui.allocate_space(ui.available_size());
                            });
                        });
                    },
                );
            });
        });
}

// ════════════════════════════════════════════════════
// 设置页面（独立窗口）
// ════════════════════════════════════════════════════

/// 绘制设置页面
pub fn draw_settings(ctx: &egui::Context) {
    // toast 自动消失
    {
        let mut state = STATE.lock().unwrap();
        if let Some(toast_time) = state.toast.as_ref().map(|_| state.toast_time) {
            if ctx.input(|i| i.time) - toast_time > 3.0 {
                state.toast = None;
            }
        }
    }

    egui::CentralPanel::default().show(ctx, |ui| {
        ui.horizontal_top(|ui| {
            // ── 左侧菜单 ──
            let menu_w = 140.0;
            ui.vertical(|ui| {
                ui.set_min_width(menu_w);
                ui.add_space(4.0);
                ui.heading("设置");
                ui.add_space(8.0);

                let tab = STATE.lock().unwrap().settings_tab;
                let tabs = [
                    (SettingsTab::General, "常规设置"),
                    (SettingsTab::Engines, "翻译服务"),
                    (SettingsTab::About, "关于"),
                ];
                for (t, label) in tabs {
                    let selected = tab == t;
                    let resp = ui.add_sized(
                        [menu_w, 32.0],
                        egui::Button::new(label)
                            .fill(if selected {
                                egui::Color32::from_rgba_unmultiplied(128, 128, 128, 60)
                            } else {
                                egui::Color32::TRANSPARENT
                            })
                            .wrap(),
                    );
                    if resp.clicked() {
                        STATE.lock().unwrap().settings_tab = t;
                    }
                }
            });

            ui.separator();

            // ── 右侧详情 ──
            let detail_w = ui.available_width();
            let detail_h = ui.available_height();
            ui.allocate_ui_with_layout(
                egui::vec2(detail_w, detail_h),
                egui::Layout::top_down(egui::Align::LEFT),
                |ui| {
                    egui::ScrollArea::vertical()
                        .auto_shrink([false; 2])
                        .show(ui, |ui| {
                            ui.add_space(4.0);
                            ui.set_min_width(detail_w - 20.0);
                            let tab = STATE.lock().unwrap().settings_tab;
                            match tab {
                                SettingsTab::General => settings_general(ui),
                                SettingsTab::Engines => settings_engines(ui),
                                SettingsTab::About => settings_about(ui),
                            }
                        });
                },
            );
        });
    });
}

// ── 常规设置 ──
fn settings_general(ui: &mut egui::Ui) {
    ui.heading("常规设置");
    ui.add_space(8.0);

    // 应用主题
    ui.label(egui::RichText::new("应用主题").strong());
    ui.add_space(2.0);
    {
        let theme = STATE.lock().unwrap().config_edit.theme.clone();
        let mut theme = theme;
        egui::ComboBox::from_id_salt("theme_combo")
            .selected_text(theme.label())
            .show_ui(ui, |ui| {
                for t in crate::config::AppTheme::all() {
                    ui.selectable_value(&mut theme, t.clone(), t.label());
                }
            });
        STATE.lock().unwrap().config_edit.theme = theme;
    }
    ui.add_space(10.0);

    // 应用字体
    ui.label(egui::RichText::new("应用字体").strong());
    ui.add_space(2.0);
    {
        let font = STATE.lock().unwrap().config_edit.font_family.clone();
        let mut font = font;
        ui.horizontal(|ui| {
            ui.add(egui::TextEdit::singleline(&mut font).desired_width(200.0));
            ui.label(
                egui::RichText::new("留空使用系统默认字体")
                    .color(egui::Color32::from_gray(120))
                    .small(),
            );
        });
        STATE.lock().unwrap().config_edit.font_family = font;
    }
    ui.add_space(10.0);

    // 应用字号
    ui.label(egui::RichText::new("应用字号").strong());
    ui.add_space(2.0);
    {
        let size = STATE.lock().unwrap().config_edit.font_size.clone();
        let mut size = size;
        egui::ComboBox::from_id_salt("fontsize_combo")
            .selected_text(size.label())
            .show_ui(ui, |ui| {
                for s in crate::config::FontSize::all() {
                    ui.selectable_value(&mut size, s.clone(), s.label());
                }
            });
        STATE.lock().unwrap().config_edit.font_size = size;
    }
    ui.add_space(16.0);
    ui.separator();
    ui.add_space(10.0);

    // 翻译设置
    ui.label(egui::RichText::new("翻译设置").strong());
    ui.add_space(4.0);
    ui.horizontal(|ui| {
        ui.label("默认源语言:");
        lang_combo_settings(ui, "default_from_combo", true);
    });
    ui.horizontal(|ui| {
        ui.label("默认目标语言:");
        lang_combo_settings(ui, "default_to_combo", false);
    });
    ui.add_space(10.0);

    // 输入翻译
    ui.checkbox(
        &mut STATE.lock().unwrap().config_edit.enable_input_translate,
        "输入翻译（唤起翻译工作台）",
    );
    ui.add_space(4.0);

    // 划词翻译
    {
        let enabled = STATE.lock().unwrap().config_edit.enable_selection_translate;
        let mut enabled = enabled;
        ui.checkbox(&mut enabled, "划词翻译");
        STATE.lock().unwrap().config_edit.enable_selection_translate = enabled;
        if enabled {
            ui.horizontal(|ui| {
                ui.label("    划词翻译快捷键:");
                let mut hk = STATE.lock().unwrap().config_edit.selection_hotkey.clone();
                ui.add(egui::TextEdit::singleline(&mut hk).desired_width(160.0));
                STATE.lock().unwrap().config_edit.selection_hotkey = hk;
            });
        }
    }
    ui.add_space(10.0);

    // 开机自启动
    {
        let auto = STATE.lock().unwrap().config_edit.auto_start;
        let mut auto = auto;
        ui.checkbox(&mut auto, "开机自启动");
        STATE.lock().unwrap().config_edit.auto_start = auto;
    }
    ui.add_space(16.0);
    ui.separator();
    ui.add_space(10.0);

    // 保存 / 恢复
    ui.horizontal(|ui| {
        if ui.button("💾 保存配置").clicked() {
            let mut s = STATE.lock().unwrap();
            match s.config_edit.save() {
                Ok(()) => {
                    s.toast = Some("配置已保存".into());
                }
                Err(e) => {
                    s.toast = Some(format!("保存失败: {}", e));
                }
            }
            s.toast_time = ui.input(|i| i.time);
        }
        if ui.button("↩ 恢复默认").clicked() {
            STATE.lock().unwrap().config_edit = AppConfig::default();
        }
    });

    // toast 提示
    let toast = STATE.lock().unwrap().toast.clone();
    if let Some(ref t) = toast {
        ui.add_space(6.0);
        ui.colored_label(egui::Color32::from_rgb(120, 200, 120), t);
    }
}

// ── 翻译服务 ──
fn settings_engines(ui: &mut egui::Ui) {
    ui.heading("翻译服务");
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
                ui.vertical(|ui| {
                    // 名称
                    ui.horizontal(|ui| {
                        ui.label("名称:");
                        let mut name_buf = name;
                        ui.add(
                            egui::TextEdit::singleline(&mut name_buf).desired_width(200.0),
                        );
                        STATE.lock().unwrap().config_edit.engines.engines[i].name = name_buf;
                    });

                    // 启用 + 类型
                    ui.horizontal(|ui| {
                        let mut en = enabled;
                        ui.checkbox(&mut en, "启用");
                        STATE.lock().unwrap().config_edit.engines.engines[i].enabled = en;

                        ui.separator();

                        ui.label("类型:");
                        let _ = egui::ComboBox::from_id_salt(format!("kind_{}", i))
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
                        let kind_label = kind.label();
                        let mut s = STATE.lock().unwrap();
                        s.config_edit.engines.engines[i].kind = match kind_label {
                            "有道翻译" => crate::translate::EngineKind::Youdao,
                            "百度翻译" => crate::translate::EngineKind::Baidu,
                            "DeepL" => crate::translate::EngineKind::DeepL,
                            _ => crate::translate::EngineKind::Custom,
                        };
                    });

                    // API Key
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

                    // Secret
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

                    // Endpoint + 删除
                    ui.horizontal(|ui| {
                        ui.label("Endpoint:");
                        let mut ep_buf = endpoint;
                        ui.add(
                            egui::TextEdit::singleline(&mut ep_buf).desired_width(220.0),
                        );
                        STATE.lock().unwrap().config_edit.engines.engines[i].endpoint = ep_buf;
                    });

                    ui.horizontal(|ui| {
                        if ui.button("🗑 删除此引擎").clicked() {
                            STATE.lock().unwrap().config_edit.engines.engines.remove(i);
                        }
                    });
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

    ui.add_space(16.0);
    ui.separator();
    ui.add_space(10.0);

    // 保存
    ui.horizontal(|ui| {
        if ui.button("💾 保存配置").clicked() {
            let mut s = STATE.lock().unwrap();
            match s.config_edit.save() {
                Ok(()) => {
                    s.toast = Some("配置已保存".into());
                }
                Err(e) => {
                    s.toast = Some(format!("保存失败: {}", e));
                }
            }
            s.toast_time = ui.input(|i| i.time);
        }
    });

    let toast = STATE.lock().unwrap().toast.clone();
    if let Some(ref t) = toast {
        ui.add_space(6.0);
        ui.colored_label(egui::Color32::from_rgb(120, 200, 120), t);
    }
}

// ── 关于 ──
fn settings_about(ui: &mut egui::Ui) {
    ui.heading("关于");
    ui.add_space(12.0);

    // 应用信息
    egui::Frame::group(ui.style())
        .inner_margin(12.0)
        .show(ui, |ui| {
            ui.vertical(|ui| {
                ui.horizontal(|ui| {
                    ui.label(
                        egui::RichText::new("EzTran")
                            .size(22.0)
                            .strong(),
                    );
                    ui.label(
                        egui::RichText::new("v0.1.0")
                            .color(egui::Color32::from_gray(140)),
                    );
                });
                ui.add_space(4.0);
                ui.label(
                    egui::RichText::new("轻量级桌面翻译工具")
                        .color(egui::Color32::from_gray(160)),
                );
                ui.add_space(4.0);
                ui.label(
                    egui::RichText::new("基于 egui + Rust 构建")
                        .color(egui::Color32::from_gray(120))
                        .small(),
                );
            });
        });

    ui.add_space(20.0);

    // 底部居中：GitHub 链接 + 检查更新
    ui.vertical_centered(|ui| {
        ui.hyperlink_to("🌐 GitHub 仓库", "https://github.com/eztran/eztran");
        ui.add_space(8.0);
        if ui.button("🔄 检查更新").clicked() {
            show_toast(ui.ctx(), "暂无可用的更新");
        }
    });

    let toast = STATE.lock().unwrap().toast.clone();
    if let Some(ref t) = toast {
        ui.add_space(6.0);
        ui.colored_label(egui::Color32::from_rgb(120, 200, 120), t);
    }
}

// ════════════════════════════════════════════════════
// 辅助函数
// ════════════════════════════════════════════════════

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
        (s.from_lang.clone(), s.to_lang.clone(), s.engine_index)
    };

    STATE.lock().unwrap().input_text = text.clone();
    do_translate(&text, &from, &to, engine_index, ctx);
}

fn show_toast(ctx: &egui::Context, msg: &str) {
    let mut s = STATE.lock().unwrap();
    s.toast = Some(msg.into());
    s.toast_time = ctx.input(|i| i.time);
}
