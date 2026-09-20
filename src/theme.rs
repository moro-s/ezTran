use std::sync::atomic::AtomicBool;

/// 缓存"当前是否为浅色主题"，由 setup_style 每帧更新，供颜色函数零锁读取
static EFFECTIVE_LIGHT: AtomicBool = AtomicBool::new(false);

/// 加载字体并注入 egui（仅初始化一次）
pub fn setup_fonts(ctx: &egui::Context) {
    use std::sync::atomic::{AtomicBool, Ordering};
    static DONE: AtomicBool = AtomicBool::new(false);
    if DONE.swap(true, Ordering::SeqCst) {
        return;
    }

    let mut fonts = egui::FontDefinitions::default();

    // 加载中文候选字体（系统默认）
    let candidates: &[&str] = &[
        "C:\\Windows\\Fonts\\simhei.ttf",
        "C:\\Windows\\Fonts\\Deng.ttf",
        "C:\\Windows\\Fonts\\msyh.ttc",
        "C:\\Windows\\Fonts\\simsun.ttc",
        "/Library/Fonts/Arial Unicode.ttf",
        "/System/Library/Fonts/PingFang.ttc",
        "/System/Library/Fonts/STHeiti Medium.ttc",
        "/usr/share/fonts/truetype/wqy/wqy-microhei.ttc",
        "/usr/share/fonts/truetype/noto/NotoSansCJK-Regular.ttc",
        "/usr/share/fonts/opentype/noto/NotoSansCJK-Regular.ttc",
        "/usr/share/fonts/wqy-microhei/wqy-microhei.ttc",
    ];

    for path in candidates {
        if let Ok(data) = std::fs::read(path) {
            if !is_valid_font(&data) {
                continue;
            }
            fonts
                .font_data
                .insert("chinese".to_owned(), std::sync::Arc::new(egui::FontData::from_owned(data)));
            fonts
                .families
                .entry(egui::FontFamily::Proportional)
                .or_default()
                .push("chinese".to_owned());
            fonts
                .families
                .entry(egui::FontFamily::Monospace)
                .or_default()
                .push("chinese".to_owned());
            break;
        }
    }

    // 加载系统符号字体，用于显示箭头等 Unicode 符号（如 ⇄）
    let symbol_candidates: &[&str] = &[
        "C:\\Windows\\Fonts\\seguisym.ttf",
        "C:\\Windows\\Fonts\\segoeui.ttf",
    ];
    for path in symbol_candidates {
        if let Ok(data) = std::fs::read(path) {
            if !is_valid_font(&data) {
                continue;
            }
            fonts
                .font_data
                .insert("symbols".to_owned(), std::sync::Arc::new(egui::FontData::from_owned(data)));
            fonts
                .families
                .entry(egui::FontFamily::Proportional)
                .or_default()
                .push("symbols".to_owned());
            fonts
                .families
                .entry(egui::FontFamily::Monospace)
                .or_default()
                .push("symbols".to_owned());
            break;
        }
    }

    ctx.set_fonts(fonts);
}

fn is_valid_font(data: &[u8]) -> bool {
    if data.len() < 4 {
        return false;
    }
    let magic = u32::from_be_bytes([data[0], data[1], data[2], data[3]]);
    matches!(
        magic,
        0x00010000 | 0x4F54544F | 0x74727565 | 0x74797031
    )
}

