use rust_agent_flow::{COMMON, COMMON_HEIGHT, COMMON_WIDTH, ResolvedNode, Size};
use gpui::*;

use crate::nodes::card::{panel_input_row, panel_section};
use crate::nodes::handles::render_port_handles;
use crate::nodes::real::chrome::{mono_line, node_shell, title_row, type_badge};
use crate::provider::{FlowPanelContext, IFlowNodeProvider};
use crate::theme::FlowTheme;
use crate::zoom::Z;

const BADGE: Rgba = Rgba {
    r: 0.35,
    g: 0.45,
    b: 0.55,
    a: 1.0,
};

pub struct RealCommonProvider;

impl IFlowNodeProvider for RealCommonProvider {
    fn node_type(&self) -> &'static str {
        COMMON
    }

    fn default_size(&self) -> Size {
        Size::new(COMMON_WIDTH, COMMON_HEIGHT)
    }

    fn render_node(&self, node: &ResolvedNode, theme: &FlowTheme) -> Div {
        let z = Z::new(node.zoom);
        let expression = node
            .data
            .get("expression")
            .and_then(|v| v.as_str())
            .unwrap_or("");

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
                    .gap(z.px(6.0))
                    .child(title_row(&node.label, theme, z))
                    .child(type_badge("动作", BADGE, theme, z)),
            )
            .child(
                div()
                    .px(z.px(8.0))
                    .py(z.px(6.0))
                    .rounded(z.px(4.0))
                    .bg(theme.canvas_background)
                    .border_1()
                    .border_color(theme.node_border)
                    .child(
                        mono_line(
                            if expression.is_empty() {
                                "// 业务逻辑".to_string()
                            } else {
                                expression.to_string()
                            },
                            theme,
                            z,
                        ),
                    ),
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
        let expression = node
            .data
            .get("expression")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string();

        div()
            .flex()
            .flex_col()
            .child(panel_section("业务动作", ctx.theme))
            .child(panel_input_row("名称", node.label.clone(), ctx.theme))
            .child(panel_input_row("逻辑", expression, ctx.theme))
    }
}
