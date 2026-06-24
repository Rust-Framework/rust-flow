//! Linear chain alignment.
//!
//! Stable algorithm: traverses nodes in topological order (Kahn's algorithm)
//! and aligns each linear successor's cross-axis center with its predecessor's,
//! producing straight edges in the main flow. Independent of node-specific
//! layout (Loop etc.) and rarely changes.

use super::{FlowGraph, LayoutDirection, PointF};

/// Whether an edge is a loop-related "special" edge that should be excluded
/// from linear chain alignment.
///
/// Excludes:
/// - `loop_body` edges (Loop → body entry)
/// - `done` edges (Loop → exit target)
/// - `loop_in` edges (body → Loop back-edge)
fn is_loop_edge(edge: &crate::graph::Edge) -> bool {
    match edge.source_port.as_deref() {
        Some("loop_body") | Some("done") => true,
        _ => edge.target_port.as_deref() == Some("loop_in"),
    }
}

/// Align linear chains along the cross-axis to eliminate unnecessary bends
/// in the main flow.
///
/// A **linear chain** is a sequence of nodes connected by non-loop edges
/// where each intermediate node has exactly one non-loop in-edge and one
/// non-loop out-edge. This function traverses nodes in topological order
/// (Kahn's algorithm) and aligns each linear successor's cross-axis center
/// with its predecessor's, producing straight edges.
///
/// **Alignment formula** (center-port alignment so the edge is a straight line):
/// - Horizontal: `next.y = curr.y + (curr.h - next.h) / 2`
///   (aligns `next.y + next.h/2 == curr.y + curr.h/2`, i.e. port Y matches)
/// - Vertical: `next.x = curr.x + (curr.w - next.w) / 2`
///
/// **Chain breaks** at:
/// - Branch sources (non-loop out-degree ≥ 2): successors are branch targets
///   whose positions are controlled by `reorder_branch_targets`.
/// - Merge targets (non-loop in-degree ≥ 2): the node is a convergence point
///   whose position is determined by dagre.
/// - Loop-related edges (excluded by [`is_loop_edge`]).
///
/// Runs after `reorder_branch_targets` (so branch target ordering is
/// preserved) and before `align_loop_*` (so loop alignment can fine-tune
/// based on the already-aligned main flow).
pub(super) fn align_linear_chain(
    graph: &FlowGraph,
    positions: &mut std::collections::HashMap<crate::graph::NodeId, PointF>,
    direction: LayoutDirection,
) {
    use std::collections::{HashMap, VecDeque};

    // 1. Compute original non-loop in-degree, out-degree, and adjacency.
    let mut orig_in_deg: HashMap<crate::graph::NodeId, usize> = HashMap::new();
    let mut orig_out_deg: HashMap<crate::graph::NodeId, usize> = HashMap::new();
    let mut out_adj: HashMap<crate::graph::NodeId, Vec<crate::graph::NodeId>> = HashMap::new();

    for node in graph.nodes() {
        orig_in_deg.entry(node.id).or_insert(0);
        orig_out_deg.entry(node.id).or_insert(0);
        out_adj.entry(node.id).or_insert(Vec::new());
    }
    for edge in graph.edges() {
        if is_loop_edge(edge) {
            continue;
        }
        *orig_in_deg.entry(edge.target).or_insert(0) += 1;
        *orig_out_deg.entry(edge.source).or_insert(0) += 1;
        out_adj.entry(edge.source).or_insert_with(Vec::new).push(edge.target);
    }

    // 2. Kahn's topological sort with remaining in-degree.
    let mut rem_in_deg = orig_in_deg.clone();
    let mut queue: VecDeque<crate::graph::NodeId> = rem_in_deg
        .iter()
        .filter(|(_, &deg)| deg == 0)
        .map(|(&id, _)| id)
        .collect();

    let mut visited: std::collections::HashSet<crate::graph::NodeId> =
        std::collections::HashSet::new();

    while let Some(u) = queue.pop_front() {
        if !visited.insert(u) {
            continue;
        }

        // 3. If u is a linear node (exactly 1 non-loop out-edge), align its
        //    successor v — but only if v is also linear (exactly 1 non-loop
        //    in-edge), i.e. v is not a merge target.
        if orig_out_deg.get(&u).copied() == Some(1) {
            if let Some(succs) = out_adj.get(&u) {
                if let Some(&v) = succs.first() {
                    if orig_in_deg.get(&v).copied() == Some(1) {
                        // Align v's cross-axis center to u's.
                        let u_pos = match positions.get(&u) {
                            Some(p) => *p,
                            None => continue,
                        };
                        let u_node = match graph.node(u) {
                            Some(n) => n,
                            None => continue,
                        };
                        let v_node = match graph.node(v) {
                            Some(n) => n,
                            None => continue,
                        };
                        if let Some(v_pos) = positions.get_mut(&v) {
                            match direction {
                                LayoutDirection::Horizontal => {
                                    v_pos.y = u_pos.y
                                        + (u_node.size.h - v_node.size.h) * 0.5;
                                }
                                LayoutDirection::Vertical => {
                                    v_pos.x = u_pos.x
                                        + (u_node.size.w - v_node.size.w) * 0.5;
                                }
                            }
                        }
                    }
                }
            }
        }

        // 4. Decrement remaining in-degree for successors and enqueue.
        if let Some(succs) = out_adj.get(&u) {
            for &v in succs {
                if let Some(deg) = rem_in_deg.get_mut(&v) {
                    *deg = deg.saturating_sub(1);
                    if *deg == 0 {
                        queue.push_back(v);
                    }
                }
            }
        }
    }
}
