//! Cycle breaking — dagre `acyclic.js` port.
//!
//! Two strategies, matching `@dagrejs/dagre`:
//! - **DFS FAS** (default, Mermaid uses this): depth-first search; any out-edge
//!   to a node currently on the recursion stack is a back-edge → added to the
//!   feedback arc set and reversed.
//! - **Greedy** (Eades–Lin–Smyth): iterative removal of sources/sinks, then
//!   reverse remaining edges. Selected via [`Acyclicer::Greedy`].
//!
//! Reversed edges are flagged `reversed = true` so `undo` can restore direction
//! and the route point list can be flipped at the end of the pipeline.

use crate::auto_layout::mermaid_dagre::graph::LayoutGraph;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Acyclicer {
    /// DFS feedback arc set (dagre / Mermaid default).
    DfsFas,
    /// Greedy Eades–Lin–Smyth heuristic.
    Greedy,
}

impl Default for Acyclicer {
    fn default() -> Self {
        Self::DfsFas
    }
}

/// Break cycles in `g` by reversing feedback edges. Returns the set of edge
/// indices that were reversed.
pub fn run(g: &mut LayoutGraph, strategy: Acyclicer) -> Vec<usize> {
    match strategy {
        Acyclicer::DfsFas => dfs_fas(g),
        Acyclicer::Greedy => greedy_fas(g),
    }
}

/// Restore original edge direction. Mirrors dagre `acyclic.undo`.
pub fn undo(g: &mut LayoutGraph, reversed: &[usize]) {
    for &idx in reversed {
        if let Some(e) = g.edges.get_mut(idx) {
            std::mem::swap(&mut e.from, &mut e.to);
            e.reversed = false;
            // Flip the route point list so it runs from original source → target.
            if e.points.len() >= 2 {
                e.points.reverse();
            }
        }
    }
}

/// DFS feedback arc set. O(V + E).
fn dfs_fas(g: &mut LayoutGraph) -> Vec<usize> {
    let n = g.nodes.len();
    // 0 = unvisited, 1 = on stack, 2 = done
    let mut state = vec![0u8; n];
    let mut reversed: Vec<usize> = Vec::new();

    // Snapshot edge endpoints so we can mutate `g.edges` (reversal) during DFS.
    let edge_ends: Vec<(usize, usize)> = g.edges.iter().map(|e| (e.from, e.to)).collect();

    // Adjacency: from -> [(edge_idx, to)]
    let mut adj: Vec<Vec<(usize, usize)>> = vec![Vec::new(); n];
    for (i, &(from, to)) in edge_ends.iter().enumerate() {
        adj[from].push((i, to));
    }

    for start in 0..n {
        if state[start] != 0 {
            continue;
        }
        dfs_visit(start, &adj, g, &mut state, &mut reversed);
    }

    reversed
}

fn dfs_visit(
    v: usize,
    adj: &[Vec<(usize, usize)>],
    g: &mut LayoutGraph,
    state: &mut [u8],
    reversed: &mut Vec<usize>,
) {
    // Iterative DFS to avoid stack overflow on large graphs.
    // Each stack frame records (node, next-neighbor-index).
    let mut stack: Vec<(usize, usize)> = vec![(v, 0)];
    state[v] = 1;

    while let Some(&(node, next)) = stack.last() {
        let neighbors = &adj[node];
        if next >= neighbors.len() {
            state[node] = 2;
            stack.pop();
            continue;
        }
        // Advance the neighbor cursor on the top frame.
        let (edge_idx, w) = neighbors[next];
        stack.last_mut().unwrap().1 = next + 1;

        match state[w] {
            0 => {
                state[w] = 1;
                stack.push((w, 0));
            }
            1 => {
                // Back-edge: reverse it to break the cycle.
                if let Some(e) = g.edges.get_mut(edge_idx) {
                    std::mem::swap(&mut e.from, &mut e.to);
                    e.reversed = true;
                }
                reversed.push(edge_idx);
            }
            _ => {}
        }
    }
}

