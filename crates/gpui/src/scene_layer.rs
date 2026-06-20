//! Viewport layering for the flow editor and mind-map view.
//!
//! Layer order: grid canvas -> node divs -> edge canvas -> edge label divs.

use rust_agent_flow::SceneFrame;
use gpui::*;

use crate::provider::FlowNodeRegistry;
use crate::render::{
    paint_dot_grid, paint_edge_path, paint_edge_with_decorations, render_node_shell,
};
use crate::theme::FlowTheme;

#[derive(Debug, Clone, Copy)]
pub struct ViewportStyle {
    pub show_grid: bool,
    pub show_arrows: bool,
    pub show_edge_labels: bool,
}

impl Default for ViewportStyle {
    fn default() -> Self {
        Self {
            show_grid: true,
            show_arrows: false,
            show_edge_labels: false,
        }
    }
}

impl ViewportStyle {
    pub fn editor() -> Self {
        Self::default()
    }

    pub fn mindmap() -> Self {
        Self {
            show_grid: false,
            show_arrows: true,
            show_edge_labels: true,
        }
    }
}

pub fn render_viewport(
    frame: SceneFrame,
    theme: FlowTheme,
    registry: &FlowNodeRegistry,
    on_bounds: impl 'static + Fn(Bounds<Pixels>, &mut App),
) -> Div {
    render_viewport_styled(frame, theme, registry, ViewportStyle::editor(), on_bounds)
}

pub fn render_viewport_styled(
    frame: SceneFrame,
    theme: FlowTheme,
    registry: &FlowNodeRegistry,
    style: ViewportStyle,
    on_bounds: impl 'static + Fn(Bounds<Pixels>, &mut App),
) -> Div {
    let edges = frame.edges.clone();
    let preview = frame.preview.clone();
    let zoom = frame.zoom;
    let theme_grid = theme.clone();
    let theme_edges = theme.clone();
    let theme_labels = theme.clone();

    let cards: Vec<Div> = frame
        .nodes
        .iter()
        .map(|node| render_node_shell(node, registry, &theme))
        .collect();

    let label_divs: Vec<Div> = if style.show_edge_labels {
        frame
            .edges
            .iter()
            .filter_map(|edge| {
                let label = edge.label.as_ref().filter(|l| !l.is_empty())?;
                let (mid_x, mid_y) = if let Some(lp) = edge.label_pos {
                    (lp.x, lp.y)
                } else {
                    ((edge.from.x + edge.to.x) * 0.5, (edge.from.y + edge.to.y) * 0.5)
                };
                let pad = label.chars().count() as f32 * 3.5 * zoom + 6.0 * zoom;
                Some(
                    div()
                        .absolute()
                        .left(px(mid_x - pad))
                        .top(px(mid_y - 8.0 * zoom))
                        .px(px(4.0 * zoom))
                        .py(px(2.0 * zoom))
                        .rounded(px(2.0 * zoom))
                        .bg(theme_labels.node_background)
                        .border_1()
                        .border_color(theme_labels.node_border)
                        .text_size(px(12.0 * zoom))
                        .text_color(theme_labels.node_title_text)
                        .child(label.clone()),
                )
            })
            .collect()
    } else {
        Vec::new()
    };

    let grid_layer = canvas(
        move |bounds, _, cx| {
            on_bounds(bounds, cx);
            bounds
        },
        move |bounds, _, window, _| {
            if style.show_grid {
                paint_dot_grid(bounds, &theme_grid, window, zoom);
            }
        },
    );

    let edge_layer = canvas(
        |bounds, _, _| bounds,
        move |bounds, _, window, _| {
            for edge in &edges {
                if style.show_arrows {
                    paint_edge_with_decorations(edge, theme_edges.edge_color, bounds, window, zoom);
                } else {
                    paint_edge_path(&edge.path, theme_edges.edge_color, bounds, window, zoom);
                }
            }
            if let Some(p) = &preview {
                paint_edge_path(
                    &p.path,
                    theme_edges.connection_preview,
                    bounds,
                    window,
                    zoom,
                );
            }
        },
    );

    div()
        .relative()
        .size_full()
        .overflow_hidden()
        .child(grid_layer)
        .children(cards)
        .child(edge_layer)
        .children(label_divs)
}
