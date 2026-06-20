use rust_agent_flow::{
    branch_collapsed, branch_node_size, parse_branch_items, BRANCH, BRANCH_HEADER, BRANCH_ROW,
    ResolvedNode, Size,
};
use gpui::*;

use crate::nodes::card::{panel_input_row, panel_section};
use crate::nodes::handles::render_port_handles;
use crate::nodes::real::chrome::{collapse_chevron, mono_line, node_shell, title_row, type_badge};
use crate::provider::{FlowPanelContext, IFlowNodeProvider};
use crate::theme::FlowTheme;
use crate::zoom::Z;

const BADGE: Rgba = Rgba {
    r: 0.55,
    g: 0.35,
    b: 0.85,
    a: 1.0,
};

pub struct RealBranchProvider;

impl IFlowNodeProvider for RealBranchProvider {
    fn node_type(&self) -> &'static str {
        BRANCH
    }

    fn default_size(&self) -> Size {
        branch_node_size(&serde_json::json!({ "branches": [] }))
    }

    fn render_node(&self, node: &ResolvedNode, theme: &FlowTheme) -> Div {
        let z = Z::new(node.zoom);
        let collapsed = branch_collapsed(&node.data);
        let branches = parse_branch_items(&node.data);

        let header = div()
            .flex()
            .flex_row()
            .items_center()
            .justify_between()
            .h(z.px(BRANCH_HEADER))
            .px(z.px(10.0))
            .border_b_1()
            .border_color(theme.node_border)
            .child(
                div()
                    .flex()
                    .flex_row()
                    .items_center()
                    .gap(z.px(6.0))
                    .child(collapse_chevron(collapsed, theme, z))
                    .child(title_row(&node.label, theme, z)),
            )
            .child(type_badge("分支", BADGE, theme, z));

        let mut body_children: Vec<Div> = vec![header];

        if !collapsed {
            for branch in &branches {
                body_children.push(
                    div()
                        .flex()
                        .flex_row()
                        .items_center()
                        .h(z.px(BRANCH_ROW))
                        .px(z.px(10.0))
                        .border_b_1()
                        .border_color(theme.node_border)
                        .child(
                            div()
                                .flex_1()
                                .flex()
                                .flex_col()
                                .gap(z.px(2.0))
                                .child(
                                    div()
                                        .text_size(z.text_xs())
                                        .font_weight(FontWeight::MEDIUM)
                                        .text_color(theme.node_title_text)
                                        .child(branch.label.clone()),
                                )
                                .child(mono_line(format!("if ({})", branch.condition), theme, z)),
                        ),
                );
            }

            body_children.push(
                div()
                    .px(z.px(10.0))
                    .py(z.px(4.0))
                    .text_size(z.text_xs())
                    .text_color(theme.body_muted_color)
                    .child("按 C 收起 / 展开"),
            );
        }

        let body = div().flex().flex_col().children(body_children);
        let card = node_shell(node, theme, body);
        let handles = render_port_handles(node, theme);
        div().size_full().child(card).children(handles)
    }

    fn render_panel(&self, ctx: &mut FlowPanelContext<'_>) -> Div {
        let node = ctx.graph.nodes.get(ctx.node_id).cloned();
        let Some(node) = node else {
            return div();
        };
        let collapsed = branch_collapsed(&node.data);
        let branches = parse_branch_items(&node.data);

        let mut panel = div()
            .flex()
            .flex_col()
            .child(panel_section("条件分支", ctx.theme))
            .child(panel_input_row("名称", node.label.clone(), ctx.theme))
            .child(panel_input_row("收起", collapsed.to_string(), ctx.theme));

        for b in branches {
            panel = panel.child(
                panel_input_row(
                    format!("{} · {}", b.label, b.id),
                    b.condition.clone(),
                    ctx.theme,
                ),
            );
        }

        panel.child(
            div()
                .text_xs()
                .text_color(ctx.theme.body_muted_color)
                .child("选中节点后按 C 切换收起"),
        )
    }
}
