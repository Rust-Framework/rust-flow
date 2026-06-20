//! Interactive flow editor — React Flow rendering model.

use std::cell::Cell;
use std::sync::Arc;

use rust_agent_flow::{
    apply_flow_orientation, builtin_type_registry, demo_document_json, document_from_any_json,
    document_needs_layout, hit_node_at, hit_port_at, mindmap_document_json,
    mindmap_layout_direction_from_json, FlowDocument, FlowEdge, FlowGraph, FlowTypeRegistry,
    LayoutDirection, LayoutOptions, Point as CorePoint, SceneFrame, Size as CoreSize, Viewport,
    viewport_from_document, flow_input_side, BRANCH, toggle_branch_collapsed,
};
use gpui::*;
use gpui_component::button::Button;
use gpui_component::{Selectable, Sizable};

use crate::interaction::InteractionState;
use crate::provider::{FlowNodeRegistry, FlowPanelContext};
use crate::scene_layer::render_viewport;
use crate::theme::FlowTheme;

pub struct FlowEditorView {
    pub graph: FlowGraph,
    pub viewport: Viewport,
    pub layout_direction: LayoutDirection,
    /// True when the source document is a `mindmap-1.0` tree (uses bidirectional tree layout).
    is_mindmap: bool,
    pub interaction: InteractionState,
    pub theme: FlowTheme,
    pub node_registry: FlowNodeRegistry,
    viewport_origin: Arc<Cell<CorePoint>>,
    viewport_screen_size: Arc<Cell<CoreSize>>,
    pending_fit: bool,
}

impl FlowEditorView {
    pub fn new(graph: FlowGraph) -> Self {
        Self {
            graph,
            viewport: Viewport::default(),
            layout_direction: LayoutDirection::LeftRight,
            is_mindmap: false,
            interaction: InteractionState::Idle,
            theme: FlowTheme::light(),
            node_registry: FlowNodeRegistry::builtin(),
            viewport_origin: Arc::new(Cell::new(CorePoint::default())),
            viewport_screen_size: Arc::new(Cell::new(Viewport::default().screen_size)),
            pending_fit: false,
        }
    }

    pub fn demo() -> Self {
        let mut view =
            Self::from_document_json(demo_document_json(), builtin_type_registry());
        view.auto_layout_default();
        view
    }

    /// Load editor from a Flow Schema or mind map JSON string.
    pub fn from_document_json(json: &str, types: FlowTypeRegistry) -> Self {
        let layout_direction = mindmap_layout_direction_from_json(json);
        let doc = document_from_any_json(json).unwrap_or_else(|_| FlowDocument::new("Untitled"));
        let is_mindmap = doc.version.starts_with("mindmap");
        let needs_layout = document_needs_layout(&doc);
        let graph = FlowGraph::from_document(&doc, &types);
        let viewport = doc
            .viewport
            .as_ref()
            .map(viewport_from_document)
            .unwrap_or_default();

        let mut view = Self {
            graph,
            viewport,
            layout_direction,
            is_mindmap,
            interaction: InteractionState::Idle,
            theme: FlowTheme::light(),
            node_registry: FlowNodeRegistry::from_type_registry(types),
            viewport_origin: Arc::new(Cell::new(CorePoint::default())),
            viewport_screen_size: Arc::new(Cell::new(Viewport::default().screen_size)),
            pending_fit: false,
        };

        if needs_layout {
            view.auto_layout_default();
        }

        view
    }

    /// Load the embedded mind map example (AI-friendly `mindmap-1.0` JSON).
    pub fn mindmap_demo() -> Self {
        Self::from_document_json(mindmap_document_json(), builtin_type_registry())
    }

    pub fn empty() -> Self {
        Self::new(FlowGraph::new("Untitled"))
    }

    /// Re-run Dagre auto-layout (React Flow standard). Press **L** or use the toolbar button.
    pub fn auto_layout(&mut self, options: LayoutOptions) {
        self.graph.auto_layout(&options);
        self.pending_fit = true;
        self.try_fit_view_to_content();
    }

    pub fn auto_layout_default(&mut self) {
        if self.is_mindmap {
            let options = match self.layout_direction {
                LayoutDirection::LeftRight => LayoutOptions::mindmap_lr(),
                LayoutDirection::TopBottom => LayoutOptions::mindmap_tree_tb(),
            };
            self.graph.auto_layout_mindmap(&options);
            self.pending_fit = true;
            self.try_fit_view_to_content();
        } else {
            self.auto_layout(LayoutOptions {
                direction: self.layout_direction,
                ..LayoutOptions::comfortable()
            });
        }
    }

    pub fn set_layout_direction(&mut self, direction: LayoutDirection) {
        self.layout_direction = direction;
        self.graph.layout_direction = direction;
        apply_flow_orientation(&mut self.graph, direction);
        self.auto_layout_default();
    }

