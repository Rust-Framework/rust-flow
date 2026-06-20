//! Layered (Sugiyama / Dagre-style) layout for directed flow graphs.
//!
//! Pipeline mirrors [@dagrejs/dagre](https://github.com/dagrejs/dagre):
//! 1. Build node-level graph from port edges
//! 2. Mark feedback edges (cycle breaking)
//! 3. Assign ranks (longest-path layering on the acyclic subgraph)
//! 4. Order nodes within ranks (barycenter crossing reduction)
//! 5. Assign coordinates

use std::collections::{HashMap, HashSet};

use crate::auto_layout::options::{LayoutDirection, LayoutOptions};
use crate::graph::FlowGraph;
use crate::id::NodeId;
use crate::math::Point;

struct LayoutNode {
    id: NodeId,
    width: f32,
    height: f32,
    rank: i32,
    order: f32,
    x: f32,
    y: f32,
    /// True for virtual (dummy) nodes inserted by normalize for long edges.
    is_virtual: bool,
}

#[derive(Clone)]
struct LayoutEdge {
    from: usize,
    to: usize,
    feedback: bool,
    /// Edge weight for barycenter calculation (Dagre / Mermaid semantics).
    /// Main edges = 4, feedback edges = 1.
    weight: f32,
}

/// Run layered auto-layout and write positions into `graph.nodes`.
pub fn layout_graph(graph: &mut FlowGraph, options: &LayoutOptions) {
    if graph.nodes.is_empty() {
        return;
    }

    let (mut nodes, mut edges) = build_layout_graph(graph);
    if nodes.is_empty() {
        return;
    }

    mark_feedback_edges(&mut edges, nodes.len());
    assign_ranks(&mut nodes, &edges);
    // Dagre normalize: split long edges (rank span > 1) with virtual dummy nodes.
    // This makes barycenter crossing reduction aware of intermediate edge paths.
    normalize_long_edges(&mut nodes, &mut edges);
    order_nodes(&mut nodes, &edges, options.ordering_iterations);
    assign_coordinates(&mut nodes, options);
    align_to_flow(&mut nodes, &edges, options);
    resolve_overlaps(&mut nodes, options.node_spacing);
    place_loop_body_children(&mut nodes, graph, options.node_spacing);

    // Only write positions for real nodes (skip virtual dummies).
    for node in &nodes {
        if node.is_virtual {
            continue;
        }
        if let Some(n) = graph.nodes.get_mut(node.id) {
            n.position = Point::new(node.x, node.y);
        }
    }
}

fn build_layout_graph(graph: &FlowGraph) -> (Vec<LayoutNode>, Vec<LayoutEdge>) {
    let mut id_to_idx: HashMap<NodeId, usize> = HashMap::new();
    let mut nodes: Vec<LayoutNode> = Vec::new();

    for (id, node) in graph.nodes.iter() {
        id_to_idx.insert(id, nodes.len());
        nodes.push(LayoutNode {
            id,
            width: node.size.width,
            height: node.size.height,
            rank: 0,
            order: 0.0,
            x: 0.0,
            y: 0.0,
            is_virtual: false,
        });
    }

    let mut edges: Vec<LayoutEdge> = Vec::new();
    let mut seen: HashSet<(usize, usize)> = HashSet::new();

    for edge in &graph.edges {
        let from_port = graph.ports.get(edge.from_port);
        let to_port = graph.ports.get(edge.to_port);
        if from_port.is_none() || to_port.is_none() {
            continue;
        }
        let from_port = from_port.unwrap();
        let to_port = to_port.unwrap();
        if from_port.node == to_port.node {
            continue;
        }
        let Some(from) = id_to_idx.get(&from_port.node).copied() else {
            continue;
        };
        let Some(to) = id_to_idx.get(&to_port.node).copied() else {
            continue;
        };
        let is_continue = to_port.name == "continue";
        if seen.insert((from, to)) {
            edges.push(LayoutEdge {
                from,
                to,
                feedback: is_continue,
                weight: if is_continue { 1.0 } else { 4.0 },
            });
        }
    }

    (nodes, edges)
}

