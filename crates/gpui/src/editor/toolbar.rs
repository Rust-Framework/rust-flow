//! 工具栏：左下角浮动面板，提供缩放、视图、布局方向、边类型、网格开关、
//! 拖拽开关、主题切换。
//!
//! 工具栏不受视口缩放影响（在内容层之外）。所有颜色取自 [`Theme`](crate::theme::Theme)，
//! 支持亮色/暗色主题切换。

use gpui::{
    div, px, Context, InteractiveElement, IntoElement, MouseButton, ParentElement, Styled,
    StatefulInteractiveElement, Window,
};
use gpui_component::StyledExt;
use rust_agent_flow::{EdgeType, PointF, RectF, SizeF, Viewport};

use super::flow_editor::{FlowEditorView, LayoutDirection};
use super::viewport;

/// 透明色（用于未激活 toggle 的占位背景）。
const TRANSPARENT: gpui::Rgba = gpui::Rgba {
    r: 1.0,
    g: 1.0,
    b: 1.0,
    a: 0.0,
};

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

    /// 切换拖拽开关。
    pub(crate) fn toggle_drag(&mut self, cx: &mut Context<Self>) {
        self.drag_enabled = !self.drag_enabled;
        cx.notify();
    }

    /// 渲染工具栏：左下角横向浮动面板。
    ///
    /// 不受缩放影响（在内容层之外）。所有颜色取自主题。
    pub(crate) fn render_toolbar(&self, cx: &Context<Self>) -> impl IntoElement {
        let scale_pct = (self.viewport.scale * 100.0) as i32;
        let t = self.theme;

        let edge_type = self.default_edge_type;
        let show_grid = self.show_grid;
        let drag_enabled = self.drag_enabled;
        let is_dark = t.is_dark;
        let grid_spacing = self.grid_spacing;

        div()
            .absolute()
            .left_3()
            .bottom_3()
            .flex()
            .flex_row()
            .items_center()
            .gap_0p5()
            .rounded_lg()
            .bg(t.toolbar_bg)
            .border_1()
            .border_color(t.toolbar_border)
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
                    .hover(|s| s.bg(t.toolbar_hover_bg))
                    .active(|s| s.bg(t.toolbar_active_bg))
                    .text_xs()
                    .font_medium()
                    .text_color(t.toolbar_text)
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
                    .text_color(t.toolbar_subtext)
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
                    .hover(|s| s.bg(t.toolbar_hover_bg))
                    .active(|s| s.bg(t.toolbar_active_bg))
                    .text_sm()
                    .font_medium()
                    .text_color(t.toolbar_text)
                    .on_mouse_down(MouseButton::Left, cx.listener(move |this, _, window, cx| {
                        this.zoom_out(window, cx);
                    }))
                    .child("\u{2212}"), // minus
            )
            // 分隔线
            .child(divider(t.toolbar_divider))
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
                    .hover(|s| s.bg(t.toolbar_hover_bg))
                    .active(|s| s.bg(t.toolbar_active_bg))
                    .text_sm()
                    .font_medium()
                    .text_color(t.toolbar_text)
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
                    .hover(|s| s.bg(t.toolbar_hover_bg))
                    .active(|s| s.bg(t.toolbar_active_bg))
                    .text_sm()
                    .font_medium()
                    .text_color(t.toolbar_text)
                    .on_mouse_down(MouseButton::Left, cx.listener(move |this, _, _, cx| {
                        this.reset_view(cx);
                    }))
                    .child("\u{27F3}"), // reset
            )
            // 分隔线
            .child(divider(t.toolbar_divider))
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
                        t.toolbar_accent
                    } else {
                        TRANSPARENT
                    })
                    .hover(|s| {
                        s.bg(if self.layout_direction == LayoutDirection::Horizontal {
                            t.toolbar_accent
                        } else {
                            t.toolbar_hover_bg
                        })
                    })
                    .text_xs()
                    .font_medium()
                    .text_color(if self.layout_direction == LayoutDirection::Horizontal {
                        t.toolbar_accent_text
                    } else {
                        t.toolbar_text
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
                        t.toolbar_accent
                    } else {
                        TRANSPARENT
                    })
                    .hover(|s| {
                        s.bg(if self.layout_direction == LayoutDirection::Vertical {
                            t.toolbar_accent
                        } else {
                            t.toolbar_hover_bg
                        })
                    })
                    .text_xs()
                    .font_medium()
                    .text_color(if self.layout_direction == LayoutDirection::Vertical {
                        t.toolbar_accent_text
                    } else {
                        t.toolbar_text
                    })
                    .on_mouse_down(MouseButton::Left, cx.listener(move |this, _, _, cx| {
                        this.set_layout_direction(LayoutDirection::Vertical, cx);
                    }))
                    .child("\u{2195}"), // ↕
            )
            // 分隔线
            .child(divider(t.toolbar_divider))
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
                        t.toolbar_toggle_bg
                    } else {
                        TRANSPARENT
                    })
                    .hover(|s| s.bg(t.toolbar_toggle_hover_bg))
                    .text_xs()
                    .font_medium()
                    .text_color(if matches!(edge_type, EdgeType::SmoothStep) {
                        t.toolbar_toggle_text
                    } else {
                        t.toolbar_subtext
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
            .child(divider(t.toolbar_divider))
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
                        t.toolbar_toggle_bg
                    } else {
                        TRANSPARENT
                    })
                    .hover(|s| s.bg(t.toolbar_toggle_hover_bg))
                    .text_xs()
                    .font_medium()
                    .text_color(if show_grid {
                        t.toolbar_toggle_text
                    } else {
                        t.toolbar_subtext
                    })
                    .on_mouse_down(MouseButton::Left, cx.listener(move |this, _, _, cx| {
                        this.show_grid = !this.show_grid;
                        cx.notify();
                    }))
                    .child("\u{25A6}"), // ▦ grid symbol
            )
            // 点阵密度切换（紧凑/标准/稀疏）
            .child(
                div()
                    .id("tb-grid-density")
                    .flex()
                    .items_center()
                    .justify_center()
                    .w(px(40.0))
                    .h(px(28.0))
                    .rounded_md()
                    .bg(if show_grid {
                        t.toolbar_toggle_bg
                    } else {
                        TRANSPARENT
                    })
                    .hover(|s| s.bg(t.toolbar_toggle_hover_bg))
                    .text_xs()
                    .font_medium()
                    .text_color(if show_grid {
                        t.toolbar_toggle_text
                    } else {
                        t.toolbar_subtext
                    })
                    .on_mouse_down(MouseButton::Left, cx.listener(move |this, _, _, cx| {
                        // 循环切换密度：紧凑(20) → 标准(28) → 稀疏(40) → 紧凑
                        let new_spacing = match this.grid_spacing as i32 {
                            20 => 28.0,
                            28 => 40.0,
                            _ => 20.0,
                        };
                        this.set_grid_spacing(new_spacing, cx);
                    }))
                    .child(match grid_spacing as i32 {
                        20 => "D1",
                        40 => "D3",
                        _ => "D2",
                    }),
            )
            // 拖拽开关
            .child(
                div()
                    .id("tb-drag")
                    .flex()
                    .items_center()
                    .justify_center()
                    .w(px(28.0))
                    .h(px(28.0))
                    .rounded_md()
                    .bg(if drag_enabled {
                        t.toolbar_toggle_bg
                    } else {
                        TRANSPARENT
                    })
                    .hover(|s| s.bg(t.toolbar_toggle_hover_bg))
                    .text_xs()
                    .font_medium()
                    .text_color(if drag_enabled {
                        t.toolbar_toggle_text
                    } else {
                        t.toolbar_subtext
                    })
                    .on_mouse_down(MouseButton::Left, cx.listener(move |this, _, _, cx| {
                        this.toggle_drag(cx);
                    }))
                    .child("\u{270E}"), // ✎ drag/move symbol
            )
            // 分隔线
            .child(divider(t.toolbar_divider))
            // 主题切换
            .child(
                div()
                    .id("tb-theme")
                    .flex()
                    .items_center()
                    .justify_center()
                    .w(px(28.0))
                    .h(px(28.0))
                    .rounded_md()
                    .hover(|s| s.bg(t.toolbar_hover_bg))
                    .active(|s| s.bg(t.toolbar_active_bg))
                    .text_sm()
                    .font_medium()
                    .text_color(t.toolbar_text)
                    .on_mouse_down(MouseButton::Left, cx.listener(move |this, _, _, cx| {
                        this.toggle_theme(cx);
                    }))
                    .child(if is_dark { "\u{2600}" } else { "\u{263D}" }), // ☀ light / ☽ dark
            )
    }
}

/// 工具栏分隔线。
fn divider(color: gpui::Rgba) -> gpui::Div {
    div().w(px(1.0)).h(px(20.0)).bg(color)
}
