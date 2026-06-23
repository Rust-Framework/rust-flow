//! Dagre layout engine (Sugiyama-style layered layout).
//!
//! Wraps the `dagre` crate — a complete Rust port of dagre.js with 20/20
//! cross-validation against the reference implementation. This is the same
//! algorithm family used by ReactFlow's official dagre examples.

use super::{LayoutDirection, LayoutEngine, LayoutResult};
use crate::geometry::PointF;
use crate::graph::FlowGraph;
use dagre::graph::{Graph, GraphOptions};
use dagre::{
    layout, LayoutOptions, NodeLabel, EdgeLabel, RankDir, Ranker,
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
            edgesep: 20.0,
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

        // Post-process: reorder branch target nodes to match exit port order.
        //
        // dagre's barycenter heuristic does not guarantee that sibling nodes
        // (e.g. Condition branch targets) are placed in the same order as
        // their exit ports. This step swaps positions among same-source
        // branch targets so that their vertical (horizontal layout) or
        // horizontal (vertical layout) order matches the port order, which
        // minimises edge crossings.
        reorder_branch_targets(graph, &mut positions, direction);

        // Post-process: reserve space for Loop back-edge routing.
        //
        // The loop back-edge routes around the body group:
        // - Horizontal: BELOW (down → left → up → right), occupying vertical
        //   space below the body group.
        // - Vertical: LEFT (left → up → right), occupying horizontal space
        //   to the left of the body group.
        // dagre doesn't know about this routing, so nodes may overlap the
        // back-edge. This step shifts nodes in the back-edge's routing
        // direction to clear the path.
        reserve_loop_back_edge_space(graph, &mut positions, direction);

        // Post-process: align the Loop node with its incoming source nodes
        // to minimise bends on `in` edges.
        //
        // Multiple edges may converge on the Loop's `in` port from different
        // cross-axis positions. This step moves the Loop node to the median
        // cross-axis position of its sources, reducing bends on the majority
        // of incoming edges. Must run BEFORE done/body target alignment so
        // they adjust to the new Loop position.
        align_loop_in_sources(graph, &mut positions, direction);

        // Post-process: align the Loop node's `done` target with the done port
        // to eliminate unnecessary bends in the done edge.
        align_loop_done_target(graph, &mut positions, direction);

        // Post-process: align the Loop node's `loop_body` target with the
        // loop_body port to eliminate unnecessary bends in the body entry edge.
        //
        // After dagre layout, the first body node may be at a different
        // cross-axis position than the Loop's loop_body port, causing the
        // loop_body edge to bend. This step aligns the body target's X
        // (vertical layout) or Y (horizontal layout) with the loop_body port.
        align_loop_body_target(graph, &mut positions, direction);

        // Post-process: align the forward chain after the Loop's `done` target
        // so each successor is aligned with its predecessor along the cross-axis.
        //
        // `align_loop_done_target` aligns the done target (e.g. Summarize) with
        // the Loop's done port, but the done target's successors (e.g. End) may
        // still be at a different cross-axis position, causing bends. This step
        // follows the single-successor chain and aligns each successor.
        align_post_done_chain(graph, &mut positions, direction);

        LayoutResult { positions }
    }
}

/// Branch port ordering: `if_N` → N, `else` → MAX (last), others → MAX.
///
/// `else` is rendered as the last row of the condition node (fallback
/// semantics), so its target should be placed last along the cross-rank
/// axis: bottom-most for horizontal layout, right-most for vertical layout.
///
/// This determines the visual top-to-bottom (horizontal layout) or
/// left-to-right (vertical layout) order of branch targets.
fn branch_port_order(port: &str) -> usize {
    if let Some(rest) = port.strip_prefix("if_") {
        rest.parse::<usize>().unwrap_or(usize::MAX)
    } else {
        usize::MAX
    }
}

