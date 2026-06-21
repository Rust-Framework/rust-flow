//! FlowEditorView：流程编辑器主视图。
//!
//! 实现 GPUI `Render`，持有图模型 + 视口 + 交互状态 + 节点注册表。
//!
//! 交互采用命中测试方案：画布统一处理鼠标事件，用几何命中测试确定点击的
//! 节点/端口，避免在每个节点 div 上绑定闭包（GPUI 的 listener 闭包无法
//! 捕获外部变量如 node_id）。
//!
//! 缩放采用 **transform-scale 方案**：所有画布内容（边+节点）包裹在统一
//! 的缩放容器中，确保缩放对称地应用到每个像素（节点大小、端口、线宽等）。

use std::sync::Arc;

use gpui::{
    canvas, div, px, Context, InteractiveElement, IntoElement, MouseButton, MouseDownEvent,
    MouseMoveEvent, MouseUpEvent, ParentElement, Pixels, Point, Render, ScrollWheelEvent, Styled,
    Window, StatefulInteractiveElement,
};
use gpui_component::StyledExt;
use rust_agent_flow::{
    point_in_rect, Edge, EdgeType, FlowGraph, Node, NodeId, PointF, PortId, PortSide, SizeF,
    Viewport,
};

use crate::edge::paint_edge_scaled;
use crate::node::{IFlowNode, NodeRegistry, NodeView};
use crate::panel::PanelView;

use super::interaction::InteractionState;
use super::viewport;

/// 端口命中区域宽度（逻辑坐标，会随缩放自动缩放）。
const PORT_HIT_WIDTH: f32 = 12.0;

/// 布局方向。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LayoutDirection {
    Horizontal,
    Vertical,
}

/// 流程编辑器主视图。
pub struct FlowEditorView {
    pub graph: FlowGraph,
    pub viewport: Viewport,
    pub interaction: InteractionState,
    pub registry: Arc<NodeRegistry>,
    pub selected: Option<NodeId>,
    /// 默认边类型（用于 DrawingEdge 临时连线）。
    pub default_edge_type: EdgeType,
    /// 布局方向。
    pub layout_direction: LayoutDirection,
}

impl FlowEditorView {
    pub fn new(graph: FlowGraph, _cx: &mut Context<Self>) -> Self {
        let mut registry = NodeRegistry::new();
        crate::builtin::register_all(&mut registry);
        Self {
            graph,
            viewport: Viewport::default(),
            interaction: InteractionState::default(),
            registry: Arc::new(registry),
            selected: None,
            default_edge_type: EdgeType::SmoothStep,
            layout_direction: LayoutDirection::Horizontal,
        }
    }

    /// 屏幕坐标（GPUI Point<Pixels>）→ 逻辑坐标（PointF）。
    fn to_logical(&self, p: Point<Pixels>) -> PointF {
        self.viewport.to_logical(PointF::new(p.x.as_f32(), p.y.as_f32()))
    }

    // ---- 工具栏操作 ----

    /// 放大。
    fn zoom_in(&mut self, cx: &mut Context<Self>) {
        let center = PointF::new(
            self.viewport.offset.x
                + (self.viewport.scale * self.viewport.offset.x
                    / self.viewport.scale),
            self.viewport.offset.y
                + (self.viewport.scale * self.viewport.offset.y
                    / self.viewport.scale),
        );
        self.viewport = viewport::handle_zoom(self.viewport, center, -60.0);
        cx.notify();
    }

    /// 缩小。
    fn zoom_out(&mut self, cx: &mut Context<Self>) {
        let center = PointF::new(
            self.viewport.offset.x
                + (self.viewport.scale * self.viewport.offset.x
                    / self.viewport.scale),
            self.viewport.offset.y
                + (self.viewport.scale * self.viewport.offset.y
                    / self.viewport.scale),
        );
        self.viewport = viewport::handle_zoom(self.viewport, center, 60.0);
        cx.notify();
    }

    /// 重置视口（归位 + 100%缩放）。
    fn reset_view(&mut self, cx: &mut Context<Self>) {
        self.viewport = Viewport::default();
        cx.notify();
    }

