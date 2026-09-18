use crate::config::AppConfig;
use crate::icon;
use crate::theme;
use crate::ui;
use crate::window;
use crate::hotkey;
use tray_icon::menu::{Menu, MenuEvent, MenuId, MenuItem, PredefinedMenuItem};
use tray_icon::{TrayIcon, TrayIconBuilder, TrayIconEvent};

/// 全局菜单 ID，供事件处理器直接比较
static TRANSLATE_MENU_ID: std::sync::LazyLock<std::sync::Mutex<Option<MenuId>>> =
    std::sync::LazyLock::new(|| std::sync::Mutex::new(None));
static SETTINGS_MENU_ID: std::sync::LazyLock<std::sync::Mutex<Option<MenuId>>> =
    std::sync::LazyLock::new(|| std::sync::Mutex::new(None));
static QUIT_MENU_ID: std::sync::LazyLock<std::sync::Mutex<Option<MenuId>>> =
    std::sync::LazyLock::new(|| std::sync::Mutex::new(None));

pub struct EzTranApp {
    _tray: Option<TrayIcon>,
}

impl EzTranApp {
    pub fn new(config: AppConfig) -> Self {
        ui::init_state(config);

        let menu = Menu::new();
        let translate_item = MenuItem::new("翻译", true, None);
        let settings_item = MenuItem::new("设置", true, None);
        let quit_item = MenuItem::new("退出", true, None);

        let _ = menu.append(&translate_item);
        let _ = menu.append(&settings_item);
        let _ = menu.append(&PredefinedMenuItem::separator());
        let _ = menu.append(&quit_item);

        *TRANSLATE_MENU_ID.lock().unwrap() = Some(translate_item.id().clone());
        *SETTINGS_MENU_ID.lock().unwrap() = Some(settings_item.id().clone());
        *QUIT_MENU_ID.lock().unwrap() = Some(quit_item.id().clone());

        let tray = TrayIconBuilder::new()
            .with_menu(Box::new(menu))
            .with_tooltip("EzTran - 轻量翻译工具")
            .with_icon(icon::create_tray_icon())
            .build()
            .ok();

        // 托盘菜单事件 — 直接在回调中处理，不依赖 update 被调用
        MenuEvent::set_event_handler(Some(|event: MenuEvent| {
            let id = &event.id;
            if QUIT_MENU_ID.lock().unwrap().as_ref().is_some_and(|qid| id == qid) {
                log::info!("[tray] 菜单事件: 退出");
                std::process::exit(0);
            }
            if SETTINGS_MENU_ID.lock().unwrap().as_ref().is_some_and(|sid| id == sid) {
                log::info!("[tray] 菜单事件: 设置");
                // 只弹出设置窗口，不恢复翻译工作台
                ui::show_settings_window();
                window::wake();
                log::logger().flush();
                return;
            }
            if TRANSLATE_MENU_ID.lock().unwrap().as_ref().is_some_and(|tid| id == tid) {
                log::info!("[tray] 菜单事件: 翻译");
                window::show_main_if_hidden();
                window::wake();
                return;
            }
            log::debug!("[tray] 菜单事件: 未匹配的 id={:?}", id);
        }));

        // 托盘图标双击 → 显示翻译工作台
        TrayIconEvent::set_event_handler(Some(|event: TrayIconEvent| {
            if let TrayIconEvent::DoubleClick { .. } = event {
                log::info!("[tray] 图标双击: 显示翻译工作台");
                window::show_main_if_hidden();
                window::wake();
            }
        }));

        window::start_repaint_thread();
        hotkey::start_global_hotkey_thread();

        EzTranApp { _tray: tray }
    }
}