/// DFS back-edge detection (feedback edges for cycle breaking).
fn mark_feedback_edges(edges: &mut [LayoutEdge], node_count: usize) {
    let mut adj: Vec<Vec<usize>> = vec![Vec::new(); node_count];
    for (i, e) in edges.iter().enumerate() {
        adj[e.from].push(i);
    }

    let mut state = vec![0u8; node_count]; // 0 unvisited, 1 on stack, 2 done

    for start in 0..node_count {
        if state[start] != 0 {
            continue;
        }
        dfs_feedback(start, &adj, edges, &mut state);
    }
}

fn dfs_feedback(
    v: usize,
    adj: &[Vec<usize>],
    edges: &mut [LayoutEdge],
    state: &mut [u8],
) {
    state[v] = 1;
    for &edge_idx in &adj[v] {
        let w = edges[edge_idx].to;
        match state[w] {
            0 => dfs_feedback(w, adj, edges, state),
            1 => edges[edge_idx].feedback = true,
            _ => {}
        }
    }
    state[v] = 2;
}

/// Longest-path rank assignment on the acyclic subgraph (non-feedback edges).
fn assign_ranks(nodes: &mut [LayoutNode], edges: &[LayoutEdge]) {
    let n = nodes.len();

    for node in nodes.iter_mut() {
        node.rank = 0;
    }

    let mut changed = true;
    let mut guard = 0;
    while changed && guard < n * 2 {
        changed = false;
        guard += 1;
        for e in edges {
            if e.feedback {
                continue;
            }
            let new_rank = nodes[e.from].rank + 1;
            if new_rank > nodes[e.to].rank {
                nodes[e.to].rank = new_rank;
                changed = true;
            }
        }
    }

    let min_rank = nodes.iter().map(|n| n.rank).min().unwrap_or(0);
    if min_rank != 0 {
        for node in nodes.iter_mut() {
            node.rank -= min_rank;
        }
    }
}

/// Dagre `normalize.run` — split edges spanning multiple ranks into single-rank
/// segments connected by virtual (dummy) nodes. This is critical for barycenter
/// crossing reduction to correctly account for long edge paths.
///
/// For an edge from rank 0 to rank 3, we insert 2 virtual nodes at ranks 1 and 2,
/// and replace the original edge with 3 single-rank edges.
fn normalize_long_edges(nodes: &mut Vec<LayoutNode>, edges: &mut Vec<LayoutEdge>) {
    // Collect edges that span more than 1 rank (non-feedback only).
    let mut long_edges: Vec<usize> = Vec::new();
    for (idx, e) in edges.iter().enumerate() {
        if e.feedback {
            continue;
        }
        let span = nodes[e.to].rank - nodes[e.from].rank;
        if span > 1 {
            long_edges.push(idx);
        }
    }

    if long_edges.is_empty() {
        return;
    }

    // Process in reverse order so earlier indices remain valid during removal.
    long_edges.sort_unstable_by(|a, b| b.cmp(a));

    for edge_idx in long_edges {
        let e = edges[edge_idx].clone();
        let from_rank = nodes[e.from].rank;
        let to_rank = nodes[e.to].rank;
        let weight = e.weight;

        // Remove the original long edge.
        edges.remove(edge_idx);

        // Insert virtual nodes for each intermediate rank.
        let mut prev_node = e.from;
        for rank in (from_rank + 1)..to_rank {
            let virtual_idx = nodes.len();
            // Virtual nodes use a small default size (Dagre uses 0×0 for dummies,
            // but we use a small size to avoid division issues in coordinate assignment).
            nodes.push(LayoutNode {
                id: NodeId::default(), // Virtual nodes have no real NodeId.
                width: 10.0,
                height: 10.0,
                rank,
                order: 0.0,
                x: 0.0,
                y: 0.0,
                is_virtual: true,
            });
            edges.push(LayoutEdge {
                from: prev_node,
                to: virtual_idx,
                feedback: false,
                weight,
            });
            prev_node = virtual_idx;
        }
        // Final segment from last virtual to target.
        edges.push(LayoutEdge {
            from: prev_node,
            to: e.to,
            feedback: false,
            weight,
        });
    }
}

fn layers(nodes: &[LayoutNode]) -> HashMap<i32, Vec<usize>> {
    let mut map: HashMap<i32, Vec<usize>> = HashMap::new();
    for (i, node) in nodes.iter().enumerate() {
        map.entry(node.rank).or_default().push(i);
    }
    for indices in map.values_mut() {
        indices.sort_by_key(|&i| nodes[i].order as i32);
    }
    map
}