    /// 适应视图（将所有节点居中显示）。
    fn fit_view(&mut self, cx: &mut Context<Self>) {
        // 计算所有节点的包围盒。
        let mut bounds = Option::<rust_agent_flow::RectF>::None;
        for node in self.graph.nodes() {
            let nb = node.bounds();
            bounds = Some(match bounds {
                Some(b) => rust_agent_flow::RectF::new(
                    PointF::new(b.origin.x.min(nb.origin.x), b.origin.y.min(nb.origin.y)),
                    SizeF::new(
                        b.right().max(nb.right()) - b.left().min(nb.left()),
                        b.bottom().max(nb.bottom()) - b.top().min(nb.top()),
                    ),
                ),
                None => nb,
            });
        }
        if let Some(b) = bounds {
            // 留出 margin，居中到视口原点附近。
            let margin = 60.0;
            self.viewport.offset = PointF::new(margin - b.origin.x, margin - b.origin.y);
            self.viewport.scale = 1.0;
        }
        cx.notify();
    }

    /// 切换布局方向。
    fn toggle_direction(&mut self, cx: &mut Context<Self>) {
        self.layout_direction = match self.layout_direction {
            LayoutDirection::Horizontal => LayoutDirection::Vertical,
            LayoutDirection::Vertical => LayoutDirection::Horizontal,
        };
        cx.notify();
    }

    /// 设置默认边类型。
    fn set_edge_type(&mut self, edge_type: EdgeType, cx: &mut Context<Self>) {
        self.default_edge_type = edge_type;
        cx.notify();
    }

    // ---- 命中测试 ----

    /// 命中测试：返回点击位置的节点和端口（如果有）。
    ///
    /// 端口命中区域：节点左右两侧 PORT_HIT_WIDTH 像素。
    /// - 右侧命中 → 出端口（Out）
    /// - 左侧命中 → 入端口（In）
    /// - 中间命中 → 节点主体（无端口）
    fn hit_test(&self, logical: PointF) -> HitResult {
        for node in self.graph.nodes() {
            let bounds = node.bounds();
            if !point_in_rect(logical, bounds) {
                // 检查端口扩展区域
                let right_port = rust_agent_flow::RectF::new(
                    PointF::new(bounds.right() - PORT_HIT_WIDTH, bounds.top()),
                    rust_agent_flow::SizeF::new(PORT_HIT_WIDTH * 2.0, bounds.size.h),
                );
                if point_in_rect(logical, right_port) {
                    return HitResult::OutPort(node.id, "out".to_string());
                }
                let left_port = rust_agent_flow::RectF::new(
                    PointF::new(bounds.left() - PORT_HIT_WIDTH, bounds.top()),
                    rust_agent_flow::SizeF::new(PORT_HIT_WIDTH * 2.0, bounds.size.h),
                );
                if point_in_rect(logical, left_port) {
                    return HitResult::InPort(node.id, "in".to_string());
                }
                continue;
            }
            // 节点内部：检查是否在右/左边缘
            let right_edge = logical.x >= bounds.right() - PORT_HIT_WIDTH;
            let left_edge = logical.x <= bounds.left() + PORT_HIT_WIDTH;
            if right_edge {
                return HitResult::OutPort(node.id, "out".to_string());
            }
            if left_edge {
                return HitResult::InPort(node.id, "in".to_string());
            }
            return HitResult::Node(node.id);
        }
        HitResult::Empty
    }

    // ---- 事件处理 ----

    fn on_mouse_down(&mut self, event: &MouseDownEvent, _window: &mut Window, cx: &mut Context<Self>) {
        let logical = self.to_logical(event.position);

        match (event.button, self.hit_test(logical)) {
            (MouseButton::Middle, _) => {
                self.interaction = InteractionState::Panning {
                    start: logical,
                    origin: self.viewport.offset,
                };
            }
            (MouseButton::Left, HitResult::OutPort(node_id, port)) => {
                self.interaction = InteractionState::DrawingEdge {
                    from_node: node_id,
                    from_port: port,
                    current: logical,
                };
            }
            (MouseButton::Left, HitResult::Node(node_id)) => {
                let node_origin = self.graph.node(node_id).map(|n| n.position).unwrap_or_default();
                self.selected = Some(node_id);
                self.interaction = InteractionState::DraggingNode {
                    node_id,
                    start: logical,
                    node_origin,
                };
            }
            (MouseButton::Left, HitResult::InPort(_, _)) => {
                // 点击入端口：暂不处理（可作为连线目标）
            }
            (MouseButton::Left, HitResult::Empty) => {
                self.selected = None;
            }
            _ => {}
        }
        cx.notify();
    }