/// Greedy Eades–Lin–Smyth feedback arc set.
///
/// Repeatedly remove sources (s1) and sinks (s2); when neither remains, remove
/// the node with max (out-degree − in-degree). Edges from already-removed
/// "right" nodes to "left" nodes are feedback edges and get reversed.
fn greedy_fas(g: &mut LayoutGraph) -> Vec<usize> {
    let n = g.nodes.len();
    let mut adj_out: Vec<Vec<usize>> = vec![Vec::new(); n]; // node -> edge idx
    let mut adj_in: Vec<Vec<usize>> = vec![Vec::new(); n];
    for (i, e) in g.edges.iter().enumerate() {
        adj_out[e.from].push(i);
        adj_in[e.to].push(i);
    }

    // `side[v]`: 0 = undecided, 1 = left (s1), 2 = right (s2).
    let mut side = vec![0u8; n];
    let mut remaining = n;
    let mut s1: Vec<usize> = Vec::new();
    let mut s2: Vec<usize> = Vec::new();

    // Track current in/out degree over undecided subgraph.
    let mut out_deg: Vec<usize> = adj_out.iter().map(|v| v.len()).collect();
    let mut in_deg: Vec<usize> = adj_in.iter().map(|v| v.len()).collect();

    while remaining > 0 {
        // Remove all sinks (out_deg == 0) in the undecided subgraph.
        let mut found_sink = true;
        while found_sink {
            found_sink = false;
            for v in 0..n {
                if side[v] == 0 && out_deg[v] == 0 {
                    side[v] = 2;
                    s2.push(v);
                    remaining -= 1;
                    found_sink = true;
                    // Decrement in-degree of predecessors still undecided.
                    for &eidx in &adj_in[v] {
                        let u = g.edges[eidx].from;
                        if side[u] == 0 {
                            out_deg[u] = out_deg[u].saturating_sub(1);
                        }
                    }
                }
            }
        }
        // Remove all sources (in_deg == 0).
        let mut found_source = true;
        while found_source {
            found_source = false;
            for v in 0..n {
                if side[v] == 0 && in_deg[v] == 0 {
                    side[v] = 1;
                    s1.push(v);
                    remaining -= 1;
                    found_source = true;
                    for &eidx in &adj_out[v] {
                        let w = g.edges[eidx].to;
                        if side[w] == 0 {
                            in_deg[w] = in_deg[w].saturating_sub(1);
                        }
                    }
                }
            }
        }
        if remaining == 0 {
            break;
        }
        // Pick the undecided node with max (out - in); break ties by index.
        let mut best = None;
        let mut best_delta = i64::MIN;
        for v in 0..n {
            if side[v] == 0 {
                let delta = out_deg[v] as i64 - in_deg[v] as i64;
                if delta > best_delta {
                    best_delta = delta;
                    best = Some(v);
                }
            }
        }
        if let Some(v) = best {
            side[v] = 1;
            s1.push(v);
            remaining -= 1;
            for &eidx in &adj_out[v] {
                let w = g.edges[eidx].to;
                if side[w] == 0 {
                    in_deg[w] = in_deg[w].saturating_sub(1);
                }
            }
            for &eidx in &adj_in[v] {
                let u = g.edges[eidx].from;
                if side[u] == 0 {
                    out_deg[u] = out_deg[u].saturating_sub(1);
                }
            }
        } else {
            break;
        }
    }

    // Order: s1 (left) followed by reversed s2 (right).
    let mut order: Vec<usize> = s1;
    s2.reverse();
    order.extend(s2);
    let mut pos = vec![0usize; n];
    for (i, &v) in order.iter().enumerate() {
        pos[v] = i;
    }

    let mut reversed: Vec<usize> = Vec::new();
    for (i, e) in g.edges.iter().enumerate() {
        // Edge goes "right → left" → feedback.
        if pos[e.to] < pos[e.from] {
            reversed.push(i);
        }
    }
    for &i in &reversed {
        let e = &mut g.edges[i];
        std::mem::swap(&mut e.from, &mut e.to);
        e.reversed = true;
    }
    reversed
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::auto_layout::mermaid_dagre::graph::LayoutGraph;
    use crate::id::NodeId;

    fn build_cycle() -> LayoutGraph {
        // A -> B -> C -> A (a 3-cycle).
        let mut g = LayoutGraph::new();
        let a = g.add_real_node(NodeId::default(), 10.0, 10.0);
        let b = g.add_real_node(NodeId::default(), 10.0, 10.0);
        let c = g.add_real_node(NodeId::default(), 10.0, 10.0);
        g.add_edge(a, b);
        g.add_edge(b, c);
        g.add_edge(c, a);
        g
    }

    fn is_acyclic(g: &LayoutGraph) -> bool {
        let n = g.nodes.len();
        let mut adj: Vec<Vec<usize>> = vec![Vec::new(); n];
        for e in &g.edges {
            adj[e.from].push(e.to);
        }
        let mut state = vec![0u8; n];
        for s in 0..n {
            if state[s] != 0 {
                continue;
            }
            let mut stack = vec![s];
            while let Some(v) = stack.pop() {
                if state[v] == 1 {
                    state[v] = 2;
                    continue;
                }
                if state[v] == 2 {
                    continue;
                }
                state[v] = 1;
                stack.push(v);
                for &w in &adj[v] {
                    if state[w] == 1 {
                        return false;
                    }
                    if state[w] == 0 {
                        stack.push(w);
                    }
                }
            }
        }
        true
    }

    #[test]
    fn dfs_fas_breaks_cycle() {
        let mut g = build_cycle();
        let rev = run(&mut g, Acyclicer::DfsFas);
        assert_eq!(rev.len(), 1);
        assert!(is_acyclic(&g));
        // Undo restores original direction.
        undo(&mut g, &rev);
        assert!(!is_acyclic(&g));
    }

    #[test]
    fn greedy_fas_breaks_cycle() {
        let mut g = build_cycle();
        let rev = run(&mut g, Acyclicer::Greedy);
        assert_eq!(rev.len(), 1);
        assert!(is_acyclic(&g));
    }

    #[test]
    fn dag_unchanged() {
        let mut g = LayoutGraph::new();
        let a = g.add_real_node(NodeId::default(), 10.0, 10.0);
        let b = g.add_real_node(NodeId::default(), 10.0, 10.0);
        let c = g.add_real_node(NodeId::default(), 10.0, 10.0);
        g.add_edge(a, b);
        g.add_edge(b, c);
        let rev = run(&mut g, Acyclicer::DfsFas);
        assert!(rev.is_empty());
        assert!(is_acyclic(&g));
    }
}
