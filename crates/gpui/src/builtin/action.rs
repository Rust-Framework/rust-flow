//! Action 节点：顺序执行步骤，In + Out 端口，白色卡片。

use gpui::AnyElement;
use rust_agent_flow::{Node, NodeSchema, PortDirection, PortSide, PortSpec, SizeF};

use crate::node::{render_node_card, NodeVisual, NodeViewCtx, IFlowNode};

use super::common::{desc_of, label_of, render_simple_panel};

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
                .with_size(SizeF::new(180.0, 80.0))
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
        let visual = NodeVisual {
            label: label_of(node),
            desc: desc_of(node),
            bg: gpui::rgb(0xffffff),
            border: gpui::rgb(0xe2e8f0),
            border_selected: gpui::rgb(0x6366f1),
            text: gpui::rgb(0x1e293b),
            subtext: gpui::rgb(0x64748b),
            show_in: true,
            show_out: true,
            in_color: gpui::rgb(0x6366f1),
            out_color: gpui::rgb(0x22c55e),
            pill: false,
        };
        let w = node.size.w * ctx.scale;
        let h = node.size.h * ctx.scale;
        render_node_card(&visual, w, h, ctx.scale, false, ctx.selected)
    }

    fn get_panel(&self, node: &Node, _ctx: &mut NodeViewCtx) -> AnyElement {
        render_simple_panel(node, "Action 节点")
    }

    fn schema(&self) -> &NodeSchema {
        &self.schema
    }
}