fn order_nodes(nodes: &mut [LayoutNode], edges: &[LayoutEdge], iterations: u32) {
    let n = nodes.len();
    let mut out_adj: Vec<Vec<usize>> = vec![Vec::new(); n];
    let mut in_adj: Vec<Vec<usize>> = vec![Vec::new(); n];
    for e in edges {
        if e.feedback {
            continue;
        }
        out_adj[e.from].push(e.to);
        in_adj[e.to].push(e.from);
    }

    for (i, node) in nodes.iter_mut().enumerate() {
        node.order = i as f32;
    }

    let max_rank = nodes.iter().map(|nd| nd.rank).max().unwrap_or(0);

    for _ in 0..iterations {
        for rank in 0..=max_rank {
            let layer: Vec<usize> = nodes
                .iter()
                .enumerate()
                .filter(|(_, nd)| nd.rank == rank)
                .map(|(i, _)| i)
                .collect();
            if layer.len() <= 1 {
                continue;
            }
            let mut sorted = layer;
            sorted.sort_by(|&a, &b| {
                let ba = weighted_barycenter(a, &out_adj, edges, nodes);
                let bb = weighted_barycenter(b, &out_adj, edges, nodes);
                ba.partial_cmp(&bb).unwrap_or(std::cmp::Ordering::Equal)
            });
            for (pos, idx) in sorted.iter().enumerate() {
                nodes[*idx].order = pos as f32;
            }
        }

        for rank in (0..=max_rank).rev() {
            let layer: Vec<usize> = nodes
                .iter()
                .enumerate()
                .filter(|(_, nd)| nd.rank == rank)
                .map(|(i, _)| i)
                .collect();
            if layer.len() <= 1 {
                continue;
            }
            let mut sorted = layer;
            sorted.sort_by(|&a, &b| {
                let ba = weighted_barycenter(a, &in_adj, edges, nodes);
                let bb = weighted_barycenter(b, &in_adj, edges, nodes);
                ba.partial_cmp(&bb).unwrap_or(std::cmp::Ordering::Equal)
            });
            for (pos, idx) in sorted.iter().enumerate() {
                nodes[*idx].order = pos as f32;
            }
        }
    }
}

/// Dagre barycenter: `Σ(weight(e) × order(u)) / Σ(weight(e))`.
fn weighted_barycenter(
    node: usize,
    neighbors: &[Vec<usize>],
    edges: &[LayoutEdge],
    nodes: &[LayoutNode],
) -> f32 {
    let neigh = &neighbors[node];
    if neigh.is_empty() {
        return nodes[node].order;
    }
    let mut weight_sum = 0.0f32;
    let mut weighted_order_sum = 0.0f32;
    for &neighbor_idx in neigh {
        // Find the edge connecting node -> neighbor_idx (or reverse for in_adj)
        let edge_weight = edges
            .iter()
            .find(|e| {
                !e.feedback
                    && ((e.from == node && e.to == neighbor_idx)
                        || (e.to == node && e.from == neighbor_idx))
            })
            .map(|e| e.weight)
            .unwrap_or(1.0);
        weight_sum += edge_weight;
        weighted_order_sum += edge_weight * nodes[neighbor_idx].order;
    }
    if weight_sum > 0.0 {
        weighted_order_sum / weight_sum
    } else {
        nodes[node].order
    }
}

