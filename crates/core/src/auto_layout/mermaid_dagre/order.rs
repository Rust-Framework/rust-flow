//! Crossing reduction — dagre `order/index.js` port.
//!
//! Barycenter heuristic with alternating up/down sweeps, keep-best layering.
//! Mirrors dagre's `initOrder` + 24-sweep loop (`lastBest < 4`).
//!
//! For each sweep direction we recompute each node's barycenter = weighted mean
//! of its neighbors' current orders, then stable-sort each layer by barycenter.
//! The layering with the fewest crossings across all sweeps is kept.

use crate::auto_layout::mermaid_dagre::graph::LayoutGraph;

const MAX_SWEEPS: u32 = 24;

/// Assign `order` to every node, minimizing crossings.
pub fn run(g: &mut LayoutGraph) {
    if g.nodes.is_empty() {
        return;
    }
    init_order(g);

    let max_rank = g.max_rank();
    if max_rank <= 0 {
        return;
    }

    let mut best: Vec<usize> = g.nodes.iter().map(|n| n.order).collect();
    let mut best_cc = cross_count(g);

    let mut last_best = 0u32;
    let mut i = 0u32;
    while last_best < 4 && i < MAX_SWEEPS {
        let down = i % 2 == 0;
        if down {
            sweep_down(g, max_rank);
        } else {
            sweep_up(g, max_rank);
        }
        let cc = cross_count(g);
        if cc < best_cc {
            best_cc = cc;
            best = g.nodes.iter().map(|n| n.order).collect();
            last_best = 0;
        } else {
            last_best += 1;
        }
        i += 1;
    }

    for (idx, &order) in best.iter().enumerate() {
        g.nodes[idx].order = order;
    }
}

/// Initial ordering: BFS from rank-0 sources, assigning order by first-seen.
/// Matches dagre `initOrder` (BFS variant).
fn init_order(g: &mut LayoutGraph) {
    let max_rank = g.max_rank();
    let mut layers: Vec<Vec<usize>> = vec![Vec::new(); (max_rank + 1) as usize];
    for (i, n) in g.nodes.iter().enumerate() {
        if n.rank >= 0 {
            layers[n.rank as usize].push(i);
        }
    }

    // Order each layer by a stable BFS from the top.
    let mut order_counter = vec![0usize; (max_rank + 1) as usize];
    let mut visited = vec![false; g.nodes.len()];

    let mut queue: std::collections::VecDeque<usize> = std::collections::VecDeque::new();
    // Seed with rank-0 nodes in their current (insertion) order.
    for &v in &layers[0] {
        queue.push_back(v);
        visited[v] = true;
    }

    while let Some(v) = queue.pop_front() {
        let r = g.nodes[v].rank as usize;
        g.nodes[v].order = order_counter[r];
        order_counter[r] += 1;
        // Enqueue successors in stable order.
        let mut succs: Vec<usize> = g.succs(v);
        succs.sort_unstable();
        for w in succs {
            if !visited[w] {
                visited[w] = true;
                queue.push_back(w);
            }
        }
    }

    // Fallback: any unvisited node (disconnected) gets the next slot in its rank.
    for (i, n) in g.nodes.iter_mut().enumerate() {
        if !visited[i] && n.rank >= 0 {
            let r = n.rank as usize;
            n.order = order_counter[r];
            order_counter[r] += 1;
        }
    }
}

/// Down sweep: layer 1..=max_rank, sort each by barycenter of predecessors.
fn sweep_down(g: &mut LayoutGraph, max_rank: i32) {
    for rank in 1..=max_rank {
        sort_layer_by_neighbor_barycenter(g, rank, true);
    }
}

/// Up sweep: layer (max_rank-1)..=0, sort each by barycenter of successors.
fn sweep_up(g: &mut LayoutGraph, max_rank: i32) {
    for rank in (0..max_rank).rev() {
        sort_layer_by_neighbor_barycenter(g, rank, false);
    }
}

/// Sort the layer at `rank` by the weighted barycenter of its neighbors.
/// `use_predecessors = true` → look at rank-1 neighbors; else rank+1 neighbors.
fn sort_layer_by_neighbor_barycenter(g: &mut LayoutGraph, rank: i32, use_predecessors: bool) {
    let layer: Vec<usize> = g
        .nodes
        .iter()
        .enumerate()
        .filter(|(_, n)| n.rank == rank)
        .map(|(i, _)| i)
        .collect();
    if layer.len() <= 1 {
        return;
    }

    // Compute barycenter for each node in the layer.
    let mut keyed: Vec<(usize, f32, usize)> = layer
        .iter()
        .map(|&v| {
            let neighbors: Vec<(usize, f32)> = if use_predecessors {
                g.in_edges(v)
                    .map(|(_, e)| (e.from, e.weight))
                    .collect()
            } else {
                g.out_edges(v)
                    .map(|(_, e)| (e.to, e.weight))
                    .collect()
            };
            let bc = barycenter(&neighbors, g, v);
            (v, bc, g.nodes[v].order)
        })
        .collect();

    // Stable sort by barycenter, tie-break by current order (keep-first, dagre v0.8.5).
    keyed.sort_by(|a, b| {
        a.1.partial_cmp(&b.1)
            .unwrap_or(std::cmp::Ordering::Equal)
            .then(a.2.cmp(&b.2))
    });

    for (pos, &(v, _, _)) in keyed.iter().enumerate() {
        g.nodes[v].order = pos;
    }
}