    /// Zoom/pan so the full graph is visible (React Flow `fitView`).
    pub fn fit_view_to_content(&mut self) {
        self.apply_canvas_metrics();
        if let Some((origin, size)) = self.graph.content_bounds() {
            self.viewport.fit_to_content(origin, size, 80.0);
        }
    }

    fn apply_canvas_metrics(&mut self) {
        let sz = self.viewport_screen_size.get();
        if sz.width > 1.0 && sz.height > 1.0 {
            self.viewport.screen_size = sz;
        }
    }

    fn try_fit_view_to_content(&mut self) {
        self.apply_canvas_metrics();
        if self.viewport.screen_size.width < 64.0 {
            return;
        }
        self.fit_view_to_content();
        self.pending_fit = false;
    }

    fn on_layout_lr_click(&mut self, _: &ClickEvent, _: &mut Window, cx: &mut Context<'_, Self>) {
        self.set_layout_direction(LayoutDirection::LeftRight);
        cx.notify();
    }

    fn on_layout_tb_click(&mut self, _: &ClickEvent, _: &mut Window, cx: &mut Context<'_, Self>) {
        self.set_layout_direction(LayoutDirection::TopBottom);
        cx.notify();
    }

    fn on_layout_click(&mut self, _: &ClickEvent, _: &mut Window, cx: &mut Context<'_, Self>) {
        self.auto_layout_default();
        cx.notify();
    }

    fn on_fit_view_click(&mut self, _: &ClickEvent, _: &mut Window, cx: &mut Context<'_, Self>) {
        self.fit_view_to_content();
        cx.notify();
    }

    fn on_mindmap_demo_click(&mut self, _: &ClickEvent, _: &mut Window, cx: &mut Context<'_, Self>) {
        let types = builtin_type_registry();
        let json = mindmap_document_json();
        self.layout_direction = mindmap_layout_direction_from_json(json);
        self.is_mindmap = true;
        let doc = document_from_any_json(json).unwrap_or_else(|_| FlowDocument::new("Mind Map"));
        self.graph = FlowGraph::from_document(&doc, &types);
        self.graph.layout_direction = self.layout_direction;
        apply_flow_orientation(&mut self.graph, self.layout_direction);
        self.auto_layout_default();
        cx.notify();
    }

    fn on_key_down(&mut self, event: &KeyDownEvent, _: &mut Window, cx: &mut Context<'_, Self>) {
        let key = event.keystroke.key.as_str();
        if key == "l" || key == "L" {
            self.auto_layout_default();
            cx.notify();
        }
        if key == "c" || key == "C" {
            if let Some(id) = self.graph.selected_nodes().first().copied() {
                if self
                    .graph
                    .nodes
                    .get(id)
                    .is_some_and(|n| n.node_type == BRANCH)
                {
                    toggle_branch_collapsed(&mut self.graph, id);
                    cx.notify();
                }
            }
        }
    }

    fn local_mouse(&self, window_pos: Point<Pixels>) -> CorePoint {
        let o = self.viewport_origin.get();
        CorePoint::new(
            f64::from(window_pos.x) as f32 - o.x,
            f64::from(window_pos.y) as f32 - o.y,
        )
    }

    fn resolve_frame(&self, preview_to: Option<CorePoint>) -> SceneFrame {
        let mut frame = SceneFrame::resolve(&self.graph, &self.viewport);
        if let (Some(from_port), Some(to)) = (
            match self.interaction {
                InteractionState::CreatingConnection { from_port, .. } => Some(from_port),
                _ => None,
            },
            preview_to,
        ) {
            if let Some((from, from_side)) = frame.port_at(&self.graph, from_port) {
                frame = frame.with_preview(from, from_side, to, flow_input_side(self.layout_direction));
            }
        }
        frame
    }

    fn on_down(&mut self, event: &MouseDownEvent, _: &mut Window, cx: &mut Context<'_, Self>) {
        match event.button {
            MouseButton::Left => {
                if let Some(pid) = hit_port_at(&self.graph, &self.viewport, self.local_mouse(event.position)) {
                    self.interaction = InteractionState::CreatingConnection {
                        from_port: pid,
                        current_mouse: event.position,
                    };
                } else if let Some(nid) = hit_node_at(
                    &self.graph,
                    self.viewport
                        .screen_to_world(self.local_mouse(event.position)),
                ) {
                    self.graph.clear_selection();
                    self.graph.select_node(nid, false);
                    let world = self
                        .viewport
                        .screen_to_world(self.local_mouse(event.position));
                    let node_pos = self
                        .graph
                        .nodes
                        .get(nid)
                        .map(|n| n.position)
                        .unwrap_or_default();
                    self.interaction = InteractionState::DraggingNode {
                        node_id: nid,
                        grab_offset: CorePoint::new(world.x - node_pos.x, world.y - node_pos.y),
                    };
                } else {
                    self.graph.clear_selection();
                    self.interaction = InteractionState::Panning {
                        last_mouse: event.position,
                    };
                }
            }
            MouseButton::Middle => {
                self.interaction = InteractionState::Panning {
                    last_mouse: event.position,
                };
            }
            _ => {}
        }
        cx.notify();
    }

