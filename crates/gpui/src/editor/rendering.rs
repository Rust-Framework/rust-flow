//! 渲染层：边、节点、属性面板的渲染方法。
//!
//! - **边**：canvas paint，逻辑坐标 + PathBuilder 变换到屏幕空间
//! - **节点**：absolute div 在内容层内，`pos × scale` 定位
//! - **面板**：右侧浮动，不受缩放影响
//!
//! ## Loop 循环体特殊处理
//!
//! 循环体节点（从 `loop_body` 出口可达的节点）始终使用**纵向端口**
//!（上进下出），无论主布局方向如何。回环边（目标端口为 `loop_in`）
//! 使用 `loop_back_path` 向下绕过 Loop 节点 + 循环体的组合边界。

use std::collections::{HashMap, HashSet, VecDeque};
use std::sync::Arc;

use gpui::{canvas, div, px, IntoElement, ParentElement, Point, Styled};
use rust_agent_flow::{Edge, EdgeType, FlowGraph, NodeId, PointF, PortSide, RectF};

use crate::edge::{paint_edge_scaled, paint_join_marker, paint_loop_back_edge};
use crate::node::{IFlowNode, NodeView};
use crate::panel::PanelView;

use super::flow_editor::{FlowEditorView, LayoutDirection};
use super::grid::paint_grid;
use super::interaction::InteractionState;
use super::ports::{port_position_by_side, resolve_port};

/// 边渲染指令（区分普通边、Loop 回环边、Join 汇聚标记边）。
enum EdgeRender {
    Normal {
        src: PointF,
        dst: PointF,
        src_side: PortSide,
        dst_side: PortSide,
        edge_type: EdgeType,
    },
    LoopBack {
        src: PointF,
        dst: PointF,
        horizontal: bool,
        node_bounds: RectF,
    },
    /// 带有 Join 汇聚标记的边：在距目标 80 单位处渲染小方块。
    Join {
        src: PointF,
        dst: PointF,
        src_side: PortSide,
        dst_side: PortSide,
        edge_type: EdgeType,
        color: gpui::Rgba, // Join 标记颜色（跟随源节点主题）
    },
}

/// 收集每个 Loop 节点关联的循环体节点组。
///
/// 循环体节点 = `loop_body` 出口的目标 + 从这些节点沿前向边可达的节点
///（排除通过 `loop_in` 回连的边和回到 Loop 节点的边）。
fn collect_loop_body_groups(graph: &FlowGraph) -> HashMap<NodeId, HashSet<NodeId>> {
    let mut groups: HashMap<NodeId, HashSet<NodeId>> = HashMap::new();

    // 找到所有 loop_body 边，按源节点（Loop 节点）分组
    for edge in graph.edges() {
        if edge.source_port.as_deref() == Some("loop_body") {
            groups.entry(edge.source).or_default().insert(edge.target);
        }
    }

    // BFS 扩展每组：沿前向边可达的节点都是循环体的一部分
    for (loop_node, body_nodes) in groups.iter_mut() {
        let mut queue: VecDeque<NodeId> = body_nodes.iter().copied().collect();
        while let Some(nid) = queue.pop_front() {
            for edge in graph.out_edges(nid) {
                // 跳过回环边（到 loop_in）
                if edge.target_port.as_deref() == Some("loop_in") {
                    continue;
                }
                // 跳过回到 Loop 节点的边（如 done）
                if edge.target == *loop_node {
                    continue;
                }
                if body_nodes.insert(edge.target) {
                    queue.push_back(edge.target);
                }
            }
        }
    }

    groups
}