/// Reorder branch target nodes so their arrangement matches the exit port
/// order of their source node.
///
/// For each source node that has multiple outgoing edges with `source_port`,
/// sort the target nodes by their port order and assign them to the sorted
/// positions currently occupied by those targets. This only permutes
/// positions among the group — it does not change the set of positions.
///
/// Only Condition-style ports (`if_N` / `else`) are reordered. Other
/// multi-port nodes (e.g. Loop with `loop_body` / `done`) are skipped to
/// avoid disturbing their specialised layout handled by `reposition_loop_body`.
fn reorder_branch_targets(
    graph: &FlowGraph,
    positions: &mut std::collections::HashMap<crate::graph::NodeId, PointF>,
    direction: LayoutDirection,
) {
    use std::collections::HashMap;

    // Collect targets grouped by source node, keeping the source_port for ordering.
    let mut groups: HashMap<crate::graph::NodeId, Vec<(String, crate::graph::NodeId)>> =
        HashMap::new();
    for edge in graph.edges() {
        if let Some(port) = &edge.source_port {
            groups
                .entry(edge.source)
                .or_default()
                .push((port.clone(), edge.target));
        }
    }

    for (_source, mut targets) in groups {
        if targets.len() < 2 {
            continue;
        }

        // Skip groups that don't have Condition-style ports (if_N / else).
        // This prevents reordering Loop's loop_body/done ports, which are
        // handled separately by `reposition_loop_body`.
        let has_cond_ports = targets
            .iter()
            .any(|(p, _)| p.starts_with("if_") || p == "else");
        if !has_cond_ports {
            continue;
        }

        // Sort targets by port order (if_0=0, if_1=1, ..., else=MAX).
        targets.sort_by_key(|(port, _)| branch_port_order(port));

        // Collect the current coordinates of these targets along the
        // cross-rank axis (Y for horizontal layout, X for vertical).
        let mut current_coords: Vec<f32> = targets
            .iter()
            .filter_map(|(_, nid)| positions.get(nid))
            .map(|p| match direction {
                LayoutDirection::Horizontal => p.y,
                LayoutDirection::Vertical => p.x,
            })
            .collect();

        if current_coords.len() != targets.len() {
            continue; // some targets missing positions — skip
        }

        // Sort the coordinates so we assign the smallest coordinate to the
        // first port (else), the next to if_0, etc.
        current_coords.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));

        // Assign sorted targets to sorted coordinates.
        for (i, (_, nid)) in targets.iter().enumerate() {
            if let Some(pos) = positions.get_mut(nid) {
                match direction {
                    LayoutDirection::Horizontal => pos.y = current_coords[i],
                    LayoutDirection::Vertical => pos.x = current_coords[i],
                }
            }
        }
    }
}

/// Back-edge routing space: margin (40) + approach_offset (40) + clearance (20).
/// This is the space the back-edge occupies beyond the body group's boundary.
const BACK_EDGE_RESERVE: f32 = 100.0;

/// Reserve space for Loop back-edge routing by shifting nodes in the back-edge's
/// routing direction.
///
/// **Horizontal layout**: back-edge routes BELOW the body group
/// (down → left → up → right). Shifts all non-body, non-Loop nodes whose Y is
/// below the body group's bottom Y down by `BACK_EDGE_RESERVE`.
///
/// **Vertical layout**: back-edge routes to the LEFT of the body group
/// (left → up → right). Shifts all non-body, non-Loop nodes whose X is left of
/// the body group's left X further left by `BACK_EDGE_RESERVE`.
///
/// **Algorithm**:
/// 1. Find all Loop nodes (sources of `loop_body` edges).
/// 2. BFS-expand each body group (forward edges, excluding `loop_in`
///    back-edges and edges back to the Loop node).
/// 3. Compute the body group's boundary (bottom Y for horizontal, left X for
///    vertical) from dagre positions.
/// 4. Shift all nodes beyond this boundary (excluding body + Loop) in the
///    routing direction.
fn reserve_loop_back_edge_space(
    graph: &FlowGraph,
    positions: &mut std::collections::HashMap<crate::graph::NodeId, PointF>,
    direction: LayoutDirection,
) {
    // Use the shared BFS body group computation (single source of truth).
    let loop_groups = graph.loop_body_groups();

    if loop_groups.is_empty() {
        return;
    }

    // For each Loop, shift nodes in the back-edge routing direction.
    for (loop_node, body_nodes) in &loop_groups {
        match direction {
            LayoutDirection::Horizontal => {
                // Horizontal: shift nodes BELOW the body group down.
                let body_bottom = body_nodes
                    .iter()
                    .filter_map(|nid| {
                        let pos = positions.get(nid)?;
                        let node = graph.node(*nid)?;
                        Some(pos.y + node.size.h)
                    })
                    .fold(f32::MIN, f32::max);

                let loop_bottom = match (positions.get(loop_node), graph.node(*loop_node)) {
                    (Some(pos), Some(node)) => pos.y + node.size.h,
                    _ => continue,
                };

                let group_bottom = body_bottom.max(loop_bottom);

                let nodes_to_shift: Vec<crate::graph::NodeId> = positions
                    .iter()
                    .filter(|(nid, pos)| {
                        !body_nodes.contains(nid) && **nid != *loop_node && pos.y > group_bottom
                    })
                    .map(|(nid, _)| *nid)
                    .collect();

                for nid in nodes_to_shift {
                    if let Some(pos) = positions.get_mut(&nid) {
                        pos.y += BACK_EDGE_RESERVE;
                    }
                }
            }
            LayoutDirection::Vertical => {
                // Vertical: shift nodes LEFT of the body group further left.
                let body_left = body_nodes
                    .iter()
                    .filter_map(|nid| {
                        let pos = positions.get(nid)?;
                        Some(pos.x)
                    })
                    .fold(f32::MAX, f32::min);

                let loop_left = match (positions.get(loop_node), graph.node(*loop_node)) {
                    (Some(pos), _) => pos.x,
                    _ => continue,
                };

                let group_left = body_left.min(loop_left);

                let nodes_to_shift: Vec<crate::graph::NodeId> = positions
                    .iter()
                    .filter(|(nid, pos)| {
                        !body_nodes.contains(nid) && **nid != *loop_node && pos.x < group_left
                    })
                    .map(|(nid, _)| *nid)
                    .collect();

                for nid in nodes_to_shift {
                    if let Some(pos) = positions.get_mut(&nid) {
                        pos.x -= BACK_EDGE_RESERVE;
                    }
                }
            }
        }
    }
}

