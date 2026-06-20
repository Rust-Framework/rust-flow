use rust_agent_flow::{COMMON, ResolvedNode, Size, VISUAL_HEIGHT};
use gpui::*;

use crate::nodes::card::{node_card, node_title, panel_input_row, panel_section};
use crate::nodes::handles::render_port_handles;
use crate::provider::{FlowPanelContext, IFlowNodeProvider};
use crate::theme::FlowTheme;

pub struct CommonNodeProvider;

impl IFlowNodeProvider for CommonNodeProvider {
    fn node_type(&self) -> &'static str {
        COMMON
    }

    fn default_size(&self) -> Size {
        Size::new(200.0, VISUAL_HEIGHT)
    }

    fn render_node(&self, node: &ResolvedNode, theme: &FlowTheme) -> Div {
        let body = node_title(node, theme);
        let card = node_card(node, theme, body);
        let handles = render_port_handles(node, theme);

        div().size_full().child(card).children(handles)
    }

    fn render_panel(&self, ctx: &mut FlowPanelContext<'_>) -> Div {
        let node = ctx.graph.nodes.get(ctx.node_id).cloned();
        let Some(node) = node else {
            return div();
        };

        let expression = node
            .data
            .get("expression")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string();

        div()
            .flex()
            .flex_col()
            .child(panel_section("常用节点", ctx.theme))
            .child(panel_input_row("标签", node.label.clone(), ctx.theme))
            .child(panel_input_row("表达式", expression, ctx.theme))
            .child(
                div()
                    .text_xs()
                    .text_color(ctx.theme.body_muted_color)
                    .child("端口: in (<-) · out (->)"),
            )
    }
}
