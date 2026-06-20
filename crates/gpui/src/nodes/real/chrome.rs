//! Shared visual primitives for realistic node chrome.

use rust_agent_flow::ResolvedNode;
use gpui::*;

use crate::theme::FlowTheme;
use crate::zoom::Z;

pub fn type_badge(label: impl Into<SharedString>, bg: Rgba, theme: &FlowTheme, z: Z) -> Div {
    div()
        .px(z.px(6.0))
        .py(z.px(2.0))
        .rounded(z.px(4.0))
        .bg(bg)
        .text_size(z.text_xs())
        .font_weight(FontWeight::MEDIUM)
        .text_color(theme.node_background)
        .child(label.into())
}

pub fn mono_line(text: impl Into<SharedString>, theme: &FlowTheme, z: Z) -> Div {
    div()
        .text_size(z.text_xs())
        .font_weight(FontWeight::MEDIUM)
        .text_color(theme.body_muted_color)
        .overflow_hidden()
        .text_ellipsis()
        .child(text.into())
}

pub fn title_row(title: impl Into<SharedString>, theme: &FlowTheme, z: Z) -> Div {
    div()
        .text_size(z.text_sm())
        .font_weight(FontWeight::SEMIBOLD)
        .text_color(theme.node_title_text)
        .overflow_hidden()
        .text_ellipsis()
        .child(title.into())
}

pub fn collapse_chevron(collapsed: bool, theme: &FlowTheme, z: Z) -> Div {
    div()
        .text_size(z.text_xs())
        .text_color(theme.body_muted_color)
        .child(if collapsed { "▸" } else { "▾" })
}

pub fn node_shell(node: &ResolvedNode, theme: &FlowTheme, body: impl IntoElement) -> Div {
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
        .rounded(z.px(8.0))
        .overflow_hidden()
        .shadow_md()
        .border_1()
        .border_color(border)
        .bg(theme.node_background)
        .child(body)
}
