/// 加载字体并注入 egui（根据配置动态切换，不再用 Once 锁死）
pub fn setup_fonts(ctx: &egui::Context) {
    use std::sync::atomic::{AtomicU64, Ordering};
    static LAST_FONT_HASH: AtomicU64 = AtomicU64::new(0);

    let font_family = crate::ui::state::STATE
        .lock()
        .unwrap()
        .config_edit
        .font_family
        .clone();

    // 用字体名做 hash，未变化则跳过
    let hash = font_family_hash(&font_family);
    if LAST_FONT_HASH.load(Ordering::SeqCst) == hash {
        return;
    }
    LAST_FONT_HASH.store(hash, Ordering::SeqCst);

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
                .insert("chinese".to_owned(), egui::FontData::from_owned(data));
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

    // 加载用户选择的自定义字体
    if !font_family.is_empty() {
        if let Some(path) = find_font_file(&font_family) {
            if let Ok(data) = std::fs::read(&path) {
                if is_valid_font(&data) {
                    fonts.font_data.insert(
                        "user_font".to_owned(),
                        egui::FontData::from_owned(data),
                    );
                    // 将用户字体放到列表最前面，优先使用
                    fonts
                        .families
                        .entry(egui::FontFamily::Proportional)
                        .or_default()
                        .insert(0, "user_font".to_owned());
                    fonts
                        .families
                        .entry(egui::FontFamily::Monospace)
                        .or_default()
                        .insert(0, "user_font".to_owned());
                    log::info!("[theme] 已加载用户字体: {} -> {}", font_family, path);
                }
            }
        } else {
            log::warn!("[theme] 未找到字体文件: {}", font_family);
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
                .insert("symbols".to_owned(), egui::FontData::from_owned(data));
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

/// 简单 hash 字体名用于变更检测
fn font_family_hash(name: &str) -> u64 {
    use std::hash::{Hash, Hasher};
    let mut hasher = std::collections::hash_map::DefaultHasher::new();
    name.hash(&mut hasher);
    hasher.finish()
}

/// 通过 Windows 注册表查找字体文件路径
#[cfg(windows)]
fn find_font_file(font_name: &str) -> Option<String> {
    use std::ffi::c_void;
    use std::os::windows::ffi::OsStrExt;

    extern "system" {
        fn RegOpenKeyExW(
            hkey: *mut c_void,
            subkey: *const u16,
            options: u32,
            access: u32,
            result: *mut *mut c_void,
        ) -> i32;
        fn RegCloseKey(hkey: *mut c_void) -> i32;
        fn RegEnumValueW(
            hkey: *mut c_void,
            index: u32,
            value_name: *mut u16,
            value_name_len: *mut u32,
            reserved: *mut u32,
            value_type: *mut u32,
            data: *mut u8,
            data_len: *mut u32,
        ) -> i32;
    }

    const HKEY_LOCAL_MACHINE: usize = 0x80000002;
    const KEY_READ: u32 = 0x20019;
    const SUBKEY: &str = "SOFTWARE\\Microsoft\\Windows NT\\CurrentVersion\\Fonts";
    let subkey_wide: Vec<u16> = std::ffi::OsStr::new(SUBKEY)
        .encode_wide()
        .chain(std::iter::once(0))
        .collect();

    let mut hkey: *mut c_void = std::ptr::null_mut();
    let status = unsafe {
        RegOpenKeyExW(
            HKEY_LOCAL_MACHINE as *mut c_void,
            subkey_wide.as_ptr(),
            0,
            KEY_READ,
            &mut hkey,
        )
    };
    if status != 0 {
        return None;
    }

    let target_lower = font_name.to_lowercase();
    let mut index = 0u32;
    let mut result = None;

    loop {
        let mut name_buf = [0u16; 256];
        let mut name_len = name_buf.len() as u32;
        let mut data_buf = [0u8; 1024];
        let mut data_len = data_buf.len() as u32;
        let mut data_type = 0u32;

        let status = unsafe {
            RegEnumValueW(
                hkey,
                index,
                name_buf.as_mut_ptr(),
                &mut name_len,
                std::ptr::null_mut(),
                &mut data_type,
                data_buf.as_mut_ptr(),
                &mut data_len,
            )
        };

        if status != 0 {
            break;
        }

        // 读取值名（UTF-16）
        let name_str = String::from_utf16_lossy(&name_buf[..name_len as usize]);
        // 值名格式: "微软雅黑 (TrueType)" -> 匹配字体名
        if name_str.to_lowercase().starts_with(&target_lower) {
            // 数据是 UTF-16LE 字符串（REG_SZ）
            let data_chars: Vec<u16> = data_buf[..data_len as usize]
                .chunks_exact(2)
                .map(|chunk| u16::from_le_bytes([chunk[0], chunk[1]]))
                .collect();
            let data_str = String::from_utf16_lossy(&data_chars);
            let file_name = data_str.trim_end_matches('\0').to_string();

            let path = if std::path::Path::new(&file_name).is_absolute() {
                file_name
            } else {
                format!("C:\\Windows\\Fonts\\{}", file_name)
            };
            result = Some(path);
            break;
        }

        index += 1;
    }

    unsafe { RegCloseKey(hkey) };
    result
}

#[cfg(not(windows))]
fn find_font_file(_font_name: &str) -> Option<String> {
    None
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
    // 高 8 位 = theme_id, 低 8 位 = font_size_id
    static LAST_KEY: AtomicU16 = AtomicU16::new(0);

    let (theme, font_size) = {
        let s = crate::ui::state::STATE.lock().unwrap();
        (s.config_edit.theme.clone(), s.config_edit.font_size.clone())
    };

    let theme_id: u8 = match theme {
        crate::config::AppTheme::Dark => 1,
        crate::config::AppTheme::Light => 2,
        crate::config::AppTheme::System => 3,
    };
    let font_id: u8 = match font_size {
        crate::config::FontSize::Small => 1,
        crate::config::FontSize::Standard => 2,
        crate::config::FontSize::Large => 3,
    };
    let key = ((theme_id as u16) << 8) | (font_id as u16);

    // 主题和字号都未变化则跳过（避免每帧重复设置）
    if LAST_KEY.load(Ordering::SeqCst) == key {
        return;
    }
    LAST_KEY.store(key, Ordering::SeqCst);

    let base_size = font_size.size();

    let mut style = (*ctx.style()).clone();
    style.spacing.item_spacing = egui::vec2(6.0, 6.0);
    style.spacing.button_padding = egui::vec2(10.0, 4.0);
    style.spacing.window_margin = egui::Margin::same(0.0);

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
    ctx.set_style(style);

    let mut vis = match theme {
        crate::config::AppTheme::Light => egui::Visuals::light(),
        _ => egui::Visuals::dark(),
    };

    // 深色主题自定义配色
    if !matches!(theme, crate::config::AppTheme::Light) {
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
    ctx.set_visuals(vis);
}