/// 计算 Loop 节点 + 其所有循环体节点的组合边界。
fn compute_loop_bounds(graph: &FlowGraph, loop_node: NodeId, body_nodes: &HashSet<NodeId>) -> RectF {
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

/// 检测汇聚目标节点：接收多条入边的节点（分支汇合点）。
///
/// 返回目标节点 ID 集合。这些节点上的入边需要渲染 Join 汇聚标记。
///
/// **检测规则**：
/// - 目标节点的入边数量 ≥ 2，且来自**不同源节点**
/// - 排除回环边（loop_in）和循环体内部边
fn detect_convergence_targets(graph: &FlowGraph, body_nodes: &HashSet<NodeId>) -> HashSet<NodeId> {
    let mut incoming: HashMap<NodeId, Vec<NodeId>> = HashMap::new();

    for edge in graph.edges() {
        // 排除回环边和循环体内部边
        if edge.target_port.as_deref() == Some("loop_in") {
            continue;
        }
        if body_nodes.contains(&edge.source) && body_nodes.contains(&edge.target) {
            continue;
        }
        incoming.entry(edge.target).or_default().push(edge.source);
    }

    incoming
        .into_iter()
        .filter(|(_target, sources)| {
            // 至少 2 个不同来源
            let unique: HashSet<_> = sources.iter().collect();
            unique.len() >= 2
        })
        .map(|(target, _)| target)
        .collect()
}

/// 计算边的端点，循环体节点强制使用纵向端口（上进下出）。
fn compute_edge_endpoints(
    edge: &Edge,
    graph: &FlowGraph,
    registry: &crate::node::NodeRegistry,
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

    // 源端点：循环体节点强制 Bottom，其他用 resolve_port
    let (src, src_side) = if src_is_body {
        (port_position_by_side(src_node, PortSide::Bottom), PortSide::Bottom)
    } else {
        match edge.source_port.as_deref() {
            Some(pid) => resolve_port(src_node, pid, registry, layout),
            None => (port_position_by_side(src_node, default_src_side), default_src_side),
        }
    };

    // 目标端点：循环体节点强制 Top，其他用 resolve_port
    let (dst, dst_side) = if dst_is_body {
        (port_position_by_side(dst_node, PortSide::Top), PortSide::Top)
    } else {
        match edge.target_port.as_deref() {
            Some(pid) => resolve_port(dst_node, pid, registry, layout),
            None => (port_position_by_side(dst_node, default_dst_side), default_dst_side),
        }
    };

    (src, src_side, dst, dst_side)
}

impl FlowEditorView {
    /// 当前视口缩放比例。
    pub(crate) fn scale(&self) -> f32 {
        self.viewport.scale
    }

    /// 渲染所有边（canvas paint），使用**逻辑坐标** + PathBuilder 变换。
    ///
    /// 边端点通过 [`compute_edge_endpoints`] 计算：
    /// - 循环体节点强制纵向端口（Top 进 / Bottom 出）
    /// - 回环边（target_port == "loop_in"）使用 `loop_back_path` 向下绕过
    /// - 其他边使用 `edge_endpoints` 默认逻辑
    pub(crate) fn render_edges(&self) -> impl IntoElement {
        let s = self.scale();
        let (src_side_default, dst_side_default) = self.port_sides();
        let layout = self.layout_direction;
        let registry = self.registry.clone();

        // 收集循环体节点分组
        let body_groups = collect_loop_body_groups(&self.graph);
        let all_body_nodes: HashSet<NodeId> =
            body_groups.values().flat_map(|s| s.iter().copied()).collect();

        // 检测汇聚目标节点（接收多条入边的节点）
        let convergence_targets = detect_convergence_targets(&self.graph, &all_body_nodes);

        // 为每条边计算渲染指令
        let edge_renders: Vec<EdgeRender> = self
            .graph
            .edges()
            .map(|edge| {
                let is_loop_back = edge.target_port.as_deref() == Some("loop_in");

                let (src, src_side, dst, dst_side) = compute_edge_endpoints(
                    edge,
                    &self.graph,
                    &registry,
                    layout,
                    &all_body_nodes,
                    src_side_default,
                    dst_side_default,
                );

                if is_loop_back {
                    // 回环边：找到目标 Loop 节点的循环体组，计算组合边界
                    let node_bounds = body_groups
                        .get(&edge.target)
                        .map(|body| compute_loop_bounds(&self.graph, edge.target, body))
                        .unwrap_or_else(|| {
                            // 回退：仅用 Loop 节点自身边界
                            self.graph.node(edge.target).map(|n| n.bounds()).unwrap_or_default()
                        });

                    EdgeRender::LoopBack {
                        src,
                        dst,
                        horizontal: matches!(layout, LayoutDirection::Horizontal),
                        node_bounds,
                    }
                } else if convergence_targets.contains(&edge.target) {
                    // 汇聚边：目标节点接收多条入边，渲染 Join 标记
                    // 颜色跟随源节点主题（Condition=橙, Loop=蓝, 默认=灰）
                    let color = match self.graph.node(edge.source).map(|n| n.kind.as_str()) {
                        Some("condition") => gpui::rgb(0xf97316),
                        Some("loop") => gpui::rgb(0x3b82f6),
                        _ => gpui::rgb(0x64748b),
                    };
                    EdgeRender::Join {
                        src,
                        dst,
                        src_side,
                        dst_side,
                        edge_type: edge.edge_type,
                        color,
                    }
                } else {
                    EdgeRender::Normal {
                        src,
                        dst,
                        src_side,
                        dst_side,
                        edge_type: edge.edge_type,
                    }
                }
            })
            .collect();

        let default_edge_type = self.default_edge_type;
        let drawing = match &self.interaction {
            InteractionState::DrawingEdge {
                from_node,
                from_port,
                current,
                ..
            } => self.graph.node(*from_node).map(|n| {
                let (src, src_side) = resolve_port(n, from_port, &registry, layout);
                let dst = *current;
                (src, dst, src_side, dst_side_default, default_edge_type)
            }),
            _ => None,
        };

        let offset_x = self.viewport.offset.x;
        let offset_y = self.viewport.offset.y;
        let show_grid = self.show_grid;

        canvas(
            |bounds, _window, _cx| bounds.size,
            move |bounds, _size, window, _cx| {
                let total_offset = Point::new(
                    px(offset_x + bounds.origin.x.as_f32()),
                    px(offset_y + bounds.origin.y.as_f32()),
                );
                if show_grid {
                    paint_grid(bounds, s, total_offset, window);
                }
                for er in &edge_renders {
                    match er {
                        EdgeRender::Normal { src, dst, src_side, dst_side, edge_type } => {
                            paint_edge_scaled(
                                *src, *dst, *src_side, *dst_side, *edge_type, s, total_offset, window,
                            );
                        }
                        EdgeRender::LoopBack { src, dst, horizontal, node_bounds } => {
                            paint_loop_back_edge(
                                *src, *dst, *horizontal, *node_bounds, s, total_offset, window,
                            );
                        }
                        EdgeRender::Join { src, dst, src_side, dst_side, edge_type, color } => {
                            paint_edge_scaled(
                                *src, *dst, *src_side, *dst_side, *edge_type, s, total_offset, window,
                            );
                            paint_join_marker(*src, *dst, *color, s, total_offset, window);
                        }
                    }
                }
                if let Some((src, dst, src_side, dst_side, edge_type)) = drawing {
                    paint_edge_scaled(
                        src, dst, src_side, dst_side, edge_type, s, total_offset, window,
                    );
                }
            },
        )
    }

    /// 渲染所有节点（absolute div 在内容层内）。
    ///
    /// 节点最终屏幕坐标 = content_offset + logical_pos × scale
    pub(crate) fn render_nodes(&self) -> Vec<gpui::AnyElement> {
        let selected = self.selected;
        let registry = &self.registry;
        let s = self.scale();
        let layout = match self.layout_direction {
            LayoutDirection::Horizontal => rust_agent_flow::LayoutDirection::Horizontal,
            LayoutDirection::Vertical => rust_agent_flow::LayoutDirection::Vertical,
        };

        // 收集循环体节点（与 render_edges 保持一致）
        let body_groups = collect_loop_body_groups(&self.graph);
        let all_body_nodes: HashSet<NodeId> =
            body_groups.values().flat_map(|s| s.iter().copied()).collect();

        self.graph
            .nodes()
            .map(|node| {
                let node_id = node.id;
                let pos = node.position;
                let flow_node = registry.get(&node.kind);
                let is_selected = selected == Some(node_id);
                let is_body = all_body_nodes.contains(&node_id);

                let view = NodeView::new(node.clone())
                    .with_flow_node_opt(flow_node)
                    .selected(is_selected)
                    .with_scale(s)
                    .with_layout(layout)
                    .with_body_mode(is_body);

                div()
                    .absolute()
                    .left(px(pos.x * s))
                    .top(px(pos.y * s))
                    .child(view)
                    .into_any_element()
            })
            .collect()
    }

    /// 渲染属性面板。
    pub(crate) fn render_panel(&self) -> Option<gpui::AnyElement> {
        let node = self.selected.and_then(|id| self.graph.node(id).cloned())?;
        let flow_node = self.registry.get(&node.kind);
        let panel = PanelView::new(node).with_flow_node_opt(flow_node);
        Some(
            div()
                .absolute()
                .right_0()
                .top_0()
                .bottom_0()
                .child(panel)
                .into_any_element(),
        )
    }
}

// ---- 视图扩展（仅在渲染层使用） ----

impl NodeView {
    pub(crate) fn with_flow_node_opt(mut self, flow_node: Option<Arc<dyn IFlowNode>>) -> Self {
        self.flow_node = flow_node;
        self
    }
}

impl PanelView {
    pub(crate) fn with_flow_node_opt(mut self, flow_node: Option<Arc<dyn IFlowNode>>) -> Self {
        self.flow_node = flow_node;
        self
    }
}
