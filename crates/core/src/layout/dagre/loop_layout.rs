//! Loop-specific post-processing.
//!
//! Volatile module: tightly coupled to Loop node port semantics and rendering
//! layer port positions. Isolated here so changes to Loop layout strategy
//! don't disturb the stable algorithms in [`super::branch`] and
//! [`super::linear`].
//!
//! ## Post-processing functions (called in order from [`super::mod`])
//!
//! 1. `reserve_loop_back_edge_space` — shift nodes below body group.
//! 2. `align_loop_in_sources` — move Loop to median of incoming sources.
//! 3. `align_loop_done_target` — straighten done edge.
//! 4. `align_loop_body_target` — position body group right of Loop.
//! 5. `align_post_done_chain` — straighten forward chain after done target.

use super::{FlowGraph, LayoutDirection, PointF};

/// Back-edge routing space: margin (40) + approach_offset (40) + clearance (20).
/// This is the space the back-edge occupies beyond the body group's boundary.
const BACK_EDGE_RESERVE: f32 = 100.0;

/// Loop node title mid-Y offset (= TITLE_H / 2 = 36 / 2 = 18).
///
/// **Must match** `TITLE_H * 0.5` in `crates/gpui/src/builtin/loop_node.rs`.
/// The `done` port is at `(right, title_mid_y)` for horizontal layout.
/// This constant is duplicated here because core crate cannot depend on gpui.
const LOOP_TITLE_MID_Y: f32 = 18.0;

/// Horizontal gap between Loop node and the first body node (to its right).
const LOOP_BODY_GAP: f32 = 80.0;

/// Vertical separation between stacked body nodes.
const LOOP_BODY_VSEP: f32 = 50.0;

/// Reserve space for Loop back-edge routing by shifting nodes in the back-edge's
/// routing direction.
///
/// **Both layouts**: back-edge routes BELOW the body group
/// (down → left → up → right). Shifts all non-body, non-Loop nodes whose Y is
/// below the body group's bottom Y down by `BACK_EDGE_RESERVE`.
///
/// This is consistent with `loop_back_path` (always 5-point below-routing)
/// and `align_loop_body_target` (always stacks body nodes to the RIGHT of
/// the Loop, vertically), so the vertical layout uses the same algorithm as
/// the horizontal layout.
///
/// **Algorithm**:
/// 1. Find all Loop nodes (sources of `loop_body` edges).
/// 2. BFS-expand each body group (forward edges, excluding `loop_in`
///    back-edges and edges back to the Loop node).
/// 3. Compute the body group's bottom Y from dagre positions.
/// 4. Shift all nodes below this boundary (excluding body + Loop) down.
pub(super) fn reserve_loop_back_edge_space(
    graph: &FlowGraph,
    positions: &mut std::collections::HashMap<crate::graph::NodeId, PointF>,
    _direction: LayoutDirection,
) {
    // Use the shared BFS body group computation (single source of truth).
    let loop_groups = graph.loop_body_groups();

    if loop_groups.is_empty() {
        return;
    }

    // For each Loop, shift nodes BELOW the body group down (both layouts).
    for (loop_node, body_nodes) in &loop_groups {
        // Body group bottom Y = max(body_node.y + body_node.h)
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
}

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
pub(super) fn align_loop_done_target(
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
pub(super) fn align_loop_in_sources(
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

/// Reposition the Loop's entire body group: place all body nodes to the
/// RIGHT of the Loop, stacked vertically (top-to-bottom).
///
/// **Both layouts** use the same positioning:
/// 1. The first body node (target of `loop_body` edge) is placed to the
///    RIGHT of the Loop, with its Top port Y at the Loop's BOTTOM
///    (`loop_pos.y + loop_node.size.h`). This ensures the `loop_body` edge
///    exits RIGHT from the Loop and bends DOWN into the body node's Top
///    port (右出-下拐), rather than being a horizontal line at the same Y.
/// 2. Each subsequent body node is placed directly below the previous one
///    with `LOOP_BODY_VSEP` vertical gap.
///
/// This produces a vertical body sub-flow (上进下出) regardless of the main
/// layout direction, matching the `force_src_bottom`/`force_dst_top` port
/// forcing in the rendering layer.
pub(super) fn align_loop_body_target(
    graph: &FlowGraph,
    positions: &mut std::collections::HashMap<crate::graph::NodeId, PointF>,
    _direction: LayoutDirection,
) {
    let loop_groups = graph.loop_body_groups();

    for (loop_node_id, body_nodes) in &loop_groups {
        let loop_node = match graph.node(*loop_node_id) {
            Some(n) => n,
            None => continue,
        };
        let loop_pos = match positions.get(loop_node_id) {
            Some(p) => *p,
            None => continue,
        };

        // Find the first body node (target of loop_body edge)
        let first_body_id = match graph
            .edges()
            .find(|e| e.source == *loop_node_id && e.source_port.as_deref() == Some("loop_body"))
        {
            Some(e) => e.target,
            None => continue,
        };

        // Position the first body node to the RIGHT of the Loop, at the Loop's
        // bottom Y level (右下角). This makes the loop_body edge go RIGHT then
        // DOWN (右出-下拐) from the loop_body port (Y = loop_pos.y + 58) to the
        // body node's Top port (Y = loop_pos.y + loop_node.size.h).
        let body_x = loop_pos.x + loop_node.size.w + LOOP_BODY_GAP;
        let mut current_y = loop_pos.y + loop_node.size.h;

        // Follow the body chain and stack vertically
        let mut current_id = first_body_id;
        let mut visited = std::collections::HashSet::new();

        loop {
            // Prevent infinite loops on cyclic body graphs
            if !visited.insert(current_id) {
                break;
            }

            let current_node = match graph.node(current_id) {
                Some(n) => n,
                None => break,
            };
            let current_height = current_node.size.h;

            if let Some(pos) = positions.get_mut(&current_id) {
                pos.x = body_x;
                pos.y = current_y;
            }

            // Find next body node in the chain (forward edge within body group)
            let next_id = match graph
                .edges()
                .find(|e| e.source == current_id && body_nodes.contains(&e.target))
            {
                Some(e) => e.target,
                None => break,
            };

            current_id = next_id;
            current_y += current_height + LOOP_BODY_VSEP;
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
pub(super) fn align_post_done_chain(
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
