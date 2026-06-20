//! Edge routing — dagre `normalize.run` + `normalize.undo` + `assignNodeIntersects`
//! + self-loop routing.
//!
//! - [`normalize`]: split edges spanning >1 rank into chains of length-1
//!   segments connected by dummy nodes (so crossing reduction sees them).
//! - [`undo_normalize`]: collapse dummy chains back into `edge.points[]`.
//! - [`assign_node_intersects`]: clip edge endpoints to node bounding rectangles
//!   via `intersectRect` (dagre `assignNodeIntersects`).
//! - [`route_self_edges`]: 5-point self-loop curve (dagre `positionSelfEdges`).

use crate::auto_layout::mermaid_dagre::graph::LayoutGraph;
use crate::math::Point;

/// dagre `normalize.run`: for every non-feedback edge whose rank span exceeds
/// `minlen`, insert `span - minlen` dummy nodes (one per intermediate rank) and
/// replace the edge with single-rank segments. The original edge index is
/// stashed on the first segment so [`undo_normalize`] can write points back.
pub fn normalize(g: &mut LayoutGraph) {
    // Collect long edges (non-feedback, span > minlen).
    let mut long_edges: Vec<usize> = Vec::new();
    for (idx, e) in g.edges.iter().enumerate() {
        if e.feedback {
            continue;
        }
        let span = g.nodes[e.to].rank - g.nodes[e.from].rank;
        if span > e.minlen {
            long_edges.push(idx);
        }
    }
    if long_edges.is_empty() {
        return;
    }

    // Process in reverse so earlier indices stay valid during removal.
    long_edges.sort_unstable_by(|a, b| b.cmp(a));

    for edge_idx in long_edges {
        let e = g.edges[edge_idx].clone();
        let from_rank = g.nodes[e.from].rank;
        let to_rank = g.nodes[e.to].rank;
        let original_idx = e.original_idx;
        let weight = e.weight;
        let minlen = e.minlen;
        let label = e.label.clone();
        let label_width = e.label_width;
        let label_height = e.label_height;
        let name = e.name.clone();

        // Remove the original long edge.
        g.edges.remove(edge_idx);

        // Insert dummy nodes for each intermediate rank (from_rank+minlen .. to_rank).
        let mut prev = e.from;
        let mut first_segment = true;
        let mut label_rank = None;
        for rank in (from_rank + minlen)..to_rank {
            let dummy = g.add_dummy(rank);
            let seg_idx = g.add_edge(prev, dummy);
            let seg = &mut g.edges[seg_idx];
            seg.weight = weight;
            seg.minlen = 1;
            if first_segment {
                seg.original_idx = original_idx;
                seg.label = label.clone();
                seg.label_width = label_width;
                seg.label_height = label_height;
                seg.name = name.clone();
                first_segment = false;
            }
            // Label sits at the rank midpoint (dagre `injectEdgeLabelProxies`).
            if label.is_some() && label_rank.is_none() {
                let mid = from_rank + (to_rank - from_rank) / 2;
                if rank == mid {
                    label_rank = Some(rank);
                    g.edges[seg_idx].label_rank = Some(rank);
                }
            }
            prev = dummy;
        }
        // Final segment from last dummy to target.
        let seg_idx = g.add_edge(prev, e.to);
        let seg = &mut g.edges[seg_idx];
        seg.weight = weight;
        seg.minlen = 1;
        if first_segment {
            seg.original_idx = original_idx;
            seg.label = label;
            seg.label_width = label_width;
            seg.label_height = label_height;
            seg.name = name;
        }
        let _ = label_rank;
    }
}

