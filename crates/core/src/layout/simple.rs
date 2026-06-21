//! Simple layered layout engine (no external dependencies).
//!
//! Implements a basic Sugiyama-style layered layout:
//! 1. Assign nodes to layers via longest-path from sources.
//! 2. Arrange nodes within each layer, centered around the layer axis.
//! 3. Compute positions based on `LayoutDirection`.

use super::{LayoutDirection, LayoutEngine, LayoutResult};
use crate::geometry::PointF;
use crate::graph::{FlowGraph, NodeId};
use std::collections::{HashMap, VecDeque};

/// Simple layered layout engine (always available, no features required).
pub struct SimpleLayout;

impl SimpleLayout {
    pub fn new() -> Self {
        Self
    }
}

impl Default for SimpleLayout {
    fn default() -> Self {
        Self::new()
    }
}

impl LayoutEngine for SimpleLayout {
    fn layout(&self, graph: &FlowGraph, direction: LayoutDirection) -> LayoutResult {
        let nodes: Vec<_> = graph.nodes().collect();
        if nodes.is_empty() {
            return LayoutResult::default();
        }

        // Build adjacency + in-degree maps.
        let mut out_edges: HashMap<NodeId, Vec<NodeId>> = HashMap::new();
        let mut in_degree: HashMap<NodeId, usize> = HashMap::new();
        for node in &nodes {
            out_edges.entry(node.id).or_default();
            in_degree.entry(node.id).or_insert(0);
        }
        for edge in graph.edges() {
            out_edges.entry(edge.source).or_default().push(edge.target);
            *in_degree.entry(edge.target).or_insert(0) += 1;
        }

        // Longest-path layer assignment (Kahn-style BFS).
        let mut layer: HashMap<NodeId, usize> = HashMap::new();
        let mut remaining = in_degree.clone();
        let mut queue: VecDeque<NodeId> = VecDeque::new();
        for node in &nodes {
            if in_degree.get(&node.id).copied().unwrap_or(0) == 0 {
                layer.insert(node.id, 0);
                queue.push_back(node.id);
            }
        }
        // Cycle fallback: pick first node if no sources.
        if queue.is_empty() {
            if let Some(first) = nodes.first() {
                layer.insert(first.id, 0);
                queue.push_back(first.id);
            }
        }
        while let Some(nid) = queue.pop_front() {
            let cur = layer.get(&nid).copied().unwrap_or(0);
            if let Some(targets) = out_edges.get(&nid) {
                for &t in targets {
                    let nl = cur + 1;
                    if nl > layer.get(&t).copied().unwrap_or(0) {
                        layer.insert(t, nl);
                    }
                    let d = remaining.get_mut(&t).unwrap();
                    *d = d.saturating_sub(1);
                    if *d == 0 {
                        queue.push_back(t);
                    }
                }
            }
        }
        // Any leftover (cycles) → layer 0.
        for node in &nodes {
            layer.entry(node.id).or_insert(0);
        }

        // Group nodes by layer.
        let max_layer = layer.values().copied().max().unwrap_or(0);
        let mut layers: Vec<Vec<NodeId>> = vec![Vec::new(); max_layer + 1];
        for node in &nodes {
            let l = layer[&node.id];
            layers[l].push(node.id);
        }

        // Geometry constants (match default node size in graph/mod.rs).
        let node_w = 180.0f32;
        let node_h = 80.0f32;
        let layer_gap = 80.0f32;
        let node_gap = 40.0f32;

        let mut positions = HashMap::new();
        match direction {
            LayoutDirection::Horizontal => {
                // Layers stack left → right; nodes within a layer stack top → bottom.
                for (l, layer_nodes) in layers.iter().enumerate() {
                    let x = l as f32 * (node_w + layer_gap);
                    let count = layer_nodes.len();
                    let total_h = count as f32 * node_h
                        + count.saturating_sub(1) as f32 * node_gap;
                    let start_y = -total_h * 0.5;
                    for (i, &nid) in layer_nodes.iter().enumerate() {
                        let y = start_y + i as f32 * (node_h + node_gap);
                        positions.insert(nid, PointF::new(x, y));
                    }
                }
            }
            LayoutDirection::Vertical => {
                // Layers stack top → bottom; nodes within a layer stack left → right.
                for (l, layer_nodes) in layers.iter().enumerate() {
                    let y = l as f32 * (node_h + layer_gap);
                    let count = layer_nodes.len();
                    let total_w = count as f32 * node_w
                        + count.saturating_sub(1) as f32 * node_gap;
                    let start_x = -total_w * 0.5;
                    for (i, &nid) in layer_nodes.iter().enumerate() {
                        let x = start_x + i as f32 * (node_w + node_gap);
                        positions.insert(nid, PointF::new(x, y));
                    }
                }
            }
        }

        LayoutResult { positions }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::graph::Edge;

    #[test]
    fn horizontal_layout_assigns_layers_left_to_right() {
        let mut g = FlowGraph::new();
        let a = g.add_node("a", serde_json::json!({}));
        let b = g.add_node("b", serde_json::json!({}));
        let c = g.add_node("c", serde_json::json!({}));
        g.add_edge(Edge::new(a, b));
        g.add_edge(Edge::new(b, c));

        let result = SimpleLayout::new().layout(&g, LayoutDirection::Horizontal);
        let pa = result.positions.get(&a).unwrap();
        let pb = result.positions.get(&b).unwrap();
        let pc = result.positions.get(&c).unwrap();
        // Layers increase left → right.
        assert!(pa.x < pb.x);
        assert!(pb.x < pc.x);
    }

    #[test]
    fn vertical_layout_assigns_layers_top_to_bottom() {
        let mut g = FlowGraph::new();
        let a = g.add_node("a", serde_json::json!({}));
        let b = g.add_node("b", serde_json::json!({}));
        g.add_edge(Edge::new(a, b));

        let result = SimpleLayout::new().layout(&g, LayoutDirection::Vertical);
        let pa = result.positions.get(&a).unwrap();
        let pb = result.positions.get(&b).unwrap();
        assert!(pa.y < pb.y);
    }
}
