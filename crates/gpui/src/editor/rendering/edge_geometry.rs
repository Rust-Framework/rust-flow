//! 稳定几何层：边端点计算、Loop 组合边界、渲染指令枚举。
//!
//! 此模块不含任何 GPUI paint/div 调用，仅做纯几何/拓扑计算。
//! 被 [`super::edges`] 渲染层和 [`crate::editor::hit_test`] 命中测试层
//! 共同复用，是两者的共享契约。

use std::collections::HashSet;

use rust_agent_flow::{Edge, FlowGraph, NodeId, PointF, PortSide, RectF};

use crate::node::NodeRegistry;
use super::super::ports::{port_position_by_side, resolve_port};
use super::super::flow_editor::LayoutDirection;

/// 边渲染指令（区分普通边、Loop 回环边）。
pub(super) enum EdgeRender {
    Normal {
        src: PointF,
        dst: PointF,
        src_side: PortSide,
        dst_side: PortSide,
        edge_type: rust_agent_flow::EdgeType,
    },
    LoopBack {
        src: PointF,
        dst: PointF,
        horizontal: bool,
        node_bounds: RectF,
        edge_type: rust_agent_flow::EdgeType,
    },
}

/// 计算 Loop 节点 + 其所有循环体节点的组合边界。
pub(super) fn compute_loop_bounds(
    graph: &FlowGraph,
    loop_node: NodeId,
    body_nodes: &HashSet<NodeId>,
) -> RectF {
    let mut bounds: Option<RectF> = None;

    if let Some(node) = graph.node(loop_node) {
        bounds = Some(node.bounds());
    }
    for &nid in body_nodes {
        if let Some(node) = graph.node(nid) {
            bounds = match bounds {
                Some(b) => Some(b.union(node.bounds())),
                None => Some(node.bounds()),
            };
        }
    }
    bounds.unwrap_or_default()
}

/// 计算边的端点。
///
/// **端口策略**（与布局方向协同，减少拐弯）：
/// - **循环体节点**：始终强制 Top/Bottom（垂直子流，上进下出），无论主布局方向如何。
///   这样 Loop 的 `loop_body` 出口（Right）→ body 入口（Top），
///   body 出口（Bottom）→ Loop 的 `loop_in` 入口（Left），回环边向下绕回。
/// - **非循环体节点**：按布局方向使用默认端口对（纵向 Top/Bottom，横向 Left/Right）。
pub(crate) fn compute_edge_endpoints(
    edge: &Edge,
    graph: &FlowGraph,
    registry: &NodeRegistry,
    layout: LayoutDirection,
    body_nodes: &HashSet<NodeId>,
    default_src_side: PortSide,
    default_dst_side: PortSide,
) -> (PointF, PortSide, PointF, PortSide) {
    let src_node = match graph.node(edge.source) {
        Some(n) => n,
        None => return (PointF::default(), default_src_side, PointF::default(), default_dst_side),
    };
    let dst_node = match graph.node(edge.target) {
        Some(n) => n,
        None => return (PointF::default(), default_src_side, PointF::default(), default_dst_side),
    };

    let src_is_body = body_nodes.contains(&edge.source);
    let dst_is_body = body_nodes.contains(&edge.target);

    // 循环体节点始终强制 Top/Bottom（垂直子流，上进下出），无论主布局方向。
    let force_src_bottom = src_is_body;
    let force_dst_top = dst_is_body;

    // 源端点
    let (src, src_side) = if force_src_bottom {
        (port_position_by_side(src_node, PortSide::Bottom), PortSide::Bottom)
    } else {
        match edge.source_port.as_deref() {
            Some(pid) => resolve_port(src_node, pid, registry, layout),
            None => (port_position_by_side(src_node, default_src_side), default_src_side),
        }
    };

    // 目标端点
    let (dst, dst_side) = if force_dst_top {
        (port_position_by_side(dst_node, PortSide::Top), PortSide::Top)
    } else {
        match edge.target_port.as_deref() {
            Some(pid) => resolve_port(dst_node, pid, registry, layout),
            None => (port_position_by_side(dst_node, default_dst_side), default_dst_side),
        }
    };

    (src, src_side, dst, dst_side)
}
