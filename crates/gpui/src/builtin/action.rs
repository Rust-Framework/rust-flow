//! Action 节点：顺序执行步骤，In + Out 端口，白色卡片。

use gpui::{div, px, AnyElement, IntoElement, ParentElement, Styled};
use rust_agent_flow::{LayoutDirection, Node, NodeSchema, PortDirection, PortSide, PortSpec, SizeF};

use crate::node::{render_node_card, NodeVisual, NodeViewCtx, IFlowNode};

use super::common::{desc_of, label_of, render_delete_button, render_simple_panel};

/// Action 节点：顺序执行步骤，In + Out 端口，白色卡片。
pub struct ActionNode {
    schema: NodeSchema,
}

impl Default for ActionNode {
    fn default() -> Self {
        Self::new()
    }
}

impl ActionNode {
    pub fn new() -> Self {
        Self {
            schema: NodeSchema::new("action", "Action")
                .with_size(SizeF::new(180.0, 35.0))
                .with_port(PortSpec::new("in", PortDirection::In, PortSide::Auto))
                .with_port(PortSpec::new("out", PortDirection::Out, PortSide::Auto)),
        }
    }
}

impl IFlowNode for ActionNode {
    fn kind(&self) -> &str {
        "action"
    }

    fn get_view(&self, node: &Node, ctx: &mut NodeViewCtx) -> AnyElement {
        let t = &ctx.theme;
        let visual = NodeVisual {
            label: label_of(node),
            desc: desc_of(node),
            bg: t.node_bg,
            border: t.node_border,
            border_selected: t.node_border_selected,
            text: t.node_text,
            subtext: t.node_subtext,
            show_in: true,
            show_out: true,
            in_color: t.node_in_dot,
            in_ring: t.node_in_ring,
            out_color: t.node_out_dot,
            out_ring: t.node_out_ring,
            port_bg: t.port_bg,
            pill: false,
        };
        let w = node.size.w * ctx.scale;
        let h = node.size.h * ctx.scale;
        let vertical = ctx.layout == LayoutDirection::Vertical;
        let card = render_node_card(&visual, w, h, ctx.scale, vertical, ctx.selected);

        // hover 时叠加删除按钮
        if ctx.hovered {
            let mut wrapper = div().relative().w(px(w)).h(px(h));
            wrapper = wrapper.child(card);
            wrapper = wrapper.child(render_delete_button(node.size.w, ctx.scale, &ctx.theme));
            wrapper.into_any_element()
        } else {
            card
        }
    }

    fn get_panel(&self, node: &Node, ctx: &mut NodeViewCtx) -> AnyElement {
        render_simple_panel(node, "Action 节点", &ctx.theme)
    }

    fn schema(&self) -> &NodeSchema {
        &self.schema
    }
}
