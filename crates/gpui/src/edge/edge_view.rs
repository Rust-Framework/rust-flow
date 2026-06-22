//! EdgeView：边渲染组件，用 canvas + PathBuilder 绘制路径。
//!
//! 支持 4 种连线算法（bezier/straight/step/smoothstep）+ 箭头。
//! 路径计算委托给 core 层 [`edge_path`] 函数，此处仅负责 GPUI 绘制。
//!
//! ## 缩放方案
//!
//! 所有绘制函数接收**逻辑坐标**，通过 `PathBuilder::scale` + `translate`
//! 统一变换到屏幕空间。这样路径几何（含 step gap、smoothstep 圆角半径）
//! 自动随缩放变化，仅需手动缩放线宽（`stroke_width * scale`）。

use gpui::{canvas, px, IntoElement, PathBuilder, Point, Pixels, Rgba, Window};
use rust_agent_flow::{
    bezier_path, loop_back_path, round_corners, smoothstep_path, step_path, straight_path, EdgeType,
    PointF, PortSide, RectF,
};

/// 默认边颜色（亮色主题下的回退值，实际颜色应由调用方通过参数传入）。
const EDGE_COLOR_DEFAULT: Rgba = Rgba {
    r: 0xb1 as f32 / 255.0,
    g: 0xb1 as f32 / 255.0,
    b: 0xb7 as f32 / 255.0,
    a: 1.0,
};

/// 边视图：持有点位和类型，`into_element` 返回 canvas 元素。
pub struct EdgeView {
    pub src: PointF,
    pub dst: PointF,
    pub src_side: PortSide,
    pub dst_side: PortSide,
    pub edge_type: EdgeType,
}

impl EdgeView {
    pub fn new(
        src: PointF,
        dst: PointF,
        src_side: PortSide,
        dst_side: PortSide,
        edge_type: EdgeType,
    ) -> Self {
        Self {
            src,
            dst,
            src_side,
            dst_side,
            edge_type,
        }
    }

    /// 返回 canvas 元素，在 paint 回调中绘制边 + 箭头。
    pub fn into_element(self) -> impl IntoElement {
        let (src, dst, src_side, dst_side, edge_type) =
            (self.src, self.dst, self.src_side, self.dst_side, self.edge_type);
        canvas(
            |bounds, _window, _cx| bounds.size,
            move |_bounds, _size, window, _cx| {
                // EdgeView 独立使用时默认 1:1 无偏移，使用默认边色。
                paint_edge_scaled(
                    src,
                    dst,
                    src_side,
                    dst_side,
                    edge_type,
                    1.0,
                    Point::new(px(0.0), px(0.0)),
                    EDGE_COLOR_DEFAULT,
                    window,
                );
            },
        )
    }
}

/// 将 core `PointF` 转换为 GPUI `Point<Pixels>`。
fn to_px(p: PointF) -> Point<Pixels> {
    Point::new(px(p.x), px(p.y))
}

/// 绘制折线（或贝塞尔曲线）。
///
/// `points` 为**逻辑坐标**，通过 `scale` + `offset` 变换到屏幕空间。
/// 线宽随 `scale` 缩放，确保缩放时视觉比例一致。
pub(crate) fn paint_polyline(
    points: &[PointF],
    is_bezier: bool,
    scale: f32,
    offset: Point<Pixels>,
    color: Rgba,
    window: &mut Window,
) {
    if points.len() < 2 {
        return;
    }
    let mut path = PathBuilder::stroke(px(1.5 * scale));
    path.scale(scale);
    path.translate(offset);
    path.move_to(to_px(points[0]));
    if is_bezier {
        // 三次贝塞尔：cubic_bezier_to(to, control_a, control_b)
        path.cubic_bezier_to(to_px(points[3]), to_px(points[1]), to_px(points[2]));
    } else {
        for p in points.iter().skip(1) {
            path.line_to(to_px(*p));
        }
    }
    if let Ok(path) = path.build() {
        window.paint_path(path, color);
    }
}

