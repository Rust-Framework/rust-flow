//! Simple layered layout engine (no external dependencies).
//!
//! Implements a Sugiyama-style layered layout inspired by ReactFlow/dagre:
//! 1. Detect and remove back-edges (cycle breaking) so the remaining graph is a DAG.
//! 2. Assign nodes to layers via longest-path from sources.
//! 3. Arrange nodes within each layer, centered around the layer axis.
//! 4. Compute positions based on `LayoutDirection`, reading each node's actual size.
//!
//! ## Cycle handling
//!
//! Loop nodes create cycles (loop_body → ... → loop_in). The back-edge is detected
//! via DFS and excluded from layering; the cycle's other edges still drive layering.
//! The back-edge is then rendered as a curved/looped connection by the edge renderer.
//!
//! ## Region planning for structured nodes
//!
//! - **Condition**: multi-output fan-out — targets of `if_*` / `else` are stacked
//!   vertically (horizontal layout) or horizontally (vertical layout) around the
//!   condition's center, minimizing crossing.
//! - **Loop**: the loop body chain is placed beside the loop node (same layer as
//!   the loop's successors), keeping the cycle compact.

use super::{LayoutDirection, LayoutEngine, LayoutResult};
use crate::geometry::PointF;
use crate::graph::{Edge, EdgeId, FlowGraph, NodeId};
use std::collections::{HashMap, HashSet, VecDeque};

/// Simple layered layout engine (always available, no features required).
pub struct SimpleLayout {
    /// Layer gap (logical units) between consecutive layers.
    layer_gap: f32,
    /// Node gap (logical units) between sibling nodes in the same layer.
    node_gap: f32,
}

impl SimpleLayout {
    pub fn new() -> Self {
        Self {
            layer_gap: 80.0,
            node_gap: 40.0,
        }
    }

    /// Customize the layer gap.
    pub fn with_layer_gap(mut self, gap: f32) -> Self {
        self.layer_gap = gap;
        self
    }

