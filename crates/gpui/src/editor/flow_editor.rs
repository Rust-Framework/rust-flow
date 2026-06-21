//! FlowEditorView：流程编辑器主视图。
//!
//! 实现 GPUI `Render`，持有图模型 + 视口 + 交互状态 + 节点注册表。
//!
//! 交互采用命中测试方案：画布统一处理鼠标事件，用几何命中测试确定点击的
//! 节点/端口，避免在每个节点 div 上绑定闭包（GPUI 的 listener 闭包无法
//! 捕获外部变量如 node_id）。

use std::sync::Arc;

use gpui::{
    canvas, div, px, Context, InteractiveElement, IntoElement, MouseButton, MouseDownEvent,
    MouseMoveEvent, MouseUpEvent, ParentElement, Pixels, Point, Render, ScrollWheelEvent, Styled,
    Window,
};
use rust_agent_flow::{
    point_in_rect, Edge, EdgeType, FlowGraph, NodeId, PointF, PortId, PortSide, Viewport,
};

use crate::node::{IFlowNode, NodeRegistry, NodeView};
use crate::panel::PanelView;

use super::interaction::InteractionState;
use super::viewport;

/// 端口命中区域宽度（节点边缘向外延伸的像素数）。
const PORT_HIT_WIDTH: f32 = 12.0;

/// 流程编辑器主视图。
pub struct FlowEditorView {
    pub graph: FlowGraph,
    pub viewport: Viewport,
    pub interaction: InteractionState,
    pub registry: Arc<NodeRegistry>,
    pub selected: Option<NodeId>,
    /// 默认边类型（用于 DrawingEdge 临时连线）。
    pub default_edge_type: EdgeType,
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
            default_edge_type: EdgeType::Bezier,
        }
    }

    /// 屏幕坐标（GPUI Point<Pixels>）→ 逻辑坐标（PointF）。
    fn to_logical(&self, p: Point<Pixels>) -> PointF {
        self.viewport.to_logical(PointF::new(p.x.as_f32(), p.y.as_f32()))
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

    /// 渲染所有边（canvas paint）。
    fn render_edges(&self) -> impl IntoElement {
        let edges: Vec<(PointF, PointF, PortSide, PortSide, EdgeType)> = self
            .graph
            .edges()
            .map(|edge| {
                let src = self.graph.node(edge.source).map(|n| n.center()).unwrap_or_default();
                let dst = self.graph.node(edge.target).map(|n| n.center()).unwrap_or_default();
                let src_screen = self.viewport.to_screen(src);
                let dst_screen = self.viewport.to_screen(dst);
                (src_screen, dst_screen, PortSide::Right, PortSide::Left, edge.edge_type)
            })
            .collect();

        // 绘制中的连线（使用默认边类型）
        let default_edge_type = self.default_edge_type;
        let drawing = match &self.interaction {
            InteractionState::DrawingEdge { from_node, current, .. } => {
                self.graph.node(*from_node).map(|n| {
                    let src = self.viewport.to_screen(n.center());
                    let dst = self.viewport.to_screen(*current);
                    (src, dst, PortSide::Right, PortSide::Left, default_edge_type)
                })
            }
            _ => None,
        };

        canvas(
            |bounds, _window, _cx| bounds.size,
            move |_bounds, _size, window, _cx| {
                for (src, dst, src_side, dst_side, edge_type) in &edges {
                    crate::edge::paint_edge(*src, *dst, *src_side, *dst_side, *edge_type, window);
                }
                if let Some((src, dst, src_side, dst_side, edge_type)) = drawing {
                    crate::edge::paint_edge(src, dst, src_side, dst_side, edge_type, window);
                }
            },
        )
    }

    /// 渲染所有节点（absolute div）。
    fn render_nodes(&self) -> Vec<gpui::AnyElement> {
        let selected = self.selected;
        let registry = &self.registry;

        self.graph
            .nodes()
            .map(|node| {
                let node_id = node.id;
                let screen = self.viewport.to_screen(node.position);
                let flow_node = registry.get(&node.kind);
                let is_selected = selected == Some(node_id);

                let view = NodeView::new(node.clone())
                    .with_flow_node_opt(flow_node)
                    .selected(is_selected);

                div()
                    .absolute()
                    .left(px(screen.x))
                    .top(px(screen.y))
                    .child(view)
                    .into_any_element()
            })
            .collect()
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

        let mut container = div()
            .size_full()
            .relative()
            .bg(gpui::rgb(0xffffff))
            .on_mouse_down(MouseButton::Left, cx.listener(Self::on_mouse_down))
            .on_mouse_down(MouseButton::Middle, cx.listener(Self::on_mouse_down))
            .on_mouse_move(cx.listener(Self::on_mouse_move))
            .on_mouse_up(MouseButton::Left, cx.listener(Self::on_mouse_up))
            .on_mouse_up(MouseButton::Middle, cx.listener(Self::on_mouse_up))
            .on_scroll_wheel(cx.listener(Self::on_scroll))
            .child(edges);

        for node_el in nodes {
            container = container.child(node_el);
        }

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
