use rust_agent_flow::{
    LOOP, LOOP_BODY_ZONE, LOOP_FOOTER, LOOP_HEADER, LOOP_HEIGHT, LOOP_WIDTH, ResolvedNode, Size,
};
use gpui::*;

use crate::nodes::card::{panel_input_row, panel_section};
use crate::nodes::handles::render_port_handles;
use crate::nodes::real::chrome::{mono_line, node_shell, title_row, type_badge};
use crate::provider::{FlowPanelContext, IFlowNodeProvider};
use crate::theme::FlowTheme;
use crate::zoom::Z;

const BADGE: Rgba = Rgba {
    r: 0.90,
    g: 0.55,
    b: 0.20,
    a: 1.0,
};

const LOOP_LINE: Rgba = Rgba {
    r: 0.90,
    g: 0.60,
    b: 0.25,
    a: 0.85,
};

pub struct RealLoopProvider;

impl IFlowNodeProvider for RealLoopProvider {
    fn node_type(&self) -> &'static str {
        LOOP
    }

    fn default_size(&self) -> Size {
        Size::new(LOOP_WIDTH, LOOP_HEIGHT)
    }

    fn render_node(&self, node: &ResolvedNode, theme: &FlowTheme) -> Div {
        let z = Z::new(node.zoom);
        let iterator = node
            .data
            .get("iterator")
            .and_then(|v| v.as_str())
            .unwrap_or("item");
        let collection = node
            .data
            .get("collection")
            .and_then(|v| v.as_str())
            .unwrap_or("items");

        let header = div()
            .flex()
            .flex_row()
            .items_center()
            .justify_between()
            .h(z.px(LOOP_HEADER))
            .px(z.px(10.0))
            .border_b_1()
            .border_color(theme.node_border)
            .child(title_row(&node.label, theme, z))
            .child(type_badge("循环", BADGE, theme, z));

        let subtitle = mono_line(format!("for {iterator} in {collection}"), theme, z);

        let body_zone = div()
            .relative()
            .h(z.px(LOOP_BODY_ZONE))
            .px(z.px(10.0))
            .py(z.px(8.0))
            .child(
                div()
                    .absolute()
                    .left(z.px(8.0))
                    .bottom(z.px(10.0))
                    .w(z.px(16.0))
                    .h(z.px(LOOP_BODY_ZONE - 24.0))
                    .border_l(z.px(2.0))
                    .border_b(z.px(2.0))
                    .border_color(LOOP_LINE)
                    .rounded_tl(z.px(6.0)),
            )
            .child(
                div()
                    .absolute()
                    .right(z.px(8.0))
                    .top(z.px(4.0))
                    .w(z.px(16.0))
                    .h(z.px(LOOP_BODY_ZONE - 8.0))
                    .border_r(z.px(2.0))
                    .border_b(z.px(2.0))
                    .border_color(LOOP_LINE)
                    .rounded_br(z.px(6.0)),
            )
            .child(
                div()
                    .size_full()
                    .rounded(z.px(6.0))
                    .border_1()
                    .border_dashed()
                    .border_color(theme.node_border)
                    .bg(theme.canvas_background)
                    .flex()
                    .flex_col()
                    .items_center()
                    .justify_center()
                    .gap(z.px(4.0))
                    .child(
                        div()
                            .text_size(z.text_xs())
                            .text_color(theme.body_muted_color)
                            .child("循环体 · 从 body 端口编排"),
                    )
                    .child(
                        div()
                            .text_size(z.text_xs())
                            .text_color(theme.port_label_color)
                            .child("末端接 continue 回环"),
                    ),
            );

        let footer = div()
            .flex()
            .flex_row()
            .items_center()
            .justify_between()
            .h(z.px(LOOP_FOOTER))
            .px(z.px(10.0))
            .text_size(z.text_xs())
            .text_color(theme.port_label_color)
            .child("continue ← 回环")
            .child("body ↓")
            .child("out → 主线");

        let inner = div()
            .flex()
            .flex_col()
            .child(header)
            .child(div().px(z.px(10.0)).pt(z.px(6.0)).child(subtitle))
            .child(body_zone)
            .child(footer);

        let card = node_shell(node, theme, inner);
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
            .child(panel_input_row("名称", node.label.clone(), ctx.theme))
            .child(panel_input_row("迭代变量", iterator, ctx.theme))
            .child(panel_input_row("集合", collection, ctx.theme))
            .child(panel_input_row("最大次数", max_iter, ctx.theme))
            .child(
                div()
                    .text_xs()
                    .text_color(ctx.theme.body_muted_color)
                    .child("body → 业务节点 · 末端接 continue 回环"),
            )
    }
}
