//! 点阵背景渲染。
//!
//! 使用 `paint_quad` 绘制每个点（小圆形 quad），利用 GPUI 的
//! `snap_bounds` + `snapped_content_mask` 机制确保像素对齐渲染。
//! 之前使用 `paint_path`（PathBuilder::fill）绘制 3px 菱形，
//! 由于 `paint_path` 不做设备像素对齐，极小路径在 `insert_primitive`
//! 的 `intersect` 裁剪后可能被判定为空而跳过，导致点阵不可见。
//!
//! 点为固定屏幕尺寸，间距随缩放变化。自适应间距限制可见点数量，
//! 保证平移时帧率稳定。

use gpui::{Bounds, Corners, Edges, Pixels, Point, Rgba, Window, quad};

/// 点阵背景间距（逻辑坐标）。
pub(crate) const GRID_SPACING: f32 = 40.0;

/// 绘制点阵背景。
///
/// 每个点用 `paint_quad` 绘制为小圆形（通过 `corner_radii` = 半径实现）。
/// `paint_quad` 内部使用 `snap_bounds`（对齐设备像素）和
/// `snapped_content_mask`（`cover_bounds` 向外取整），确保小尺寸图形
/// 不会被 content_mask 裁剪掉。
///
/// `dot_color` 来自主题，支持亮色/暗色主题切换。
pub(crate) fn paint_grid(
    bounds: Bounds<Pixels>,
    scale: f32,
    offset: Point<Pixels>,
    dot_color: Rgba,
    window: &mut Window,
) {
    let w = bounds.size.width.as_f32();
    let h = bounds.size.height.as_f32();
    // 防御：无效 bounds 不绘制
    if w <= 0.0 || h <= 0.0 || scale <= 0.0 {
        return;
    }

    let ox = offset.x.as_f32();
    let oy = offset.y.as_f32();

    // 自适应间距：屏幕间距过小时翻倍
    let mut spacing = GRID_SPACING;
    while spacing * scale < 20.0 {
        spacing *= 2.0;
    }

    // 可见逻辑范围
    let min_lx = (bounds.origin.x.as_f32() - ox) / scale;
    let min_ly = (bounds.origin.y.as_f32() - oy) / scale;
    let max_lx = (bounds.origin.x.as_f32() + w - ox) / scale;
    let max_ly = (bounds.origin.y.as_f32() + h - oy) / scale;

    let start_x = (min_lx / spacing).floor() * spacing;
    let start_y = (min_ly / spacing).floor() * spacing;

    // 点的屏幕尺寸（直径 3px，半径 1.5px）
    let dot_size = 3.0_f32;
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