    fn on_move(&mut self, event: &MouseMoveEvent, _: &mut Window, cx: &mut Context<'_, Self>) {
        let pos = event.position;
        match self.interaction {
            InteractionState::DraggingNode {
                node_id,
                grab_offset,
            } => {
                let world = self.viewport.screen_to_world(self.local_mouse(pos));
                if let Some(node) = self.graph.nodes.get_mut(node_id) {
                    node.position =
                        CorePoint::new(world.x - grab_offset.x, world.y - grab_offset.y);
                }
            }
            InteractionState::CreatingConnection { from_port, .. } => {
                self.interaction = InteractionState::CreatingConnection {
                    from_port,
                    current_mouse: pos,
                };
            }
            InteractionState::Panning { last_mouse } => {
                let cur = self.local_mouse(pos);
                let last = self.local_mouse(last_mouse);
                self.viewport
                    .pan_by(CorePoint::new(cur.x - last.x, cur.y - last.y));
                self.interaction = InteractionState::Panning { last_mouse: pos };
            }
            _ => {}
        }
        cx.notify();
    }

    fn on_up(&mut self, event: &MouseUpEvent, _: &mut Window, cx: &mut Context<'_, Self>) {
        if let InteractionState::DraggingNode { node_id, .. } = self.interaction {
            if self.viewport.snap_enabled {
                if let Some(node) = self.graph.nodes.get_mut(node_id) {
                    node.position = self.viewport.snap_point(node.position);
                }
            }
        }

        if let InteractionState::CreatingConnection { from_port, .. } = self.interaction {
            if let Some(to_port) =
                hit_port_at(&self.graph, &self.viewport, self.local_mouse(event.position))
            {
                if to_port != from_port {
                    let from_is_input = self
                        .graph
                        .ports
                        .get(from_port)
                        .map(|p| p.is_input())
                        .unwrap_or(false);
                    let to_is_input = self
                        .graph
                        .ports
                        .get(to_port)
                        .map(|p| p.is_input())
                        .unwrap_or(false);
                    let (src, tgt) = if !from_is_input && to_is_input {
                        (from_port, to_port)
                    } else if from_is_input && !to_is_input {
                        (to_port, from_port)
                    } else {
                        (from_port, from_port)
                    };
                    if src != tgt {
                        self.graph.add_edge(FlowEdge::new(src, tgt));
                    }
                }
            }
        }
        self.interaction = InteractionState::Idle;
        cx.notify();
    }

    fn on_scroll(&mut self, event: &ScrollWheelEvent, _: &mut Window, cx: &mut Context<'_, Self>) {
        let dy: f32 = f64::from(event.delta.pixel_delta(px(40.0)).y) as f32;
        let factor = if dy > 0.0 { 1.1 } else { 0.909 };
        self.viewport.zoom_at(factor, self.local_mouse(event.position));
        cx.notify();
    }
}

impl Render for FlowEditorView {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<'_, Self>) -> impl IntoElement {
        if self.pending_fit {
            self.try_fit_view_to_content();
        }

        let stats = self.graph.stats();
        let vp = self.viewport;
        let theme = self.theme.clone();
        let weak = cx.weak_entity();
        let origin_cell = self.viewport_origin.clone();
        let screen_size_cell = self.viewport_screen_size.clone();

        let preview_to = match self.interaction {
            InteractionState::CreatingConnection { current_mouse, .. } => {
                Some(self.local_mouse(current_mouse))
            }
            _ => None,
        };

        let frame = self.resolve_frame(preview_to);
        let registry = self.node_registry.clone();
        let panel = self.render_property_panel();
        let is_panning = matches!(self.interaction, InteractionState::Panning { .. });