    fn on_mouse_move(&mut self, event: &MouseMoveEvent, _window: &mut Window, cx: &mut Context<Self>) {
        let current = self.to_logical(event.position);
        match &mut self.interaction {
            InteractionState::Panning { start, origin } => {
                self.viewport.offset = viewport::handle_pan(*origin, *start, current);
                cx.notify();
            }
            InteractionState::DraggingNode {
                node_id,
                start,
                node_origin,
            } => {
                let node_id = *node_id;
                let start = *start;
                let node_origin = *node_origin;
                if let Some(node) = self.graph.node_mut(node_id) {
                    node.position = PointF::new(
                        node_origin.x + (current.x - start.x),
                        node_origin.y + (current.y - start.y),
                    );
                }
                cx.notify();
            }
            InteractionState::DrawingEdge { current: cur, .. } => {
                *cur = current;
                cx.notify();
            }
            InteractionState::Idle => {}
        }
    }

    fn on_mouse_up(&mut self, event: &MouseUpEvent, _window: &mut Window, cx: &mut Context<Self>) {
        let logical = self.to_logical(event.position);

        match &self.interaction {
            InteractionState::DrawingEdge { from_node, from_port, .. } => {
                let from_node = *from_node;
                let from_port = from_port.clone();
                // 命中测试目标入端口
                if let HitResult::InPort(to_node, to_port) = self.hit_test(logical) {
                    if from_node != to_node {
                        let mut edge = Edge::new(from_node, to_node);
                        edge.source_port = Some(from_port);
                        edge.target_port = Some(to_port);
                        self.graph.add_edge(edge);
                    }
                }
                self.interaction = InteractionState::Idle;
            }
            InteractionState::DraggingNode { .. } | InteractionState::Panning { .. } => {
                self.interaction = InteractionState::Idle;
            }
            InteractionState::Idle => {}
        }
        cx.notify();
    }

    fn on_scroll(&mut self, event: &ScrollWheelEvent, _window: &mut Window, cx: &mut Context<Self>) {
        let mouse_logical = self.to_logical(event.position);
        // GPUI 滚轮 delta：向上为负（放大），向下为正（缩小）
        let delta = event.delta.pixel_delta(px(20.0)).y.as_f32();
        self.viewport = viewport::handle_zoom(self.viewport, mouse_logical, delta);
        cx.notify();
    }

    // ---- 渲染 ----

    /// 渲染所有边（canvas paint），使用逻辑坐标。
    ///
    /// 边在 **内容层** 内渲染，随 transform-scale 统一缩放，
    /// 因此线宽、箭头大小都会随缩放比例变化。
    fn render_edges(&self) -> impl IntoElement {
        let edges: Vec<(PointF, PointF, PortSide, PortSide, EdgeType)> = self
            .graph
            .edges()
            .map(|edge| {
                let src = port_position_from_side(&edge, &self.graph, PortSide::Right);
                let dst = port_position_from_side(&edge, &self.graph, PortSide::Left);
                (src, dst, PortSide::Right, PortSide::Left, edge.edge_type)
            })
            .collect();

        // 绘制中的连线（使用默认边类型）
        let default_edge_type = self.default_edge_type;
        let drawing = match &self.interaction {
            InteractionState::DrawingEdge { from_node, current, .. } => {
                self.graph.node(*from_node).map(|n| {
                    let src = port_position(n, PortSide::Right);
                    let dst = *current;
                    (src, dst, PortSide::Right, PortSide::Left, default_edge_type)
                })
            }
            _ => None,
        };

        canvas(
            |bounds, _window, _cx| bounds.size,
            move |_bounds, _size, window, _cx| {
                for (src, dst, src_side, dst_side, edge_type) in &edges {
                    paint_edge_scaled(*src, *dst, *src_side, *dst_side, *edge_type, window);
                }
                if let Some((src, dst, src_side, dst_side, edge_type)) = drawing {
                    paint_edge_scaled(src, dst, src_side, dst_side, edge_type, window);
                }
            },
        )
    }

