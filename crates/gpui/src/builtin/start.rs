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
        let visual = NodeVisual {
            label: label_of(node),
            desc: None,
            bg: gpui::rgb(0x22c55e),
            border: gpui::rgb(0x16a34a),
            border_selected: gpui::rgb(0x15803d),
            text: gpui::rgb(0xffffff),
            subtext: gpui::rgb(0xdcfce7),
            show_in: false,
            show_out: true,
            in_color: gpui::rgb(0x6366f1),
            out_color: gpui::rgb(0xffffff),
            pill: true,
        };
        let w = node.size.w * ctx.scale;
        let h = node.size.h * ctx.scale;
        let vertical = ctx.layout == LayoutDirection::Vertical;
        render_node_card(&visual, w, h, ctx.scale, vertical, ctx.selected)
    }

    fn get_panel(&self, node: &Node, _ctx: &mut NodeViewCtx) -> AnyElement {
        render_simple_panel(node, "Start 节点")
    }

    fn schema(&self) -> &NodeSchema {
        &self.schema
    }
}
