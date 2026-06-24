//! 边渲染层：canvas paint + 「+」按钮 + tooltip。
//!
//! 易变模块：paint 回调、视觉样式、主题色均在此调整。
//! 端点计算委托给稳定的 [`super::edge_geometry`]。

use std::collections::{HashMap, HashSet};

use gpui::{canvas, div, px, IntoElement, ParentElement, Point, Styled};
use gpui_component::{Icon, IconName, Sizable};
use rust_agent_flow::{NodeId, PointF, PortSide};

use crate::edge::{paint_edge_scaled, paint_loop_back_edge};
use super::super::interaction::InteractionState;
use super::super::ports::resolve_port;
use crate::i18n;

use super::edge_geometry::{compute_edge_endpoints, compute_loop_bounds, EdgeRender};
use super::super::flow_editor::{FlowEditorView, LayoutDirection};
use super::super::grid::paint_grid;

impl FlowEditorView {
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
                                *src, *dst, *src_side, *dst_side, *edge_type,
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
    /// 按钮位置 = 源端口 + 沿端口 side 轴向偏移 25px（逻辑坐标）。
    /// - 横向布局（src_side=Right）：按钮在源端口右侧 25px，Y 与端口齐平
    /// - 纵向布局（src_side=Bottom）：按钮在源端口下方 25px，X 与端口齐平
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

        let hint = i18n::t(self.language, i18n::TKey::EdgePlusHint);
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
}
