//! Read-only mind-map / flowchart view — compact TB layout, no node dragging.

use std::cell::Cell;
use std::sync::Arc;

use rust_agent_flow::{
    apply_flow_orientation, builtin_type_registry, document_from_any_json,
    document_needs_layout, FlowDocument, FlowGraph, LayoutDirection, LayoutOptions, Point as CorePoint,
    SceneFrame, Size as CoreSize, Viewport, mermaid_layout_direction, mermaid_to_flow_document,
};
use gpui::*;
use gpui::prelude::FluentBuilder;

use crate::interaction::InteractionState;
use crate::provider::FlowNodeRegistry;
use crate::scene_layer::{render_viewport_styled, ViewportStyle};
use crate::theme::FlowTheme;

/// Static flowchart / mind-map canvas (pan + zoom only; nodes are not draggable).
pub struct MindMapView {
    pub graph: FlowGraph,
    pub viewport: Viewport,
    pub theme: FlowTheme,
    pub node_registry: FlowNodeRegistry,
    interaction: InteractionState,
    viewport_origin: Arc<Cell<CorePoint>>,
    viewport_screen_size: Arc<Cell<CoreSize>>,
    pending_fit: bool,
}

impl MindMapView {
    pub fn from_document(doc: &FlowDocument) -> Self {
        let types = builtin_type_registry();
        let needs_layout = document_needs_layout(doc);
        let graph = FlowGraph::from_document(doc, &types);
        let viewport = Viewport::default();

        let mut view = Self {
            graph,
            viewport,
            theme: FlowTheme::light(),
            node_registry: FlowNodeRegistry::mindmap(),
            interaction: InteractionState::Idle,
            viewport_origin: Arc::new(Cell::new(CorePoint::default())),
            viewport_screen_size: Arc::new(Cell::new(Viewport::default().screen_size)),
            pending_fit: false,
        };

        view.graph.layout_direction = LayoutDirection::TopBottom;
        apply_flow_orientation(&mut view.graph, LayoutDirection::TopBottom);

        if needs_layout {
            view.auto_layout();
        }

        view
    }

    /// Load from Mermaid `graph TB` text or `mindmap-1.0` JSON.
    pub fn from_text(text: &str) -> Self {
        let trimmed = text.trim();
        let doc = if trimmed.starts_with("graph ") || trimmed.starts_with("flowchart ") {
            mermaid_to_flow_document(text).unwrap_or_else(|_| FlowDocument::new("Mind Map"))
        } else {
            document_from_any_json(text).unwrap_or_else(|_| FlowDocument::new("Mind Map"))
        };
        let mut view = Self::from_document(&doc);
        if trimmed.starts_with("graph ") || trimmed.starts_with("flowchart ") {
            let dir = mermaid_layout_direction(text);
            view.graph.layout_direction = dir;
            apply_flow_orientation(&mut view.graph, dir);
            view.auto_layout();
        }
        view
    }

    /// Embedded orchestrator workflow example (Mermaid TB).
    pub fn orchestrator_demo() -> Self {
        Self::from_text(ORCHESTRATOR_MERMAID)
    }

    pub fn auto_layout(&mut self) {
        self.graph.auto_layout_mermaid(&LayoutOptions::mermaid_flowchart_tb());
        self.pending_fit = true;
        self.try_fit_view_to_content();
    }

    pub fn fit_view_to_content(&mut self) {
        self.apply_canvas_metrics();
        if let Some((origin, size)) = self.graph.content_bounds() {
            self.viewport.fit_to_content(origin, size, 48.0);
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

    fn local_mouse(&self, window_pos: Point<Pixels>) -> CorePoint {
        let o = self.viewport_origin.get();
        CorePoint::new(
            f64::from(window_pos.x) as f32 - o.x,
            f64::from(window_pos.y) as f32 - o.y,
        )
    }

    fn on_down(&mut self, event: &MouseDownEvent, _: &mut Window, cx: &mut Context<'_, Self>) {
        if event.button == MouseButton::Left || event.button == MouseButton::Middle {
            self.interaction = InteractionState::Panning {
                last_mouse: event.position,
            };
        }
        cx.notify();
    }

    fn on_move(&mut self, event: &MouseMoveEvent, _: &mut Window, cx: &mut Context<'_, Self>) {
        if let InteractionState::Panning { last_mouse } = self.interaction {
            let cur = self.local_mouse(event.position);
            let last = self.local_mouse(last_mouse);
            self.viewport
                .pan_by(CorePoint::new(cur.x - last.x, cur.y - last.y));
            self.interaction = InteractionState::Panning {
                last_mouse: event.position,
            };
        }
        cx.notify();
    }

    fn on_up(&mut self, _: &MouseUpEvent, _: &mut Window, cx: &mut Context<'_, Self>) {
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

impl Render for MindMapView {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<'_, Self>) -> impl IntoElement {
        if self.pending_fit {
            self.try_fit_view_to_content();
        }

        let theme = self.theme.clone();
        let weak = cx.weak_entity();
        let origin_cell = self.viewport_origin.clone();
        let screen_size_cell = self.viewport_screen_size.clone();
        let frame = SceneFrame::resolve(&self.graph, &self.viewport);
        let registry = self.node_registry.clone();
        let is_panning = matches!(self.interaction, InteractionState::Panning { .. });

        div()
            .flex()
            .flex_col()
            .size_full()
            .bg(theme.canvas_background)
            .child(
                div()
                    .flex()
                    .flex_row()
                    .h(px(36.0))
                    .px(px(12.0))
                    .items_center()
                    .border_b_1()
                    .border_color(theme.node_border)
                    .child(
                        div()
                            .text_sm()
                            .font_weight(FontWeight::SEMIBOLD)
                            .text_color(theme.node_title_text)
                            .child(self.graph.name.clone()),
                    )
                    .child(div().flex_1())
                    .child(
                        div()
                            .text_xs()
                            .text_color(theme.body_muted_color)
                            .child("只读 · 滚轮缩放 · 拖拽平移"),
                    ),
            )
            .child(
                div()
                    .flex_1()
                    .relative()
                    .overflow_hidden()
                    .when(is_panning, |el| el.cursor_grab())
                    .on_mouse_down(MouseButton::Left, cx.listener(Self::on_down))
                    .on_mouse_down(MouseButton::Middle, cx.listener(Self::on_down))
                    .on_mouse_move(cx.listener(Self::on_move))
                    .on_mouse_up(MouseButton::Left, cx.listener(Self::on_up))
                    .on_mouse_up(MouseButton::Middle, cx.listener(Self::on_up))
                    .on_scroll_wheel(cx.listener(Self::on_scroll))
                    .child({
                        render_viewport_styled(
                            frame,
                            theme,
                            &registry,
                            ViewportStyle::mindmap(),
                            move |bounds, cx| {
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
                            },
                        )
                    }),
            )
    }
}

pub const ORCHESTRATOR_MERMAID: &str = r#"
graph TB
    U[用户任务] --> O[Orchestrator 主编排]
    O --> P[planner 规划]
    O --> E[explorer 探索]
    O --> CA[coder-alpha 并行 A]
    O --> CB[coder-beta 并行 B]
    O --> T[tester 验证]
    O --> R[reviewer 审查]
    T -->|FAIL| O
    R -->|阻塞项| O
    T -->|PASS| D[交付]
    R -->|通过| D
"#;
