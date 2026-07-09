//! Main flow alignment.
//!
//! Stable algorithm: identifies the "main flow" — the primary path through the
//! graph that excludes branch targets (`if_N`/`else` targets) and loop body
//! nodes — and aligns all main-flow nodes to a single cross-axis line.
//!
//! This complements [`super::linear::align_linear_chain`], which only aligns
//! simple 1-in-1-out chains and **breaks at merge points** (in-degree ≥ 2).
//! After `align_linear_chain` straightens simple chains, this step pulls the
//! remaining main-flow nodes (that span across merge/branch points) onto the
//! same cross-axis line, using the median of already-aligned nodes as the
//! target.
//!
//! ## Algorithm
//!
//! 1. Collect **branch targets**: nodes targeted by `if_N`/`else` ports.
//! 2. Collect **excluded nodes**: loop nodes + their body nodes.
//! 3. Main-flow nodes = all nodes NOT in branch targets and NOT excluded.
//! 4. Compute the median cross-axis center of main-flow nodes.
//! 5. Align all main-flow nodes to that median.

use super::{FlowGraph, LayoutDirection, PointF};

/// Check if a port name is a Condition-style branch port (`if_N` or `else`).
fn is_branch_port(port: &str) -> bool {
    port.starts_with("if_") || port == "else"
}

/// Collect branch target node IDs: nodes targeted by `if_N`/`else` ports.
fn collect_branch_targets(graph: &FlowGraph) -> std::collections::HashSet<crate::graph::NodeId> {
    graph
        .edges()
        .filter(|e| {
            e.source_port
                .as_deref()
                .map_or(false, is_branch_port)
        })
        .map(|e| e.target)
        .collect()
}

