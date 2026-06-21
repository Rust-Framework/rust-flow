//! 端口位置计算。
//!
//! 支持两种端口位置模式：
//! 1. **自定义位置**：节点覆写 [`IFlowNode::port_position`]，按 port_id 返回精确位置
//!    （用于 Condition 多出口、Loop 循环回环端口等结构化节点）。
//! 2. **默认 side-based**：按 side 计算节点边缘中点（用于 Start/End/Action 简单节点）。
//!
//! 端口 side 从 schema 获取（非 Auto 时用 schema 值，Auto 时按布局方向回退）。

use rust_agent_flow::{Edge, FlowGraph, Node, NodeId, PointF, PortDirection, PortId, PortSide};

use crate::node::NodeRegistry;

use super::flow_editor::LayoutDirection;

/// 端口命中区域宽度（逻辑坐标，会随缩放自动缩放）。
pub(crate) const PORT_HIT_WIDTH: f32 = 12.0;

/// 端口圆心相对节点边缘的偏移（逻辑坐标）。
///
/// 0.0 = 端口圆心在节点边缘上（半内半外），与 view.rs 中端口视觉位置对齐。
pub(crate) const PORT_RADIUS: f32 = 0.0;

/// 根据布局方向和端口方向返回默认 side。
///
/// 横向：In→Left, Out→Right；纵向：In→Top, Out→Bottom。
pub(crate) fn default_side(dir: PortDirection, layout: LayoutDirection) -> PortSide {
    match (dir, layout) {
        (PortDirection::In, LayoutDirection::Horizontal) => PortSide::Left,
        (PortDirection::Out, LayoutDirection::Horizontal) => PortSide::Right,
        (PortDirection::In, LayoutDirection::Vertical) => PortSide::Top,
        (PortDirection::Out, LayoutDirection::Vertical) => PortSide::Bottom,
    }
}

/// 按 side 计算节点边缘中点位置（默认算法）。
///
/// 横向布局：Y = 节点中心 Y（端点 Y + 端点 H/2）
/// 纵向布局：X = 节点中心 X（端点 X + 端点 W/2）
pub(crate) fn port_position_by_side(node: &Node, side: PortSide) -> PointF {
    let right = node.position.x + node.size.w;
    let left = node.position.x;
    let top = node.position.y;
    let bottom = node.position.y + node.size.h;
    let mid_x = node.position.x + node.size.w * 0.5;
    let mid_y = node.position.y + node.size.h * 0.5;
    match side {
        PortSide::Right => PointF::new(right + PORT_RADIUS, mid_y),
        PortSide::Left => PointF::new(left - PORT_RADIUS, mid_y),
        PortSide::Top => PointF::new(mid_x, top - PORT_RADIUS),
        PortSide::Bottom => PointF::new(mid_x, bottom + PORT_RADIUS),
        PortSide::Auto => PointF::new(right + PORT_RADIUS, mid_y),
    }
}

/// 解析端口的 side（从 schema 获取，Auto 回退到布局方向默认值）。
pub(crate) fn port_side(
    registry: &NodeRegistry,
    kind: &str,
    port_id: &str,
    layout: LayoutDirection,
) -> PortSide {
    if let Some(flow_node) = registry.get(kind) {
        if let Some(spec) = flow_node.schema().ports.iter().find(|p| p.id == port_id) {
            if spec.side != PortSide::Auto {
                return spec.side;
            }
            return default_side(spec.direction, layout);
        }
    }
    // 回退：未注册节点或端口不存在，按布局方向假设 In/Out
    layout_default_for_unknown(port_id, layout)
}

/// 未注册节点的端口 side 回退（兼容 fallback 渲染的 "in"/"out" 端口）。
fn layout_default_for_unknown(port_id: &str, layout: LayoutDirection) -> PortSide {
    match (port_id, layout) {
        ("in", LayoutDirection::Horizontal) => PortSide::Left,
        ("out", LayoutDirection::Horizontal) => PortSide::Right,
        ("in", LayoutDirection::Vertical) => PortSide::Top,
        ("out", LayoutDirection::Vertical) => PortSide::Bottom,
        _ => PortSide::Right,
    }
}

