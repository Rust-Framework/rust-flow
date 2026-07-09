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

/// 边渲染指令（区分普通边、Loop 回环边、路由边）。
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
    /// 障碍感知路由边：由 Grid A* 计算的避障 waypoints。
    ///
    /// 优先级最高：当 `cached_edge_routes` 命中时使用此变体。
    /// 路由失败或未缓存的边回退到 `Normal`（ReactFlow 几何算法）。
    Routed {
        waypoints: Vec<PointF>,
        edge_type: rust_agent_flow::EdgeType,
    },
}

/// 计算 Loop 节点 + 其所有循环体节点的组合边界。
pub(crate) fn compute_loop_bounds(
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
/// **循环体节点布局上下文**：body 节点（在 `body_nodes` 集合中）始终使用
/// `LayoutDirection::Vertical` 作为有效布局方向，因为 `align_loop_body_target`
/// 将 body 节点纵向堆叠。这不是覆写 side——节点自身的 `port_position` 回调
/// 仍决定最终 side：fixed 端口忽略 layout，Auto 端口按 Vertical 返回 Top/Bottom。
///
/// **浮动边**（无 port_id）：按布局主轴判断正向/反向，而非按 `max(dx,dy)`
/// 选主轴（旧 `compute_side_from_position` 在纵向布局下会让水平排列的节点
/// 错误返回 Left/Right）。正向用 `default_*_side`，反向翻转。body 节点固定
/// 使用 Bottom（出）/ Top（入），因为 body 节点始终纵向编排。
pub(crate) fn compute_edge_endpoints(
    edge: &Edge,
    graph: &FlowGraph,
    registry: &NodeRegistry,
    layout: LayoutDirection,
    default_src_side: PortSide,
    default_dst_side: PortSide,
    body_nodes: &HashSet<NodeId>,
) -> (PointF, PortSide, PointF, PortSide) {
    let src_node = match graph.node(edge.source) {
        Some(n) => n,
        None => return (PointF::default(), default_src_side, PointF::default(), default_dst_side),
    };
    let dst_node = match graph.node(edge.target) {
        Some(n) => n,
        None => return (PointF::default(), default_src_side, PointF::default(), default_dst_side),
    };

    let src_layout = if body_nodes.contains(&edge.source) {
        LayoutDirection::Vertical
    } else {
        layout
    };
    let dst_layout = if body_nodes.contains(&edge.target) {
        LayoutDirection::Vertical
    } else {
        layout
    };

    // 浮动边正向/反向：按布局主轴判断，src 和 dst 共用同一个 forward。
    // 纵向看 dy，横向看 dx；正向用 default_side，反向翻转。
    let forward = match layout {
        LayoutDirection::Horizontal => dst_node.center().x >= src_node.center().x,
        LayoutDirection::Vertical => dst_node.center().y >= src_node.center().y,
    };

    let (src, src_side) = match edge.source_port.as_deref() {
        Some(pid) => resolve_port(src_node, pid, registry, src_layout),
        None => {
            let side = if body_nodes.contains(&edge.source) {
                PortSide::Bottom
            } else if forward {
                default_src_side
            } else {
                flip_side(default_src_side)
            };
            (port_position_by_side(src_node, side), side)
        }
    };

    let (dst, dst_side) = match edge.target_port.as_deref() {
        Some(pid) => resolve_port(dst_node, pid, registry, dst_layout),
        None => {
            let side = if body_nodes.contains(&edge.target) {
                PortSide::Top
            } else if forward {
                default_dst_side
            } else {
                flip_side(default_dst_side)
            };
            (port_position_by_side(dst_node, side), side)
        }
    };

    (src, src_side, dst, dst_side)
}

/// 翻转端口 side（浮动边反向连接时使用）。
fn flip_side(side: PortSide) -> PortSide {
    match side {
        PortSide::Top => PortSide::Bottom,
        PortSide::Bottom => PortSide::Top,
        PortSide::Left => PortSide::Right,
        PortSide::Right => PortSide::Left,
        PortSide::Auto => PortSide::Auto,
    }
}
