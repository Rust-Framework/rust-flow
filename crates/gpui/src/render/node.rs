use rust_agent_flow::ResolvedNode;
use gpui::*;

use crate::provider::FlowNodeRegistry;
use crate::theme::FlowTheme;
use crate::zoom::Z;

/// Framework shell: absolute positioning only — no card chrome or colors.
pub fn render_node_shell(
    node: &ResolvedNode,
    registry: &FlowNodeRegistry,
    theme: &FlowTheme,
) -> Div {
    let z = Z::new(node.zoom);
    let provider = registry.get(&node.node_type);
    let content = provider.render_node(node, theme);
    let w = node.screen_size.width;
    let h = node.screen_size.height;

    div()
        .absolute()
        .left(px(node.screen_pos.x))
        .top(px(node.screen_pos.y))
        .w(px(w))
        .h(px(h))
        .cursor_default()
        .child(z.cascade_text(div().size_full().child(content)))
}
