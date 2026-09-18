//! 公共 UI 组件 — toast 提示、动画工具函数、表单布局。

use crate::ui::state::STATE;

// ── Toast 提示 ──

/// 显示 toast 提示（3 秒后自动消失）
pub fn show_toast(ctx: &egui::Context, msg: &str) {
    let mut s = STATE.lock().unwrap();
    s.toast = Some(msg.into());
    s.toast_time = ctx.input(|i| i.time);
}

/// toast 自动消失检查（3 秒后清除），在每帧绘制开始时调用
pub fn update_toast(ctx: &egui::Context) {
    let mut state = STATE.lock().unwrap();
    if let Some(toast_time) = state.toast.as_ref().map(|_| state.toast_time) {
        if ctx.input(|i| i.time) - toast_time > 3.0 {
            state.toast = None;
        }
    }
}

/// 渲染 toast 提示（如果存在），在 UI 底部调用
pub fn render_toast(ui: &mut egui::Ui) {
    let toast = STATE.lock().unwrap().toast.clone();
    if let Some(ref t) = toast {
        ui.add_space(6.0);
        ui.colored_label(crate::theme::toast_color(), t);
    }
}

// ── 表单布局 ──

/// 表单行：左侧固定宽度 label，右侧填充输入控件，实现对齐
pub fn form_row(ui: &mut egui::Ui, label_text: &str, add_content: impl FnOnce(&mut egui::Ui)) {
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

// ── 动画工具函数 ──

/// 平滑过渡的 hover alpha 值（0.0 = 完全透明，1.0 = 完全显示）
pub fn hover_anim_alpha(ui: &mut egui::Ui, resp: &egui::Response) -> f32 {
    let target = if resp.hovered() {
        1.0
    } else if resp.is_pointer_button_down_on() {
        0.7
    } else {
        0.0
    };
    anim_towards(ui, resp.id.with("hover_anim"), target, 0.2)
}

/// 持久化动画值，每帧向 target 逼近（帧率无关）
pub fn anim_towards(ui: &mut egui::Ui, id: egui::Id, target: f32, speed: f32) -> f32 {
    let dt = ui.input(|i| i.unstable_dt).min(0.1) as f32;
    let current = ui.memory(|m| m.data.get_temp::<f32>(id).unwrap_or(0.0));
    let new = current + (target - current) * (1.0 - (1.0 - speed).powf(dt * 60.0));
    ui.memory_mut(|m| m.data.insert_temp(id, new));
    new
}

/// 线性插值两个颜色
pub fn lerp_color(a: egui::Color32, b: egui::Color32, t: f32) -> egui::Color32 {
    let t = t.clamp(0.0, 1.0);
    egui::Color32::from_rgb(
        (a.r() as f32 + (b.r() as f32 - a.r() as f32) * t) as u8,
        (a.g() as f32 + (b.g() as f32 - a.g() as f32) * t) as u8,
        (a.b() as f32 + (b.b() as f32 - a.b() as f32) * t) as u8,
    )
}