        div()
            .flex()
            .flex_col()
            .size_full()
            .bg(theme.canvas_background)
            .on_key_down(cx.listener(Self::on_key_down))
            .child(self.render_toolbar(cx, stats, vp))
            .child(
                div()
                    .flex_1()
                    .flex()
                    .flex_row()
                    .overflow_hidden()
                    .child({
                        let canvas = div()
                            .flex_1()
                            .relative()
                            .overflow_hidden()
                            .bg(theme.canvas_background)
                            .on_mouse_down(MouseButton::Left, cx.listener(Self::on_down))
                            .on_mouse_down(MouseButton::Middle, cx.listener(Self::on_down))
                            .on_mouse_move(cx.listener(Self::on_move))
                            .on_mouse_up(MouseButton::Left, cx.listener(Self::on_up))
                            .on_mouse_up(MouseButton::Middle, cx.listener(Self::on_up))
                            .on_scroll_wheel(cx.listener(Self::on_scroll))
                            .child({
                                render_viewport(frame, theme.clone(), &registry, move |bounds, cx| {
                                    origin_cell.set(CorePoint::new(
                                        f32::from(bounds.origin.x),
                                        f32::from(bounds.origin.y),
                                    ));
                                    screen_size_cell.set(CoreSize::new(
                                        f32::from(bounds.size.width),
                                        f32::from(bounds.size.height),
                                    ));
                                    weak.update(cx, |view, cx| {
                                        if view.pending_fit
                                            && screen_size_cell.get().width > 64.0
                                        {
                                            view.try_fit_view_to_content();
                                            cx.notify();
                                        }
                                    })
                                    .ok();
                                })
                            });
                        if is_panning {
                            canvas.cursor_grabbing()
                        } else {
                            canvas.cursor_grab()
                        }
                    })
                    .child(panel),
            )
    }
}

impl FlowEditorView {
    fn render_toolbar(
        &self,
        cx: &mut Context<'_, Self>,
        stats: rust_agent_flow::GraphStats,
        vp: Viewport,
    ) -> Div {
        let theme = self.theme.clone();
        let is_lr = self.layout_direction == LayoutDirection::LeftRight;
        let is_tb = self.layout_direction == LayoutDirection::TopBottom;
        div()
            .flex()
            .flex_row()
            .h(px(40.0))
            .bg(theme.canvas_background)
            .border_b_1()
            .border_color(theme.node_border)
            .px(px(12.0))
            .items_center()
            .gap(px(8.0))
            .child(
                div()
                    .text_sm()
                    .font_weight(FontWeight::SEMIBOLD)
                    .text_color(theme.node_title_text)
                    .child(self.graph.name.clone()),
            )
            .child(
                div()
                    .text_xs()
                    .text_color(theme.body_muted_color)
                    .child(format!(
                        "Nodes: {}  Edges: {}  Zoom: {:.0}%",
                        stats.node_count,
                        stats.edge_count,
                        vp.zoom * 100.0
                    )),
            )
            .child(div().flex_1())
            .child(
                Button::new("flow-layout-lr")
                    .label("横向")
                    .small()
                    .outline()
                    .selected(is_lr)
                    .on_click(cx.listener(Self::on_layout_lr_click)),
            )
            .child(
                Button::new("flow-layout-tb")
                    .label("纵向")
                    .small()
                    .outline()
                    .selected(is_tb)
                    .on_click(cx.listener(Self::on_layout_tb_click)),
            )
            .child(
                Button::new("flow-auto-layout")
                    .label("重新排版")
                    .small()
                    .outline()
                    .on_click(cx.listener(Self::on_layout_click)),
            )
            .child(
                Button::new("flow-fit-view")
                    .label("适应视图")
                    .small()
                    .outline()
                    .on_click(cx.listener(Self::on_fit_view_click)),
            )
            .child(
                Button::new("flow-mindmap-demo")
                    .label("思维导图")
                    .small()
                    .outline()
                    .on_click(cx.listener(Self::on_mindmap_demo_click)),
            )
    }

    fn render_property_panel(&mut self) -> Div {
        let theme = self.theme.clone();
        let selected = self.graph.selected_nodes().first().copied();

        let body = if let Some(node_id) = selected {
            let node_type = self
                .graph
                .nodes
                .get(node_id)
                .map(|n| n.node_type.clone())
                .unwrap_or_default();
            let notify = Arc::new(|| {});
            let mut panel_ctx = FlowPanelContext {
                graph: &mut self.graph,
                node_id,
                theme: &theme,
                notify,
            };
            self.node_registry
                .get(&node_type)
                .render_panel(&mut panel_ctx)
        } else {
            div()
                .text_sm()
                .text_color(theme.body_muted_color)
                .child("选择节点以编辑属性")
        };

        div()
            .w(px(260.0))
            .h_full()
            .flex()
            .flex_col()
            .border_l_1()
            .border_color(theme.node_border)
            .bg(theme.node_background)
            .child(
                div()
                    .px(px(12.0))
                    .py(px(10.0))
                    .border_b_1()
                    .border_color(theme.node_border)
                    .text_sm()
                    .font_weight(FontWeight::SEMIBOLD)
                    .text_color(theme.node_title_text)
                    .child("属性面板"),
            )
            .child(div().flex_1().overflow_hidden().px(px(12.0)).py(px(10.0)).child(body))
    }
}