    /// 渲染所有节点（absolute div），使用逻辑坐标。
    ///
    /// 节点在 **内容层** 内定位，位置为逻辑坐标，
    /// 随外层 transform-scale 统一缩放。
    fn render_nodes(&self) -> Vec<gpui::AnyElement> {
        let selected = self.selected;
        let registry = &self.registry;

        self.graph
            .nodes()
            .map(|node| {
                let node_id = node.id;
                let pos = node.position;
                let flow_node = registry.get(&node.kind);
                let is_selected = selected == Some(node_id);

                let view = NodeView::new(node.clone())
                    .with_flow_node_opt(flow_node)
                    .selected(is_selected);

                div()
                    .absolute()
                    .left(px(pos.x))
                    .top(px(pos.y))
                    .child(view)
                    .into_any_element()
            })
            .collect()
    }

    /// 渲染工具栏（ReactFlow 风格的浮动控制面板）。
    ///
    /// 工具栏位于右下角，不受缩放影响（在内容层之外）。
    fn render_toolbar(&self, cx: &Context<Self>) -> impl IntoElement {
        let scale_pct = (self.viewport.scale * 100.0) as i32;
        let _direction_label = match self.layout_direction {
            LayoutDirection::Horizontal => "H",
            LayoutDirection::Vertical => "V",
        };

        let edge_type = self.default_edge_type;

        div()
            .absolute()
            .right_2()
            .bottom_2()
            .flex()
            .flex_col()
            .gap_0p5()
            .rounded_lg()
            .bg(gpui::rgba(0xffffffee)) // 白色 0.93 不透明度
            .border_1()
            .border_color(gpui::rgb(0xe2e8f0))
            .shadow_lg()
            .p_1()
            // 缩放百分比显示
            .child(
                div()
                    .w(px(36.0))
                    .text_xs()
                    .text_color(gpui::rgb(0x64748b))
                    .text_center()
                    .font_medium()
                    .child(format!("{}%", scale_pct)),
            )
            // 第一行：放大 / 缩小 / 适应 / 重置
            .child(
                div()
                    .flex()
                    .gap_0p5()
                    // 放大
                    .child(
                        div()
                            .id("tb-zoom-in")
                            .flex()
                            .items_center()
                            .justify_center()
                            .w(px(32.0))
                            .h(px(28.0))
                            .rounded_md()
                            .hover(|s| s.bg(gpui::rgb(0xf1f5f9)))
                            .active(|s| s.bg(gpui::rgb(0xe2e8f0)))
                            .text_sm()
                            .font_medium()
                            .text_color(gpui::rgb(0x475569))
                            .on_mouse_down(MouseButton::Left, cx.listener(move |this, _, _, cx| {
                                this.zoom_in(cx);
                            }))
                            .child("+"),
                    )
                    // 缩小
                    .child(
                        div()
                            .id("tb-zoom-out")
                            .flex()
                            .items_center()
                            .justify_center()
                            .w(px(32.0))
                            .h(px(28.0))
                            .rounded_md()
                            .hover(|s| s.bg(gpui::rgb(0xf1f5f9)))
                            .active(|s| s.bg(gpui::rgb(0xe2e8f0)))
                            .text_sm()
                            .font_medium()
                            .text_color(gpui::rgb(0x475569))
                            .on_mouse_down(MouseButton::Left, cx.listener(move |this, _, _, cx| {
                                this.zoom_out(cx);
                            }))
                            .child("\u{2212}"), // minus sign
                    )
                    // 适应视图
                    .child(
                        div()
                            .id("tb-fit")
                            .flex()
                            .items_center()
                            .justify_center()
                            .w(px(32.0))
                            .h(px(28.0))
                            .rounded_md()
                            .hover(|s| s.bg(gpui::rgb(0xf1f5f9)))
                            .active(|s| s.bg(gpui::rgb(0xe2e8f0)))
                            .text_sm()
                            .font_medium()
                            .text_color(gpui::rgb(0x475569))
                            .on_mouse_down(MouseButton::Left, cx.listener(move |this, _, _, cx| {
                                this.fit_view(cx);
                            }))
                            .child("\u{25A1}"), // square (fit)
                    )
                    // 重置
                    .child(
                        div()
                            .id("tb-reset")
                            .flex()
                            .items_center()
                            .justify_center()
                            .w(px(32.0))
                            .h(px(28.0))
                            .rounded_md()
                            .hover(|s| s.bg(gpui::rgb(0xf1f5f9)))
                            .active(|s| s.bg(gpui::rgb(0xe2e8f0)))
                            .text_sm()
                            .font_medium()
                            .text_color(gpui::rgb(0x475569))
                            .on_mouse_down(MouseButton::Left, cx.listener(move |this, _, _, cx| {
                                this.reset_view(cx);
                            }))
                            .child("\u{27F3}"), // reset/reload
                    ),
            )
            // 分隔线
            .child(
                div()
                    .h(px(1.0))
                    .w(px(140.0))
                    .bg(gpui::rgb(0xe2e8f0)),
            )
            // 第二行：布局方向切换
            .child(
                div()
                    .flex()
                    .gap_0p5()
                    // 水平布局
                    .child(
                        div()
                            .id("tb-dir-h")
                            .flex()
                            .items_center()
                            .justify_center()
                            .w(px(32.0))
                            .h(px(28.0))
                            .rounded_md()
                            .bg(if self.layout_direction == LayoutDirection::Horizontal {
                                gpui::rgb(0x6366f1)
                            } else {
                                gpui::Rgba { r: 1.0, g: 1.0, b: 1.0, a: 0.0 }
                            })
                            .hover(|s| {
                                s.bg(if self.layout_direction == LayoutDirection::Horizontal {
                                    gpui::rgb(0x6366f1)
                                } else {
                                    gpui::rgb(0xf1f5f9)
                                })
                            })
                            .text_xs()
                            .font_medium()
                            .text_color(if self.layout_direction == LayoutDirection::Horizontal {
                                gpui::rgb(0xffffff)
                            } else {
                                gpui::rgb(0x475569)
                            })
                            .on_mouse_down(MouseButton::Left, cx.listener(move |this, _, _, cx| {
                                this.layout_direction = LayoutDirection::Horizontal;
                                cx.notify();
                            }))
                            .child("\u{2194}"), // ↔
                    )
                    // 垂直布局
                    .child(
                        div()
                            .id("tb-dir-v")
                            .flex()
                            .items_center()
                            .justify_center()
                            .w(px(32.0))
                            .h(px(28.0))
                            .rounded_md()
                            .bg(if self.layout_direction == LayoutDirection::Vertical {
                                gpui::rgb(0x6366f1)
                            } else {
                                gpui::Rgba { r: 1.0, g: 1.0, b: 1.0, a: 0.0 }
                            })
                            .hover(|s| {
                                s.bg(if self.layout_direction == LayoutDirection::Vertical {
                                    gpui::rgb(0x6366f1)
                                } else {
                                    gpui::rgb(0xf1f5f9)
                                })
                            })
                            .text_xs()
                            .font_medium()
                            .text_color(if self.layout_direction == LayoutDirection::Vertical {
                                gpui::rgb(0xffffff)
                            } else {
                                gpui::rgb(0x475569)
                            })
                            .on_mouse_down(MouseButton::Left, cx.listener(move |this, _, _, cx| {
                                this.layout_direction = LayoutDirection::Vertical;
                                cx.notify();
                            }))
                            .child("\u{2195}"), // ↕
                    ),
            )
            // 分隔线
            .child(
                div()
                    .h(px(1.0))
                    .w(px(140.0))
                    .bg(gpui::rgb(0xe2e8f0)),
            )
            // 第三行：边类型选择
            .child(
                div()
                    .flex()
                    .flex_col()
                    .gap_0p5()
                    .p_0p5()
                    // Bezier
                    .child(edge_type_button("Bezier", EdgeType::Bezier, edge_type, cx))
                    // Straight
                    .child(edge_type_button("Straight", EdgeType::Straight, edge_type, cx))
                    // Step (角)
                    .child(edge_type_button("Step", EdgeType::Step, edge_type, cx))
                    // SmoothStep (圆角)
                    .child(edge_type_button("Smooth", EdgeType::SmoothStep, edge_type, cx)),
            )
    }

