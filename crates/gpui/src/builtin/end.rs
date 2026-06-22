//! End 节点：流程终点，仅 In 端口，红色药丸形。

use gpui::AnyElement;
use rust_agent_flow::{LayoutDirection, Node, NodeSchema, PortDirection, PortSide, PortSpec, SizeF};

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
                .with_size(SizeF::new(120.0, 35.0))
                .with_port(PortSpec::new("in", PortDirection::In, PortSide::Auto)),
        }
    }
}

impl IFlowNode for EndNode {
    fn kind(&self) -> &str {
        "end"
    }

    fn get_view(&self, node: &Node, ctx: &mut NodeViewCtx) -> AnyElement {
        let t = &ctx.theme;
        let visual = NodeVisual {
            label: label_of(node),
            desc: None,
            bg: t.end_bg,
            border: t.end_border,
            border_selected: t.end_border_selected,
            text: t.end_text,
            subtext: t.end_subtext,
            show_in: true,
            show_out: false,
            in_color: t.end_in_dot,
            in_ring: t.node_in_ring,
            out_color: t.node_out_dot,
            out_ring: t.node_out_ring,
            port_bg: t.port_bg,
            pill: true,
        };
        let w = node.size.w * ctx.scale;
        let h = node.size.h * ctx.scale;
        let vertical = ctx.layout == LayoutDirection::Vertical;
        render_node_card(&visual, w, h, ctx.scale, vertical, ctx.selected)
    }

    fn get_panel(&self, node: &Node, ctx: &mut NodeViewCtx) -> AnyElement {
        render_simple_panel(node, "End 节点", &ctx.theme)
    }

    fn schema(&self) -> &NodeSchema {
        &self.schema
    }
}
