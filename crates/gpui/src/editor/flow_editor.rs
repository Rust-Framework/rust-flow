//! FlowEditorView：流程编辑器主视图。
//!
//! 实现 GPUI `Render`，持有图模型 + 视口 + 交互状态 + 节点注册表。
//!
//! 交互采用命中测试方案：画布统一处理鼠标事件，用几何命中测试确定点击的
//! 节点/端口，避免在每个节点 div 上绑定闭包（GPUI 的 listener 闭包无法
//! 捕获外部变量如 node_id）。
//!
//! 缩放方案：
//! - **节点**：逐元素手动缩放（`pos * scale`、`size * scale`），因 GPUI
//!   的 div 不支持 CSS transform-scale。
//! - **边**：在逻辑坐标中计算路径几何（含 step gap、smoothstep 圆角），
//!   通过 `PathBuilder::scale` + `translate` 统一变换到屏幕空间。线宽
//!   手动乘以 `scale`。这样所有几何参数随缩放等比变化，避免错位。

use std::sync::Arc;

use gpui::{
    canvas, div, px, Context, CursorStyle, InteractiveElement, IntoElement, MouseButton,
    MouseDownEvent, MouseMoveEvent, MouseUpEvent, ParentElement, PathBuilder, Pixels, Point,
    Render, ScrollWheelEvent, Styled, Window, StatefulInteractiveElement,
};
use gpui_component::StyledExt;
use rust_agent_flow::{
    point_in_rect, Edge, EdgeType, FlowGraph, Node, NodeId, PointF, PortId, PortSide, SizeF,
    Viewport,
};
use rust_agent_flow::{
    LayoutDirection as CoreLayoutDirection, LayoutEngine, LayoutResult, SimpleLayout,
};

use crate::edge::paint_edge_scaled;
use crate::node::{IFlowNode, NodeRegistry, NodeView};
use crate::panel::PanelView;

use super::interaction::InteractionState;
use super::viewport;

/// 端口命中区域宽度（逻辑坐标，会随缩放自动缩放）。
const PORT_HIT_WIDTH: f32 = 12.0;

