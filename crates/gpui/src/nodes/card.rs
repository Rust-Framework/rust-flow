use rust_agent_flow::{NODE_PAD, ResolvedNode};
use gpui::*;

use crate::theme::FlowTheme;
use crate::zoom::Z;

/// Shared card chrome helper for providers that use the default card layout.
pub fn node_card(node: &ResolvedNode, theme: &FlowTheme, body: impl IntoElement) -> Div {
    let z = Z::new(node.zoom);
    let border = if node.selected {
        theme.node_border_selected
    } else {
        theme.node_border
    };

    div()
        .size_full()
        .flex()
        .flex_col()
        .rounded(z.px(4.0))
        .overflow_hidden()
        .shadow_md()
        .border_1()
        .border_color(border)
        .bg(theme.node_background)
        .child(
            div()
                .flex_1()
                .flex()
                .flex_col()
                .justify_center()
                .px(z.px(NODE_PAD))
                .py(z.px(NODE_PAD))
                .child(body),
        )
}

pub fn node_title(node: &ResolvedNode, theme: &FlowTheme) -> Div {
    let z = Z::new(node.zoom);
    div()
        .text_size(z.text_sm())
        .font_weight(FontWeight::MEDIUM)
        .text_color(theme.node_title_text)
        .whitespace_nowrap()
        .overflow_hidden()
        .text_ellipsis()
        .child(node.label.clone())
}

pub fn panel_section(title: impl Into<SharedString>, theme: &FlowTheme) -> Div {
    div()
        .text_xs()
        .font_weight(FontWeight::SEMIBOLD)
        .text_color(theme.body_muted_color)
        .mb(px(4.0))
        .child(title.into())
}

pub fn panel_input_row(
    label: impl Into<SharedString>,
    value: impl Into<SharedString>,
    theme: &FlowTheme,
) -> Div {
    div()
        .flex()
        .flex_col()
        .gap(px(4.0))
        .mb(px(10.0))
        .child(
            div()
                .text_xs()
                .text_color(theme.body_muted_color)
                .child(label.into()),
        )
        .child(
            div()
                .px(px(8.0))
                .py(px(6.0))
                .rounded(px(4.0))
                .border_1()
                .border_color(theme.node_border)
                .bg(theme.canvas_background)
                .text_sm()
                .text_color(theme.node_title_text)
                .child(value.into()),
        )
}
