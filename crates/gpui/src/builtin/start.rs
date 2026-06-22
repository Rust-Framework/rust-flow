//! Start 节点：流程起点，仅 Out 端口，绿色药丸形。

use gpui::AnyElement;
use rust_agent_flow::{LayoutDirection, Node, NodeSchema, PortDirection, PortSide, PortSpec, SizeF};

use crate::node::{render_node_card, NodeVisual, NodeViewCtx, IFlowNode};

use super::common::{label_of, render_simple_panel};

/// Start 节点：流程起点，仅 Out 端口，绿色药丸形。
pub struct StartNode {
    schema: NodeSchema,
}

impl Default for StartNode {
    fn default() -> Self {
        Self::new()
    }
}

impl StartNode {
    pub fn new() -> Self {
        Self {
            schema: NodeSchema::new("start", "Start")
                .with_size(SizeF::new(120.0, 35.0))
                .with_port(PortSpec::new("out", PortDirection::Out, PortSide::Auto)),
        }
    }
}

impl IFlowNode for StartNode {
    fn kind(&self) -> &str {
        "start"
    }

    fn get_view(&self, node: &Node, ctx: &mut NodeViewCtx) -> AnyElement {
        let t = &ctx.theme;
        let visual = NodeVisual {
            label: label_of(node),
            desc: None,
            bg: t.start_bg,
            border: t.start_border,
            border_selected: t.start_border_selected,
            text: t.start_text,
            subtext: t.start_subtext,
            show_in: false,
            show_out: true,
            in_color: t.node_in_dot,
            in_ring: t.node_in_ring,
            out_color: t.start_out_dot,
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
        render_simple_panel(node, "Start 节点", &ctx.theme)
    }

    fn schema(&self) -> &NodeSchema {
        &self.schema
    }
}
