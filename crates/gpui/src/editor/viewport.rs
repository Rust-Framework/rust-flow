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
/// **成熟方案（ReactFlow / tldraw）**：纯屏幕空间 delta 直接加到 pan offset。
/// - `origin`：鼠标按下时的视口 offset（屏幕空间）
/// - `start_screen`：鼠标按下位置（屏幕坐标）
/// - `current_screen`：鼠标当前位置（屏幕坐标）
///
/// 数学：`new_offset = origin + (current_screen - start_screen)`
/// 鼠标移动多少像素，画布跟随多少像素，实现 1:1 平移。
///
/// 注意：不能用逻辑坐标做 delta，因为平移过程中 viewport.offset 持续变化，
/// 会导致 `to_logical(current)` 产生反馈抖动。
pub fn handle_pan(origin: PointF, start_screen: PointF, current_screen: PointF) -> PointF {
    PointF::new(
        origin.x + (current_screen.x - start_screen.x),
        origin.y + (current_screen.y - start_screen.y),
    )
}