/// 计算指定端口的精确位置（统一入口）。
///
/// 1. 先尝试 `IFlowNode::port_position`（自定义位置，方向感知）
///    - 若返回自定义位置，**从位置自动推导 side**（离哪条边最近）
/// 2. 回退到 side-based 计算
///
/// 返回 `(位置, side)`。side 用于边的路径算法（bezier/smoothstep 方向控制）。
pub(crate) fn resolve_port(
    node: &Node,
    port_id: &str,
    registry: &NodeRegistry,
    layout: LayoutDirection,
) -> (PointF, PortSide) {
    // 1. 尝试自定义位置（传入 core LayoutDirection）
    if let Some(flow_node) = registry.get(&node.kind) {
        let pid: PortId = port_id.to_string();
        let core_layout = match layout {
            LayoutDirection::Horizontal => rust_agent_flow::LayoutDirection::Horizontal,
            LayoutDirection::Vertical => rust_agent_flow::LayoutDirection::Vertical,
        };
        if let Some(pos) = flow_node.port_position(node, &pid, core_layout) {
            // 从自定义位置自动推导 side（离哪条边最近）
            let side = derive_side_from_position(node, &pos);
            return (pos, side);
        }
    }

    // 2. 回退到 side-based
    let side = port_side(registry, &node.kind, port_id, layout);
    (port_position_by_side(node, side), side)
}

/// 从端口位置自动推导 side（离节点哪条边最近）。
///
/// 用于自定义 `port_position` 返回位置后，推导边的出入方向。
/// 例如 Loop 的 loop_body 在横向布局下位于顶边（Top），纵向布局下位于右边（Right）。
fn derive_side_from_position(node: &Node, pos: &PointF) -> PortSide {
    let left = node.position.x;
    let right = node.position.x + node.size.w;
    let top = node.position.y;
    let bottom = node.position.y + node.size.h;

    let dist_left = (pos.x - left).abs();
    let dist_right = (pos.x - right).abs();
    let dist_top = (pos.y - top).abs();
    let dist_bottom = (pos.y - bottom).abs();

    let min = dist_left.min(dist_right).min(dist_top).min(dist_bottom);
    if min == dist_top {
        PortSide::Top
    } else if min == dist_bottom {
        PortSide::Bottom
    } else if min == dist_left {
        PortSide::Left
    } else {
        PortSide::Right
    }
}

/// 计算边的源端口和目标端口位置。
///
/// 优先使用 edge.source_port / edge.target_port（精确端口），
/// 否则回退到布局方向默认 side。
pub(crate) fn edge_endpoints(
    edge: &Edge,
    graph: &FlowGraph,
    registry: &NodeRegistry,
    layout: LayoutDirection,
    default_src_side: PortSide,
    default_dst_side: PortSide,
) -> (PointF, PortSide, PointF, PortSide) {
    let src = compute_endpoint(
        edge.source,
        edge.source_port.as_deref(),
        graph,
        registry,
        layout,
        default_src_side,
    );
    let dst = compute_endpoint(
        edge.target,
        edge.target_port.as_deref(),
        graph,
        registry,
        layout,
        default_dst_side,
    );
    (src.0, src.1, dst.0, dst.1)
}

/// 计算单个端点位置（内部辅助）。
fn compute_endpoint(
    node_id: NodeId,
    port_id: Option<&str>,
    graph: &FlowGraph,
    registry: &NodeRegistry,
    layout: LayoutDirection,
    default_side: PortSide,
) -> (PointF, PortSide) {
    let node = match graph.node(node_id) {
        Some(n) => n,
        None => return (PointF::default(), default_side),
    };

    match port_id {
        Some(pid) => resolve_port(node, pid, registry, layout),
        None => (port_position_by_side(node, default_side), default_side),
    }
}
