//! 视口交互：封装鼠标事件到视口数学的映射。
//!
//! 视口数学（pan/zoom 变换）已在 core 层 [`Viewport`] 实现，此处仅封装
//! 事件→数学的映射，保持 GPUI 类型与数学逻辑解耦。

use rust_agent_flow::{PointF, Viewport};

/// 处理滚轮缩放，以鼠标位置（逻辑坐标）为锚点。
///
/// `delta` 为滚轮 delta 值：`< 0` 向上滚（放大），`> 0` 向下滚（缩小）。
/// 返回新的 Viewport。
pub fn handle_zoom(viewport: Viewport, mouse_logical: PointF, delta: f32) -> Viewport {
    // delta < 0 → 放大（scale 增大）；delta > 0 → 缩小。
    let factor = if delta < 0.0 { 1.1 } else { 1.0 / 1.1 };
    let new_scale = viewport.scale * factor;
    // zoom_around 接收 screen 坐标锚点，但我们在逻辑空间操作。
    // 转换：mouse_screen = viewport.to_screen(mouse_logical)
    let mouse_screen = viewport.to_screen(mouse_logical);
    viewport.zoom_around(mouse_screen, new_scale)
}

/// 处理平移拖拽，返回新的 offset。
///
/// `origin` 为视口 offset 起点，`start` 为鼠标按下起点（逻辑坐标），
/// `current` 为当前鼠标位置（逻辑坐标）。
///
/// 注意：平移在屏幕空间更直观，但此处统一用逻辑坐标。
/// 实际实现中，鼠标移动 delta 在屏幕空间，需转换为逻辑空间 delta 后应用到 offset。
pub fn handle_pan(origin: PointF, start: PointF, current: PointF) -> PointF {
    // offset 是逻辑原点在屏幕空间的偏移。
    // 鼠标在屏幕空间移动 (current - start)（逻辑坐标差，但平移不涉及缩放变换）。
    // 新 offset = origin + (current - start)
    PointF::new(
        origin.x + (current.x - start.x),
        origin.y + (current.y - start.y),
    )
}
