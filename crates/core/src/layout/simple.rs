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
//! - **Loop**: the loop body chain is placed **vertically** to the right of the
//!   loop node (top-in / bottom-out, regardless of main direction). The layout
//!   engine reserves space for the body chain and shifts subsequent nodes to
//!   avoid overlap. The back-edge path routes below the combined bounds.

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

        // Post-process: reposition loop body nodes vertically and reserve space.
        post_process_loop_bodies(
            &mut positions,
            graph,
            &sizes,
            direction,
            self.layer_gap,
            self.node_gap,
        );

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

// ─── Loop body post-processing ──────────────────────────────────────────────

/// Back-edge path margin (must match `loop_back_path` in `edge_path.rs`).
const BACK_EDGE_MARGIN: f32 = 40.0;

/// Collect loop body nodes grouped by their parent Loop node.
///
/// Body nodes = `loop_body` 出口的目标 + 从这些节点沿前向边可达的节点
///（排除通过 `loop_in` 回连的边和回到 Loop 节点的边）。
fn collect_loop_body_groups(graph: &FlowGraph) -> HashMap<NodeId, HashSet<NodeId>> {
    let mut groups: HashMap<NodeId, HashSet<NodeId>> = HashMap::new();

    for edge in graph.edges() {
        if edge.source_port.as_deref() == Some("loop_body") {
            groups.entry(edge.source).or_default().insert(edge.target);
        }
    }

    for (loop_node, body_nodes) in groups.iter_mut() {
        let mut queue: std::collections::VecDeque<NodeId> = body_nodes.iter().copied().collect();
        while let Some(nid) = queue.pop_front() {
            for edge in graph.out_edges(nid) {
                if edge.target_port.as_deref() == Some("loop_in") {
                    continue;
                }
                if edge.target == *loop_node {
                    continue;
                }
                if body_nodes.insert(edge.target) {
                    queue.push_back(edge.target);
                }
            }
        }
    }

    groups
}