impl eframe::App for EzTranApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        window::set_ctx(ctx);
        window::try_set_round_corners();

        theme::setup_fonts(ctx);
        theme::setup_style(ctx);

        // 消费全局热键标志
        // 输入翻译：唤起翻译工作台并清空上次内容
        if hotkey::consume_input_translate() {
            log::info!("[hotkey] 输入翻译快捷键触发 — 唤起工作台");
            {
                let mut s = crate::ui::state::STATE.lock().unwrap();
                s.input_text.clear();
                s.result = None;
            }
            window::show_main_if_hidden();
            window::wake();
        }
        // 划词翻译：获取选中文本 → 显示窗口 → 自动翻译
        if hotkey::consume_selection_translate() {
            log::info!("[hotkey] 划词翻译快捷键触发");
            handle_selection_translate(ctx);
        }

        // Alt+F4 → 隐藏到托盘
        window::handle_close_request(ctx);

        // 设置窗口（独立视口，主窗口隐藏时也需渲染）
        let settings_visible = ui::is_settings_visible();
        let main_hidden = window::is_main_hidden();
        if settings_visible {
            log::debug!("[update] 调用 draw_settings (main_hidden={})", main_hidden);
            ui::draw_settings(ctx);
        }

        // 翻译历史弹窗
        {
            let show_history = ui::state::STATE.lock().unwrap().show_history;
            if show_history {
                ui::history::draw_history(ctx);
            }
        }

        // 执行延迟隐藏
        if window::process_pending_hide() {
            log::debug!("[update] process_pending_hide 已处理，跳过本帧绘制");
            return;
        }

        // 主窗口隐藏时跳过绘制
        if window::is_main_hidden() {
            return;
        }

        // 窗口尺寸未就绪时跳过绘制
        let screen = ctx.screen_rect();
        if screen.height() < 80.0 || screen.width() < 10.0 {
            return;
        }

        // 首次启动将主窗口居中
        center_main_window_on_first_launch(ctx);

        draw_titlebar(ctx);
        ui::draw_translate(ctx);

        // 无边框窗口边框缩放（在所有面板绘制之后，用透明交互区覆盖窗口边缘）
        draw_resize_borders(ctx);
    }

    fn on_exit(&mut self, _gl: Option<&eframe::glow::Context>) {
        self._tray.take();
    }
}

/// 首次启动将主窗口居中（仅执行一次）
fn center_main_window_on_first_launch(ctx: &egui::Context) {
    use std::sync::atomic::{AtomicBool, Ordering};
    static CENTERED: AtomicBool = AtomicBool::new(false);
    if CENTERED.swap(true, Ordering::SeqCst) {
        return;
    }
    let (win_w, win_h) = (820.0, 520.0);
    let (screen_w, screen_h) = window::get_screen_size();
    let pos = egui::pos2(
        ((screen_w as f32 - win_w) / 2.0).max(0.0),
        ((screen_h as f32 - win_h) / 2.0).max(0.0),
    );
    log::info!(
        "[window] 主窗口首次启动居中: pos=({:.0},{:.0}) screen={}x{}",
        pos.x, pos.y, screen_w, screen_h
    );
    ctx.send_viewport_cmd(egui::ViewportCommand::OuterPosition(pos));
}

/// 划词翻译处理：从热键线程获取选中文本 → 显示窗口 → 自动翻译
fn handle_selection_translate(ctx: &egui::Context) {
    let clip_text = hotkey::take_selection_text();

    window::show_main_if_hidden();
    window::wake();

    if let Some(text) = clip_text {
        if !text.trim().is_empty() {
            {
                let mut s = crate::ui::state::STATE.lock().unwrap();
                s.input_text = text.clone();
            }
            let (from, to, engine_idx) = {
                let s = crate::ui::state::STATE.lock().unwrap();
                (s.from_lang.clone(), s.to_lang.clone(), s.engine_index)
            };
            crate::ui::translate::do_translate(&text, &from, &to, engine_idx, ctx);
        }
    }
}

