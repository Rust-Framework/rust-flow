//! Dagre layout engine (Sugiyama-style layered layout).
//!
//! Wraps the `dagre` crate — a complete Rust port of dagre.js with 20/20
//! cross-validation against the reference implementation. This is the same
//! algorithm family used by ReactFlow's official dagre examples.
//!
//! ## Module structure
//!
//! | Sub-module | Responsibility | Stability |
//! |------------|---------------|-----------|
//! | [`mod`] (this file) | Dagre graph construction, layout call, post-processing orchestration | Core skeleton |
//! | [`branch`] | Branch target reordering (`if_N`/`else` port order) | Stable algorithm |
//! | [`linear`] | Linear chain cross-axis alignment (Kahn topological sort) | Stable algorithm |
//! | [`loop_layout`] | Loop-specific post-processing (body positioning, back-edge space, done/body alignment) | Volatile — coupled to Loop node port semantics |
//! | [`tests`] | Unit tests | Stable |

use super::{LayoutDirection, LayoutEngine, LayoutResult};
use crate::geometry::PointF;
use crate::graph::FlowGraph;
use dagre::graph::{Graph, GraphOptions};
use dagre::{layout, EdgeLabel, LayoutOptions, NodeLabel, RankDir, Ranker};

mod branch;
mod linear;
mod loop_layout;

#[cfg(test)]
mod tests;

use branch::reorder_branch_targets;
use linear::align_linear_chain;
use loop_layout::{
    align_loop_body_target, align_loop_done_target, align_loop_in_sources,
    align_post_done_chain, reserve_loop_back_edge_space,
};

/// Dagre-based layout engine.
pub struct DagreLayout {
    nodesep: f64,
    ranksep: f64,
}

impl DagreLayout {
    pub fn new() -> Self {
        Self {
            nodesep: 40.0,
            ranksep: 80.0,
        }
    }

    /// Customize the node separation (gap between sibling nodes in the same rank).
    pub fn with_nodesep(mut self, sep: f64) -> Self {
        self.nodesep = sep;
        self
    }

    /// Customize the rank separation (gap between consecutive ranks/layers).
    pub fn with_ranksep(mut self, sep: f64) -> Self {
        self.ranksep = sep;
        self
    }
}

impl Default for DagreLayout {
    fn default() -> Self {
        Self::new()
    }
}

impl LayoutEngine for DagreLayout {
    fn layout(&self, graph: &FlowGraph, direction: LayoutDirection) -> LayoutResult {
        let rankdir = match direction {
            LayoutDirection::Vertical => RankDir::TB,
            LayoutDirection::Horizontal => RankDir::LR,
        };

        let mut g = Graph::<NodeLabel, EdgeLabel>::with_options(GraphOptions {
            directed: true,
            multigraph: false,
            compound: false,
        });

        let opts = LayoutOptions {
            rankdir,
            nodesep: self.nodesep,
            ranksep: self.ranksep,
            edgesep: 30.0,
            marginx: 40.0,
            marginy: 40.0,
            ranker: Ranker::NetworkSimplex,
            ..Default::default()
        };

        // Map slotmap NodeId → dagre string id (use a stable index).
        let mut id_map: std::collections::HashMap<crate::graph::NodeId, String> =
            std::collections::HashMap::new();
        for (i, node) in graph.nodes().enumerate() {
            let key = i.to_string();
            id_map.insert(node.id, key.clone());
            let label = NodeLabel {
                width: node.size.w as f64,
                height: node.size.h as f64,
                ..Default::default()
            };
            g.set_node(key, Some(label));
        }
        for edge in graph.edges() {
            if let (Some(s), Some(t)) = (id_map.get(&edge.source), id_map.get(&edge.target)) {
                // Assign edge weights and minlen to guide dagre's layout:
                // - `loop_body` and `done` edges get HIGH weight → dagre avoids
                //   reversing them, keeping the body group and exit node in the
                //   forward rank.
                // - `loop_in` back-edge gets LOW weight → dagre prefers to
                //   reverse it, breaking the cycle without disturbing the
                //   main flow.
                // - `done` edge gets minlen=2 → forces the done target to rank
                //   2 (below the body group at rank 1), preventing it from
                //   being placed inside the back-edge U-shape.
                let (weight, minlen) = match edge.source_port.as_deref() {
                    Some("loop_body") => (100, 1),
                    Some("done") => (100, 2),
                    Some("loop_in") => (1, 1),
                    _ => (1, 1),
                };
                let label = EdgeLabel {
                    weight,
                    minlen,
                    ..Default::default()
                };
                g.set_edge(s.clone(), t.clone(), Some(label), None);
            }
        }

        layout(&mut g, Some(opts));

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

        // ── Post-processing pipeline (order-sensitive) ──
        //
        // 1. Branch target reordering — match exit port order (if_N/else).
        // 2. Linear chain alignment — straighten main flow (Kahn topo sort).
        // 3. Loop back-edge space reservation — shift nodes below body group.
        // 4. Loop `in` source alignment — move Loop to median of sources.
        //    (must run before 5/6 so they adjust to the new Loop position)
        // 5. Loop `done` target alignment — straighten done edge.
        // 6. Loop `loop_body` target alignment — position body group right of Loop.
        // 7. Post-done chain alignment — straighten forward chain after done target.
        reorder_branch_targets(graph, &mut positions, direction);
        align_linear_chain(graph, &mut positions, direction);
        reserve_loop_back_edge_space(graph, &mut positions, direction);
        align_loop_in_sources(graph, &mut positions, direction);
        align_loop_done_target(graph, &mut positions, direction);
        align_loop_body_target(graph, &mut positions, direction);
        align_post_done_chain(graph, &mut positions, direction);

        LayoutResult { positions }
    }
}
