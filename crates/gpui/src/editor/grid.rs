//! 点阵背景渲染。
//!
//! 使用 `paint_quad` 绘制每个点（小圆形 quad），利用 GPUI 的
//! `snap_bounds` + `snapped_content_mask` 机制确保像素对齐渲染。
//!
//! **缩放同步**：间距和点大小均以**逻辑坐标**为基准，屏幕值 = 逻辑值 ×
//! `scale`。放大时屏幕间距增大（点变疏）、点变大；缩小时屏幕间距减小
//!（点变密）、点变小。点阵与节点内容同步缩放。
//!
//! **自适应稀疏化**：当缩放极小（屏幕间距 < 10px）时翻倍逻辑间距，限制
//! 可见点数量保证帧率。
//!
//! **性能优化**：点尺寸调细（2.0px 逻辑直径），颜色调淡（由主题提供半透明色），
//! 减少视觉干扰和绘制开销。点阵与边在同一 canvas 层渲染，避免额外分层开销。

use gpui::{Bounds, Corners, Edges, Pixels, Point, Rgba, Window, quad};

/// 默认逻辑点阵间距（逻辑坐标）。
pub(crate) const DEFAULT_GRID_SPACING: f32 = 28.0;

/// 绘制点阵背景。
///
/// `logical_spacing` 为逻辑间距（与节点坐标同一空间），屏幕间距 =
/// `logical_spacing × scale`，随缩放等比变化。点大小同样随缩放等比变化
///（屏幕直径 = `2.0 × scale`，钳位 ≥ 1.0px 保证可见）。
///
/// 每个点用 `paint_quad` 绘制为小圆形（通过 `corner_radii` = 半径实现）。
/// `paint_quad` 内部使用 `snap_bounds`（对齐设备像素）和
/// `snapped_content_mask`（`cover_bounds` 向外取整），确保小尺寸图形
/// 不会被 content_mask 裁剪掉。
///
/// `dot_color` 来自主题（建议半透明），支持亮色/暗色主题切换。
pub(crate) fn paint_grid(
    bounds: Bounds<Pixels>,
    scale: f32,
    logical_spacing: f32,
    offset: Point<Pixels>,
    dot_color: Rgba,
    window: &mut Window,
) {
    let w = bounds.size.width.as_f32();
    let h = bounds.size.height.as_f32();
    // 防御：无效 bounds 不绘制
    if w <= 0.0 || h <= 0.0 || scale <= 0.0 || logical_spacing <= 0.0 {
        return;
    }

    let ox = offset.x.as_f32();
    let oy = offset.y.as_f32();

    // 逻辑间距固定，屏幕间距 = 逻辑间距 × scale（跟随缩放）
    let mut spacing = logical_spacing;
    // 自适应稀疏化：屏幕间距过小时翻倍逻辑间距，限制可见点数量
    while spacing * scale < 10.0 {
        spacing *= 2.0;
    }

    // 可见逻辑范围
    let min_lx = (bounds.origin.x.as_f32() - ox) / scale;
    let min_ly = (bounds.origin.y.as_f32() - oy) / scale;
    let max_lx = (bounds.origin.x.as_f32() + w - ox) / scale;
    let max_ly = (bounds.origin.y.as_f32() + h - oy) / scale;

    let start_x = (min_lx / spacing).floor() * spacing;
    let start_y = (min_ly / spacing).floor() * spacing;

    // 点的屏幕尺寸随缩放等比变化（逻辑直径 2px × scale），钳位保证可见
    // 调细：从 3.0 降至 2.0，减少视觉干扰和绘制面积
    let dot_size = (2.0 * scale).max(1.0);
    let half = dot_size / 2.0;

    let mut gy = start_y;
    while gy <= max_ly {
        let mut gx = start_x;
        while gx <= max_lx {
            let sx = gx * scale + ox;
            let sy = gy * scale + oy;
            // 用 paint_quad 绘制圆形点：corner_radii = half 使正方形变为圆。
            let dot_bounds = Bounds::new(
                Point::new(gpui::px(sx - half), gpui::px(sy - half)),
                gpui::Size::new(gpui::px(dot_size), gpui::px(dot_size)),
            );
            window.paint_quad(quad(
                dot_bounds,
                Corners::all(gpui::px(half)),
                dot_color,
                Edges::default(),
                gpui::transparent_black(),
                gpui::BorderStyle::default(),
            ));
            gx += spacing;
        }
        gy += spacing;
    }
}