/// Post-process: reposition loop body nodes vertically beside their Loop node,
/// and shift other nodes to reserve space for the body chain + back-edge path.
///
/// **Body chain layout**: nodes are stacked vertically (top → bottom) to the
/// right of the Loop node, using Top/Bottom ports (纵向布局).
///
/// **Space reservation**:
/// - Horizontal: shift nodes to the right of the Loop node further right
///   by (body chain width + layer_gap).
/// - Vertical: shift nodes below the Loop node further down
///   by (body chain height - loop height + back-edge margin + node_gap).
fn post_process_loop_bodies(
    positions: &mut HashMap<NodeId, PointF>,
    graph: &FlowGraph,
    sizes: &HashMap<NodeId, (f32, f32)>,
    direction: LayoutDirection,
    layer_gap: f32,
    node_gap: f32,
) {
    let body_groups = collect_loop_body_groups(graph);
    if body_groups.is_empty() {
        return;
    }

    let all_body_nodes: HashSet<NodeId> =
        body_groups.values().flat_map(|s| s.iter().copied()).collect();

    for (loop_node, body_nodes) in &body_groups {
        let loop_pos = match positions.get(loop_node) {
            Some(&p) => p,
            None => continue,
        };
        let (loop_w, loop_h) = sizes.get(loop_node).copied().unwrap_or((220.0, 80.0));

        // Order body nodes by current Y (top → bottom) to preserve chain order
        let mut body_list: Vec<NodeId> = body_nodes.iter().copied().collect();
        body_list.sort_by(|&a, &b| {
            let ya = positions.get(&a).map(|p| p.y).unwrap_or(0.0);
            let yb = positions.get(&b).map(|p| p.y).unwrap_or(0.0);
            ya.partial_cmp(&yb).unwrap_or(std::cmp::Ordering::Equal)
        });

        // Position body nodes vertically to the right of the Loop node
        let body_x = loop_pos.x + loop_w + layer_gap;
        let mut body_y = loop_pos.y;
        let mut max_body_w = 0.0f32;
        let mut chain_bottom = loop_pos.y + loop_h;

        for &nid in &body_list {
            let (w, h) = sizes.get(&nid).copied().unwrap_or((180.0, 35.0));
            positions.insert(nid, PointF::new(body_x, body_y));
            body_y += h + node_gap;
            max_body_w = max_body_w.max(w);
            chain_bottom = chain_bottom.max(body_y);
        }

        // Shift non-body nodes to avoid overlap with body chain + back-edge path.
        //
        // Design principle: **main-flow nodes follow the main axis** (Loop's center X).
        // Body space calculation is purely for **collision avoidance**, not alignment.
        // - Horizontal: only shift nodes whose Y overlaps with body chain's Y range.
        //   Main-flow successors (via done) are typically below the body chain and keep
        //   their original X position on the main axis.
        // - Vertical: shift nodes below the body chain down to clear it + back-edge margin.
        match direction {
            LayoutDirection::Horizontal => {
                let body_top = loop_pos.y;
                let body_bottom = chain_bottom;
                let shift_x = max_body_w + layer_gap;
                for (nid, pos) in positions.iter_mut() {
                    if all_body_nodes.contains(nid) || *nid == *loop_node {
                        continue;
                    }
                    // Only shift if node is to the right of Loop AND vertically overlaps
                    // with the body chain. Nodes entirely below the body chain stay on
                    // the main axis (no horizontal shift).
                    let node_h = sizes.get(nid).map(|(_, h)| *h).unwrap_or(35.0);
                    let node_bottom = pos.y + node_h;
                    if pos.x >= loop_pos.x + loop_w
                        && pos.y < body_bottom
                        && node_bottom > body_top
                    {
                        pos.x += shift_x;
                    }
                }
            }
            LayoutDirection::Vertical => {
                // Shift nodes below the Loop node further down
                // (body chain extends below loop + back-edge margin)
                let extra_h =
                    (chain_bottom - loop_pos.y - loop_h + BACK_EDGE_MARGIN + node_gap).max(0.0);
                for (nid, pos) in positions.iter_mut() {
                    if all_body_nodes.contains(nid) || *nid == *loop_node {
                        continue;
                    }
                    if pos.y >= loop_pos.y + loop_h {
                        pos.y += extra_h;
                    }
                }
            }
        }
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

    #[test]
    fn loop_body_nodes_are_vertical_and_reserve_space() {
        // Loop → (loop_body) → Body1 → Body2 → (loop_in) Loop
        //                  → (done) → After  (main flow, should stay on main axis)
        let mut g = FlowGraph::new();
        let before = g.add_node("a", serde_json::json!({}));
        let loop_n = g.add_node("loop", serde_json::json!({}));
        let body1 = g.add_node("b", serde_json::json!({}));
        let body2 = g.add_node("c", serde_json::json!({}));
        let after = g.add_node("d", serde_json::json!({}));

        g.add_edge(Edge::new(before, loop_n));
        let mut e_body = Edge::new(loop_n, body1);
        e_body.source_port = Some("loop_body".to_string());
        g.add_edge(e_body);
        g.add_edge(Edge::new(body1, body2));
        let mut e_back = Edge::new(body2, loop_n);
        e_back.target_port = Some("loop_in".to_string());
        g.add_edge(e_back);
        let mut e_done = Edge::new(loop_n, after);
        e_done.source_port = Some("done".to_string());
        g.add_edge(e_done);

        let result = SimpleLayout::new().layout(&g, LayoutDirection::Horizontal);

        let pl = result.positions.get(&loop_n).unwrap();
        let pb1 = result.positions.get(&body1).unwrap();
        let pb2 = result.positions.get(&body2).unwrap();
        let pa = result.positions.get(&after).unwrap();

        // Body nodes are to the right of the Loop node
        assert!(pb1.x > pl.x, "body1 should be right of loop");
        assert!(pb2.x > pl.x, "body2 should be right of loop");

        // Body nodes are stacked vertically (body1 above body2)
        assert!(pb1.y < pb2.y, "body1 should be above body2");

        // Body nodes share the same X (vertical chain)
        assert!((pb1.x - pb2.x).abs() < 1.0, "body nodes should share X");

        // Main-flow node (after/done) stays on main axis — NOT shifted right by body width.
        // It should be to the right of Loop (next layer) but NOT shifted further right.
        // The key: after.x should be close to what it would be without body nodes,
        // i.e., it follows the main layer progression, not the body chain width.
        assert!(pa.x > pl.x, "after should be right of loop (next layer)");
        // After should be below the body chain (vertical placement in same/next layer)
        // and its X should NOT include the body_width shift
        let loop_right = pl.x + 220.0; // default loop width
        // After's X should be reasonably close to normal layer position,
        // not pushed way right by body chain
        assert!(
            pa.x < loop_right + 400.0,
            "after should not be excessively shifted right by body chain"
        );
    }
}