/// 绘制箭头（在 dst 点画三角形，方向由最后一段决定）。
///
/// `points` 为**逻辑坐标**，箭头尺寸（8.0 逻辑单位）随 `scale` 自动缩放。
pub(crate) fn paint_arrow(
    points: &[PointF],
    scale: f32,
    offset: Point<Pixels>,
    color: Rgba,
    window: &mut Window,
) {
    if points.len() < 2 {
        return;
    }
    let tip = points[points.len() - 1];
    let prev = points[points.len() - 2];

    let dx = tip.x - prev.x;
    let dy = tip.y - prev.y;
    let len = (dx * dx + dy * dy).sqrt();
    if len < 1e-6 {
        return;
    }
    let ux = dx / len;
    let uy = dy / len;

    // 箭头尺寸为逻辑单位，PathBuilder::scale 会自动缩放到屏幕空间。
    let size = 8.0;
    let left = PointF::new(
        tip.x - ux * size - uy * size * 0.5,
        tip.y - uy * size + ux * size * 0.5,
    );
    let right = PointF::new(
        tip.x - ux * size + uy * size * 0.5,
        tip.y - uy * size - ux * size * 0.5,
    );

    let mut path = PathBuilder::fill();
    path.scale(scale);
    path.translate(offset);
    path.move_to(to_px(tip));
    path.line_to(to_px(left));
    path.line_to(to_px(right));
    path.line_to(to_px(tip));
    if let Ok(path) = path.build() {
        window.paint_path(path, color);
    }
}

/// 统一边渲染入口：计算路径点 + 绘制折线 + 绘制箭头。
///
/// **坐标空间**：`src`/`dst` 为**逻辑坐标**，由 `scale` + `offset`
/// 通过 PathBuilder 变换统一映射到屏幕空间。
///
/// - `scale`：视口缩放比例
/// - `offset`：屏幕偏移 = `viewport.offset + canvas bounds.origin`
/// - `color`：边描边色（来自主题）
///
/// 路径几何（含 step 的 20px gap、smoothstep 的 12px 圆角）在逻辑空间
/// 计算，随 `scale` 自动缩放，确保缩放时连线与节点保持几何一致。
#[allow(clippy::too_many_arguments)]
pub(crate) fn paint_edge_scaled(
    src: PointF,
    dst: PointF,
    src_side: PortSide,
    dst_side: PortSide,
    edge_type: EdgeType,
    scale: f32,
    offset: Point<Pixels>,
    color: Rgba,
    window: &mut Window,
) {
    let points = match edge_type {
        EdgeType::Straight => straight_path(src, dst),
        EdgeType::Bezier => bezier_path(src, dst, src_side, dst_side, 0.5),
        EdgeType::Step => step_path(src, dst, src_side, dst_side),
        EdgeType::SmoothStep => smoothstep_path(src, dst, src_side, dst_side, 12.0),
    };
    let is_bezier = edge_type == EdgeType::Bezier && points.len() == 4;
    paint_polyline(&points, is_bezier, scale, offset, color, window);
    paint_arrow(&points, scale, offset, color, window);
}

/// 渲染 Loop 回环边：使用 `loop_back_path` 绕过 Loop 节点下方。
///
/// `node_bounds` 应包含 Loop 节点 + 所有循环体节点的组合边界，
/// 确保回环路径从最后循环体节点的底部 → 向下 → 向左 → 向上 → 回到 loop_in。
///
/// **样式一致性**：当 `edge_type` 为 `SmoothStep` 时，对 `loop_back_path`
/// 产生的折线应用 `round_corners` 圆角处理，与普通边保持一致的圆角风格。
///
/// **颜色区分**：回环边使用 `color` 参数（来自主题的回环边色），与普通边
/// 的默认色区分，突出循环语义。
#[allow(clippy::too_many_arguments)]
pub(crate) fn paint_loop_back_edge(
    src: PointF,
    dst: PointF,
    horizontal: bool,
    node_bounds: RectF,
    edge_type: EdgeType,
    scale: f32,
    offset: Point<Pixels>,
    color: Rgba,
    window: &mut Window,
) {
    let raw = loop_back_path(src, dst, horizontal, node_bounds);
    // Apply rounded corners for SmoothStep to match the theme.
    let points = match edge_type {
        EdgeType::SmoothStep => round_corners(&raw, 12.0),
        _ => raw,
    };
    paint_polyline(&points, false, scale, offset, color, window);
    paint_arrow(&points, scale, offset, color, window);
}