fn assign_coordinates(nodes: &mut [LayoutNode], options: &LayoutOptions) {
    let layer_map = layers(nodes);
    let ranks: Vec<i32> = layer_map.keys().copied().collect();
    let max_rank = ranks.iter().max().copied().unwrap_or(0);

    let mut rank_primary_size: HashMap<i32, f32> = HashMap::new();

    for rank in 0..=max_rank {
        let indices = layer_map.get(&rank).cloned().unwrap_or_default();
        if indices.is_empty() {
            continue;
        }
        let mut sorted = indices;
        sorted.sort_by(|&a, &b| {
            nodes[a]
                .order
                .partial_cmp(&nodes[b].order)
                .unwrap_or(std::cmp::Ordering::Equal)
        });

        let max_w = sorted
            .iter()
            .map(|&i| nodes[i].width)
            .fold(0.0f32, f32::max);
        rank_primary_size.insert(rank, max_w);
    }

    let mut rank_primary_pos: HashMap<i32, f32> = HashMap::new();
    let mut primary = options.margin;
    for rank in 0..=max_rank {
        rank_primary_pos.insert(rank, primary);
        if let Some(w) = rank_primary_size.get(&rank) {
            primary += *w + options.rank_spacing;
        }
    }

    for rank in 0..=max_rank {
        let indices = layer_map.get(&rank).cloned().unwrap_or_default();
        if indices.is_empty() {
            continue;
        }
        let mut sorted = indices;
        sorted.sort_by(|&a, &b| {
            nodes[a]
                .order
                .partial_cmp(&nodes[b].order)
                .unwrap_or(std::cmp::Ordering::Equal)
        });

        // Top-align layers — per-layer centering causes tall nodes to overlap neighbors.
        let mut cross = options.margin;
        let primary_pos = rank_primary_pos.get(&rank).copied().unwrap_or(options.margin);

        for idx in sorted {
            match options.direction {
                LayoutDirection::LeftRight => {
                    nodes[idx].x = primary_pos;
                    nodes[idx].y = cross;
                    cross += nodes[idx].height + options.node_spacing;
                }
                LayoutDirection::TopBottom => {
                    nodes[idx].x = cross;
                    nodes[idx].y = primary_pos;
                    cross += nodes[idx].width + options.node_spacing;
                }
            }
        }
    }
}

/// Align nodes with their predecessors/successors on the cross axis (vertical in LR layout).
fn align_to_flow(nodes: &mut [LayoutNode], edges: &[LayoutEdge], options: &LayoutOptions) {
    let max_rank = nodes.iter().map(|n| n.rank).max().unwrap_or(0);

    for _ in 0..4 {
        for rank in 1..=max_rank {
            let layer: Vec<usize> = nodes
                .iter()
                .enumerate()
                .filter(|(_, n)| n.rank == rank)
                .map(|(i, _)| i)
                .collect();
            for idx in layer {
                let preds: Vec<usize> = edges
                    .iter()
                    .filter(|e| !e.feedback && e.to == idx)
                    .map(|e| e.from)
                    .collect();
                if preds.is_empty() {
                    continue;
                }
                let target_center = preds
                    .iter()
                    .map(|&p| center_cross(&nodes[p], options.direction))
                    .sum::<f32>()
                    / preds.len() as f32;
                set_cross_center(&mut nodes[idx], options.direction, target_center);
            }
        }

        for rank in (0..max_rank).rev() {
            let layer: Vec<usize> = nodes
                .iter()
                .enumerate()
                .filter(|(_, n)| n.rank == rank)
                .map(|(i, _)| i)
                .collect();
            for idx in layer {
                let succs: Vec<usize> = edges
                    .iter()
                    .filter(|e| !e.feedback && e.from == idx)
                    .map(|e| e.to)
                    .collect();
                if succs.is_empty() {
                    continue;
                }
                let target_center = succs
                    .iter()
                    .map(|&s| center_cross(&nodes[s], options.direction))
                    .sum::<f32>()
                    / succs.len() as f32;
                set_cross_center(&mut nodes[idx], options.direction, target_center);
            }
        }

        separate_same_rank(nodes, options);
    }
}

fn center_cross(node: &LayoutNode, direction: LayoutDirection) -> f32 {
    match direction {
        LayoutDirection::LeftRight => node.y + node.height * 0.5,
        LayoutDirection::TopBottom => node.x + node.width * 0.5,
    }
}

fn set_cross_center(node: &mut LayoutNode, direction: LayoutDirection, center: f32) {
    match direction {
        LayoutDirection::LeftRight => node.y = center - node.height * 0.5,
        LayoutDirection::TopBottom => node.x = center - node.width * 0.5,
    }
}

fn cross_span(node: &LayoutNode, direction: LayoutDirection) -> f32 {
    match direction {
        LayoutDirection::LeftRight => node.height,
        LayoutDirection::TopBottom => node.width,
    }
}

fn cross_pos(node: &LayoutNode, direction: LayoutDirection) -> f32 {
    match direction {
        LayoutDirection::LeftRight => node.y,
        LayoutDirection::TopBottom => node.x,
    }
}

fn set_cross_pos(node: &mut LayoutNode, direction: LayoutDirection, pos: f32) {
    match direction {
        LayoutDirection::LeftRight => node.y = pos,
        LayoutDirection::TopBottom => node.x = pos,
    }
}

