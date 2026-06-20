use rust_agent_flow::{handle_dot_origin, ResolvedNode, HANDLE_R};
use gpui::*;

use crate::theme::FlowTheme;
use crate::zoom::Z;

/// Port handles centered on each border edge (half outside), shared by all providers.
pub fn render_port_handles(node: &ResolvedNode, theme: &FlowTheme) -> Vec<Div> {
    let z = Z::new(node.zoom);
    let handle_r = HANDLE_R * z.raw();
    let handle_d = handle_r * 2.0;
    let mut handles = Vec::new();

    for (_, pid) in node.inputs.iter().chain(node.outputs.iter()) {
        let Some(center) = node.port_local.get(pid) else {
            continue;
        };
        let is_input = node.inputs.iter().any(|(_, id)| id == pid);
        let color = if is_input {
            theme.port_color_input
        } else {
            theme.port_color_output
        };
        let origin = handle_dot_origin(*center, handle_r);
        handles.push(
            div()
                .absolute()
                .left(px(origin.x))
                .top(px(origin.y))
                .w(px(handle_d))
                .h(px(handle_d))
                .rounded_full()
                .bg(color)
                .border(z.px(2.0))
                .border_color(theme.node_background),
        );
    }

    handles
}
