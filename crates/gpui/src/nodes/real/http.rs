use rust_agent_flow::{COMMON_WIDTH, HTTP, HTTP_HEIGHT, ResolvedNode, Size};
use gpui::*;

use crate::nodes::card::{panel_input_row, panel_section};
use crate::nodes::handles::render_port_handles;
use crate::nodes::real::chrome::{mono_line, node_shell, title_row, type_badge};
use crate::provider::{FlowPanelContext, IFlowNodeProvider};
use crate::theme::FlowTheme;
use crate::zoom::Z;

const BADGE: Rgba = Rgba {
    r: 0.25,
    g: 0.50,
    b: 0.85,
    a: 1.0,
};

pub struct RealHttpProvider;

impl IFlowNodeProvider for RealHttpProvider {
    fn node_type(&self) -> &'static str {
        HTTP
    }

    fn default_size(&self) -> Size {
        Size::new(COMMON_WIDTH, HTTP_HEIGHT)
    }

    fn render_node(&self, node: &ResolvedNode, theme: &FlowTheme) -> Div {
        let z = Z::new(node.zoom);
        let method = node
            .data
            .get("method")
            .and_then(|v| v.as_str())
            .unwrap_or("GET")
            .to_string();
        let url = node
            .data
            .get("url")
            .and_then(|v| v.as_str())
            .unwrap_or("/api/...")
            .to_string();

        let body = div()
            .flex()
            .flex_col()
            .p(z.px(10.0))
            .gap(z.px(6.0))
            .child(
                div()
                    .flex()
                    .flex_row()
                    .items_center()
                    .justify_between()
                    .child(title_row(&node.label, theme, z))
                    .child(type_badge("HTTP", BADGE, theme, z)),
            )
            .child(
                div()
                    .flex()
                    .flex_row()
                    .gap(z.px(6.0))
                    .items_center()
                    .child(
                        div()
                            .px(z.px(6.0))
                            .py(z.px(2.0))
                            .rounded(z.px(4.0))
                            .bg(theme.canvas_background)
                            .border_1()
                            .border_color(theme.node_border)
                            .text_size(z.text_xs())
                            .font_weight(FontWeight::BOLD)
                            .text_color(theme.node_title_text)
                            .child(method.clone()),
                    )
                    .child(mono_line(url.clone(), theme, z)),
            );

        let card = node_shell(node, theme, body);
        let handles = render_port_handles(node, theme);
        div().size_full().child(card).children(handles)
    }

    fn render_panel(&self, ctx: &mut FlowPanelContext<'_>) -> Div {
        let node = ctx.graph.nodes.get(ctx.node_id).cloned();
        let Some(node) = node else {
            return div();
        };
        let method = node
            .data
            .get("method")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string();
        let url = node
            .data
            .get("url")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string();

        div()
            .flex()
            .flex_col()
            .child(panel_section("HTTP 请求", ctx.theme))
            .child(panel_input_row("名称", node.label.clone(), ctx.theme))
            .child(panel_input_row("方法", method, ctx.theme))
            .child(panel_input_row("URL", url, ctx.theme))
    }
}