    /// 渲染属性面板。
    fn render_panel(&self) -> Option<gpui::AnyElement> {
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

/// 构建一个边类型选择按钮。
fn edge_type_button(
    label: &str,
    etype: EdgeType,
    current: EdgeType,
    cx: &Context<FlowEditorView>,
) -> impl IntoElement {
    let is_active = current == etype;
    let etype = etype;
    let label_owned = label.to_string();
    div()
        .id(format!("tb-edge-{label:?}"))
        .flex()
        .items_center()
        .justify_center()
        .w(px(132.0))
        .h(px(24.0))
        .rounded_md()
        .px_1()
        .bg(if is_active {
            gpui::rgb(0xede9fe) // 浅靛蓝背景
        } else {
            gpui::Rgba { r: 1.0, g: 1.0, b: 1.0, a: 0.0 }
        })
        .hover(|s| {
            s.bg(if is_active {
                gpui::rgb(0xede9fe)
            } else {
                gpui::rgb(0xf8fafc)
            })
        })
        .text_xs()
        .font_medium()
        .text_color(if is_active {
            gpui::rgb(0x6366f1)
        } else {
            gpui::rgb(0x64748b)
        })
        .on_mouse_down(MouseButton::Left, cx.listener(move |this, _, _, cx| {
            this.set_edge_type(etype, cx);
        }))
        .child(label_owned)
}

/// 命中测试结果。
enum HitResult {
    /// 空白区域。
    Empty,
    /// 节点主体（非端口区域）。
    Node(NodeId),
    /// 出端口（节点右侧）。
    OutPort(NodeId, PortId),
    /// 入端口（节点左侧）。
    InPort(NodeId, PortId),
}

impl Render for FlowEditorView {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let edges = self.render_edges();
        let nodes = self.render_nodes();
        let panel = self.render_panel();
        let toolbar = self.render_toolbar(cx);

        // 内容层的变换参数。
        let offset = self.viewport.offset;

        // ====== 外层容器：全屏，处理事件 ======
        let mut container = div()
            .size_full()
            .relative()
            .bg(gpui::rgb(0xf8fafc))
            .overflow_hidden()
            .on_mouse_down(MouseButton::Left, cx.listener(Self::on_mouse_down))
            .on_mouse_down(MouseButton::Middle, cx.listener(Self::on_mouse_down))
            .on_mouse_move(cx.listener(Self::on_mouse_move))
            .on_mouse_up(MouseButton::Left, cx.listener(Self::on_mouse_up))
            .on_mouse_up(MouseButton::Middle, cx.listener(Self::on_mouse_up))
            .on_scroll_wheel(cx.listener(Self::on_scroll));

        // ====== 内容层：包含边和节点，统一缩放 ======
        // 使用绝对定位 + 偏移实现平移。
        // 所有子元素（边 canvas + 节点 div）都使用逻辑坐标，
        // 缩放通过外层 transform 统一应用（如果平台支持）；
        // 否则各元素在渲染时自行应用 scale。
        let mut content = div()
            .absolute()
            .left(px(offset.x))
            .top(px(offset.y))
            .size_full()
            .child(edges);

        for node_el in nodes {
            content = content.child(node_el);
        }

        container = container.child(content);

        // ====== 工具栏：不受缩放影响 ======
        container = container.child(toolbar);

        // ====== 属性面板：不受缩放影响 ======
        if let Some(panel_el) = panel {
            container = container.child(panel_el);
        }

        container
    }
}

// ---- 辅助 ----

impl NodeView {
    fn with_flow_node_opt(mut self, flow_node: Option<Arc<dyn IFlowNode>>) -> Self {
        self.flow_node = flow_node;
        self
    }
}

impl PanelView {
    fn with_flow_node_opt(mut self, flow_node: Option<Arc<dyn IFlowNode>>) -> Self {
        self.flow_node = flow_node;
        self
    }
}

/// 计算节点某侧端口的实际位置（节点边缘中点）。
///
/// - Right → 右边缘中点
/// - Left  → 左边缘中点
/// - Top   → 上边缘中点
/// - Bottom → 下边缘中点
fn port_position(node: &Node, side: PortSide) -> PointF {
    let right = node.position.x + node.size.w;
    let left = node.position.x;
    let top = node.position.y;
    let bottom = node.position.y + node.size.h;
    let mid_x = node.position.x + node.size.w * 0.5;
    let mid_y = node.position.y + node.size.h * 0.5;
    match side {
        PortSide::Right => PointF::new(right, mid_y),
        PortSide::Left => PointF::new(left, mid_y),
        PortSide::Top => PointF::new(mid_x, top),
        PortSide::Bottom => PointF::new(mid_y, bottom),
        PortSide::Auto => PointF::new(right, mid_y),
    }
}

/// 根据边的源/目标节点获取对应侧的端口位置。
fn port_position_from_side(edge: &Edge, graph: &FlowGraph, side: PortSide) -> PointF {
    let node_id = if side == PortSide::Right {
        edge.source
    } else {
        edge.target
    };
    graph
        .node(node_id)
        .map(|n| port_position(n, side))
        .unwrap_or_default()
}
