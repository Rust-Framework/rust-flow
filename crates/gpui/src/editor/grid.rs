//! 点阵背景渲染。
//!
//! 每个点作为独立的 fill path 单独绘制，避免多子路径 fill 在某些
//! lyon FillTessellator 实现下的渲染缺陷。点为固定屏幕尺寸（2px 半径），
//! 间距随缩放变化。自适应间距限制可见点数量，保证平移时帧率稳定。

use gpui::{px, Bounds, PathBuilder, Point, Pixels, Rgba, Window};

/// 点阵背景间距（逻辑坐标）。
pub(crate) const GRID_SPACING: f32 = 40.0;

/// 绘制点阵背景。
///
/// 点为固定屏幕尺寸（2px 半径），间距随缩放变化。自适应间距：当屏幕
/// 间距 < 20px 时将逻辑间距翻倍，限制点数量上限，避免低缩放时点爆炸。
///
/// 每个点单独构造 fill path 并 paint，确保渲染可靠性。
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

    let dot_r = 1.5_f32;

    let mut gy = start_y;
    while gy <= max_ly {
        let mut gx = start_x;
        while gx <= max_lx {
            let sx = gx * scale + ox;
            let sy = gy * scale + oy;
            // 每个点单独绘制，避免多子路径 fill 的渲染问题。
            let mut path = PathBuilder::fill();
            path.move_to(Point::new(px(sx - dot_r), px(sy)));
            path.line_to(Point::new(px(sx), px(sy - dot_r)));
            path.line_to(Point::new(px(sx + dot_r), px(sy)));
            path.line_to(Point::new(px(sx), px(sy + dot_r)));
            path.close();
            if let Ok(path) = path.build() {
                window.paint_path(path, dot_color);
            }
            gx += spacing;
        }
        gy += spacing;
    }
}