/// Keep nodes in the same rank from overlapping on the cross axis.
fn separate_same_rank(nodes: &mut [LayoutNode], options: &LayoutOptions) {
    let direction = options.direction;
    let max_rank = nodes.iter().map(|n| n.rank).max().unwrap_or(0);

    for rank in 0..=max_rank {
        let mut layer: Vec<usize> = nodes
            .iter()
            .enumerate()
            .filter(|(_, n)| n.rank == rank)
            .map(|(i, _)| i)
            .collect();
        layer.sort_by(|&a, &b| {
            cross_pos(&nodes[a], direction)
                .partial_cmp(&cross_pos(&nodes[b], direction))
                .unwrap_or(std::cmp::Ordering::Equal)
        });

        for k in 1..layer.len() {
            let prev = layer[k - 1];
            let curr = layer[k];
            let min_pos = cross_pos(&nodes[prev], direction) + cross_span(&nodes[prev], direction)
                + options.node_spacing;
            if cross_pos(&nodes[curr], direction) < min_pos {
                set_cross_pos(&mut nodes[curr], direction, min_pos);
            }
        }
    }
}

pub(crate) fn rects_overlap(
    ax: f32,
    ay: f32,
    aw: f32,
    ah: f32,
    bx: f32,
    by: f32,
    bw: f32,
    bh: f32,
    padding: f32,
) -> bool {
    ax < bx + bw + padding
        && ax + aw + padding > bx
        && ay < by + bh + padding
        && ay + ah + padding > by
}

/// Global overlap removal — columns are independent after layering so boxes can still intersect.
fn resolve_overlaps(nodes: &mut [LayoutNode], padding: f32) {
    let n = nodes.len();
    for _ in 0..64 {
        let mut moved = false;
        for i in 0..n {
            for j in i + 1..n {
                let a = &nodes[i];
                let b = &nodes[j];
                if !rects_overlap(
                    a.x, a.y, a.width, a.height, b.x, b.y, b.width, b.height, padding,
                ) {
                    continue;
                }
                let push_down = a.y + a.height + padding - b.y;
                let push_up = b.y + b.height + padding - a.y;
                if push_down > 0.0 && (push_down <= push_up || push_up <= 0.0) {
                    nodes[j].y += push_down;
                    moved = true;
                } else if push_up > 0.0 {
                    nodes[i].y += push_up;
                    moved = true;
                }
            }
        }
        if !moved {
            break;
        }
    }
}

