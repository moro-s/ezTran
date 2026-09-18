use crate::config::AppConfig;
use crate::icon;
use crate::theme;
use crate::ui;
use crate::ui::components::{anim_towards, hover_anim_alpha, lerp_color};
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
                // 置顶（钉住状态高亮背景，未钉住透明）
                let pinned = window::is_pinned();
                let pin_resp = ui.allocate_response(egui::vec2(46.0, 34.0), egui::Sense::click());
                draw_titlebar_btn_anim(
                    ui,
                    &pin_resp,
                    pinned,
                    egui::Color32::from_rgb(70, 120, 200),
                    "\u{1F4CC}",
                    if pinned { egui::Color32::WHITE } else { theme::titlebar_text() },
                );
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
                let close_resp = titlebar_button_red(ui, "\u{2715}");
                if close_resp.clicked() {
                    window::hide_main();
                }
            });
        });
}

/// 标题栏按钮（透明背景，hover 时平滑过渡到指定颜色）
fn titlebar_button(
    ui: &mut egui::Ui,
    icon: &str,
    hover_color: egui::Color32,
) -> egui::Response {
    let resp = ui.allocate_response(egui::vec2(46.0, 34.0), egui::Sense::click());
    let alpha = hover_anim_alpha(ui, &resp);
    if alpha > 0.01 {
        let c = hover_color;
        let a = (alpha * c.a() as f32) as u8;
        ui.painter().rect_filled(
            resp.rect,
            6.0,
            egui::Color32::from_rgba_unmultiplied(c.r(), c.g(), c.b(), a),
        );
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

/// 关闭按钮（hover 时平滑过渡到红色背景，图标渐变为白色）
fn titlebar_button_red(ui: &mut egui::Ui, icon: &str) -> egui::Response {
    let resp = ui.allocate_response(egui::vec2(46.0, 34.0), egui::Sense::click());
    let alpha = hover_anim_alpha(ui, &resp);
    if alpha > 0.01 {
        let a = (alpha * 255.0) as u8;
        ui.painter().rect_filled(
            resp.rect,
            6.0,
            egui::Color32::from_rgba_unmultiplied(232, 17, 35, a),
        );
    }
    let base = theme::titlebar_text();
    let icon_color = lerp_color(base, egui::Color32::WHITE, alpha);
    ui.painter().text(
        resp.rect.center(),
        egui::Align2::CENTER_CENTER,
        icon,
        egui::FontId::proportional(14.0),
        icon_color,
    );
    resp
}

/// 钉住/激活态按钮的动画绘制
fn draw_titlebar_btn_anim(
    ui: &mut egui::Ui,
    resp: &egui::Response,
    active: bool,
    active_color: egui::Color32,
    icon: &str,
    icon_color: egui::Color32,
) {
    let target = if active { 1.0 } else { 0.0 };
    let alpha = anim_towards(ui, resp.id.with("pin_anim"), target, 0.15);

    if alpha > 0.01 {
        let hover_boost = if resp.hovered() { 30.0 } else { 0.0 };
        let c = active_color;
        let a = ((alpha * 255.0) + hover_boost).min(255.0) as u8;
        ui.painter().rect_filled(
            resp.rect,
            0.0,
            egui::Color32::from_rgba_unmultiplied(c.r(), c.g(), c.b(), a),
        );
    } else if resp.hovered() {
        let c = theme::titlebar_btn_hover();
        ui.painter().rect_filled(resp.rect, 0.0, c);
    }

    ui.painter().text(
        resp.rect.center(),
        egui::Align2::CENTER_CENTER,
        icon,
        egui::FontId::proportional(14.0),
        icon_color,
    );
}

/// 绘制无边框窗口的角落缩放手柄。
/// 仅在窗口四个角放置小交互区，hover 时在该角对应的两条边框上绘制蓝色高亮，
/// 拖拽时通过 WM_NCLBUTTONDOWN 让系统接管。
fn draw_resize_borders(ctx: &egui::Context) {
    use crate::window;

    let screen = ctx.screen_rect();
    let corner_size = 16.0;

    // 最大化时不显示缩放手柄
    if ctx.input(|i| i.viewport().maximized.unwrap_or(false)) {
        return;
    }

    // 鼠标位置（屏幕坐标）
    let mouse_pos = ctx.input(|i| i.pointer.hover_pos());

    // 四个角：交互矩形、命中代码、该角对应的两条边线段（全局坐标）
    let corners = [
        // 右下角
        (
            egui::Rect::from_min_size(
                egui::pos2(screen.right() - corner_size, screen.bottom() - corner_size),
                egui::vec2(corner_size, corner_size),
            ),
            window::HTBOTTOMRIGHT,
            [
                (egui::pos2(screen.right() - corner_size, screen.bottom()),
                 egui::pos2(screen.right(), screen.bottom())),
                (egui::pos2(screen.right(), screen.bottom() - corner_size),
                 egui::pos2(screen.right(), screen.bottom())),
            ],
        ),
        // 左下角
        (
            egui::Rect::from_min_size(
                egui::pos2(screen.left(), screen.bottom() - corner_size),
                egui::vec2(corner_size, corner_size),
            ),
            window::HTBOTTOMLEFT,
            [
                (egui::pos2(screen.left(), screen.bottom() - corner_size),
                 egui::pos2(screen.left(), screen.bottom())),
                (egui::pos2(screen.left(), screen.bottom()),
                 egui::pos2(screen.left() + corner_size, screen.bottom())),
            ],
        ),
        // 右上角
        (
            egui::Rect::from_min_size(
                egui::pos2(screen.right() - corner_size, screen.top()),
                egui::vec2(corner_size, corner_size),
            ),
            window::HTTOPRIGHT,
            [
                (egui::pos2(screen.right() - corner_size, screen.top()),
                 egui::pos2(screen.right(), screen.top())),
                (egui::pos2(screen.right(), screen.top()),
                 egui::pos2(screen.right(), screen.top() + corner_size)),
            ],
        ),
        // 左上角
        (
            egui::Rect::from_min_size(screen.min, egui::vec2(corner_size, corner_size)),
            window::HTTOPLEFT,
            [
                (egui::pos2(screen.left(), screen.top()),
                 egui::pos2(screen.left() + corner_size, screen.top())),
                (egui::pos2(screen.left(), screen.top()),
                 egui::pos2(screen.left(), screen.top() + corner_size)),
            ],
        ),
    ];

    let painter = ctx.layer_painter(egui::LayerId::new(
        egui::Order::Foreground,
        egui::Id::new("resize_corners"),
    ));

    for (rect, hit_code, edges) in &corners {
        let hovered = mouse_pos.map(|m| rect.contains(m)).unwrap_or(false);

        // hover 时绘制蓝色边框高亮
        if hovered {
            let highlight = egui::Color32::from_rgb(100, 160, 255);
            let stroke = egui::Stroke::new(2.0_f32, highlight);
            for (start, end) in edges {
                painter.line_segment([*start, *end], stroke);
            }
        }

        // 交互检测：鼠标在角内 + 按下 → 开始缩放
        if hovered && ctx.input(|i| i.pointer.primary_pressed()) {
            window::start_resize(*hit_code);
        }
    }
}