/// Loop node title mid-Y offset (= TITLE_H / 2 = 36 / 2 = 18).
///
/// **Must match** `TITLE_H * 0.5` in `crates/gpui/src/builtin/loop_node.rs`.
/// The `done` port is at `(right, title_mid_y)` for horizontal layout.
/// This constant is duplicated here because core crate cannot depend on gpui.
const LOOP_TITLE_MID_Y: f32 = 18.0;

/// Loop node `loop_body` port X ratio for vertical layout.
///
/// `loop_body` port is at `(x + w * 0.25, bottom)` in vertical layout,
/// offset from center to avoid overlapping with `done` port at `(mid_x, bottom)`.
const LOOP_BODY_PORT_X_RATIO: f32 = 0.25;

/// Align the Loop node's `done` target with the `done` port position to
/// eliminate unnecessary bends in the done edge.
///
/// After dagre layout and back-edge space reservation, the done target may
/// be at a different cross-axis position than the Loop's done port, causing
/// the done edge to bend unnecessarily. This function aligns the done
/// target's Y (horizontal layout) or X (vertical layout) with the Loop's
/// done port, producing a straight done edge.
///
/// Only the cross-axis position is adjusted; the flow-axis position
/// (determined by dagre + back-edge reservation) is preserved, so the done
/// target stays at rank 2 (below the body group) and outside the back-edge
/// routing area.
///
/// Loop node port positions (from `loop_node.rs`):
/// - Horizontal: `done` = (right, title_mid_y) = (x + w, y + 18)
/// - Vertical:   `done` = (bottom, mid_x)     = (x + w/2, y + h)
fn align_loop_done_target(
    graph: &FlowGraph,
    positions: &mut std::collections::HashMap<crate::graph::NodeId, PointF>,
    direction: LayoutDirection,
) {
    for edge in graph.edges() {
        if edge.source_port.as_deref() != Some("done") {
            continue;
        }
        let loop_node = match graph.node(edge.source) {
            Some(n) => n,
            None => continue,
        };
        let target_node = match graph.node(edge.target) {
            Some(n) => n,
            None => continue,
        };
        let loop_pos = match positions.get(&edge.source) {
            Some(p) => *p,
            None => continue,
        };
        let target_pos = match positions.get_mut(&edge.target) {
            Some(p) => p,
            None => continue,
        };

        match direction {
            LayoutDirection::Horizontal => {
                // done port Y = loop_pos.y + LOOP_TITLE_MID_Y (title_mid_y)
                // Target entry port (left) Y = target_pos.y + target.h / 2
                // Straight edge: target_pos.y + target.h / 2 = loop_pos.y + LOOP_TITLE_MID_Y
                target_pos.y = loop_pos.y + LOOP_TITLE_MID_Y - target_node.size.h * 0.5;
            }
            LayoutDirection::Vertical => {
                // done port X = loop_pos.x + loop_node.w / 2 (mid_x)
                // Target entry port (top) X = target_pos.x + target.w / 2
                // Straight edge: target_pos.x + target.w / 2 = loop_pos.x + loop_node.w / 2
                target_pos.x = loop_pos.x + loop_node.size.w * 0.5 - target_node.size.w * 0.5;
            }
        }
    }
}