/// Place loop body targets below the loop node when connected via `body` port.
fn place_loop_body_children(nodes: &mut [LayoutNode], graph: &FlowGraph, spacing: f32) {
    let id_to_idx: HashMap<NodeId, usize> = nodes
        .iter()
        .enumerate()
        .map(|(i, n)| (n.id, i))
        .collect();

    for edge in &graph.edges {
        let from_port = graph.ports.get(edge.from_port);
        let to_port = graph.ports.get(edge.to_port);
        if from_port.is_none() || to_port.is_none() {
            continue;
        }
        let from_port = from_port.unwrap();
        let to_port = to_port.unwrap();
        if from_port.name != "body" {
            continue;
        }
        let Some(&from_idx) = id_to_idx.get(&from_port.node) else {
            continue;
        };
        let Some(&to_idx) = id_to_idx.get(&to_port.node) else {
            continue;
        };
        let min_y = nodes[from_idx].y + nodes[from_idx].height + spacing;
        if nodes[to_idx].y < min_y {
            nodes[to_idx].y = min_y;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::graph::FlowGraph;
    use crate::node_type::{BRANCH, COMMON};
    use crate::port::PortDirection;
    use crate::math::Point;

    #[test]
    fn layout_chain_increases_primary_axis() {
        let mut graph = FlowGraph::new("chain");
        let n1 = graph.add_typed_node(COMMON, "A", Point::new(0.0, 0.0));
        let n2 = graph.add_typed_node(COMMON, "B", Point::new(0.0, 0.0));
        let n3 = graph.add_typed_node(COMMON, "C", Point::new(0.0, 0.0));

        let p1_out = graph
            .ports
            .iter()
            .find(|(_, p)| p.node == n1 && p.direction == PortDirection::Output)
            .map(|(id, _)| id)
            .unwrap();
        let p2_in = graph
            .ports
            .iter()
            .find(|(_, p)| p.node == n2 && p.direction == PortDirection::Input)
            .map(|(id, _)| id)
            .unwrap();
        let p2_out = graph
            .ports
            .iter()
            .find(|(_, p)| p.node == n2 && p.direction == PortDirection::Output)
            .map(|(id, _)| id)
            .unwrap();
        let p3_in = graph
            .ports
            .iter()
            .find(|(_, p)| p.node == n3 && p.direction == PortDirection::Input)
            .map(|(id, _)| id)
            .unwrap();

        graph.add_edge(crate::edge::FlowEdge::new(p1_out, p2_in));
        graph.add_edge(crate::edge::FlowEdge::new(p2_out, p3_in));

        layout_graph(&mut graph, &LayoutOptions::default());

        let x1 = graph.nodes[n1].position.x;
        let x2 = graph.nodes[n2].position.x;
        let x3 = graph.nodes[n3].position.x;
        assert!(x1 < x2 && x2 < x3);
    }

    #[test]
    fn layout_branch_spreads_false_true() {
        let mut graph = FlowGraph::new("branch");
        let src = graph.add_typed_node(COMMON, "Src", Point::new(0.0, 0.0));
        let branch = graph.add_typed_node(BRANCH, "If", Point::new(0.0, 0.0));
        let t = graph.add_typed_node(COMMON, "True", Point::new(0.0, 0.0));
        let f = graph.add_typed_node(COMMON, "False", Point::new(0.0, 0.0));

        let src_out = graph
            .ports
            .iter()
            .find(|(_, p)| p.node == src && p.direction == PortDirection::Output)
            .map(|(id, _)| id)
            .unwrap();
        let br_in = graph
            .ports
            .iter()
            .find(|(_, p)| p.node == branch && p.direction == PortDirection::Input)
            .map(|(id, _)| id)
            .unwrap();
        let br_true = graph
            .ports
            .iter()
            .find(|(_, p)| p.node == branch && p.name == "true")
            .map(|(id, _)| id)
            .unwrap();
        let br_false = graph
            .ports
            .iter()
            .find(|(_, p)| p.node == branch && p.name == "false")
            .map(|(id, _)| id)
            .unwrap();
        let t_in = graph
            .ports
            .iter()
            .find(|(_, p)| p.node == t && p.direction == PortDirection::Input)
            .map(|(id, _)| id)
            .unwrap();
        let f_in = graph
            .ports
            .iter()
            .find(|(_, p)| p.node == f && p.direction == PortDirection::Input)
            .map(|(id, _)| id)
            .unwrap();

        graph.add_edge(crate::edge::FlowEdge::new(src_out, br_in));
        graph.add_edge(crate::edge::FlowEdge::new(br_true, t_in));
        graph.add_edge(crate::edge::FlowEdge::new(br_false, f_in));

        layout_graph(&mut graph, &LayoutOptions::default());

        let bx = graph.nodes[branch].position.x;
        assert!(graph.nodes[t].position.x > bx);
        assert!(graph.nodes[f].position.x > bx);
        assert_ne!(
            graph.nodes[t].position.y,
            graph.nodes[f].position.y
        );
    }

    #[test]
    fn layout_demo_no_node_overlap() {
        use crate::demo_graph_from_document;
        use crate::orientation::loop_container_overlap_allowed;

        let graph = demo_graph_from_document();
        let padding = 8.0;
        let ids: Vec<_> = graph.nodes.iter().map(|(id, _)| id).collect();
        for i in 0..ids.len() {
            for j in i + 1..ids.len() {
                if loop_container_overlap_allowed(&graph, ids[i], ids[j]) {
                    continue;
                }
                let a = &graph.nodes[ids[i]];
                let b = &graph.nodes[ids[j]];
                assert!(
                    !rects_overlap(
                        a.position.x,
                        a.position.y,
                        a.size.width,
                        a.size.height,
                        b.position.x,
                        b.position.y,
                        b.size.width,
                        b.size.height,
                        padding,
                    ),
                    "overlap between '{}' and '{}'",
                    a.label,
                    b.label
                );
            }
        }
    }

    #[test]
    fn layout_demo_graph_invariants() {
        use crate::check_frame;
        use crate::demo_chain_graph;
        use crate::scene::SceneFrame;
        use crate::viewport::Viewport;

        let mut graph = demo_chain_graph();
        layout_graph(&mut graph, &LayoutOptions::default());
        let frame = SceneFrame::resolve(&graph, &Viewport::default());
        check_frame(&frame).expect("layout invariants");
    }
}