/// 点阵背景间距（逻辑坐标）。
const GRID_SPACING: f32 = 40.0;

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
    /// 默认边类型（用于 DrawingEdge 临时连线 + 全局切换）。
    pub default_edge_type: EdgeType,
    /// 布局方向（决定边的端口侧：Horizontal=Right/Left, Vertical=Bottom/Top）。
    pub layout_direction: LayoutDirection,
    /// 是否显示点阵背景。
    pub show_grid: bool,
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
            show_grid: true,
        }
    }

    /// 屏幕坐标（GPUI Point<Pixels>）→ 逻辑坐标（PointF）。
    fn to_logical(&self, p: Point<Pixels>) -> PointF {
        self.viewport.to_logical(PointF::new(p.x.as_f32(), p.y.as_f32()))
    }

    /// 根据布局方向返回 (源端口侧, 目标端口侧)。
    fn port_sides(&self) -> (PortSide, PortSide) {
        match self.layout_direction {
            LayoutDirection::Horizontal => (PortSide::Right, PortSide::Left),
            LayoutDirection::Vertical => (PortSide::Bottom, PortSide::Top),
        }
    }

    /// 运行布局引擎，按当前布局方向重新排列所有节点位置。
    ///
    /// 使用内置 [`SimpleLayout`]（无外部依赖），保持节点拓扑分层结构。
    /// 切换方向时调用此方法即可重新排版。
    fn relayout(&mut self) {
        let dir = match self.layout_direction {
            LayoutDirection::Horizontal => CoreLayoutDirection::Horizontal,
            LayoutDirection::Vertical => CoreLayoutDirection::Vertical,
        };
        let result: LayoutResult = SimpleLayout::new().layout(&self.graph, dir);
        for (node_id, pos) in result.positions {
            if let Some(node) = self.graph.node_mut(node_id) {
                node.position = pos;
            }
        }
    }

    /// 切换布局方向并重新排版节点位置。
    fn set_layout_direction(&mut self, dir: LayoutDirection, cx: &mut Context<Self>) {
        if self.layout_direction == dir {
            return;
        }
        self.layout_direction = dir;
        self.relayout();
        cx.notify();
    }

    // ---- 工具栏操作 ----

    /// 放大（以视口可见区域中心为锚点）。
    fn zoom_in(&mut self, window: &Window, cx: &mut Context<Self>) {
        let size = window.viewport_size();
        let center_screen = PointF::new(size.width.as_f32() * 0.5, size.height.as_f32() * 0.5);
        let center_logical = self.viewport.to_logical(center_screen);
        self.viewport = viewport::handle_zoom(self.viewport, center_logical, -60.0);
        cx.notify();
    }

    /// 缩小（以视口可见区域中心为锚点）。
    fn zoom_out(&mut self, window: &Window, cx: &mut Context<Self>) {
        let size = window.viewport_size();
        let center_screen = PointF::new(size.width.as_f32() * 0.5, size.height.as_f32() * 0.5);
        let center_logical = self.viewport.to_logical(center_screen);
        self.viewport = viewport::handle_zoom(self.viewport, center_logical, 60.0);
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

    // ---- 命中测试 ----

    /// 命中测试：返回点击位置的节点和端口（如果有）。
    ///
    /// 端口命中区域：节点左右两侧 PORT_HIT_WIDTH 像素。
    /// - 右侧命中 → 出端口（Out）
    /// - 左侧命中 → 入端口（In）
    /// - 中间命中 → 节点主体（无端口）
    fn hit_test(&self, logical: PointF) -> HitResult {
        let is_vertical = self.layout_direction == LayoutDirection::Vertical;
        for node in self.graph.nodes() {
            let bounds = node.bounds();
            if !point_in_rect(logical, bounds) {
                // 检查端口扩展区域
                if is_vertical {
                    let bottom_port = rust_agent_flow::RectF::new(
                        PointF::new(bounds.left(), bounds.bottom() - PORT_HIT_WIDTH),
                        rust_agent_flow::SizeF::new(bounds.size.w, PORT_HIT_WIDTH * 2.0),
                    );
                    if point_in_rect(logical, bottom_port) {
                        return HitResult::OutPort(node.id, "out".to_string());
                    }
                    let top_port = rust_agent_flow::RectF::new(
                        PointF::new(bounds.left(), bounds.top() - PORT_HIT_WIDTH),
                        rust_agent_flow::SizeF::new(bounds.size.w, PORT_HIT_WIDTH * 2.0),
                    );
                    if point_in_rect(logical, top_port) {
                        return HitResult::InPort(node.id, "in".to_string());
                    }
                } else {
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
                }
                continue;
            }
            // 节点内部：检查是否在出/入端口边缘
            if is_vertical {
                let bottom_edge = logical.y >= bounds.bottom() - PORT_HIT_WIDTH;
                let top_edge = logical.y <= bounds.top() + PORT_HIT_WIDTH;
                if bottom_edge {
                    return HitResult::OutPort(node.id, "out".to_string());
                }
                if top_edge {
                    return HitResult::InPort(node.id, "in".to_string());
                }
            } else {
                let right_edge = logical.x >= bounds.right() - PORT_HIT_WIDTH;
                let left_edge = logical.x <= bounds.left() + PORT_HIT_WIDTH;
                if right_edge {
                    return HitResult::OutPort(node.id, "out".to_string());
                }
                if left_edge {
                    return HitResult::InPort(node.id, "in".to_string());
                }
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
                // 中键拖拽：屏幕坐标起点 + 视口 offset 起点
                let start_screen = PointF::new(
                    event.position.x.as_f32(),
                    event.position.y.as_f32(),
                );
                self.interaction = InteractionState::Panning {
                    start_screen,
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
                // 左键拖拽空白区域 → 平移画布（屏幕坐标起点）
                let start_screen = PointF::new(
                    event.position.x.as_f32(),
                    event.position.y.as_f32(),
                );
                self.selected = None;
                self.interaction = InteractionState::Panning {
                    start_screen,
                    origin: self.viewport.offset,
                };
            }
            _ => {}
        }
        cx.notify();
    }

    fn on_mouse_move(&mut self, event: &MouseMoveEvent, _window: &mut Window, cx: &mut Context<Self>) {
        // 预计算屏幕 + 逻辑坐标，避免 match &mut self.interaction 时借用冲突。
        let screen = PointF::new(
            event.position.x.as_f32(),
            event.position.y.as_f32(),
        );
        let logical = self.to_logical(event.position);

        match &mut self.interaction {
            InteractionState::Panning { start_screen, origin } => {
                // 纯屏幕空间 delta 平移（ReactFlow 成熟方案）：
                // new_offset = origin + (current_screen - start_screen)
                // 避免逻辑坐标反馈抖动，1:1 跟随鼠标。
                self.viewport.offset =
                    viewport::handle_pan(*origin, *start_screen, screen);
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
                        node_origin.x + (logical.x - start.x),
                        node_origin.y + (logical.y - start.y),
                    );
                }
                cx.notify();
            }
            InteractionState::DrawingEdge { current: cur, .. } => {
                *cur = logical;
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

    /// 当前视口缩放比例。
    fn scale(&self) -> f32 {
        self.viewport.scale
    }

    /// 渲染所有边（canvas paint），使用**逻辑坐标** + PathBuilder 变换。
    ///
    /// 边端点以逻辑坐标收集，在 canvas paint 回调中通过 `PathBuilder::scale`
    /// + `translate` 统一变换到屏幕空间。`offset` = `viewport.offset` +
    /// `canvas bounds.origin`（canvas 在窗口中的绝对位置），确保边与节点
    /// （通过 div 布局定位）的屏幕坐标完全一致。
    fn render_edges(&self) -> impl IntoElement {
        let s = self.scale();
        let (src_side_default, dst_side_default) = self.port_sides();
        // 收集边端点（逻辑坐标）
        let edges: Vec<(PointF, PointF, PortSide, PortSide, EdgeType)> = self
            .graph
            .edges()
            .map(|edge| {
                let src = port_position_from_side(&edge, &self.graph, src_side_default);
                let dst = port_position_from_side(&edge, &self.graph, dst_side_default);
                (src, dst, src_side_default, dst_side_default, edge.edge_type)
            })
            .collect();

        let default_edge_type = self.default_edge_type;
        let drawing = match &self.interaction {
            InteractionState::DrawingEdge { from_node, current, .. } => {
                self.graph.node(*from_node).map(|n| {
                    let src = port_position(n, src_side_default);
                    let dst = *current;
                    (src, dst, src_side_default, dst_side_default, default_edge_type)
                })
            }
            _ => None,
        };

        let offset_x = self.viewport.offset.x;
        let offset_y = self.viewport.offset.y;
        let show_grid = self.show_grid;

        // 使用稳定 id 避免 GPU 表面在每次 render 时重建（消除拖动闪烁）
        canvas(
            |bounds, _window, _cx| bounds.size,
            move |bounds, _size, window, _cx| {
                // 总偏移 = viewport.offset + canvas 在窗口中的绝对位置
                let total_offset = Point::new(
                    px(offset_x + bounds.origin.x.as_f32()),
                    px(offset_y + bounds.origin.y.as_f32()),
                );
                // 点阵背景（在边之前绘制，确保边在网格之上）
                if show_grid {
                    paint_grid(bounds, s, total_offset, window);
                }
                for (src, dst, src_side, dst_side, edge_type) in &edges {
                    paint_edge_scaled(
                        *src, *dst, *src_side, *dst_side, *edge_type, s, total_offset, window,
                    );
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
    fn render_nodes(&self) -> Vec<gpui::AnyElement> {
        let selected = self.selected;
        let registry = &self.registry;
        let s = self.scale();
        let vertical = self.layout_direction == LayoutDirection::Vertical;

        self.graph
            .nodes()
            .map(|node| {
                let node_id = node.id;
                let pos = node.position;
                let flow_node = registry.get(&node.kind);
                let is_selected = selected == Some(node_id);

                let view = NodeView::new(node.clone())
                    .with_flow_node_opt(flow_node)
                    .selected(is_selected)
                    .with_scale(s)
                    .with_vertical(vertical);

                div()
                    .absolute()
                    .left(px(pos.x * s))
                    .top(px(pos.y * s))
                    .child(view)
                    .into_any_element()
            })
            .collect()
    }

    /// 渲染工具栏：左下角横向浮动面板。
    ///
    /// 不受缩放影响（在内容层之外）。
    fn render_toolbar(&self, cx: &Context<Self>) -> impl IntoElement {
        let scale_pct = (self.viewport.scale * 100.0) as i32;

        let edge_type = self.default_edge_type;
        let show_grid = self.show_grid;

        div()
            .absolute()
            .left_3()
            .bottom_3()
            .flex()
            .flex_row()
            .items_center()
            .gap_0p5()
            .rounded_lg()
            .bg(gpui::rgba(0xffffffee))
            .border_1()
            .border_color(gpui::rgb(0xe2e8f0))
            .shadow_lg()
            .p_1()
            // 缩放百分比 + 放大/缩小
            .child(
                div()
                    .id("tb-zoom-in")
                    .flex()
                    .items_center()
                    .justify_center()
                    .w(px(28.0))
                    .h(px(28.0))
                    .rounded_md()
                    .hover(|s| s.bg(gpui::rgb(0xf1f5f9)))
                    .active(|s| s.bg(gpui::rgb(0xe2e8f0)))
                    .text_xs()
                    .font_medium()
                    .text_color(gpui::rgb(0x475569))
                    .on_mouse_down(MouseButton::Left, cx.listener(move |this, _, window, cx| {
                        this.zoom_in(window, cx);
                    }))
                    .child("+"),
            )
            .child(
                div()
                    .w(px(40.0))
                    .h(px(28.0))
                    .rounded_md()
                    .flex()
                    .items_center()
                    .justify_center()
                    .text_xs()
                    .font_medium()
                    .text_color(gpui::rgb(0x64748b))
                    .child(format!("{}%", scale_pct)),
            )
            .child(
                div()
                    .id("tb-zoom-out")
                    .flex()
                    .items_center()
                    .justify_center()
                    .w(px(28.0))
                    .h(px(28.0))
                    .rounded_md()
                    .hover(|s| s.bg(gpui::rgb(0xf1f5f9)))
                    .active(|s| s.bg(gpui::rgb(0xe2e8f0)))
                    .text_sm()
                    .font_medium()
                    .text_color(gpui::rgb(0x475569))
                    .on_mouse_down(MouseButton::Left, cx.listener(move |this, _, window, cx| {
                        this.zoom_out(window, cx);
                    }))
                    .child("\u{2212}"), // minus
            )
            // 分隔线
            .child(
                div()
                    .w(px(1.0))
                    .h(px(20.0))
                    .bg(gpui::rgb(0xe2e8f0)),
            )
            // 适应视图 / 重置
            .child(
                div()
                    .id("tb-fit")
                    .flex()
                    .items_center()
                    .justify_center()
                    .w(px(28.0))
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
                    .child("\u{25A1}"), // fit
            )
            .child(
                div()
                    .id("tb-reset")
                    .flex()
                    .items_center()
                    .justify_center()
                    .w(px(28.0))
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
                    .child("\u{27F3}"), // reset
            )
            // 分隔线
            .child(
                div()
                    .w(px(1.0))
                    .h(px(20.0))
                    .bg(gpui::rgb(0xe2e8f0)),
            )
            // 布局方向切换
            .child(
                div()
                    .id("tb-dir-h")
                    .flex()
                    .items_center()
                    .justify_center()
                    .w(px(28.0))
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
                        this.set_layout_direction(LayoutDirection::Horizontal, cx);
                    }))
                    .child("\u{2194}"), // ↔
            )
            .child(
                div()
                    .id("tb-dir-v")
                    .flex()
                    .items_center()
                    .justify_center()
                    .w(px(28.0))
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
                        this.set_layout_direction(LayoutDirection::Vertical, cx);
                    }))
                    .child("\u{2195}"), // ↕
            )
            // 分隔线
            .child(
                div()
                    .w(px(1.0))
                    .h(px(20.0))
                    .bg(gpui::rgb(0xe2e8f0)),
            )
            // 边类型选择（紧凑下拉式）
            .child(
                div()
                    .id("tb-edge-type")
                    .flex()
                    .items_center()
                    .justify_center()
                    .w(px(60.0))
                    .h(px(28.0))
                    .rounded_md()
                    .bg(if matches!(edge_type, EdgeType::SmoothStep) {
                        gpui::rgb(0xede9fe)
                    } else {
                        gpui::Rgba { r: 1.0, g: 1.0, b: 1.0, a: 0.0 }
                    })
                    .hover(|s| s.bg(gpui::rgb(0xf8fafc)))
                    .text_xs()
                    .font_medium()
                    .text_color(if matches!(edge_type, EdgeType::SmoothStep) {
                        gpui::rgb(0x6366f1)
                    } else {
                        gpui::rgb(0x64748b)
                    })
                    .on_mouse_down(MouseButton::Left, cx.listener(move |this, _, _, cx| {
                        // 循环切换边类型，同时更新所有已有边
                        let new_type = match this.default_edge_type {
                            EdgeType::Straight => EdgeType::Bezier,
                            EdgeType::Bezier => EdgeType::Step,
                            EdgeType::Step => EdgeType::SmoothStep,
                            EdgeType::SmoothStep => EdgeType::Straight,
                        };
                        this.default_edge_type = new_type;
                        for edge in this.graph.edges_mut() {
                            edge.edge_type = new_type;
                        }
                        cx.notify();
                    }))
                    .child(match edge_type {
                        EdgeType::Bezier => "Bezier",
                        EdgeType::Straight => "Straight",
                        EdgeType::Step => "Step",
                        EdgeType::SmoothStep => "Smooth",
                    }),
            )
            // 分隔线
            .child(
                div()
                    .w(px(1.0))
                    .h(px(20.0))
                    .bg(gpui::rgb(0xe2e8f0)),
            )
            // 点阵背景开关
            .child(
                div()
                    .id("tb-grid")
                    .flex()
                    .items_center()
                    .justify_center()
                    .w(px(28.0))
                    .h(px(28.0))
                    .rounded_md()
                    .bg(if show_grid {
                        gpui::rgb(0xede9fe)
                    } else {
                        gpui::Rgba { r: 1.0, g: 1.0, b: 1.0, a: 0.0 }
                    })
                    .hover(|s| s.bg(gpui::rgb(0xf8fafc)))
                    .text_xs()
                    .font_medium()
                    .text_color(if show_grid {
                        gpui::rgb(0x6366f1)
                    } else {
                        gpui::rgb(0x64748b)
                    })
                    .on_mouse_down(MouseButton::Left, cx.listener(move |this, _, _, cx| {
                        this.show_grid = !this.show_grid;
                        cx.notify();
                    }))
                    .child("\u{25A6}"), // ▦ grid symbol
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

/// 绘制点阵背景。
///
/// 点为固定屏幕尺寸（1.5px 半径），间距随缩放变化。自适应间距：当屏幕
/// 间距 < 20px 时将逻辑间距翻倍，限制点数量上限，避免低缩放时点爆炸。
///
/// 性能优化（参考 ReactFlow / tldraw 成熟方案）：
/// - 所有点收集到**单个 fill path**，一次 `paint_path` 提交，减少 draw call。
/// - 使用 `move_to` / `line_to` 显式构造矩形子路径（避免 `add_polygon` 在
///   多子路径 fill 下的渲染缺陷）。
/// - 自适应间距限制可见点数量，保证平移时帧率稳定。
fn paint_grid(
    bounds: gpui::Bounds<Pixels>,
    scale: f32,
    offset: Point<Pixels>,
    window: &mut Window,
) {
    let w = bounds.size.width.as_f32();
    let h = bounds.size.height.as_f32();
    // 防御：无效 bounds 不绘制
    if w <= 0.0 || h <= 0.0 || scale <= 0.0 {
        return;
    }

    let ox = offset.x.as_f32();
    let oy = offset.y.as_f32();

    // 自适应间距：屏幕间距过小时翻倍
    let mut spacing = GRID_SPACING;
    while spacing * scale < 20.0 {
        spacing *= 2.0;
    }

    // 可见逻辑范围
    let min_lx = (bounds.origin.x.as_f32() - ox) / scale;
    let min_ly = (bounds.origin.y.as_f32() - oy) / scale;
    let max_lx = (bounds.origin.x.as_f32() + w - ox) / scale;
    let max_ly = (bounds.origin.y.as_f32() + h - oy) / scale;

    let start_x = (min_lx / spacing).floor() * spacing;
    let start_y = (min_ly / spacing).floor() * spacing;

    let dot_color = gpui::rgb(0xcbd5e1);
    let dot_r = 1.5_f32;

    // 单个 fill path 收集所有点，一次提交。
    let mut path = PathBuilder::fill();
    let mut count: usize = 0;
    let mut gy = start_y;
    while gy <= max_ly {
        let mut gx = start_x;
        while gx <= max_lx {
            let sx = gx * scale + ox;
            let sy = gy * scale + oy;
            // 显式构造矩形子路径（move_to + line_to + 闭合）。
            // 比 add_polygon 更可靠：确保每个子路径被正确加入 fill。
            path.move_to(Point::new(px(sx - dot_r), px(sy - dot_r)));
            path.line_to(Point::new(px(sx + dot_r), px(sy - dot_r)));
            path.line_to(Point::new(px(sx + dot_r), px(sy + dot_r)));
            path.line_to(Point::new(px(sx - dot_r), px(sy + dot_r)));
            path.line_to(Point::new(px(sx - dot_r), px(sy - dot_r)));
            count += 1;
            gx += spacing;
        }
        gy += spacing;
    }

    if count > 0 {
        if let Ok(path) = path.build() {
            window.paint_path(path, dot_color);
        }
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
        let toolbar = self.render_toolbar(cx);

        let offset = self.viewport.offset;

        // ====== 外层容器：全屏，处理事件 ======
        // 光标：平移中 → grabbing（ClosedHand），空闲 → grab（OpenHand）
        let is_panning = matches!(self.interaction, InteractionState::Panning { .. });
        let mut container = div()
            .size_full()
            .relative()
            .bg(gpui::rgb(0xf8fafc))
            .overflow_hidden()
            .cursor(if is_panning {
                CursorStyle::ClosedHand
            } else {
                CursorStyle::OpenHand
            })
            .on_mouse_down(MouseButton::Left, cx.listener(Self::on_mouse_down))
            .on_mouse_down(MouseButton::Middle, cx.listener(Self::on_mouse_down))
            .on_mouse_move(cx.listener(Self::on_mouse_move))
            .on_mouse_up(MouseButton::Left, cx.listener(Self::on_mouse_up))
            .on_mouse_up(MouseButton::Middle, cx.listener(Self::on_mouse_up))
            .on_scroll_wheel(cx.listener(Self::on_scroll));

        // ====== 边（Canvas）：直接放在容器根层级 ======
        // 边在逻辑坐标中计算路径，paint 时通过 PathBuilder::scale + translate
        // 变换到屏幕空间。translate = viewport.offset + canvas bounds.origin，
        // 确保与节点（通过 div offset + pos×scale 定位）的屏幕坐标一致。
        container = container.child(edges);

        // ====== 内容层：仅包含节点，通过 offset + scale 定位 ======
        // 节点最终屏幕坐标 = container_origin + offset + logical_pos × scale
        let mut content = div()
            .absolute()
            .left(px(offset.x))
            .top(px(offset.y));

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

/// 端口外环半径（逻辑坐标），与 view.rs 中 port_outer_half/s 对应。
const PORT_RADIUS: f32 = 5.0;

/// 计算节点某侧端口的实际位置（端口圆圈中心，完全在节点外部）。
///
/// - Right → 右边缘外侧 PORT_RADIUS
/// - Left  → 左边缘外侧 PORT_RADIUS
/// - Top   → 上边缘外侧 PORT_RADIUS
/// - Bottom → 下边缘外侧 PORT_RADIUS
fn port_position(node: &Node, side: PortSide) -> PointF {
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
        PortSide::Auto => PointF::new(right + PORT_RADIUS, mid_y),
    }
}

/// 根据边的源/目标节点获取对应侧的端口位置。
fn port_position_from_side(edge: &Edge, graph: &FlowGraph, side: PortSide) -> PointF {
    // 出端口（Right/Bottom）→ source 节点；入端口（Left/Top）→ target 节点
    let node_id = match side {
        PortSide::Right | PortSide::Bottom => edge.source,
        PortSide::Left | PortSide::Top => edge.target,
        PortSide::Auto => edge.source,
    };
    graph
        .node(node_id)
        .map(|n| port_position(n, side))
        .unwrap_or_default()
}
