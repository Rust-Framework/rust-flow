//! 点阵背景渲染。
//!
//! 性能优化（参考 ReactFlow / tldraw 成熟方案）：
//! - 所有点收集到**单个 fill path**，一次 `paint_path` 提交，减少 draw call。
//! - 使用 `move_to` / `line_to` 显式构造矩形子路径（避免 `add_polygon` 在
//!   多子路径 fill 下的渲染缺陷）。
//! - 自适应间距限制可见点数量，保证平移时帧率稳定。

use gpui::{px, Bounds, PathBuilder, Point, Pixels, Window};

/// 点阵背景间距（逻辑坐标）。
pub(crate) const GRID_SPACING: f32 = 40.0;

/// 绘制点阵背景。
///
/// 点为固定屏幕尺寸（1.5px 半径），间距随缩放变化。自适应间距：当屏幕
/// 间距 < 20px 时将逻辑间距翻倍，限制点数量上限，避免低缩放时点爆炸。
pub(crate) fn paint_grid(
    bounds: Bounds<Pixels>,
    scale: f32,
    offset: Point<Pixels>,
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

    let dot_color = gpui::rgb(0x94a3b8); // slate-400，在 0xf8fafc 背景上有足够对比度
    let dot_r = 2.0_f32;

    // 单个 fill path 收集所有点，一次提交。
    let mut path = PathBuilder::fill();
    let mut count: usize = 0;
    let mut gy = start_y;
    while gy <= max_ly {
        let mut gx = start_x;
        while gx <= max_lx {
            let sx = gx * scale + ox;
            let sy = gy * scale + oy;
            // 显式构造矩形子路径（move_to + line_to + 闭合）。
            // 比 add_polygon 更可靠：确保每个子路径被正确加入 fill。
            path.move_to(Point::new(px(sx - dot_r), px(sy - dot_r)));
            path.line_to(Point::new(px(sx + dot_r), px(sy - dot_r)));
            path.line_to(Point::new(px(sx + dot_r), px(sy + dot_r)));
            path.line_to(Point::new(px(sx - dot_r), px(sy + dot_r)));
            path.line_to(Point::new(px(sx - dot_r), px(sy - dot_r)));
            count += 1;
            gx += spacing;
        }
        gy += spacing;
    }

    if count > 0 {
        if let Ok(path) = path.build() {
            window.paint_path(path, dot_color);
        }
    }
}