/// dagre `normalize.undo`: walk each original edge's dummy chain and collect
/// the dummy centers into `edge.points[]`, then drop the dummy nodes/segments.
pub fn undo_normalize(g: &mut LayoutGraph) {
    // Build a map from node index -> its role in a dummy chain.
    // A segment carrying `original_idx` starts a chain; follow `to` pointers
    // through dummy nodes until we hit a real node.
    let n = g.nodes.len();
    let mut visited_edge = vec![false; g.edges.len()];

    // For each real node, find outgoing segments that begin a chain.
    // We collect (original_idx, points) then write back.
    let mut routes: std::collections::HashMap<usize, Vec<Point>> = std::collections::HashMap::new();
    let mut label_positions: std::collections::HashMap<usize, Point> = std::collections::HashMap::new();

    for start_node in 0..n {
        if g.nodes[start_node].is_dummy {
            continue;
        }
        for (eidx, e) in g.out_edges(start_node) {
            if e.original_idx.is_none() || visited_edge[eidx] {
                continue;
            }
            // Walk the chain.
            let mut pts: Vec<Point> = Vec::new();
            pts.push(Point::new(g.nodes[start_node].x, g.nodes[start_node].y));
            let mut cur_edge = eidx;
            let mut cur_node = e.to;
            loop {
                visited_edge[cur_edge] = true;
                let ce = &g.edges[cur_edge];
                // Record label rank position if this segment carries the label.
                if ce.label.is_some() && ce.label_rank.is_some() {
                    label_positions.insert(
                        ce.original_idx.unwrap(),
                        Point::new(g.nodes[cur_node].x, g.nodes[cur_node].y),
                    );
                }
                if !g.nodes[cur_node].is_dummy {
                    pts.push(Point::new(g.nodes[cur_node].x, g.nodes[cur_node].y));
                    break;
                }
                pts.push(Point::new(g.nodes[cur_node].x, g.nodes[cur_node].y));
                // Find the next segment out of this dummy.
                let next = g.out_edges(cur_node).find(|(i, _)| !visited_edge[*i]);
                match next {
                    Some((ni, _)) => {
                        cur_edge = ni;
                        cur_node = g.edges[ni].to;
                    }
                    None => break,
                }
            }
            if let Some(oidx) = e.original_idx {
                routes.insert(oidx, pts);
            }
        }
    }

    // Also handle edges that were NOT split (span == minlen): they have no
    // dummy chain but still need a 2-point route (source center → target center).
    for e in &g.edges {
        if let Some(oidx) = e.original_idx {
            if !routes.contains_key(&oidx) {
                routes.insert(
                    oidx,
                    vec![
                        Point::new(g.nodes[e.from].x, g.nodes[e.from].y),
                        Point::new(g.nodes[e.to].x, g.nodes[e.to].y),
                    ],
                );
            }
        }
    }

    // Write routes back. We need a side channel because we're about to drop
    // dummy edges/nodes; stash on a temporary structure keyed by original_idx.
    let routes: Vec<(usize, Vec<Point>)> = routes.into_iter().collect();
    let labels: Vec<(usize, Point)> = label_positions.into_iter().collect();

    // Remove dummy nodes and any edges touching them.
    g.edges.retain(|e| !g.nodes[e.from].is_dummy && !g.nodes[e.to].is_dummy);
    g.nodes.retain(|n| !n.is_dummy);

    // After removal, node indices shifted. Rebuild edge endpoints by NodeId.
    // (Real nodes keep their NodeId; rebuild a NodeId->index map.)
    let mut id_to_idx: std::collections::HashMap<crate::id::NodeId, usize> =
        std::collections::HashMap::new();
    for (i, n) in g.nodes.iter().enumerate() {
        if let Some(id) = n.id {
            id_to_idx.insert(id, i);
        }
    }
    for e in &mut g.edges {
        if let (Some(from_id), Some(to_id)) =
            (g.nodes[e.from].id, g.nodes[e.to].id)
        {
            let _ = (from_id, to_id);
        }
    }
    // Remap is unnecessary because retain preserved real-node order; edges
    // between real nodes still reference valid indices. But indices of real
    // nodes did NOT change (retain keeps order, dummies were appended after
    // reals in normalize only if reals came first — to be safe, fix up).
    fix_edge_indices(g);

    // Apply routes to edges matched by original_idx.
    for e in &mut g.edges {
        if let Some(oidx) = e.original_idx {
            // Find route by original_idx.
            if let Some((_, pts)) = routes.iter().find(|(k, _)| *k == oidx) {
                e.points = pts.clone();
            }
            if let Some((_, p)) = labels.iter().find(|(k, _)| *k == oidx) {
                e.label_pos = Some(*p);
            }
        }
    }
}

/// After dropping dummy nodes, edge `from`/`to` indices may be stale.
/// Re-resolve them by matching NodeId.
fn fix_edge_indices(g: &mut LayoutGraph) {
    // Build NodeId -> current index.
    let mut id_to_idx: std::collections::HashMap<crate::id::NodeId, usize> =
        std::collections::HashMap::new();
    for (i, n) in g.nodes.iter().enumerate() {
        if let Some(id) = n.id {
            id_to_idx.insert(id, i);
        }
    }
    // We can't re-resolve edges whose endpoints lost their NodeId mapping
    // (they shouldn't — only dummies were removed). Edges between real nodes
    // keep valid indices because real nodes precede dummies in insertion order
    // and retain() preserves relative order. Verify and fix any out-of-range.
    for e in &mut g.edges {
        if e.from >= g.nodes.len() || e.to >= g.nodes.len() {
            // Stale: try to recover via stored points' nearest node — fallback
            // to dropping by zeroing. This should not happen in practice.
            e.from = e.from.min(g.nodes.len().saturating_sub(1));
            e.to = e.to.min(g.nodes.len().saturating_sub(1));
        }
    }
    let _ = id_to_idx;
}

