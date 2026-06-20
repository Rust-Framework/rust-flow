use rust_agent_flow::{BRANCH, ResolvedNode, Size};
use gpui::*;

use crate::nodes::card::{node_card, panel_input_row, panel_section};
use crate::nodes::handles::render_port_handles;
use crate::provider::{FlowPanelContext, IFlowNodeProvider};
use crate::theme::FlowTheme;

pub struct BranchNodeProvider;

impl IFlowNodeProvider for BranchNodeProvider {
    fn node_type(&self) -> &'static str {
        BRANCH
    }

    fn default_size(&self) -> Size {
        Size::new(180.0, 72.0)
    }

    fn render_node(&self, node: &ResolvedNode, theme: &FlowTheme) -> Div {
        let condition = node
            .data
            .get("condition")
            .and_then(|v| v.as_str())
            .unwrap_or("");

        let body = div()
            .flex()
            .flex_col()
            .gap(px(2.0))
            .child(
                div()
                    .text_sm()
                    .font_weight(FontWeight::SEMIBOLD)
                    .text_color(theme.node_title_text)
                    .child("If / Else"),
            )
            .child(
                div()
                    .text_xs()
                    .text_color(theme.body_muted_color)
                    .overflow_hidden()
                    .text_ellipsis()
                    .child(if condition.is_empty() {
                        "条件未设定".into()
                    } else {
                        format!("if ({condition})")
                    }),
            )
            .child(
                div()
                    .flex()
                    .flex_row()
                    .justify_between()
                    .text_xs()
                    .text_color(theme.port_label_color)
                    .child("true")
                    .child("false"),
            );

        let card = node_card(node, theme, body);
        let handles = render_port_handles(node, theme);

        div().size_full().child(card).children(handles)
    }

    fn render_panel(&self, ctx: &mut FlowPanelContext<'_>) -> Div {
        let node = ctx.graph.nodes.get(ctx.node_id).cloned();
        let Some(node) = node else {
            return div();
        };

        let condition = node
            .data
            .get("condition")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string();

        div()
            .flex()
            .flex_col()
            .child(panel_section("条件分支", ctx.theme))
            .child(panel_input_row("标签", node.label.clone(), ctx.theme))
            .child(panel_input_row("条件表达式", condition, ctx.theme))
            .child(
                div()
                    .text_xs()
                    .text_color(ctx.theme.body_muted_color)
                    .child("端口: in (<-) · true / false (->)"),
            )
    }
}