/// 获取图标纹理（缓存）
fn get_icon_texture(ctx: &egui::Context) -> egui::TextureHandle {
    use std::sync::OnceLock;
    static TEX: OnceLock<egui::TextureHandle> = OnceLock::new();
    if let Some(tex) = TEX.get() {
        return tex.clone();
    }
    let (rgba, w, h) = icon::icon_rgba();
    let tex = ctx.load_texture(
        "titlebar_icon",
        egui::ColorImage {
            size: [w as usize, h as usize],
            pixels: rgba
                .chunks_exact(4)
                .map(|c| egui::Color32::from_rgba_unmultiplied(c[0], c[1], c[2], c[3]))
                .collect(),
        },
        egui::TextureOptions::LINEAR,
    );
    let _ = TEX.set(tex.clone());
    tex
}

/// 绘制自绘标题栏（置顶 / 最小化 / 最大化 / 关闭）
fn draw_titlebar(ctx: &egui::Context) {
    let titlebar_bg = theme::titlebar_bg();
    egui::TopBottomPanel::top("custom_titlebar")
        .exact_height(34.0)
        .frame(
            egui::Frame::default()
                .fill(titlebar_bg)
                .inner_margin(egui::Margin::same(0.0))
                .rounding(egui::Rounding {
                    nw: 8.0,
                    ne: 8.0,
                    sw: 0.0,
                    se: 0.0,
                }),
        )
        .show(ctx, |ui| {
            ui.set_min_height(34.0);
            ui.spacing_mut().item_spacing = egui::vec2(0.0, 0.0);

            ui.with_layout(egui::Layout::left_to_right(egui::Align::Center), |ui| {
                // ── 左侧图标 ──
                ui.add_space(8.0);
                let tex = get_icon_texture(ctx);
                let img_resp = ui.add(
                    egui::Image::from_texture(&tex).fit_to_exact_size(egui::vec2(18.0, 18.0)),
                );

                let title_drag = ui.interact(
                    img_resp.rect,
                    ui.id().with("title_drag"),
                    egui::Sense::drag(),
                );

                // ── 中间空白（可拖动）──
                let btn_w = 46.0 * 4.0;
                let drag_w = (ui.available_width() - btn_w).max(0.0);
                let drag_resp =
                    ui.allocate_response(egui::vec2(drag_w, 34.0), egui::Sense::drag());

                for resp in [title_drag, drag_resp] {
                    if resp.drag_started() {
                        window::start_drag();
                    }
                }

                // ── 右侧按钮 ──
                // 置顶
                let pinned = window::is_pinned();
                let pin_resp = titlebar_button(ui, "\u{1F4CC}", theme::titlebar_btn_hover());
                if pinned {
                    ui.painter().rect_filled(
                        pin_resp.rect,
                        0.0,
                        theme::pin_overlay(),
                    );
                    ui.painter().text(
                        pin_resp.rect.center(),
                        egui::Align2::CENTER_CENTER,
                        "\u{1F4CC}",
                        egui::FontId::proportional(14.0),
                        theme::titlebar_text(),
                    );
                }
                if pin_resp.clicked() {
                    let new_pinned = window::toggle_pin();
                    let level = if new_pinned {
                        egui::WindowLevel::AlwaysOnTop
                    } else {
                        egui::WindowLevel::Normal
                    };
                    ctx.send_viewport_cmd(egui::ViewportCommand::WindowLevel(level));
                }

                // 最小化
                let min_resp = titlebar_button(ui, "\u{2014}", theme::titlebar_btn_hover());
                if min_resp.clicked() {
                    window::minimize_main();
                }

                // 最大化/还原
                let maximized = ctx.input(|i| i.viewport().maximized.unwrap_or(false));
                let max_icon = if maximized { "\u{2750}" } else { "\u{25A2}" };
                let max_resp = titlebar_button(ui, max_icon, theme::titlebar_btn_hover());
                if max_resp.clicked() {
                    window::toggle_maximize(maximized);
                }

                // 关闭
                let close_resp =
                    titlebar_button(ui, "\u{2715}", egui::Color32::from_rgb(232, 17, 35));
                if close_resp.clicked() {
                    window::hide_main();
                }
            });
        });
}

