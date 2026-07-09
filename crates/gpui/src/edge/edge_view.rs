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
    bezier_path, loop_back_path, round_corners, smoothstep_path, step_path,
    straight_path, EdgeType, PointF, PortSide, RectF,
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
/// `line_width` 为逻辑线宽，随 `scale` 缩放，确保缩放时视觉比例一致。
///
/// `dashed` 为 true 时使用虚线样式（用于回环边等语义区分）。
pub(crate) fn paint_polyline(
    points: &[PointF],
    is_bezier: bool,
    dashed: bool,
    line_width: f32,
    scale: f32,
    offset: Point<Pixels>,
    color: Rgba,
    window: &mut Window,
) {
    if points.len() < 2 {
        return;
    }
    let mut path = PathBuilder::stroke(px(line_width * scale));
    if dashed {
        path = path.dash_array(&[px(6.0 * scale), px(4.0 * scale)]);
    }
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
///
/// **贝塞尔曲线特殊处理**：对于三次贝塞尔，`points = [P0, C1, C2, P3]`，
/// 控制点 `C2` 不在曲线上。直接用 `C2 → P3` 方向虽然数学上是正确的切线
/// 方向，但当控制点距离较远时，箭头方向与曲线在端点附近的视觉方向可能
/// 不一致。因此对贝塞尔曲线，在 t=0.9 处采样曲线点，用 `B(1.0) - B(0.9)`
/// 的割线方向作为箭头方向，使箭头与曲线在端点附近的视觉走向一致。
pub(crate) fn paint_arrow(
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
    let tip = points[points.len() - 1];

    // 计算箭头方向（单位向量）。
    let (ux, uy) = if is_bezier && points.len() == 4 {
        // 贝塞尔曲线：在 t=0.9 处采样，用割线方向匹配视觉走向。
        // B(t) = (1-t)³P0 + 3(1-t)²t·P1 + 3(1-t)t²·P2 + t³·P3
        let (p0, p1, p2, p3) = (points[0], points[1], points[2], points[3]);
        let t = 0.9;
        let s = 1.0 - t;
        let bx = s * s * s * p0.x + 3.0 * s * s * t * p1.x + 3.0 * s * t * t * p2.x + t * t * t * p3.x;
        let by = s * s * s * p0.y + 3.0 * s * s * t * p1.y + 3.0 * s * t * t * p2.y + t * t * t * p3.y;
        let dx = p3.x - bx;
        let dy = p3.y - by;
        let len = (dx * dx + dy * dy).sqrt();
        if len < 1e-6 {
            return;
        }
        (dx / len, dy / len)
    } else {
        // 折线：用最后两个点的方向。
        let prev = points[points.len() - 2];
        let dx = tip.x - prev.x;
        let dy = tip.y - prev.y;
        let len = (dx * dx + dy * dy).sqrt();
        if len < 1e-6 {
            return;
        }
        (dx / len, dy / len)
    };

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
    paint_polyline(&points, is_bezier, false, 1.5, scale, offset, color, window);
    paint_arrow(&points, is_bezier, scale, offset, color, window);
}

/// 渲染 Loop 回环边：使用 `loop_back_path` 绕过 Loop 节点下方/左侧。
///
/// `node_bounds` 应包含 Loop 节点 + 所有循环体节点的组合边界。
/// - **横向布局**：回环边从 body 底部出 → 向下 → 向左 → 向上 → 右进 loop_in
/// - **纵向布局**：回环边从 body 底部出 → 向左 → 向上 → 右进 loop_in（绕左侧）
///
/// **样式一致性**：当 `edge_type` 为 `SmoothStep` 或 `Bezier` 时，对
/// `loop_back_path` 产生的折线应用 `round_corners` 圆角处理，与普通边
/// 保持一致的圆角风格。
///
/// **虚线 + 细淡样式**：回环边使用更细的线宽（1.0 vs 普通边 1.5）+ 虚线
/// + 降低透明度（alpha × 0.65），与主流程边视觉区分，弱化回环语义连线、
/// 突出主流程。
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
    // Apply rounded corners for SmoothStep and Bezier.
    //
    // Bezier 使用更大圆角半径（24.0 vs SmoothStep 的 12.0），使回环边在
    // 贝塞尔模式下呈现更平滑的曲线视觉效果，与 SmoothStep 产生明显区分。
    // 回环边必须绕过循环体组合边界，无法使用单段三次贝塞尔直线连接，
    // 因此采用 round_corners 对 U 型折线做圆角化处理。
    let points = match edge_type {
        EdgeType::Bezier => round_corners(&raw, 24.0),
        EdgeType::SmoothStep => round_corners(&raw, 12.0),
        _ => raw,
    };
    // 回环边：细线宽 + 虚线 + 降低透明度，弱化视觉权重。
    let faded = Rgba {
        a: color.a * 0.65,
        ..color
    };
    paint_polyline(&points, false, true, 1.0, scale, offset, faded, window);
    paint_arrow(&points, false, scale, offset, faded, window);
}

/// 绘制路由边：对 A* 产生的 waypoints 应用圆角后绘制折线 + 箭头。
///
/// 路由边由障碍感知 Grid A* 算法计算，waypoints 已经过路径简化
///（仅保留方向变化的拐点）。本函数按 `edge_type` 决定圆角策略：
/// - `Bezier`：`round_corners(waypoints, 24.0)` — 更大圆角模拟平滑曲线
/// - `SmoothStep`：`round_corners(waypoints, 12.0)` — 与普通 SmoothStep 一致
/// - `Step` / `Straight`：直接使用 waypoints（直角折线 / 简化后的直线）
///
/// 路由边始终用折线绘制（`round_corners` 已采样曲线为多点折线），
/// `is_bezier=false` 确保 `paint_arrow` 用最后两点方向计算箭头。
#[allow(clippy::too_many_arguments)]
pub(crate) fn paint_edge_routed(
    waypoints: &[PointF],
    edge_type: EdgeType,
    scale: f32,
    offset: Point<Pixels>,
    color: Rgba,
    window: &mut Window,
) {
    if waypoints.len() < 2 {
        return;
    }
    let points = match edge_type {
        EdgeType::Bezier => round_corners(waypoints, 24.0),
        EdgeType::SmoothStep => round_corners(waypoints, 12.0),
        _ => waypoints.to_vec(),
    };
    paint_polyline(&points, false, false, 1.5, scale, offset, color, window);
    paint_arrow(&points, false, scale, offset, color, window);
}
