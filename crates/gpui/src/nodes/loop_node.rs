use rust_agent_flow::{LOOP, ResolvedNode, Size};
use gpui::*;

use crate::nodes::card::{node_card, panel_input_row, panel_section};
use crate::nodes::handles::render_port_handles;
use crate::provider::{FlowPanelContext, IFlowNodeProvider};
use crate::theme::FlowTheme;

pub struct LoopNodeProvider;

impl IFlowNodeProvider for LoopNodeProvider {
    fn node_type(&self) -> &'static str {
        LOOP
    }

    fn default_size(&self) -> Size {
        Size::new(200.0, 64.0)
    }

    fn render_node(&self, node: &ResolvedNode, theme: &FlowTheme) -> Div {
        let iterator = node
            .data
            .get("iterator")
            .and_then(|v| v.as_str())
            .unwrap_or("item");
        let collection = node
            .data
            .get("collection")
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
                    .child(node.label.clone()),
            )
            .child(
                div()
                    .text_xs()
                    .text_color(theme.body_muted_color)
                    .overflow_hidden()
                    .text_ellipsis()
                    .child(if collection.is_empty() {
                        format!("for {iterator} in ...")
                    } else {
                        format!("for {iterator} in {collection}")
                    }),
            )
            .child(
                div()
                    .text_xs()
                    .text_color(theme.port_label_color)
                    .child("body ↓"),
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

        let iterator = node
            .data
            .get("iterator")
            .and_then(|v| v.as_str())
            .unwrap_or("item")
            .to_string();
        let collection = node
            .data
            .get("collection")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string();
        let max_iter = node
            .data
            .get("max_iterations")
            .and_then(|v| v.as_u64())
            .unwrap_or(1000)
            .to_string();

        div()
            .flex()
            .flex_col()
            .child(panel_section("循环遍历", ctx.theme))
            .child(panel_input_row("标签", node.label.clone(), ctx.theme))
            .child(panel_input_row("迭代变量", iterator, ctx.theme))
            .child(panel_input_row("集合", collection, ctx.theme))
            .child(panel_input_row("最大迭代次数", max_iter, ctx.theme))
            .child(
                div()
                    .text_xs()
                    .text_color(ctx.theme.body_muted_color)
                    .child("端口: in (<-) · out (->) · body (↓)"),
            )
    }
}