/// 设置 egui 主题配色和字号（根据配置动态切换，不再用 Once 锁死）
pub fn setup_style(ctx: &egui::Context) {
    use std::sync::atomic::{AtomicU16, Ordering};
    // bit 5+ = theme_id, bit 1-2 = font_size_id, bit 0 = effective_light
    static LAST_KEY: AtomicU16 = AtomicU16::new(0);

    let (theme, font_size) = {
        let s = crate::ui::state::STATE.lock().unwrap();
        (s.config_edit.theme.clone(), s.config_edit.font_size.clone())
    };

    // 计算有效浅色状态：System 时读取系统实际深浅模式
    let effective_light = match theme {
        crate::config::AppTheme::Light => true,
        crate::config::AppTheme::Dark => false,
        crate::config::AppTheme::System => system_is_light(),
    };
    // 更新颜色缓存（供 is_light 零锁读取）
    EFFECTIVE_LIGHT.store(effective_light, Ordering::Relaxed);

    let theme_id: u16 = match theme {
        crate::config::AppTheme::Dark => 1,
        crate::config::AppTheme::Light => 2,
        crate::config::AppTheme::System => 3,
    };
    let font_id: u16 = match font_size {
        crate::config::FontSize::Small => 1,
        crate::config::FontSize::Standard => 2,
        crate::config::FontSize::Large => 3,
    };
    let light_bit: u16 = if effective_light { 1 } else { 0 };
    let key = (theme_id << 5) | (font_id << 1) | light_bit;

    // 主题/字号/系统深浅都未变化则跳过（避免每帧重复设置）
    if LAST_KEY.load(Ordering::Relaxed) == key {
        return;
    }
    LAST_KEY.store(key, Ordering::Relaxed);

    let base_size = font_size.size();

    let current_theme = ctx.theme();
    let mut style = (*ctx.style_of(current_theme)).clone();
    style.spacing.item_spacing = egui::vec2(6.0, 6.0);
    style.spacing.button_padding = egui::vec2(10.0, 4.0);
    style.spacing.window_margin = egui::Margin::same(0);
    // 统一所有交互控件高度
    style.spacing.interact_size.y = 26.0;
    // 统一 TextEdit 内边距
    style.spacing.text_edit_width = 220.0;

    // 根据字号配置设置 text_styles
    use egui::{FontFamily, FontId, TextStyle};
    style.text_styles = [
        (TextStyle::Heading, FontId::new(base_size + 6.0, FontFamily::Proportional)),
        (TextStyle::Name("Heading2".into()), FontId::new(base_size + 3.0, FontFamily::Proportional)),
        (TextStyle::Body, FontId::new(base_size, FontFamily::Proportional)),
        (TextStyle::Monospace, FontId::new(base_size - 1.0, FontFamily::Monospace)),
        (TextStyle::Button, FontId::new(base_size, FontFamily::Proportional)),
        (TextStyle::Small, FontId::new(base_size - 3.0, FontFamily::Proportional)),
    ]
    .into();
    ctx.set_style_of(current_theme, style);

    let mut vis = if effective_light {
        egui::Visuals::light()
    } else {
        egui::Visuals::dark()
    };

    // 深色主题自定义配色
    if !effective_light {
        vis.panel_fill = egui::Color32::from_rgb(43, 43, 43);
        vis.window_fill = egui::Color32::from_rgb(30, 30, 30);
        vis.widgets.noninteractive.bg_fill = egui::Color32::from_rgb(43, 43, 43);
        vis.widgets.inactive.bg_fill = egui::Color32::from_rgb(60, 60, 60);
        vis.widgets.hovered.bg_fill = egui::Color32::from_rgb(76, 76, 76);
        vis.widgets.active.bg_fill = egui::Color32::from_rgb(80, 80, 80);
        vis.widgets.noninteractive.fg_stroke =
            egui::Stroke::new(1.0_f32, egui::Color32::from_rgb(224, 224, 224));
        vis.widgets.inactive.fg_stroke =
            egui::Stroke::new(1.0_f32, egui::Color32::from_rgb(224, 224, 224));
        vis.widgets.noninteractive.bg_stroke = egui::Stroke::new(0.0_f32, egui::Color32::TRANSPARENT);
        vis.widgets.inactive.bg_stroke = egui::Stroke::new(0.0_f32, egui::Color32::TRANSPARENT);
        vis.selection.bg_fill = egui::Color32::from_rgba_unmultiplied(128, 128, 128, 128);
        vis.selection.stroke = egui::Stroke::new(1.0_f32, egui::Color32::from_rgba_unmultiplied(128, 128, 128, 128));
    }

    vis.hyperlink_color = egui::Color32::from_rgb(137, 180, 250);

    // 所有控件统一圆角
    let rounding = egui::CornerRadius::same(6);
    vis.widgets.noninteractive.corner_radius = rounding;
    vis.widgets.inactive.corner_radius = rounding;
    vis.widgets.hovered.corner_radius = rounding;
    vis.widgets.active.corner_radius = rounding;
    vis.widgets.open.corner_radius = rounding;
    vis.window_corner_radius = egui::CornerRadius::same(8);
    vis.menu_corner_radius = rounding;

    ctx.set_visuals(vis);
}