/// 标题栏按钮（透明背景，hover 时变色）
fn titlebar_button(
    ui: &mut egui::Ui,
    icon: &str,
    hover_color: egui::Color32,
) -> egui::Response {
    let resp = ui.allocate_response(egui::vec2(46.0, 34.0), egui::Sense::click());
    if resp.hovered() {
        ui.painter().rect_filled(resp.rect, 0.0, hover_color);
    }
    ui.painter().text(
        resp.rect.center(),
        egui::Align2::CENTER_CENTER,
        icon,
        egui::FontId::proportional(14.0),
        theme::titlebar_text(),
    );
    resp
}

/// 绘制无边框窗口的透明缩放边框。
/// 在窗口四边和四角放置透明交互区，拖拽时通过 WM_NCLBUTTONDOWN 让系统接管缩放。
fn draw_resize_borders(ctx: &egui::Context) {
    use crate::window;
    use egui::Sense;

    let screen = ctx.screen_rect();
    let b = 6.0; // 边框宽度（像素）
    let c = 12.0; // 角落尺寸（像素）

    // 最大化时不显示缩放边框
    if ctx.input(|i| i.viewport().maximized.unwrap_or(false)) {
        return;
    }

    egui::Area::new(egui::Id::new("resize_borders"))
        .order(egui::Order::Foreground)
        .fixed_pos(screen.min)
        .interactable(true)
        .show(ctx, |ui| {
            ui.set_min_size(screen.size());
            // ── 四条边 ──
            // 左边
            let r = ui.allocate_rect(
                egui::Rect::from_min_size(screen.min, egui::vec2(b, screen.height())),
                Sense::drag(),
            );
            if r.drag_started() {
                window::start_resize(window::HTLEFT);
            }
            // 右边
            let r = ui.allocate_rect(
                egui::Rect::from_min_size(
                    egui::pos2(screen.right() - b, screen.top()),
                    egui::vec2(b, screen.height()),
                ),
                Sense::drag(),
            );
            if r.drag_started() {
                window::start_resize(window::HTRIGHT);
            }
            // 上边
            let r = ui.allocate_rect(
                egui::Rect::from_min_size(screen.min, egui::vec2(screen.width(), b)),
                Sense::drag(),
            );
            if r.drag_started() {
                window::start_resize(window::HTTOP);
            }
            // 下边
            let r = ui.allocate_rect(
                egui::Rect::from_min_size(
                    egui::pos2(screen.left(), screen.bottom() - b),
                    egui::vec2(screen.width(), b),
                ),
                Sense::drag(),
            );
            if r.drag_started() {
                window::start_resize(window::HTBOTTOM);
            }

            // ── 四个角 ──
            // 左上
            let r = ui.allocate_rect(
                egui::Rect::from_min_size(screen.min, egui::vec2(c, c)),
                Sense::drag(),
            );
            if r.drag_started() {
                window::start_resize(window::HTTOPLEFT);
            }
            // 右上
            let r = ui.allocate_rect(
                egui::Rect::from_min_size(
                    egui::pos2(screen.right() - c, screen.top()),
                    egui::vec2(c, c),
                ),
                Sense::drag(),
            );
            if r.drag_started() {
                window::start_resize(window::HTTOPRIGHT);
            }
            // 左下
            let r = ui.allocate_rect(
                egui::Rect::from_min_size(
                    egui::pos2(screen.left(), screen.bottom() - c),
                    egui::vec2(c, c),
                ),
                Sense::drag(),
            );
            if r.drag_started() {
                window::start_resize(window::HTBOTTOMLEFT);
            }
            // 右下
            let r = ui.allocate_rect(
                egui::Rect::from_min_size(
                    egui::pos2(screen.right() - c, screen.bottom() - c),
                    egui::vec2(c, c),
                ),
                Sense::drag(),
            );
            if r.drag_started() {
                window::start_resize(window::HTBOTTOMRIGHT);
            }
        });
}
