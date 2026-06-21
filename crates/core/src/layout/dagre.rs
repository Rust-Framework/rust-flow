//! Dagre layout engine (Sugiyama-style layered layout).
//!
//! Wraps the `mermaid-dagre` crate (imported as `dagre_rust`).

use super::{LayoutDirection, LayoutEngine, LayoutResult};
use crate::geometry::PointF;
use crate::graph::FlowGraph;

/// Dagre-based layout engine.
pub struct DagreLayout;

impl DagreLayout {
    pub fn new() -> Self {
        Self
    }
}

impl Default for DagreLayout {
    fn default() -> Self {
        Self::new()
    }
}

impl LayoutEngine for DagreLayout {
    fn layout(&self, graph: &FlowGraph, direction: LayoutDirection) -> LayoutResult {
        use dagre_rust::{
            layout, EdgeLabel, GraphLabel, GraphOptions, LayoutGraph, NodeLabel, RankDir,
        };

        let rankdir = match direction {
            LayoutDirection::Vertical => RankDir::TB,
            LayoutDirection::Horizontal => RankDir::LR,
        };

        let mut g = LayoutGraph::with_options(&GraphOptions {
            directed: true,
            multigraph: false,
            compound: false,
        });
        g.set_graph(GraphLabel {
            rankdir,
            nodesep: 50.0,
            ranksep: 60.0,
            ..Default::default()
        });

        // Map slotmap NodeId → dagre string id (use a stable index).
        let mut id_map: std::collections::HashMap<crate::graph::NodeId, String> =
            std::collections::HashMap::new();
        for (i, node) in graph.nodes().enumerate() {
            let key = i.to_string();
            id_map.insert(node.id, key.clone());
            g.set_node(
                &key,
                Some(NodeLabel {
                    width: node.size.w as f64,
                    height: node.size.h as f64,
                    ..Default::default()
                }),
            );
        }
        for edge in graph.edges() {
            if let (Some(s), Some(t)) = (id_map.get(&edge.source), id_map.get(&edge.target)) {
                g.set_edge(s, t, Some(EdgeLabel::default()), None);
            }
        }

        layout(&mut g);

        let mut positions = std::collections::HashMap::new();
        for (node_id, key) in &id_map {
            if let Some(label) = g.node(key) {
                if let (Some(x), Some(y)) = (label.x, label.y) {
                    // dagre returns centre coordinates; convert to top-left.
                    positions.insert(
                        *node_id,
                        PointF::new(
                            (x - label.width * 0.5) as f32,
                            (y - label.height * 0.5) as f32,
                        ),
                    );
                }
            }
        }
        LayoutResult { positions }
    }
}