// ── 主题感知颜色 ──

/// 当前是否为浅色主题（读取 setup_style 维护的缓存，零锁）
fn is_light() -> bool {
    EFFECTIVE_LIGHT.load(std::sync::atomic::Ordering::Relaxed)
}

/// 检测系统当前是否为浅色模式（读取注册表 AppsUseLightTheme）
#[cfg(windows)]
fn system_is_light() -> bool {
    use std::ffi::c_void;
    extern "system" {
        fn RegOpenKeyExW(
            hkey: *mut c_void,
            lpsubkey: *const u16,
            reserved: u32,
            samdesired: u32,
            phkresult: *mut *mut c_void,
        ) -> i32;
        fn RegQueryValueExW(
            hkey: *mut c_void,
            lpvaluename: *const u16,
            lpreserved: *const u32,
            lptype: *mut u32,
            lpdata: *mut u8,
            lpcbdata: *mut u32,
        ) -> i32;
        fn RegCloseKey(hkey: *mut c_void) -> i32;
    }
    const HKEY_CURRENT_USER: usize = 0x80000001;
    const KEY_READ: u32 = 0x20019;
    const REG_DWORD: u32 = 4;

    let subkey: Vec<u16> = "Software\\Microsoft\\Windows\\CurrentVersion\\Themes\\Personalize"
        .encode_utf16()
        .chain(std::iter::once(0))
        .collect();
    let value: Vec<u16> = "AppsUseLightTheme"
        .encode_utf16()
        .chain(std::iter::once(0))
        .collect();

    let mut hkey: *mut c_void = std::ptr::null_mut();
    let rc = unsafe {
        RegOpenKeyExW(
            HKEY_CURRENT_USER as *mut c_void,
            subkey.as_ptr(),
            0,
            KEY_READ,
            &mut hkey,
        )
    };
    if rc != 0 || hkey.is_null() {
        return false;
    }
    let mut data: u32 = 0;
    let mut len: u32 = 4;
    let mut kind: u32 = 0;
    let rc = unsafe {
        RegQueryValueExW(
            hkey,
            value.as_ptr(),
            std::ptr::null(),
            &mut kind,
            &mut data as *mut u32 as *mut u8,
            &mut len,
        )
    };
    unsafe { RegCloseKey(hkey) };
    rc == 0 && kind == REG_DWORD && data != 0
}

/// 非 Windows 平台默认深色
#[cfg(not(windows))]
fn system_is_light() -> bool {
    false
}

/// 标题栏背景色
pub fn titlebar_bg() -> egui::Color32 {
    if is_light() {
        egui::Color32::from_rgb(243, 243, 243)
    } else {
        egui::Color32::from_rgb(37, 37, 38)
    }
}

/// 标题栏按钮 hover 色
pub fn titlebar_btn_hover() -> egui::Color32 {
    if is_light() {
        egui::Color32::from_rgb(229, 229, 229)
    } else {
        egui::Color32::from_rgb(60, 60, 60)
    }
}

/// 标题栏按钮文字色
pub fn titlebar_text() -> egui::Color32 {
    if is_light() {
        egui::Color32::from_gray(60)
    } else {
        egui::Color32::from_gray(200)
    }
}

