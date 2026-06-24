//! 渲染层：边、节点、属性面板的渲染方法。
//!
//! - **边**：canvas paint，逻辑坐标 + PathBuilder 变换到屏幕空间
//! - **节点**：absolute div 在内容层内，`pos × scale` 定位
//! - **面板**：右侧浮动，不受缩放影响
//!
//! ## Loop 循环体特殊处理
//!
//! 循环体节点（从 `loop_body` 出口可达的节点）始终使用**纵向端口**
//!（上进下出），无论主布局方向是横向还是纵向。回环边（目标端口为
//! `loop_in`）使用 `loop_back_path` 向下绕过 Loop 节点 + 循环体的组合边界。

use std::collections::{HashMap, HashSet};
use std::sync::Arc;

use gpui::{canvas, div, px, App, AppContext, Entity, IntoElement, ParentElement, Point, Styled};
use gpui_component::{Icon, IconName, Sizable};
use rust_agent_flow::{Edge, EdgeType, FlowGraph, NodeId, PointF, PortSide, RectF};

use crate::edge::{paint_edge_scaled, paint_loop_back_edge};
use crate::node::{ActionCallback, IFlowNode, NodeAction, NodeView};
use crate::panel::PanelView;

use super::flow_editor::{FlowEditorView, LayoutDirection};
use super::grid::paint_grid;
use super::interaction::InteractionState;
use super::ports::{port_position_by_side, resolve_port};

