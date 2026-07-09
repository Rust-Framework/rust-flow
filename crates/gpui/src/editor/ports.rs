//! 端口位置计算。
//!
//! 支持两种端口位置模式：
//! 1. **自定义位置 + 显式 side**：节点覆写 [`IFlowNode::port_position`]，返回
//!    `(位置, side)`（用于 Condition 多出口、Loop 循环回环端口等结构化节点）。
//! 2. **默认 side-based**：按 side 计算节点边缘中点（用于 Start/End/Action 简单节点）。
//!
//! 端口 side 由节点显式声明（强弱约束模型）：强约束端口（fixed=true）的 side
//! 由节点实现决定，外部布局层只读不写；弱约束端口（fixed=false, Auto）按布局
//! 方向回退。

use rust_agent_flow::{Node, PointF, PortDirection, PortId, PortSide};

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
        PortSide::Auto => {
            debug_assert!(false, "PortSide::Auto must be resolved before position calculation");
            PointF::new(right + PORT_RADIUS, mid_y)
        }
    }
}

/// 解析端口的 side（从 schema 获取，Auto 回退到布局方向默认值）。
///
/// **强弱约束感知**：
/// - 强约束（fixed=true）：始终返回 spec.side，外部不可覆盖
/// - 弱约束（fixed=false）：spec.side != Auto 时用 spec.side；Auto 按布局方向回退
pub(crate) fn port_side(
    registry: &NodeRegistry,
    kind: &str,
    port_id: &str,
    layout: LayoutDirection,
) -> PortSide {
    if let Some(flow_node) = registry.get(kind) {
        if let Some(spec) = flow_node.schema().ports.iter().find(|p| p.id == port_id) {
            // Strong constraint (fixed=true): always use declared side.
            // Weak constraint (fixed=false): Auto → default_side by layout.
            if spec.fixed || spec.side != PortSide::Auto {
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
/// 1. 先尝试 `IFlowNode::port_position`（自定义位置 + 显式 side）
///    - 返回 `Some((pos, side))` 时直接使用，side 为节点显式声明
/// 2. 回退到 side-based 计算（schema side，Auto 按布局方向回退）
///
/// 返回 `(位置, side)`。side 用于边的路径算法（bezier/smoothstep 方向控制）。
pub(crate) fn resolve_port(
    node: &Node,
    port_id: &str,
    registry: &NodeRegistry,
    layout: LayoutDirection,
) -> (PointF, PortSide) {
    if let Some(flow_node) = registry.get(&node.kind) {
        let pid: PortId = port_id.to_string();
        let core_layout = match layout {
            LayoutDirection::Horizontal => rust_agent_flow::LayoutDirection::Horizontal,
            LayoutDirection::Vertical => rust_agent_flow::LayoutDirection::Vertical,
        };
        if let Some((pos, side)) = flow_node.port_position(node, &pid, core_layout) {
            return (pos, side);
        }
    }

    // 回退到 side-based
    let side = port_side(registry, &node.kind, port_id, layout);
    (port_position_by_side(node, side), side)
}