/// Align main-flow nodes to the median cross-axis coordinate.
///
/// Main-flow nodes are nodes that are:
/// - NOT branch targets (`if_N`/`else` targets — positioned by `reorder_branch_targets`)
/// - NOT loop body nodes (positioned by `align_loop_body_target`)
/// - NOT loop nodes themselves (positioned by `align_loop_in_sources`)
///
/// This runs AFTER `align_linear_chain` (which straightens simple 1-in-1-out
/// chains) and BEFORE `reserve_loop_back_edge_space` (so Loop and its
/// successors are on the main-flow line before space reservation). The
/// median is dominated by the already-aligned simple-chain nodes, pulling
/// merge-point successors (e.g. Adapter after Search+ToolCall merge) onto
/// the same line.
pub(super) fn align_main_flow(
    graph: &FlowGraph,
    positions: &mut std::collections::HashMap<crate::graph::NodeId, PointF>,
    direction: LayoutDirection,
    loop_groups: &std::collections::HashMap<
        crate::graph::NodeId,
        std::collections::HashSet<crate::graph::NodeId>,
    >,
) {
    use std::collections::HashSet;

    let branch_targets = collect_branch_targets(graph);

    // Collect all body nodes + loop nodes into the excluded set.
    let mut excluded: HashSet<crate::graph::NodeId> = HashSet::new();
    for (loop_node, body_nodes) in loop_groups {
        excluded.insert(*loop_node);
        excluded.extend(body_nodes.iter().copied());
    }

    // Main-flow nodes.
    let main_flow_nodes: Vec<crate::graph::NodeId> = graph
        .nodes()
        .map(|n| n.id)
        .filter(|id| !branch_targets.contains(id) && !excluded.contains(id))
        .collect();

    if main_flow_nodes.len() < 2 {
        return;
    }

    // Collect cross-axis center coordinates of all main-flow nodes.
    let mut cross_coords: Vec<f32> = main_flow_nodes
        .iter()
        .filter_map(|id| {
            let pos = positions.get(id)?;
            let node = graph.node(*id)?;
            Some(match direction {
                LayoutDirection::Horizontal => pos.y + node.size.h * 0.5,
                LayoutDirection::Vertical => pos.x + node.size.w * 0.5,
            })
        })
        .collect();

    if cross_coords.is_empty() {
        return;
    }

    cross_coords.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
    let median = cross_coords[cross_coords.len() / 2];

    // Align all main-flow nodes to the median cross-axis center.
    for id in main_flow_nodes {
        let node = match graph.node(id) {
            Some(n) => n,
            None => continue,
        };
        if let Some(pos) = positions.get_mut(&id) {
            match direction {
                LayoutDirection::Horizontal => {
                    pos.y = median - node.size.h * 0.5;
                }
                LayoutDirection::Vertical => {
                    pos.x = median - node.size.w * 0.5;
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::geometry::SizeF;
    use crate::graph::{Edge, FlowGraph};

    fn build_graph() -> (FlowGraph, [crate::graph::NodeId; 6]) {
        let mut g = FlowGraph::new();
        let a = g.add_node_with_size("action", serde_json::json!({}), SizeF::new(100.0, 40.0));
        let b = g.add_node_with_size("condition", serde_json::json!({}), SizeF::new(100.0, 80.0));
        let c = g.add_node_with_size("action", serde_json::json!({}), SizeF::new(100.0, 40.0));
        let d = g.add_node_with_size("action", serde_json::json!({}), SizeF::new(100.0, 40.0));
        let e = g.add_node_with_size("action", serde_json::json!({}), SizeF::new(100.0, 40.0));
        let f = g.add_node_with_size("action", serde_json::json!({}), SizeF::new(100.0, 40.0));

        // A → B (main flow)
        g.add_edge(Edge::new(a, b));
        // B → C (if_0 branch target)
        let mut e1 = Edge::new(b, c);
        e1.source_port = Some("if_0".to_string());
        g.add_edge(e1);
        // B → D (else branch target)
        let mut e2 = Edge::new(b, d);
        e2.source_port = Some("else".to_string());
        g.add_edge(e2);
        // C → E (merge)
        g.add_edge(Edge::new(c, e));
        // D → E (merge)
        g.add_edge(Edge::new(d, e));
        // E → F (main flow after merge)
        g.add_edge(Edge::new(e, f));

        (g, [a, b, c, d, e, f])
    }

    #[test]
    fn excludes_branch_targets_and_aligns_main_flow() {
        let (graph, [a, b, c, d, e, f]) = build_graph();

        let mut positions = std::collections::HashMap::new();
        // A and B on the main line (y center = 100)
        positions.insert(a, PointF::new(0.0, 80.0)); // center y = 100
        positions.insert(b, PointF::new(200.0, 60.0)); // center y = 100
        // C and D are branch targets at different Y
        positions.insert(c, PointF::new(400.0, 0.0)); // center y = 20
        positions.insert(d, PointF::new(400.0, 200.0)); // center y = 220
        // E (merge) and F are off the main line
        positions.insert(e, PointF::new(600.0, 150.0)); // center y = 170
        positions.insert(f, PointF::new(800.0, 130.0)); // center y = 150

        let loop_groups = std::collections::HashMap::new();
        align_main_flow(&graph, &mut positions, LayoutDirection::Horizontal, &loop_groups);

        // Main flow nodes: A, B, E, F (C and D are branch targets, excluded)
        // Median of [100, 100, 170, 150] sorted = [100, 100, 150, 170] → median = 150
        // But wait: median index = 4/2 = 2 → 150.0
        // Actually let me recalculate: centers are A=100, B=100, E=170, F=150
        // sorted: [100, 100, 150, 170], median index = 2 → 150
        let a_center = positions[&a].y + 20.0;
        let b_center = positions[&b].y + 40.0;
        let e_center = positions[&e].y + 20.0;
        let f_center = positions[&f].y + 20.0;

        // All main-flow nodes should be aligned to median=150
        assert!((a_center - 150.0).abs() < 0.01, "A center y = {}", a_center);
        assert!((b_center - 150.0).abs() < 0.01, "B center y = {}", b_center);
        assert!((e_center - 150.0).abs() < 0.01, "E center y = {}", e_center);
        assert!((f_center - 150.0).abs() < 0.01, "F center y = {}", f_center);

        // Branch targets C and D should NOT be moved
        assert_eq!(positions[&c], PointF::new(400.0, 0.0), "C should not be moved");
        assert_eq!(positions[&d], PointF::new(400.0, 200.0), "D should not be moved");
    }

    #[test]
    fn handles_empty_graph() {
        let graph = FlowGraph::new();
        let mut positions = std::collections::HashMap::new();
        let loop_groups = std::collections::HashMap::new();
        // Should not panic
        align_main_flow(&graph, &mut positions, LayoutDirection::Horizontal, &loop_groups);
    }

    #[test]
    fn vertical_layout_aligns_x() {
        let (graph, [a, b, c, d, e, f]) = build_graph();

        let mut positions = std::collections::HashMap::new();
        positions.insert(a, PointF::new(80.0, 0.0)); // center x = 130
        positions.insert(b, PointF::new(60.0, 200.0)); // center x = 110
        positions.insert(c, PointF::new(0.0, 400.0)); // center x = 50
        positions.insert(d, PointF::new(300.0, 400.0)); // center x = 350
        positions.insert(e, PointF::new(150.0, 600.0)); // center x = 200
        positions.insert(f, PointF::new(130.0, 800.0)); // center x = 180

        let loop_groups = std::collections::HashMap::new();
        align_main_flow(&graph, &mut positions, LayoutDirection::Vertical, &loop_groups);

        // Main flow: A, B, E, F. Centers: 130, 110, 200, 180
        // sorted: [110, 130, 180, 200], median index = 2 → 180
        let a_center = positions[&a].x + 50.0;
        let b_center = positions[&b].x + 50.0;
        let e_center = positions[&e].x + 50.0;
        let f_center = positions[&f].x + 50.0;

        assert!((a_center - 180.0).abs() < 0.01, "A center x = {}", a_center);
        assert!((b_center - 180.0).abs() < 0.01, "B center x = {}", b_center);
        assert!((e_center - 180.0).abs() < 0.01, "E center x = {}", e_center);
        assert!((f_center - 180.0).abs() < 0.01, "F center x = {}", f_center);
    }
}
