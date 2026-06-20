use rust_agent_flow::{COMMON_WIDTH, ResolvedNode, Size, TRIGGER, TRIGGER_HEIGHT};
use gpui::*;

use crate::nodes::card::{panel_input_row, panel_section};
use crate::nodes::handles::render_port_handles;
use crate::nodes::real::chrome::{mono_line, node_shell, title_row, type_badge};
use crate::provider::{FlowPanelContext, IFlowNodeProvider};
use crate::theme::FlowTheme;
use crate::zoom::Z;

const BADGE: Rgba = Rgba {
    r: 0.20,
    g: 0.65,
    b: 0.45,
    a: 1.0,
};

pub struct RealTriggerProvider;

impl IFlowNodeProvider for RealTriggerProvider {
    fn node_type(&self) -> &'static str {
        TRIGGER
    }

    fn default_size(&self) -> Size {
        Size::new(COMMON_WIDTH, TRIGGER_HEIGHT)
    }

    fn render_node(&self, node: &ResolvedNode, theme: &FlowTheme) -> Div {
        let z = Z::new(node.zoom);
        let event = node
            .data
            .get("event")
            .and_then(|v| v.as_str())
            .unwrap_or("order.created");

        let body = div()
            .flex()
            .flex_row()
            .items_center()
            .justify_between()
            .p(z.px(10.0))
            .gap(z.px(8.0))
            .child(
                div()
                    .flex()
                    .flex_col()
                    .gap(z.px(2.0))
                    .child(title_row(&node.label, theme, z))
                    .child(mono_line(event, theme, z)),
            )
            .child(type_badge("触发", BADGE, theme, z));

        let card = node_shell(node, theme, body);
        let handles = render_port_handles(node, theme);
        div().size_full().child(card).children(handles)
    }

    fn render_panel(&self, ctx: &mut FlowPanelContext<'_>) -> Div {
        let node = ctx.graph.nodes.get(ctx.node_id).cloned();
        let Some(node) = node else {
            return div();
        };
        let event = node
            .data
            .get("event")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string();

        div()
            .flex()
            .flex_col()
            .child(panel_section("事件触发", ctx.theme))
            .child(panel_input_row("名称", node.label.clone(), ctx.theme))
            .child(panel_input_row("事件", event, ctx.theme))
    }
}