/// 面板背景色（左右翻译面板）
pub fn panel_bg() -> egui::Color32 {
    if is_light() {
        egui::Color32::from_rgb(255, 255, 255)
    } else {
        egui::Color32::from_rgb(30, 30, 30)
    }
}

/// 主体内容区背景色
pub fn central_bg() -> egui::Color32 {
    if is_light() {
        egui::Color32::from_rgb(245, 245, 245)
    } else {
        egui::Color32::from_rgb(43, 43, 43)
    }
}

/// 标签文字色（如"原文""译文"）
pub fn label_secondary() -> egui::Color32 {
    if is_light() {
        egui::Color32::from_rgb(90, 90, 90)
    } else {
        egui::Color32::from_rgb(170, 170, 170)
    }
}

/// 次要文字色（占位提示）
pub fn text_hint() -> egui::Color32 {
    if is_light() {
        egui::Color32::from_gray(160)
    } else {
        egui::Color32::from_gray(90)
    }
}

/// 次要文字色（更淡）
pub fn text_hint_dim() -> egui::Color32 {
    if is_light() {
        egui::Color32::from_gray(180)
    } else {
        egui::Color32::from_gray(70)
    }
}

/// 译文文字色
pub fn text_translated() -> egui::Color32 {
    if is_light() {
        egui::Color32::from_gray(40)
    } else {
        egui::Color32::from_gray(230)
    }
}

/// 源文文字色
pub fn text_source() -> egui::Color32 {
    if is_light() {
        egui::Color32::from_gray(60)
    } else {
        egui::Color32::from_gray(200)
    }
}

/// 正在翻译提示文字色
pub fn text_loading() -> egui::Color32 {
    if is_light() {
        egui::Color32::from_gray(130)
    } else {
        egui::Color32::from_gray(120)
    }
}

/// 箭头符号色
/// 快捷键输入框背景色
pub fn hotkey_input_bg(focused: bool, hovered: bool) -> egui::Color32 {
    if is_light() {
        if focused {
            egui::Color32::from_rgb(204, 204, 204)
        } else if hovered {
            egui::Color32::from_rgb(229, 229, 229)
        } else {
            egui::Color32::from_rgb(240, 240, 240)
        }
    } else {
        if focused {
            egui::Color32::from_rgb(80, 80, 80)
        } else if hovered {
            egui::Color32::from_rgb(76, 76, 76)
        } else {
            egui::Color32::from_rgb(60, 60, 60)
        }
    }
}

/// 快捷键输入框文字色
pub fn hotkey_input_text() -> egui::Color32 {
    if is_light() {
        egui::Color32::from_gray(50)
    } else {
        egui::Color32::from_gray(200)
    }
}

/// 历史弹窗中元信息文字色
pub fn history_meta() -> egui::Color32 {
    if is_light() {
        egui::Color32::from_gray(130)
    } else {
        egui::Color32::from_gray(110)
    }
}

/// 历史弹窗中空状态文字色
pub fn history_empty() -> egui::Color32 {
    if is_light() {
        egui::Color32::from_gray(150)
    } else {
        egui::Color32::from_gray(100)
    }
}

/// 历史表格交替行背景色
pub fn history_row_alt() -> egui::Color32 {
    if is_light() {
        egui::Color32::from_gray(245)
    } else {
        egui::Color32::from_gray(45)
    }
}

/// 错误提示文字色
pub fn error_text() -> egui::Color32 {
    if is_light() {
        egui::Color32::from_rgb(200, 50, 50)
    } else {
        egui::Color32::from_rgb(230, 120, 120)
    }
}

/// toast 成功提示文字色
pub fn toast_color() -> egui::Color32 {
    if is_light() {
        egui::Color32::from_rgb(60, 160, 60)
    } else {
        egui::Color32::from_rgb(120, 200, 120)
    }
}