/// Align the Loop node's cross-axis position with the median of its incoming
/// source nodes' exit port positions, reducing bends on `in` edges.
///
/// Multiple edges may converge on the Loop's `in` port from different
/// cross-axis positions. This step moves the Loop node to the median
/// cross-axis position of its sources, minimising bends on the majority of
/// incoming edges.
///
/// Only the cross-axis position is adjusted; the flow-axis position
/// (determined by dagre's rank assignment) is preserved.
///
/// **Must run BEFORE** `align_loop_done_target` and `align_loop_body_target`
/// so they adjust to the new Loop position.
fn align_loop_in_sources(
    graph: &FlowGraph,
    positions: &mut std::collections::HashMap<crate::graph::NodeId, PointF>,
    direction: LayoutDirection,
) {
    // Find all Loop nodes (sources of `loop_body` edges).
    let loop_nodes: Vec<crate::graph::NodeId> = graph
        .edges()
        .filter(|e| e.source_port.as_deref() == Some("loop_body"))
        .map(|e| e.source)
        .collect();

    for loop_node_id in loop_nodes {
        let loop_node = match graph.node(loop_node_id) {
            Some(n) => n,
            None => continue,
        };

        // Find incoming edges that are NOT loop_in back-edges.
        // These are `in` edges (main flow into the Loop).
        let source_coords: Vec<f32> = graph
            .in_edges(loop_node_id)
            .filter(|e| e.target_port.as_deref() != Some("loop_in"))
            .filter_map(|e| {
                let src = graph.node(e.source)?;
                let pos = positions.get(&e.source)?;
                // Source exit port cross-axis coordinate:
                // - Horizontal: src Right port Y = pos.y + src.h / 2
                // - Vertical: src Bottom port X = pos.x + src.w / 2
                Some(match direction {
                    LayoutDirection::Horizontal => pos.y + src.size.h * 0.5,
                    LayoutDirection::Vertical => pos.x + src.size.w * 0.5,
                })
            })
            .collect();

        if source_coords.is_empty() {
            continue;
        }

        // Compute median of source exit port coordinates.
        let mut sorted = source_coords.clone();
        sorted.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
        let median = sorted[sorted.len() / 2];

        // Align Loop's cross-axis so `in` port matches median source exit.
        if let Some(loop_pos_mut) = positions.get_mut(&loop_node_id) {
            match direction {
                LayoutDirection::Horizontal => {
                    // in port Y = loop_pos.y + LOOP_TITLE_MID_Y
                    // Want: loop_pos.y + LOOP_TITLE_MID_Y = median
                    loop_pos_mut.y = median - LOOP_TITLE_MID_Y;
                }
                LayoutDirection::Vertical => {
                    // in port X = loop_pos.x + loop_w / 2
                    // Want: loop_pos.x + loop_w / 2 = median
                    loop_pos_mut.x = median - loop_node.size.w * 0.5;
                }
            }
        }
    }
}