/// 边渲染指令（区分普通边、Loop 回环边）。
enum EdgeRender {
    Normal {
        src: PointF,
        dst: PointF,
        src_side: PortSide,
        dst_side: PortSide,
        edge_type: EdgeType,
        /// 中间层障碍物（按 rank 分组），用于通道分配路由。
        obstacles_by_rank: Vec<Vec<RectF>>,
        horizontal: bool,
    },
    LoopBack {
        src: PointF,
        dst: PointF,
        horizontal: bool,
        node_bounds: RectF,
        edge_type: EdgeType,
    },
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

/// 计算边的中间层障碍物（按 rank 分组）。
///
/// 对于边 src→dst，收集所有 rank 在 (src.rank, dst.rank) 之间的节点
/// （排除 src、dst 自身和隐藏节点），按 rank 分组返回矩形列表。
/// 用于通道分配避障路由：仅在跨层边（dst.rank > src.rank + 1）时产生障碍物。
///
/// **Loop body 节点特殊处理**：
/// - 若边的源或目标是 body 节点（子流边），跳过障碍物计算（返回空），
///   因为 body 节点使用纵向端口，通道分配假设纯水平/垂直流不适用。
/// - 若边是主流边（源和目标都不是 body 节点），排除 body 节点避免
///   不必要的绕行——body 节点位于 Loop 节点下方的独立区域，不在主流路径上。
fn compute_obstacles_by_rank(
    edge: &Edge,
    graph: &FlowGraph,
    ranks: &HashMap<NodeId, i32>,
    hidden_nodes: &HashSet<NodeId>,
    all_body_nodes: &HashSet<NodeId>,
) -> Vec<Vec<RectF>> {
    let src_rank = ranks.get(&edge.source).copied().unwrap_or(0);
    let dst_rank = ranks.get(&edge.target).copied().unwrap_or(0);

    // 不跨层或反向边：无中间层障碍
    if dst_rank <= src_rank + 1 {
        return Vec::new();
    }

    // 子流边（源或目标是 body 节点）：跳过障碍物计算
    // body 节点使用纵向端口（Top/Bottom），与主流方向不一致，
    // 通道分配假设纯水平/垂直流，对混合端口边不适用。
    if all_body_nodes.contains(&edge.source) || all_body_nodes.contains(&edge.target) {
        return Vec::new();
    }

    // 收集中间层节点（排除 body 节点，避免对主流边产生不必要绕行）
    let mut by_rank: HashMap<i32, Vec<RectF>> = HashMap::new();
    for node in graph.nodes() {
        if node.id == edge.source || node.id == edge.target {
            continue;
        }
        if hidden_nodes.contains(&node.id) {
            continue;
        }
        // 排除 body 节点：它们位于 Loop 节点下方的独立子流区域，
        // 不在主流路径上，作为障碍物会导致不必要的绕行。
        if all_body_nodes.contains(&node.id) {
            continue;
        }
        if let Some(&rank) = ranks.get(&node.id) {
            if rank > src_rank && rank < dst_rank {
                by_rank.entry(rank).or_default().push(node.bounds());
            }
        }
    }

    // 按 rank 排序输出
    let mut sorted_ranks: Vec<i32> = by_rank.keys().copied().collect();
    sorted_ranks.sort();
    sorted_ranks
        .into_iter()
        .map(|r| by_rank.remove(&r).unwrap())
        .collect()
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

impl FlowEditorView {
    /// 当前视口缩放比例。
    pub(crate) fn scale(&self) -> f32 {
        self.viewport.scale
    }

    /// 渲染所有边（canvas paint），使用**逻辑坐标** + PathBuilder 变换。
    ///
    /// 边端点通过 [`compute_edge_endpoints`] 计算：
    /// - 循环体节点始终强制 Top/Bottom（垂直子流，上进下出），无论主布局方向
    /// - 非循环体节点按布局方向使用默认端口对
    /// - 回环边（target_port == "loop_in"）使用 `loop_back_path` 向下绕过
    ///
    /// `body_groups` 由调用方（`render`）计算一次并传入，避免与 `render_nodes`
    /// 重复执行 BFS 遍历（O(V+E)）。
    pub(crate) fn render_edges(
        &self,
        body_groups: &HashMap<NodeId, HashSet<NodeId>>,
    ) -> impl IntoElement {
        let s = self.scale();
        let (src_side_default, dst_side_default) = self.port_sides();
        let layout = self.layout_direction;
        let registry = self.registry.clone();
        let edge_default_color = self.theme.edge_default;
        let edge_loop_back_color = self.theme.edge_loop_back;
        let grid_dot_color = self.theme.grid_dot;
        let grid_spacing = self.grid_spacing;

        let all_body_nodes: HashSet<NodeId> =
            body_groups.values().flat_map(|s| s.iter().copied()).collect();

        // 收集被收起的循环体节点：当 Loop 节点的 body_collapsed == true 时，
        // 其循环体节点已隐藏，连接到这些节点的边也不渲染。
        let mut hidden_nodes: HashSet<NodeId> = HashSet::new();
        for (loop_node, body_nodes) in body_groups {
            if let Some(ln) = self.graph.node(*loop_node) {
                let body_collapsed = ln
                    .data
                    .get("body_collapsed")
                    .and_then(|v| v.as_bool())
                    .unwrap_or(false);
                if body_collapsed {
                    hidden_nodes.extend(body_nodes.iter().copied());
                }
            }
        }

        let horizontal_layout = matches!(layout, LayoutDirection::Horizontal);

        // 为每条边计算渲染指令（跳过连接到隐藏循环体节点的边）
        let edge_renders: Vec<EdgeRender> = self
            .graph
            .edges()
            .filter(|edge| {
                // 隐藏连接到已收起循环体节点的边
                !hidden_nodes.contains(&edge.source) && !hidden_nodes.contains(&edge.target)
            })
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
                        horizontal: horizontal_layout,
                        node_bounds,
                        edge_type: edge.edge_type,
                    }
                } else {
                    // 计算中间层障碍物（用于通道分配避障路由）
                    let obstacles_by_rank = compute_obstacles_by_rank(
                        edge,
                        &self.graph,
                        &self.cached_ranks,
                        &hidden_nodes,
                        &all_body_nodes,
                    );

                    EdgeRender::Normal {
                        src,
                        dst,
                        src_side,
                        dst_side,
                        edge_type: edge.edge_type,
                        obstacles_by_rank,
                        horizontal: horizontal_layout,
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
                    paint_grid(bounds, s, grid_spacing, total_offset, grid_dot_color, window);
                }
                for er in &edge_renders {
                    match er {
                        EdgeRender::Normal { src, dst, src_side, dst_side, edge_type, obstacles_by_rank, horizontal } => {
                            paint_edge_scaled(
                                *src, *dst, *src_side, *dst_side, *edge_type,
                                obstacles_by_rank, *horizontal,
                                s, total_offset,
                                edge_default_color, window,
                            );
                        }
                        EdgeRender::LoopBack { src, dst, horizontal, node_bounds, edge_type } => {
                            paint_loop_back_edge(
                                *src, *dst, *horizontal, *node_bounds, *edge_type, s, total_offset,
                                edge_loop_back_color, window,
                            );
                        }
                    }
                }
                if let Some((src, dst, src_side, dst_side, edge_type)) = drawing {
                    paint_edge_scaled(
                        src, dst, src_side, dst_side, edge_type,
                        &[], false,
                        s, total_offset,
                        edge_default_color, window,
                    );
                }
            },
        )
        .size_full()
    }

    /// 渲染所有可见边的「+」按钮（div 覆盖层）。
    ///
    /// 按钮位置 = 源端口 + 沿端口 side 轴向偏移 10px（逻辑坐标）。
    /// - 横向布局（src_side=Right）：按钮在源端口右侧 10px，Y 与端口齐平
    /// - 纵向布局（src_side=Bottom）：按钮在源端口下方 10px，X 与端口齐平
    ///
    /// 使用 `compute_edge_endpoints` 获取精确端口位置和 side，确保按钮中心
    /// 落在 smoothstep/bezier 路径的起始段上（路径从端口沿 side 方向出发）。
    /// 同一节点不同端口（如 Condition 的 if_0/if_1/else）的按钮不会重叠。
    ///
    /// 跳过回环边（target_port == "loop_in"）和连接到隐藏循环体节点的边。
    /// 按钮位置计算与 `hit_test_edge_plus` 保持完全一致。
    pub(crate) fn render_edge_plus_buttons(
        &self,
        body_groups: &HashMap<NodeId, HashSet<NodeId>>,
    ) -> impl IntoElement {
        let s = self.scale();
        let offset_x = self.viewport.offset.x;
        let offset_y = self.viewport.offset.y;
        let bg = self.theme.edge_plus_bg;
        let border = self.theme.edge_plus_border;
        let text_color = self.theme.toolbar_text;
        let layout = self.layout_direction;
        let registry = self.registry.clone();
        let (src_side_default, dst_side_default) = self.port_sides();

        // 所有循环体节点（用于 compute_edge_endpoints 的端口侧强制）
        let all_body_nodes: HashSet<NodeId> =
            body_groups.values().flat_map(|s| s.iter().copied()).collect();

        // 收集隐藏节点（收起的循环体）
        let mut hidden_nodes: HashSet<NodeId> = HashSet::new();
        for (loop_node, body_nodes) in body_groups {
            if let Some(ln) = self.graph.node(*loop_node) {
                let body_collapsed = ln
                    .data
                    .get("body_collapsed")
                    .and_then(|v| v.as_bool())
                    .unwrap_or(false);
                if body_collapsed {
                    hidden_nodes.extend(body_nodes.iter().copied());
                }
            }
        }

        let buttons: Vec<_> = self
            .graph
            .edges()
            .filter(|edge| edge.target_port.as_deref() != Some("loop_in"))
            .filter(|edge| {
                !hidden_nodes.contains(&edge.source) && !hidden_nodes.contains(&edge.target)
            })
            .filter_map(|edge| {
                // 使用端口位置计算按钮位置
                let (src, src_side, dst, dst_side) = compute_edge_endpoints(
                    edge,
                    &self.graph,
                    &registry,
                    layout,
                    &all_body_nodes,
                    src_side_default,
                    dst_side_default,
                );

                // 检查源节点是否要求按钮放在目标端（如 Loop 节点的 done 出口）
                let at_target = self
                    .graph
                    .node(edge.source)
                    .and_then(|n| registry.get(&n.kind))
                    .map(|fn_| fn_.plus_button_at_target(edge.source_port.as_deref()))
                    .unwrap_or(false);

                // 按端口 side 的轴向偏移 25px，确保按钮中心在连线路径上。
                // at_target=true → 用目标端口 + dst_side 外法线方向（朝源节点）
                // at_target=false → 用源端口 + src_side 外法线方向（朝目标节点）
                let (base, side) = if at_target {
                    (dst, dst_side)
                } else {
                    (src, src_side)
                };
                let (dx, dy) = match side {
                    PortSide::Right => (25.0, 0.0),
                    PortSide::Bottom => (0.0, 25.0),
                    PortSide::Left => (-25.0, 0.0),
                    PortSide::Top => (0.0, -25.0),
                    PortSide::Auto => (25.0, 0.0),
                };
                let button_pos = PointF::new(base.x + dx, base.y + dy);

                let screen_x = offset_x + button_pos.x * s;
                let screen_y = offset_y + button_pos.y * s;
                Some((screen_x, screen_y))
            })
            .map(|(x, y)| {
                div()
                    .absolute()
                    .left(px(x - 10.0))
                    .top(px(y - 10.0))
                    .w(px(20.0))
                    .h(px(20.0))
                    .rounded_full()
                    .bg(bg)
                    .border_1()
                    .border_dashed()
                    .border_color(border)
                    .flex()
                    .items_center()
                    .justify_center()
                    .text_color(text_color)
                    .child(Icon::new(IconName::Plus).xsmall())
            })
            .collect();

        div()
            .absolute()
            .left_0()
            .top_0()
            .size_full()
            .children(buttons)
    }

    /// 渲染悬停「+」按钮时的 tooltip。
    ///
    /// 当 `hovered_plus` 为 Some 时，找到对应边的 + 按钮屏幕坐标，
    /// 在按钮下方渲染一个带背景的小 tooltip 文本。位置计算与
    /// `render_edge_plus_buttons` / `hit_test_edge_plus` 完全一致。
    pub(crate) fn render_plus_tooltip(&self) -> Option<impl IntoElement> {
        let edge_id = self.hovered_plus?;
        let edge = self.graph.edge(edge_id)?;

        let s = self.scale();
        let offset_x = self.viewport.offset.x;
        let offset_y = self.viewport.offset.y;
        let layout = self.layout_direction;
        let registry = self.registry.clone();
        let (src_side_default, dst_side_default) = self.port_sides();

        let all_body_nodes: HashSet<NodeId> = self
            .cached_body_groups
            .values()
            .flat_map(|s| s.iter().copied())
            .collect();

        let (src, src_side, dst, dst_side) = compute_edge_endpoints(
            edge,
            &self.graph,
            &registry,
            layout,
            &all_body_nodes,
            src_side_default,
            dst_side_default,
        );

        // 检查源节点是否要求按钮放在目标端（与 render_edge_plus_buttons 一致）
        let at_target = self
            .graph
            .node(edge.source)
            .and_then(|n| registry.get(&n.kind))
            .map(|fn_| fn_.plus_button_at_target(edge.source_port.as_deref()))
            .unwrap_or(false);

        let (base, side) = if at_target {
            (dst, dst_side)
        } else {
            (src, src_side)
        };
        // + 按钮位置 = 端口 + 轴向偏移 25px（与 render_edge_plus_buttons 一致）
        let (dx, dy) = match side {
            PortSide::Right => (25.0, 0.0),
            PortSide::Bottom => (0.0, 25.0),
            PortSide::Left => (-25.0, 0.0),
            PortSide::Top => (0.0, -25.0),
            PortSide::Auto => (25.0, 0.0),
        };
        let button_pos = PointF::new(base.x + dx, base.y + dy);
        let screen_x = offset_x + button_pos.x * s;
        let screen_y = offset_y + button_pos.y * s;

        let hint = crate::i18n::t(self.language, crate::i18n::TKey::EdgePlusHint);
        let bg = self.theme.edge_plus_bg;
        let text_color = self.theme.toolbar_text;

        // tooltip 位于 + 按钮右下方，偏移 16px 避免遮挡按钮
        let tooltip_x = screen_x + 16.0;
        let tooltip_y = screen_y + 16.0;

        Some(
            div()
                .absolute()
                .left(px(tooltip_x))
                .top(px(tooltip_y))
                .px(px(8.0))
                .py(px(4.0))
                .rounded(px(4.0))
                .bg(bg)
                .border_1()
                .border_color(self.theme.edge_plus_border)
                .text_size(px(12.0))
                .text_color(text_color)
                .child(hint.to_string()),
        )
    }

    /// 渲染所有节点（absolute div 在内容层内）。
    ///
    /// 节点最终屏幕坐标 = content_offset + logical_pos × scale
    ///
    /// 为每个节点创建动作回调闭包，捕获 `node_id` 和 `entity`，
    /// 通过 `cx.update_entity` 调用 `handle_node_action`。
    ///
    /// `body_groups` 由调用方（`render`）计算一次并传入，避免与 `render_edges`
    /// 重复执行 BFS 遍历（O(V+E)）。
    pub(crate) fn render_nodes(
        &self,
        entity: Entity<Self>,
        body_groups: &HashMap<NodeId, HashSet<NodeId>>,
    ) -> Vec<gpui::AnyElement> {
        let selected = self.selected;
        let registry = &self.registry;
        let s = self.scale();
        let layout = match self.layout_direction {
            LayoutDirection::Horizontal => rust_agent_flow::LayoutDirection::Horizontal,
            LayoutDirection::Vertical => rust_agent_flow::LayoutDirection::Vertical,
        };
        let theme = self.theme;
        let hovered = self.hovered;

        let all_body_nodes: HashSet<NodeId> =
            body_groups.values().flat_map(|s| s.iter().copied()).collect();

        // 收集被收起的循环体节点：当 Loop 节点的 body_collapsed == true 时，
        // 其循环体节点不渲染（隐藏），但保留拓扑边。
        let mut hidden_nodes: HashSet<NodeId> = HashSet::new();
        for (loop_node, body_nodes) in body_groups {
            if let Some(ln) = self.graph.node(*loop_node) {
                let body_collapsed = ln
                    .data
                    .get("body_collapsed")
                    .and_then(|v| v.as_bool())
                    .unwrap_or(false);
                if body_collapsed {
                    hidden_nodes.extend(body_nodes.iter().copied());
                }
            }
        }

        self.graph
            .nodes()
            .map(|node| {
                let node_id = node.id;
                let pos = node.position;
                let flow_node = registry.get(&node.kind);
                let is_selected = selected == Some(node_id);
                let is_body = all_body_nodes.contains(&node_id);
                let is_hovered = hovered == Some(node_id);

                // 被收起的循环体节点：不渲染（返回空 div 占位，保持布局位置）
                if hidden_nodes.contains(&node_id) {
                    return div()
                        .absolute()
                        .left(px(pos.x * s))
                        .top(px(pos.y * s))
                        .into_any_element();
                }

                // 创建动作回调：闭包捕获 node_id 和 entity
                let on_action: ActionCallback = {
                    let entity = entity.clone();
                    Arc::new(move |action: NodeAction, cx: &mut App| {
                        cx.update_entity(&entity, |view: &mut FlowEditorView, cx| {
                            view.handle_node_action(node_id, action, cx);
                        });
                    })
                };

                let view = NodeView::new(node.clone())
                    .with_flow_node_opt(flow_node)
                    .selected(is_selected)
                    .with_scale(s)
                    .with_layout(layout)
                    .with_body_mode(is_body)
                    .with_theme(theme)
                    .with_hovered(is_hovered)
                    .with_on_action(Some(on_action))
                    .with_language(self.language);

                div()
                    .absolute()
                    .left(px(pos.x * s))
                    .top(px(pos.y * s))
                    .child(view)
                    .into_any_element()
            })
            .collect()
    }

    /// 确保属性面板视图与选中节点同步。
    ///
    /// 选中节点变化时创建新 PanelView，节点数据变化时同步更新。
    /// 返回 `Option<Entity<PanelView>>` 供 render 方法作为 child 添加。
    pub(crate) fn ensure_panel_view(
        &mut self,
        entity: Entity<Self>,
        window: &mut gpui::Window,
        cx: &mut gpui::Context<Self>,
    ) -> Option<Entity<PanelView>> {
        // 选中节点为 None 时，清理 panel_view
        let selected_id = match self.selected {
            Some(id) => id,
            None => {
                self.panel_view = None;
                return None;
            }
        };

        let node = self.graph.node(selected_id).cloned()?;
        // 如果节点已被删除（graph 中不存在），清理 panel_view
        if self.graph.node(selected_id).is_none() {
            self.panel_view = None;
            return None;
        }

        // 检查是否需要重建 PanelView（选中节点变化或 panel_view 为空）
        let need_rebuild = self
            .panel_view
            .as_ref()
            .map(|pv| {
                pv.read(cx).node.id != selected_id
            })
            .unwrap_or(true);

        if need_rebuild {
            let node_id = node.id;
            let flow_node = self.registry.get(&node.kind);

            // 创建动作回调：闭包捕获 node_id 和 entity
            let on_action: ActionCallback = {
                let entity = entity.clone();
                Arc::new(move |action: NodeAction, cx: &mut App| {
                    cx.update_entity(&entity, |view: &mut FlowEditorView, cx| {
                        view.handle_node_action(node_id, action, cx);
                    });
                })
            };

            self.panel_view = Some(PanelView::new(
                node,
                flow_node,
                self.theme,
                Some(on_action),
                self.syntax_service.clone(),
                self.language,
                window,
                cx,
            ));
        } else {
            // 同步节点数据到现有 PanelView
            if let Some(pv) = &self.panel_view {
                pv.update(cx, |view, cx| {
                    view.sync_from_node(node, window, cx);
                });
            }
        }

        self.panel_view.clone()
    }
}

// ---- 视图扩展（仅在渲染层使用） ----

impl NodeView {
    pub(crate) fn with_flow_node_opt(mut self, flow_node: Option<Arc<dyn IFlowNode>>) -> Self {
        self.flow_node = flow_node;
        self
    }
}
