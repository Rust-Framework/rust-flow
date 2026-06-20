//! Automatic graph layout via [`dagre`] (React Flow / @dagrejs/dagre standard).

mod dagre_layout;
mod layered;
mod mermaid_layout;
mod overlap;
mod options;

pub use layered::layout_graph;
pub use mermaid_layout::{detect_feedback_edges, layout_graph_mermaid};
pub use overlap::{compute_edge_route_offsets, resolve_graph_overlaps, EdgeRouteOffset};
pub use options::{LayoutDirection, LayoutOptions};

use crate::FlowGraph;
use crate::node_sync::sync_all_structured_nodes;
use crate::orientation::apply_flow_orientation;

impl FlowGraph {
    /// Apply layered auto-layout to all nodes (Dagre / React Flow standard).
    pub fn auto_layout(&mut self, options: &LayoutOptions) {
        self.layout_direction = options.direction;
        apply_flow_orientation(self, options.direction);
        sync_all_structured_nodes(self);
        self.dagre_edge_routes.clear();
        dagre_layout::layout_graph_dagre(self, options);
    }

    /// Mermaid flowchart / mind-map layout (centered ranks, feedback minlen).
    pub fn auto_layout_mermaid(&mut self, options: &LayoutOptions) {
        self.layout_direction = options.direction;
        apply_flow_orientation(self, options.direction);
        sync_all_structured_nodes(self);
        layout_graph_mermaid(self, options);
    }

    /// Convenience: default left-to-right layout.
    pub fn auto_layout_default(&mut self) {
        self.auto_layout(&LayoutOptions::default());
    }
}