/// Align the Loop node's `loop_body` target with the `loop_body` port position
/// to eliminate unnecessary bends in the body entry edge.
///
/// After dagre layout, the first body node may be at a different cross-axis
/// position than the Loop's `loop_body` port, causing the loop_body edge to
/// bend. This step aligns the body target's cross-axis position with the
/// loop_body port.
///
/// Port positions:
/// - Horizontal: `loop_body` at `(right, body_mid_y)` = `(x + w, y + 58)`
///   Body uses Left port (default): entry at `(body_x, body_y + body_h / 2)`
///   Straight: `body_y + body_h / 2 = loop_y + 58`
/// - Vertical: `loop_body` at `(x + w * 0.25, bottom)` = `(x + w*0.25, y + 80)`
///   Body uses Top port (forced): entry at `(body_x + body_w / 2, body_y)`
///   Straight: `body_x + body_w / 2 = loop_x + loop_w * 0.25`
fn align_loop_body_target(
    graph: &FlowGraph,
    positions: &mut std::collections::HashMap<crate::graph::NodeId, PointF>,
    direction: LayoutDirection,
) {
    for edge in graph.edges() {
        if edge.source_port.as_deref() != Some("loop_body") {
            continue;
        }
        let loop_node = match graph.node(edge.source) {
            Some(n) => n,
            None => continue,
        };
        let target_node = match graph.node(edge.target) {
            Some(n) => n,
            None => continue,
        };
        let loop_pos = match positions.get(&edge.source) {
            Some(p) => *p,
            None => continue,
        };
        let target_pos = match positions.get_mut(&edge.target) {
            Some(p) => p,
            None => continue,
        };

        match direction {
            LayoutDirection::Horizontal => {
                // 循环体节点始终从 Top 进入（纵向编排），无论主布局方向。
                // loop_body port X = loop_pos.x + loop_w * LOOP_BODY_PORT_X_RATIO
                // Body Top port X = target_pos.x + target.w / 2
                // Straight: target_pos.x + target.w / 2 = loop_pos.x + loop_w * RATIO
                target_pos.x = loop_pos.x + loop_node.size.w * LOOP_BODY_PORT_X_RATIO
                    - target_node.size.w * 0.5;
            }
            LayoutDirection::Vertical => {
                // loop_body port X = loop_pos.x + loop_w * LOOP_BODY_PORT_X_RATIO
                // Body Top port X = target_pos.x + target.w / 2
                // Straight: target_pos.x + target.w / 2 = loop_pos.x + loop_w * RATIO
                target_pos.x = loop_pos.x + loop_node.size.w * LOOP_BODY_PORT_X_RATIO
                    - target_node.size.w * 0.5;
            }
        }
    }
}

