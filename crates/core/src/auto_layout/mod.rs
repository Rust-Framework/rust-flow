//! Automatic graph layout via [`dagre`] (React Flow / @dagrejs/dagre standard).

mod dagre_layout;
mod layered;
mod mermaid_dagre;
mod mermaid_layout;
mod mindmap_layout;
mod overlap;
mod options;

pub use layered::layout_graph;
pub use mermaid_dagre::{
    layout_graph_mermaid_v2, MermaidLayoutConfig,
};
pub use mermaid_layout::{detect_feedback_edges, layout_graph_mermaid};
pub use mindmap_layout::layout_mindmap;
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

    /// Mermaid flowchart layout via the from-scratch dagre pipeline
    /// ([`mermaid_dagre`]). Produces Mermaid-quality orthogonal edge routing
    /// without depending on the external `dagre` crate. Implements the full
    /// pipeline: DFS FAS cycle breaking, network-simplex ranking, barycenter
    /// crossing reduction, Brandes-Köpf coordinate assignment, and dummy-node
    /// edge routing with rect intersection.
    pub fn auto_layout_mermaid_v2(&mut self, options: &LayoutOptions) {
        self.layout_direction = options.direction;
        apply_flow_orientation(self, options.direction);
        sync_all_structured_nodes(self);
        layout_graph_mermaid_v2(self, options);
    }

    /// Mind map tree layout — root centered with bidirectional child distribution.
    /// Use this for `mindmap-1.0` documents instead of `auto_layout` / `auto_layout_mermaid`.
    pub fn auto_layout_mindmap(&mut self, options: &LayoutOptions) {
        self.layout_direction = options.direction;
        apply_flow_orientation(self, options.direction);
        sync_all_structured_nodes(self);
        layout_mindmap(self, options);
    }

    /// Convenience: default left-to-right layout.
    pub fn auto_layout_default(&mut self) {
        self.auto_layout(&LayoutOptions::default());
    }
}