/// Weighted barycenter: Σ(weight * order(neighbor)) / Σ(weight).
/// Falls back to the node's own order when it has no neighbors on that side.
fn barycenter(neighbors: &[(usize, f32)], g: &LayoutGraph, v: usize) -> f32 {
    if neighbors.is_empty() {
        return g.nodes[v].order as f32;
    }
    let mut wsum = 0.0f32;
    let mut ord_sum = 0.0f32;
    for &(u, w) in neighbors {
        wsum += w;
        ord_sum += w * g.nodes[u].order as f32;
    }
    if wsum > 0.0 {
        ord_sum / wsum
    } else {
        g.nodes[v].order as f32
    }
}

/// Count crossings across all adjacent layer pairs.
/// O(E1 * E2) per pair — matches dagre `crossCount`.
fn cross_count(g: &LayoutGraph) -> usize {
    let max_rank = g.max_rank();
    let mut total = 0usize;
    for rank in 0..max_rank {
        total += crossings_between(g, rank, rank + 1);
    }
    total
}

fn crossings_between(g: &LayoutGraph, rank_a: i32, rank_b: i32) -> usize {
    // Collect edges from rank_a → rank_b as (order_in_a, order_in_b).
    let mut pairs: Vec<(usize, usize)> = Vec::new();
    for (_eidx, e) in g.edges.iter().enumerate() {
        let (from_rank, to_rank, from_order, to_order) =
            if g.nodes[e.from].rank == rank_a && g.nodes[e.to].rank == rank_b {
                (g.nodes[e.from].rank, g.nodes[e.to].rank, g.nodes[e.from].order, g.nodes[e.to].order)
            } else if g.nodes[e.to].rank == rank_a && g.nodes[e.from].rank == rank_b {
                // Reversed edge during cycle breaking still spans these ranks.
                (g.nodes[e.to].rank, g.nodes[e.from].rank, g.nodes[e.to].order, g.nodes[e.from].order)
            } else {
                continue;
            };
        let _ = (from_rank, to_rank);
        pairs.push((from_order, to_order));
    }
    if pairs.len() < 2 {
        return 0;
    }
    // Sort by source order, then count inversions in target order via merge sort.
    pairs.sort_unstable_by(|a, b| a.0.cmp(&b.0).then(a.1.cmp(&b.1)));
    let targets: Vec<usize> = pairs.iter().map(|p| p.1).collect();
    count_inversions(&targets)
}

fn count_inversions(arr: &[usize]) -> usize {
    if arr.len() < 2 {
        return 0;
    }
    let mut a = arr.to_vec();
    let mut buf = vec![0usize; arr.len()];
    merge_count(&mut a, &mut buf, 0, arr.len())
}

fn merge_count(a: &mut [usize], buf: &mut [usize], lo: usize, hi: usize) -> usize {
    if hi - lo < 2 {
        return 0;
    }
    let mid = lo + (hi - lo) / 2;
    let mut inv = merge_count(a, buf, lo, mid);
    inv += merge_count(a, buf, mid, hi);

    let mut i = lo;
    let mut j = mid;
    let mut k = lo;
    while i < mid && j < hi {
        if a[i] <= a[j] {
            buf[k] = a[i];
            i += 1;
        } else {
            buf[k] = a[j];
            j += 1;
            inv += mid - i;
        }
        k += 1;
    }
    while i < mid {
        buf[k] = a[i];
        i += 1;
        k += 1;
    }
    while j < hi {
        buf[k] = a[j];
        j += 1;
        k += 1;
    }
    a[lo..hi].copy_from_slice(&buf[lo..hi]);
    inv
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::auto_layout::mermaid_dagre::graph::LayoutGraph;
    use crate::id::NodeId;

    #[test]
    fn order_eliminates_simple_crossing() {
        // Layer 0: A, B ; Layer 1: C, D
        // Edges A->D, B->C  (crossing). After ordering, should become A->C, B->D
        // arrangement (i.e. C under A, D under B) with 0 crossings.
        let mut g = LayoutGraph::new();
        let a = g.add_real_node(NodeId::default(), 10.0, 10.0);
        let b = g.add_real_node(NodeId::default(), 10.0, 10.0);
        let c = g.add_real_node(NodeId::default(), 10.0, 10.0);
        let d = g.add_real_node(NodeId::default(), 10.0, 10.0);
        g.nodes[a].rank = 0;
        g.nodes[b].rank = 0;
        g.nodes[c].rank = 1;
        g.nodes[d].rank = 1;
        // Seed orders so A<B and C<D initially (which crosses).
        g.nodes[a].order = 0;
        g.nodes[b].order = 1;
        g.nodes[c].order = 0;
        g.nodes[d].order = 1;
        g.add_edge(a, d);
        g.add_edge(b, c);
        run(&mut g);
        assert_eq!(cross_count(&g), 0, "expected zero crossings after ordering");
    }

    #[test]
    fn order_preserves_chain_ranks() {
        let mut g = LayoutGraph::new();
        let a = g.add_real_node(NodeId::default(), 10.0, 10.0);
        let b = g.add_real_node(NodeId::default(), 10.0, 10.0);
        let c = g.add_real_node(NodeId::default(), 10.0, 10.0);
        g.nodes[a].rank = 0;
        g.nodes[b].rank = 1;
        g.nodes[c].rank = 2;
        g.add_edge(a, b);
        g.add_edge(b, c);
        run(&mut g);
        assert_eq!(cross_count(&g), 0);
    }

    #[test]
    fn inversions_counted_correctly() {
        assert_eq!(count_inversions(&[1, 2, 3, 4]), 0);
        assert_eq!(count_inversions(&[4, 3, 2, 1]), 6);
        assert_eq!(count_inversions(&[2, 1, 3]), 1);
    }
}