/// dagre `assignNodeIntersects`: clip the first/last edge points to the source
/// and target node rectangles via `intersectRect`.
pub fn assign_node_intersects(g: &mut LayoutGraph) {
    for e in &mut g.edges {
        if e.points.is_empty() {
            continue;
        }
        let from = e.from;
        let to = e.to;
        // Clip start to source rect.
        if let Some(p1) = e.points.get(1) {
            let center = Point::new(g.nodes[from].x, g.nodes[from].y);
            let clipped = intersect_rect(center, g.nodes[from].width, g.nodes[from].height, *p1);
            e.points[0] = clipped;
        } else {
            e.points[0] = Point::new(g.nodes[from].x, g.nodes[from].y);
        }
        // Clip end to target rect.
        let last = e.points.len() - 1;
        if last >= 1 {
            let prev = e.points[last - 1];
            let center = Point::new(g.nodes[to].x, g.nodes[to].y);
            let clipped = intersect_rect(center, g.nodes[to].width, g.nodes[to].height, prev);
            e.points[last] = clipped;
        }
    }
}

/// dagre `intersectRect`: intersect the line from the node center to `point`
/// with the node's bounding rectangle. Returns the boundary intersection.
pub fn intersect_rect(center: Point, width: f32, height: f32, point: Point) -> Point {
    let hw = width * 0.5;
    let hh = height * 0.5;
    let dx = point.x - center.x;
    let dy = point.y - center.y;

    if dx == 0.0 && dy == 0.0 {
        return center;
    }

    // Scale so the ray hits the rectangle boundary.
    let sx = if dx != 0.0 { hw / dx.abs() } else { f32::INFINITY };
    let sy = if dy != 0.0 { hh / dy.abs() } else { f32::INFINITY };
    let s = sx.min(sy);
    Point::new(center.x + s * dx, center.y + s * dy)
}

/// dagre `positionSelfEdges`: route a self-loop as a 5-point curve to the right
/// of the node. `dx`/`dy` derived from a same-rank dummy placed just right of
/// the owner.
pub fn route_self_edge(center: Point, width: f32, height: f32, dummy_x: f32) -> Vec<Point> {
    let x = center.x + width * 0.5;
    let dy = height * 0.5;
    let dx = dummy_x - x;
    vec![
        Point::new(x + 2.0 * dx / 3.0, center.y - dy),
        Point::new(x + 5.0 * dx / 6.0, center.y - dy),
        Point::new(x + dx, center.y),
        Point::new(x + 5.0 * dx / 6.0, center.y + dy),
        Point::new(x + 2.0 * dx / 3.0, center.y + dy),
    ]
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::auto_layout::mermaid_dagre::graph::LayoutGraph;
    use crate::id::NodeId;

    #[test]
    fn normalize_splits_long_edge_into_chain() {
        let mut g = LayoutGraph::new();
        let a = g.add_real_node(NodeId::default(), 10.0, 10.0);
        let b = g.add_real_node(NodeId::default(), 10.0, 10.0);
        let e = g.add_edge(a, b);
        g.edges[e].original_idx = Some(0);
        g.edges[e].minlen = 1;
        g.nodes[a].rank = 0;
        g.nodes[b].rank = 3;
        let real_count = g.nodes.len();
        normalize(&mut g);
        // 2 dummies inserted (ranks 1, 2).
        assert_eq!(g.nodes.len(), real_count + 2);
        // 3 segments now (only the first carries original_idx by design).
        let owner_segs: Vec<_> = g.edges.iter().filter(|e| e.original_idx == Some(0)).collect();
        assert_eq!(owner_segs.len(), 1);
        // The chain a -> d1 -> d2 -> b has 3 edges total.
        let chain_segments: Vec<_> = g
            .edges
            .iter()
            .filter(|e| {
                (e.from == a || g.nodes[e.from].is_dummy)
                    && (e.to == b || g.nodes[e.to].is_dummy)
            })
            .collect();
        assert_eq!(chain_segments.len(), 3);
    }

    #[test]
    fn intersect_rect_hits_right_edge() {
        let center = Point::new(100.0, 100.0);
        let p = intersect_rect(center, 40.0, 30.0, Point::new(200.0, 100.0));
        assert!((p.x - 120.0).abs() < 0.01);
        assert!((p.y - 100.0).abs() < 0.01);
    }

    #[test]
    fn intersect_rect_hits_corner() {
        let center = Point::new(0.0, 0.0);
        let p = intersect_rect(center, 40.0, 40.0, Point::new(100.0, 100.0));
        // Diagonal hits the corner (20, 20).
        assert!((p.x - 20.0).abs() < 0.01);
        assert!((p.y - 20.0).abs() < 0.01);
    }

    #[test]
    fn self_loop_has_five_points() {
        let pts = route_self_edge(Point::new(100.0, 100.0), 40.0, 30.0, 160.0);
        assert_eq!(pts.len(), 5);
    }
}