/// Align the forward chain after the Loop's `done` target so that each
/// successor is aligned with its predecessor along the cross-axis,
/// eliminating unnecessary bends in the main flow.
///
/// After [`align_loop_done_target`] aligns the done target (e.g. Summarize)
/// with the Loop's done port, the done target's successors (e.g. End) may
/// still be at a different cross-axis position, causing the connecting edge
/// to bend. This function follows the single-successor chain from each done
/// target and aligns each successor's cross-axis with its predecessor.
///
/// **Alignment formula** (simple nodes with center ports):
/// - Horizontal: `next.y = curr.y + (curr.h - next.h) / 2`
///   (aligns port Y = center Y for both nodes)
/// - Vertical: `next.x = curr.x + (curr.w - next.w) / 2`
///
/// Only chains with exactly one forward edge at each step are aligned —
/// branching points are left untouched. Back-edges (`loop_in`) and
/// `loop_body` edges are excluded. A visited set prevents infinite loops
/// on cyclic graphs.
fn align_post_done_chain(
    graph: &FlowGraph,
    positions: &mut std::collections::HashMap<crate::graph::NodeId, PointF>,
    direction: LayoutDirection,
) {
    use std::collections::HashSet;

    // Find all done targets (e.g. Summarize).
    let done_targets: Vec<crate::graph::NodeId> = graph
        .edges()
        .filter(|e| e.source_port.as_deref() == Some("done"))
        .map(|e| e.target)
        .collect();

    for done_target in done_targets {
        let mut visited: HashSet<crate::graph::NodeId> = HashSet::new();
        visited.insert(done_target);

        let mut current = done_target;
        loop {
            // Forward edges: exclude back-edges (loop_in) and loop_body edges.
            let forward_edges: Vec<&crate::graph::Edge> = graph
                .out_edges(current)
                .filter(|e| e.target_port.as_deref() != Some("loop_in"))
                .filter(|e| e.source_port.as_deref() != Some("loop_body"))
                .collect();

            // Only align when there's exactly one forward edge (no branching).
            if forward_edges.len() != 1 {
                break;
            }

            let next = forward_edges[0].target;
            // Cycle guard.
            if !visited.insert(next) {
                break;
            }

            let curr_pos = match positions.get(&current) {
                Some(p) => *p,
                None => break,
            };
            let curr_node = match graph.node(current) {
                Some(n) => n,
                None => break,
            };
            let next_node = match graph.node(next) {
                Some(n) => n,
                None => break,
            };
            let next_pos = match positions.get_mut(&next) {
                Some(p) => p,
                None => break,
            };

            match direction {
                LayoutDirection::Horizontal => {
                    // Align Y so center ports match:
                    // next.y + next.h/2 = curr.y + curr.h/2
                    next_pos.y = curr_pos.y + (curr_node.size.h - next_node.size.h) * 0.5;
                }
                LayoutDirection::Vertical => {
                    // Align X so center ports match:
                    // next.x + next.w/2 = curr.x + curr.w/2
                    next_pos.x = curr_pos.x + (curr_node.size.w - next_node.size.w) * 0.5;
                }
            }

            current = next;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::graph::{Edge, FlowGraph};
    use crate::SizeF;

    #[test]
    fn dagre_layouts_simple_chain() {
        let mut g = FlowGraph::new();
        let a = g.add_node_with_size("action", serde_json::json!({}), SizeF::new(100.0, 40.0));
        let b = g.add_node_with_size("action", serde_json::json!({}), SizeF::new(100.0, 40.0));
        let c = g.add_node_with_size("action", serde_json::json!({}), SizeF::new(100.0, 40.0));
        g.add_edge(Edge::new(a, b));
        g.add_edge(Edge::new(b, c));

        let result = DagreLayout::new().layout(&g, LayoutDirection::Horizontal);
        // All three nodes should have positions.
        assert_eq!(result.positions.len(), 3);
        // In horizontal layout, A should be left of B, B left of C.
        let pa = result.positions[&a];
        let pb = result.positions[&b];
        let pc = result.positions[&c];
        assert!(pa.x < pb.x, "A ({}) should be left of B ({})", pa.x, pb.x);
        assert!(pb.x < pc.x, "B ({}) should be left of C ({})", pb.x, pc.x);
    }

    #[test]
    fn dagre_handles_cycle_for_loop() {
        // Loop: A → B → A (back-edge). dagre should handle the cycle
        // without panicking and still produce positions for all nodes.
        let mut g = FlowGraph::new();
        let a = g.add_node_with_size("action", serde_json::json!({}), SizeF::new(100.0, 40.0));
        let b = g.add_node_with_size("action", serde_json::json!({}), SizeF::new(100.0, 40.0));
        g.add_edge(Edge::new(a, b));
        g.add_edge(Edge::new(b, a)); // back-edge

        let result = DagreLayout::new().layout(&g, LayoutDirection::Vertical);
        assert_eq!(result.positions.len(), 2);
    }

    #[test]
    fn branch_targets_reordered_to_match_port_order() {
        // Condition node with 3 branches: if_0, if_1, else → targets T1, T2, T0.
        // After layout, targets must be ordered top-to-bottom (horizontal layout)
        // matching port order: if_0 → if_1 → else (else is last / fallback).
        let mut g = FlowGraph::new();
        let cond = g.add_node_with_size("condition", serde_json::json!({}), SizeF::new(200.0, 100.0));
        let t0 = g.add_node_with_size("action", serde_json::json!({}), SizeF::new(100.0, 40.0));
        let t1 = g.add_node_with_size("action", serde_json::json!({}), SizeF::new(100.0, 40.0));
        let t2 = g.add_node_with_size("action", serde_json::json!({}), SizeF::new(100.0, 40.0));
        let sink = g.add_node_with_size("end", serde_json::json!({}), SizeF::new(100.0, 40.0));

        let mut e_else = Edge::new(cond, t0);
        e_else.source_port = Some("else".to_string());
        let mut e_if0 = Edge::new(cond, t1);
        e_if0.source_port = Some("if_0".to_string());
        let mut e_if1 = Edge::new(cond, t2);
        e_if1.source_port = Some("if_1".to_string());
        g.add_edge(e_else);
        g.add_edge(e_if0);
        g.add_edge(e_if1);
        // All targets converge to sink — forces them into the same rank.
        g.add_edge(Edge::new(t0, sink));
        g.add_edge(Edge::new(t1, sink));
        g.add_edge(Edge::new(t2, sink));

        let result = DagreLayout::new().layout(&g, LayoutDirection::Horizontal);

        // In horizontal layout, targets should be ordered by Y:
        //   if_0 (t1) top-most, if_1 (t2) middle, else (t0) bottom (fallback).
        let y_else = result.positions[&t0].y;
        let y_if0 = result.positions[&t1].y;
        let y_if1 = result.positions[&t2].y;
        assert!(
            y_if0 <= y_if1 && y_if1 <= y_else,
            "if_0 ({}) should be above if_1 ({}) above else ({})",
            y_if0, y_if1, y_else
        );
    }
}
