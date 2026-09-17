use tray_icon::Icon;

/// 生成图标 RGBA 数据（圆角黑底 + 白色 T 字）
fn create_icon_rgba() -> (Vec<u8>, u32, u32) {
    let (w, h) = (64u32, 64u32);
    let mut rgba = Vec::with_capacity((w * h * 4) as usize);

    let radius = 14.0_f32;
    let bar_y_min = 16.0;
    let bar_y_max = 26.0;
    let bar_x_min = 16.0;
    let bar_x_max = 48.0;
    let stem_x_min = 28.0;
    let stem_x_max = 36.0;
    let stem_y_min = 26.0;
    let stem_y_max = 50.0;

    for y in 0..h {
        for x in 0..w {
            let fx = x as f32;
            let fy = y as f32;

            let in_bg = {
                let dx = (fx - w as f32 / 2.0).abs();
                let dy = (fy - h as f32 / 2.0).abs();
                let half_w = w as f32 / 2.0;
                let half_h = h as f32 / 2.0;
                let corner_dx = (dx - (half_w - radius)).max(0.0);
                let corner_dy = (dy - (half_h - radius)).max(0.0);
                corner_dx * corner_dx + corner_dy * corner_dy <= radius * radius
                    || dx <= half_w - radius
                    || dy <= half_h - radius
            };

            let in_t = (fy >= bar_y_min
                && fy <= bar_y_max
                && fx >= bar_x_min
                && fx <= bar_x_max)
                || (fx >= stem_x_min && fx <= stem_x_max && fy >= stem_y_min && fy <= stem_y_max);

            if in_bg && in_t {
                rgba.extend_from_slice(&[245, 245, 245, 255]);
            } else if in_bg {
                rgba.extend_from_slice(&[30, 30, 30, 255]);
            } else {
                rgba.extend_from_slice(&[0, 0, 0, 0]);
            }
        }
    }
    (rgba, w, h)
}

/// 生成托盘图标
pub fn create_tray_icon() -> Icon {
    let (rgba, w, h) = create_icon_rgba();
    Icon::from_rgba(rgba, w, h)
        .unwrap_or_else(|_| Icon::from_rgba(vec![30, 30, 30, 255], 1, 1).unwrap())
}

/// 生成窗口图标数据（与托盘图标一致）
pub fn create_window_icon() -> egui::IconData {
    let (rgba, w, h) = create_icon_rgba();
    egui::IconData {
        rgba,
        width: w,
        height: h,
    }
}

/// 返回图标 RGBA 数据及尺寸
pub fn icon_rgba() -> (Vec<u8>, u32, u32) {
    create_icon_rgba()
}
