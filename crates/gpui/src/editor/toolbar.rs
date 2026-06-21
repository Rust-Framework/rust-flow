//! 工具栏：左下角浮动面板，提供缩放、视图、布局方向、边类型、网格开关。
//!
//! 工具栏不受视口缩放影响（在内容层之外）。

use gpui::{
    div, px, Context, InteractiveElement, IntoElement, MouseButton, ParentElement, Styled,
    StatefulInteractiveElement, Window,
};
use gpui_component::StyledExt;
use rust_agent_flow::{EdgeType, PointF, RectF, SizeF, Viewport};

use super::flow_editor::{FlowEditorView, LayoutDirection};
use super::viewport;

impl FlowEditorView {
    /// 放大（以视口可见区域中心为锚点）。
    pub(crate) fn zoom_in(&mut self, window: &Window, cx: &mut Context<Self>) {
        let size = window.viewport_size();
        let center_screen = PointF::new(size.width.as_f32() * 0.5, size.height.as_f32() * 0.5);
        let center_logical = self.viewport.to_logical(center_screen);
        self.viewport = viewport::handle_zoom(self.viewport, center_logical, -60.0);
        cx.notify();
    }

    /// 缩小（以视口可见区域中心为锚点）。
    pub(crate) fn zoom_out(&mut self, window: &Window, cx: &mut Context<Self>) {
        let size = window.viewport_size();
        let center_screen = PointF::new(size.width.as_f32() * 0.5, size.height.as_f32() * 0.5);
        let center_logical = self.viewport.to_logical(center_screen);
        self.viewport = viewport::handle_zoom(self.viewport, center_logical, 60.0);
        cx.notify();
    }

    /// 重置视口（归位 + 100%缩放）。
    pub(crate) fn reset_view(&mut self, cx: &mut Context<Self>) {
        self.viewport = Viewport::default();
        cx.notify();
    }

    /// 适应视图（将所有节点居中显示）。
    pub(crate) fn fit_view(&mut self, cx: &mut Context<Self>) {
        // 计算所有节点的包围盒。
        let mut bounds = Option::<RectF>::None;
        for node in self.graph.nodes() {
            let nb = node.bounds();
            bounds = Some(match bounds {
                Some(b) => RectF::new(
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

    /// 渲染工具栏：左下角横向浮动面板。
    ///
    /// 不受缩放影响（在内容层之外）。
    pub(crate) fn render_toolbar(&self, cx: &Context<Self>) -> impl IntoElement {
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
}
