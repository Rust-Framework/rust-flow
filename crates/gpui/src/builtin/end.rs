//! End 节点：流程终点，仅 In 端口，红色药丸形。

use gpui::AnyElement;
use rust_agent_flow::{Node, NodeSchema, PortDirection, PortSide, PortSpec, SizeF};

use crate::node::{render_node_card, NodeVisual, NodeViewCtx, IFlowNode};

use super::common::{label_of, render_simple_panel};

/// End 节点：流程终点，仅 In 端口，红色药丸形。
pub struct EndNode {
    schema: NodeSchema,
}

impl Default for EndNode {
    fn default() -> Self {
        Self::new()
    }
}

impl EndNode {
    pub fn new() -> Self {
        Self {
            schema: NodeSchema::new("end", "End")
                .with_size(SizeF::new(120.0, 60.0))
                .with_port(PortSpec::new("in", PortDirection::In, PortSide::Auto)),
        }
    }
}

impl IFlowNode for EndNode {
    fn kind(&self) -> &str {
        "end"
    }

    fn get_view(&self, node: &Node, ctx: &mut NodeViewCtx) -> AnyElement {
        let visual = NodeVisual {
            label: label_of(node),
            desc: None,
            bg: gpui::rgb(0xef4444),
            border: gpui::rgb(0xdc2626),
            border_selected: gpui::rgb(0xb91c1c),
            text: gpui::rgb(0xffffff),
            subtext: gpui::rgb(0xfee2e2),
            show_in: true,
            show_out: false,
            in_color: gpui::rgb(0xffffff),
            out_color: gpui::rgb(0x22c55e),
            pill: true,
        };
        let w = node.size.w * ctx.scale;
        let h = node.size.h * ctx.scale;
        render_node_card(&visual, w, h, ctx.scale, false, ctx.selected)
    }

    fn get_panel(&self, node: &Node, _ctx: &mut NodeViewCtx) -> AnyElement {
        render_simple_panel(node, "End 节点")
    }

    fn schema(&self) -> &NodeSchema {
        &self.schema
    }
}
