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
/// **端口策略**（强弱约束模型）：
/// - 端口 side 由节点声明（PortSpec + IFlowNode::port_position），外部只读不写
/// - 强约束端口（fixed=true）：side 由节点实现决定，本函数不覆盖
/// - 弱约束端口（fixed=false, side=Auto）：按布局方向回退到默认 side
///
/// 循环体节点的端口 side 不再被外部强制为 Top/Bottom。循环体的垂直子流语义
/// 由 Loop 节点的 loop_body/loop_in 强约束 side + 边路径算法协同保证。
pub(crate) fn compute_edge_endpoints(
    edge: &Edge,
    graph: &FlowGraph,
    registry: &NodeRegistry,
    layout: LayoutDirection,
    _body_nodes: &HashSet<NodeId>,
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

    let (src, src_side) = match edge.source_port.as_deref() {
        Some(pid) => resolve_port(src_node, pid, registry, layout),
        None => (port_position_by_side(src_node, default_src_side), default_src_side),
    };

    let (dst, dst_side) = match edge.target_port.as_deref() {
        Some(pid) => resolve_port(dst_node, pid, registry, layout),
        None => (port_position_by_side(dst_node, default_dst_side), default_dst_side),
    };

    (src, src_side, dst, dst_side)
}
