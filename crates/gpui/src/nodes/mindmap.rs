//! Flat rectangular node chrome for read-only mind-map / flowchart view (Mermaid-style).

use rust_agent_flow::{COMMON, mindmap_node_size, ResolvedNode, Size};
use gpui::*;

use crate::provider::{FlowPanelContext, IFlowNodeProvider};
use crate::theme::FlowTheme;
use crate::zoom::Z;

const NODE_BG: Rgba = Rgba {
    r: 0.94,
    g: 0.94,
    b: 0.94,
    a: 1.0,
};

pub struct MindMapNodeProvider;

impl IFlowNodeProvider for MindMapNodeProvider {
    fn node_type(&self) -> &'static str {
        COMMON
    }

    fn default_size(&self) -> Size {
        mindmap_node_size("")
    }

    fn render_node(&self, node: &ResolvedNode, theme: &FlowTheme) -> Div {
        let z = Z::new(node.zoom);
        div()
            .size_full()
            .flex()
            .items_center()
            .justify_center()
            .px(z.px(12.0))
            .py(z.px(8.0))
            .bg(NODE_BG)
            .border(z.px(1.0))
            .border_color(theme.node_border)
            .child(
                div()
                    .text_size(z.text_sm())
                    .text_color(theme.node_title_text)
                    .text_center()
                    .line_height(z.px(18.0))
                    .child(node.label.clone()),
            )
    }

    fn render_panel(&self, _ctx: &mut FlowPanelContext<'_>) -> Div {
        div()
    }
}