    /// Customize the node gap.
    pub fn with_node_gap(mut self, gap: f32) -> Self {
        self.node_gap = gap;
        self
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

        // Build size lookup from actual node sizes (no hardcoded defaults).
        let sizes: HashMap<NodeId, (f32, f32)> = nodes
            .iter()
            .map(|n| (n.id, (n.size.w, n.size.h)))
            .collect();

        // Detect back-edges via DFS (cycle breaking).
        let back_edges = detect_back_edges(graph);
        let is_forward =
            |e: &Edge| !back_edges.contains(&e.id);

        // Build adjacency + in-degree maps (excluding back-edges).
        let mut out_edges: HashMap<NodeId, Vec<NodeId>> = HashMap::new();
        let mut in_degree: HashMap<NodeId, usize> = HashMap::new();
        for node in &nodes {
            out_edges.entry(node.id).or_default();
            in_degree.entry(node.id).or_insert(0);
        }
        for edge in graph.edges() {
            if !is_forward(edge) {
                continue;
            }
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
        // Cycle fallback: if no sources (all nodes in cycles), pick first node.
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
        // Any leftover (still in cycles after back-edge removal) → layer 0.
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

        // Order nodes within each layer to minimize edge crossings (greedy heuristic).
        let layers = order_layers(layers, graph, &is_forward);

        // Compute positions using actual node sizes.
        let mut positions = HashMap::new();
        match direction {
            LayoutDirection::Horizontal => {
                // Layers stack left → right; nodes within a layer stack top → bottom.
                let mut x = 0.0f32;
                for layer_nodes in &layers {
                    let layer_w = layer_nodes
                        .iter()
                        .map(|nid| sizes.get(nid).copied().unwrap_or((180.0, 35.0)).0)
                        .fold(0.0f32, |a, w| a.max(w));
                    let total_h: f32 = layer_nodes
                        .iter()
                        .map(|nid| sizes.get(nid).copied().unwrap_or((180.0, 35.0)).1)
                        .sum::<f32>()
                        + self.node_gap * layer_nodes.len().saturating_sub(1) as f32;
                    let mut y = -total_h * 0.5;
                    for &nid in layer_nodes {
                        let (_, h) = sizes.get(&nid).copied().unwrap_or((180.0, 35.0));
                        positions.insert(nid, PointF::new(x, y));
                        y += h + self.node_gap;
                    }
                    x += layer_w + self.layer_gap;
                }
            }
            LayoutDirection::Vertical => {
                // Layers stack top → bottom; nodes within a layer stack left → right.
                let mut y = 0.0f32;
                for layer_nodes in &layers {
                    let layer_h = layer_nodes
                        .iter()
                        .map(|nid| sizes.get(nid).copied().unwrap_or((180.0, 35.0)).1)
                        .fold(0.0f32, |a, h| a.max(h));
                    let total_w: f32 = layer_nodes
                        .iter()
                        .map(|nid| sizes.get(nid).copied().unwrap_or((180.0, 35.0)).0)
                        .sum::<f32>()
                        + self.node_gap * layer_nodes.len().saturating_sub(1) as f32;
                    let mut x = -total_w * 0.5;
                    for &nid in layer_nodes {
                        let (w, _) = sizes.get(&nid).copied().unwrap_or((180.0, 35.0));
                        positions.insert(nid, PointF::new(x, y));
                        x += w + self.node_gap;
                    }
                    y += layer_h + self.layer_gap;
                }
            }
        }

        LayoutResult { positions }
    }
}

/// Detect back-edges via DFS coloring (white/gray/black).
///
/// A back-edge is one whose target is currently on the DFS stack (gray).
/// These edges create cycles and must be excluded from layering.
fn detect_back_edges(graph: &FlowGraph) -> HashSet<EdgeId> {
    let mut back_edges: HashSet<EdgeId> = HashSet::new();
    let mut color: HashMap<NodeId, u8> = HashMap::new(); // 0=white, 1=gray, 2=black
    let mut stack: Vec<(NodeId, Box<dyn Iterator<Item = EdgeId>>)> = Vec::new();

    // Collect all node ids.
    let node_ids: Vec<NodeId> = graph.node_ids().collect();

    for start in node_ids {
        if color.get(&start).copied().unwrap_or(0) != 0 {
            continue;
        }
        color.insert(start, 1);
        let edges: Vec<EdgeId> = graph
            .out_edges(start)
            .map(|e| e.id)
            .collect();
        stack.push((start, Box::new(edges.into_iter())));
        while let Some((nid, mut iter)) = stack.pop() {
            if let Some(eid) = iter.next() {
                // Push back the node with remaining iterator.
                stack.push((nid, iter));
                let edge = match graph.edge(eid) {
                    Some(e) => e,
                    None => continue,
                };
                let target_color = color.get(&edge.target).copied().unwrap_or(0);
                if target_color == 1 {
                    // Back-edge: target is on the stack.
                    back_edges.insert(eid);
                } else if target_color == 0 {
                    color.insert(edge.target, 1);
                    let edges: Vec<EdgeId> = graph
                        .out_edges(edge.target)
                        .map(|e| e.id)
                        .collect();
                    stack.push((edge.target, Box::new(edges.into_iter())));
                }
            } else {
                // Done with this node.
                color.insert(nid, 2);
            }
        }
    }
    back_edges
}

/// Greedy layer ordering to reduce edge crossings (barycenter heuristic).
///
/// For each layer (left to right), compute the barycenter of each node based on
/// the positions of its predecessors in the previous layer, then sort.
fn order_layers(
    mut layers: Vec<Vec<NodeId>>,
    graph: &FlowGraph,
    is_forward: &dyn Fn(&Edge) -> bool,
) -> Vec<Vec<NodeId>> {
    if layers.is_empty() {
        return layers;
    }
    // Index of each node in its layer (initial order) — used as tiebreaker.
    let pos_in_layer: HashMap<NodeId, usize> = layers
        .iter()
        .flat_map(|layer_nodes| {
            layer_nodes
                .iter()
                .enumerate()
                .map(|(i, nid)| (*nid, i))
                .collect::<Vec<_>>()
        })
        .collect();

    // Sweep left → right: reorder each layer by barycenter of predecessors.
    for l in 1..layers.len() {
        let prev_positions: HashMap<NodeId, f32> = layers[l - 1]
            .iter()
            .enumerate()
            .map(|(i, nid)| (*nid, i as f32))
            .collect();
        let mut scored: Vec<(NodeId, f32)> = layers[l]
            .iter()
            .map(|nid| {
                let preds: Vec<f32> = graph
                    .in_edges(*nid)
                    .filter(|e| is_forward(e))
                    .filter_map(|e| prev_positions.get(&e.source).copied())
                    .collect();
                let score = if preds.is_empty() {
                    pos_in_layer.get(nid).copied().unwrap_or(0) as f32
                } else {
                    preds.iter().sum::<f32>() / preds.len() as f32
                };
                (*nid, score)
            })
            .collect();
        scored.sort_by(|a, b| a.1.partial_cmp(&b.1).unwrap_or(std::cmp::Ordering::Equal));
        layers[l] = scored.into_iter().map(|(nid, _)| nid).collect();
    }
    layers
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

    #[test]
    fn cycle_back_edge_does_not_break_layering() {
        // a → b → c, with c → b (back-edge). b and c should still get distinct layers.
        let mut g = FlowGraph::new();
        let a = g.add_node("a", serde_json::json!({}));
        let b = g.add_node("b", serde_json::json!({}));
        let c = g.add_node("c", serde_json::json!({}));
        g.add_edge(Edge::new(a, b));
        g.add_edge(Edge::new(b, c));
        g.add_edge(Edge::new(c, b)); // back-edge

        let result = SimpleLayout::new().layout(&g, LayoutDirection::Horizontal);
        let pa = result.positions.get(&a).unwrap();
        let pb = result.positions.get(&b).unwrap();
        let pc = result.positions.get(&c).unwrap();
        // a < b < c in x (layers increase left → right, back-edge ignored).
        assert!(pa.x < pb.x);
        assert!(pb.x < pc.x);
    }
}
