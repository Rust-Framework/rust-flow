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

/// 计算边的端点。
///
/// **端口策略**（与布局方向协同，减少拐弯）：
/// - **循环体节点**：始终强制 Top/Bottom（垂直子流，上进下出），无论主布局方向如何。
///   这样 Loop 的 `loop_body` 出口（Right）→ body 入口（Top），
///   body 出口（Bottom）→ Loop 的 `loop_in` 入口（Left），回环边向下绕回。
/// - **非循环体节点**：按布局方向使用默认端口对（纵向 Top/Bottom，横向 Left/Right）。
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
                        horizontal: matches!(layout, LayoutDirection::Horizontal),
                        node_bounds,
                        edge_type: edge.edge_type,
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
                    paint_grid(bounds, s, grid_spacing, total_offset, grid_dot_color, window);
                }
                for er in &edge_renders {
                    match er {
                        EdgeRender::Normal { src, dst, src_side, dst_side, edge_type } => {
                            paint_edge_scaled(
                                *src, *dst, *src_side, *dst_side, *edge_type, s, total_offset,
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
                        src, dst, src_side, dst_side, edge_type, s, total_offset,
                        edge_default_color, window,
                    );
                }
            },
        )
        .size_full()
    }

    /// 渲染所有可见边中点的「+」按钮（div 覆盖层）。
    ///
    /// 按钮位置 = viewport.offset + edge_midpoint × scale
    /// 跳过回环边（target_port == "loop_in"）和连接到隐藏循环体节点的边。
    /// 中点计算与 `hit_test_edge_plus` 保持一致（节点中心中点）。
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
                let src = self.graph.node(edge.source)?;
                let dst = self.graph.node(edge.target)?;
                let src_center = PointF::new(
                    src.position.x + src.size.w * 0.5,
                    src.position.y + src.size.h * 0.5,
                );
                let dst_center = PointF::new(
                    dst.position.x + dst.size.w * 0.5,
                    dst.position.y + dst.size.h * 0.5,
                );
                let mid = PointF::new(
                    (src_center.x + dst_center.x) * 0.5,
                    (src_center.y + dst_center.y) * 0.5,
                );
                let screen_x = offset_x + mid.x * s;
                let screen_y = offset_y + mid.y * s;
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
