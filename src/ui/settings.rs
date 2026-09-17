use crate::config::{AppConfig, LANGUAGES};
use crate::ui::state::{SettingsTab, STATE};
use crate::ui::translate::show_toast;

/// 绘制设置页面（独立 OS 窗口）
pub fn draw_settings(ctx: &egui::Context) {
    log::info!("[settings] draw_settings 被调用，创建/更新设置视口");

    // toast 自动消失
    {
        let mut state = STATE.lock().unwrap();
        if let Some(toast_time) = state.toast.as_ref().map(|_| state.toast_time) {
            if ctx.input(|i| i.time) - toast_time > 3.0 {
                state.toast = None;
            }
        }
    }

    ctx.show_viewport_immediate(
        egui::ViewportId::from_hash_of("settings"),
        egui::ViewportBuilder::default()
            .with_title("设置")
            .with_inner_size([640.0, 480.0])
            .with_min_inner_size([500.0, 360.0]),
        |ctx, _class| {
            // 关闭按钮 → 隐藏设置窗口
            if ctx.input(|i| i.viewport().close_requested()) {
                ctx.send_viewport_cmd(egui::ViewportCommand::CancelClose);
                crate::ui::state::hide_settings_window();
                return;
            }

            egui::CentralPanel::default().show(ctx, |ui| {
                ui.horizontal_top(|ui| {
                    // ── 左侧菜单 ──
                    let menu_w = 140.0;
                    ui.vertical(|ui| {
                        ui.set_min_width(menu_w);
                        ui.add_space(4.0);

                        let mut tab = STATE.lock().unwrap().settings_tab;
                        let tabs = [
                            (SettingsTab::General, "常规设置"),
                            (SettingsTab::Engines, "翻译服务"),
                            (SettingsTab::About, "关于"),
                        ];
                        for (t, label) in tabs {
                            ui.selectable_value(&mut tab, t, label);
                        }
                        STATE.lock().unwrap().settings_tab = tab;
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
        },
    );
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
        ui.horizontal(|ui| {
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
        let fonts = crate::font_list::get_system_fonts();
        let selected_text = if font.is_empty() {
            "系统默认".to_string()
        } else {
            font.clone()
        };
        egui::ComboBox::from_id_salt("font_combo")
            .selected_text(selected_text)
            .width(220.0)
            .show_ui(ui, |ui| {
                if ui.selectable_label(font.is_empty(), "系统默认").clicked() {
                    font.clear();
                }
                for f in fonts {
                    if ui.selectable_label(*f == font, f).clicked() {
                        font = f.clone();
                    }
                }
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
        ui.horizontal(|ui| {
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
    form_row(ui, "默认源语言", |ui| {
        lang_combo_settings(ui, "default_from_combo", true);
    });
    form_row(ui, "默认目标语言", |ui| {
        lang_combo_settings(ui, "default_to_combo", false);
    });
    ui.add_space(10.0);

    // 输入翻译
    {
        let enabled = STATE.lock().unwrap().config_edit.enable_input_translate;
        let mut enabled = enabled;
        ui.checkbox(&mut enabled, "输入翻译");
        STATE.lock().unwrap().config_edit.enable_input_translate = enabled;
        if enabled {
            form_row(ui, "快捷键", |ui| {
                let mut hk = STATE.lock().unwrap().config_edit.input_hotkey.clone();
                crate::hotkey::hotkey_input(ui, "input_hotkey_field", &mut hk);
                STATE.lock().unwrap().config_edit.input_hotkey = hk;
            });
        }
    }
    ui.add_space(4.0);

    // 划词翻译
    {
        let enabled = STATE.lock().unwrap().config_edit.enable_selection_translate;
        let mut enabled = enabled;
        ui.checkbox(&mut enabled, "划词翻译");
        STATE.lock().unwrap().config_edit.enable_selection_translate = enabled;
        if enabled {
            form_row(ui, "快捷键", |ui| {
                let mut hk = STATE.lock().unwrap().config_edit.selection_hotkey.clone();
                crate::hotkey::hotkey_input(ui, "selection_hotkey_field", &mut hk);
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
            let mut saved_ok = false;
            match s.config_edit.save() {
                Ok(()) => {
                    s.toast = Some("配置已保存".into());
                    saved_ok = true;
                }
                Err(e) => {
                    s.toast = Some(format!("保存失败: {}", e));
                }
            }
            s.toast_time = ui.input(|i| i.time);
            drop(s);
            if saved_ok {
                crate::hotkey::reregister_hotkeys();
            }
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
                    form_row(ui, "名称", |ui| {
                        let mut name_buf = name;
                        ui.add(
                            egui::TextEdit::singleline(&mut name_buf).desired_width(200.0),
                        );
                        STATE.lock().unwrap().config_edit.engines.engines[i].name = name_buf;
                    });

                    // 启用 + 类型
                    form_row(ui, "启用", |ui| {
                        let mut en = enabled;
                        ui.checkbox(&mut en, "");
                        STATE.lock().unwrap().config_edit.engines.engines[i].enabled = en;
                        ui.separator();
                        ui.label("类型:");
                        let mut kind_val = kind;
                        ui.horizontal(|ui| {
                            for k in crate::translate::EngineKind::all() {
                                ui.selectable_value(&mut kind_val, k.clone(), k.label());
                            }
                        });
                        STATE.lock().unwrap().config_edit.engines.engines[i].kind = kind_val;
                    });

                    // API Key
                    form_row(ui, "API Key", |ui| {
                        let mut key_buf = api_key;
                        ui.add(
                            egui::TextEdit::singleline(&mut key_buf)
                                .password(true)
                                .desired_width(220.0),
                        );
                        STATE.lock().unwrap().config_edit.engines.engines[i].api_key = key_buf;
                    });

                    // Secret
                    form_row(ui, "Secret", |ui| {
                        let mut secret_buf = api_secret;
                        ui.add(
                            egui::TextEdit::singleline(&mut secret_buf)
                                .password(true)
                                .desired_width(220.0),
                        );
                        STATE.lock().unwrap().config_edit.engines.engines[i].api_secret =
                            secret_buf;
                    });

                    // Endpoint
                    form_row(ui, "Endpoint", |ui| {
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
            .push(crate::translate::EngineConfig::default());
    }

    ui.add_space(16.0);
    ui.separator();
    ui.add_space(10.0);

    // 保存
    ui.horizontal(|ui| {
        if ui.button("💾 保存配置").clicked() {
            let mut s = STATE.lock().unwrap();
            let mut saved_ok = false;
            match s.config_edit.save() {
                Ok(()) => {
                    s.toast = Some("配置已保存".into());
                    saved_ok = true;
                }
                Err(e) => {
                    s.toast = Some(format!("保存失败: {}", e));
                }
            }
            s.toast_time = ui.input(|i| i.time);
            drop(s);
            if saved_ok {
                crate::hotkey::reregister_hotkeys();
            }
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
                    ui.label(egui::RichText::new("EzTran").size(22.0).strong());
                    ui.label(egui::RichText::new("v0.1.0").color(egui::Color32::from_gray(140)));
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

/// 表单行：左侧固定宽度 label，右侧填充输入控件，实现对齐
fn form_row(ui: &mut egui::Ui, label_text: &str, add_content: impl FnOnce(&mut egui::Ui)) {
    const LABEL_WIDTH: f32 = 90.0;
    ui.horizontal(|ui| {
        let label_resp = ui.add(
            egui::Label::new(egui::RichText::new(label_text).strong())
                .wrap(),
        );
        let pad = (LABEL_WIDTH - label_resp.rect.width()).max(0.0);
        ui.add_space(pad);
        add_content(ui);
    });
}
