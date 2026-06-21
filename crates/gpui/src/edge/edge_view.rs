//! EdgeView：边渲染组件，用 canvas + PathBuilder 绘制路径。
//!
//! 支持 4 种连线算法（bezier/straight/step/smoothstep）+ 箭头。
//! 路径计算委托给 core 层 [`edge_path`] 函数，此处仅负责 GPUI 绘制。

use gpui::{canvas, px, IntoElement, PathBuilder, Point, Pixels, Window};
use rust_agent_flow::{
    bezier_path, smoothstep_path, step_path, straight_path, EdgeType, PointF, PortSide,
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
                paint_edge(src, dst, src_side, dst_side, edge_type, window);
            },
        )
    }
}

/// 将 core `PointF` 转换为 GPUI `Point<Pixels>`。
fn to_px(p: PointF) -> Point<Pixels> {
    Point::new(px(p.x), px(p.y))
}

/// 绘制折线（或贝塞尔曲线）。
pub(crate) fn paint_polyline(points: &[PointF], is_bezier: bool, window: &mut Window) {
    if points.len() < 2 {
        return;
    }
    let mut path = PathBuilder::stroke(px(1.5));
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
        window.paint_path(path, gpui::black());
    }
}

/// 绘制箭头（在 dst 点画三角形，方向由最后一段决定）。
pub(crate) fn paint_arrow(points: &[PointF], window: &mut Window) {
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
    path.move_to(to_px(tip));
    path.line_to(to_px(left));
    path.line_to(to_px(right));
    path.line_to(to_px(tip));
    if let Ok(path) = path.build() {
        window.paint_path(path, gpui::black());
    }
}

/// 统一边渲染入口：计算路径点 + 绘制折线 + 绘制箭头。
/// 供 EdgeView::into_element 和 FlowEditorView::render_edges 共用。
pub(crate) fn paint_edge(
    src: PointF,
    dst: PointF,
    src_side: PortSide,
    dst_side: PortSide,
    edge_type: EdgeType,
    window: &mut Window,
) {
    let points = match edge_type {
        EdgeType::Straight => straight_path(src, dst),
        EdgeType::Bezier => bezier_path(src, dst, src_side, dst_side, 0.5),
        EdgeType::Step => step_path(src, dst, src_side, dst_side),
        EdgeType::SmoothStep => smoothstep_path(src, dst, src_side, dst_side, 8.0),
    };
    let is_bezier = edge_type == EdgeType::Bezier && points.len() == 4;
    paint_polyline(&points, is_bezier, window);
    paint_arrow(&points, window);
}
