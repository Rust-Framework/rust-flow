//! Branch target reordering.
//!
//! Stable algorithm: reorders Condition-style branch targets (`if_N`/`else`)
//! to match their exit port order, minimising edge crossings. This is
//! independent of node-specific layout (Loop etc.) and rarely changes.

use super::{FlowGraph, LayoutDirection, PointF};

/// Minimum cross-axis separation between branch targets (logical pixels).
///
/// Ensures branch targets (e.g. Condition's if_0/if_1/else targets) don't
/// visually黏连 even when dagre places them at similar cross-axis
/// coordinates. This happens when targets are in different ranks — dagre's
/// `nodesep` only applies within the same rank, so cross-rank targets can
/// end up at nearly identical X (vertical layout) or Y (horizontal layout)
/// positions.
const MIN_BRANCH_CROSS_SEP: f32 = 40.0;

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
/// sort the target nodes by their port order and:
/// 1. **Unify the main-axis coordinate** (X for horizontal, Y for vertical)
///    to the median of current values, ensuring all branch targets align in
///    the same column (horizontal) or row (vertical).
/// 2. **Distribute the cross-axis coordinate** (Y for horizontal, X for
///    vertical) — width-aware cumulative allocation when dagre's spacing is
///    too tight, even distribution when adequate. This prevents the
///   黏连/重叠 that occurs when dagre places branch targets at different
///    ranks (where `nodesep` doesn't apply) near the same cross-axis
///    coordinate.
///
/// Only Condition-style ports (`if_N` / `else`) are reordered. Other
/// multi-port nodes (e.g. Loop with `loop_body` / `done`) are skipped to
/// avoid disturbing their specialised layout handled by `reposition_loop_body`.
pub(super) fn reorder_branch_targets(
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

        // Collect the current coordinates of these targets.
        // main_axis = X (horizontal) / Y (vertical) — flow direction
        // cross_axis = Y (horizontal) / X (vertical) — branch stacking direction
        let current_coords: Vec<(f32, f32)> = targets
            .iter()
            .filter_map(|(_, nid)| positions.get(nid))
            .map(|p| match direction {
                LayoutDirection::Horizontal => (p.x, p.y),
                LayoutDirection::Vertical => (p.y, p.x),
            })
            .collect();

        if current_coords.len() != targets.len() {
            continue; // some targets missing positions — skip
        }

        // Unify main-axis coordinate: use the median so all branch targets
        // align in the same column (horizontal) or row (vertical).
        let mut main_coords: Vec<f32> = current_coords.iter().map(|(m, _)| *m).collect();
        main_coords.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
        let median_main = main_coords[main_coords.len() / 2];

        // Collect cross-axis coordinates and sort to find [min, max].
        let mut cross_coords: Vec<f32> = current_coords.iter().map(|(_, c)| *c).collect();
        cross_coords.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
        let cross_min = *cross_coords.first().unwrap_or(&0.0);
        let cross_max = *cross_coords.last().unwrap_or(&0.0);
        let n = targets.len();

        // Collect each target's cross-axis size (width for vertical, height for horizontal).
        // Used to compute the minimum span required to avoid overlap.
        let cross_sizes: Vec<f32> = targets
            .iter()
            .filter_map(|(_, nid)| {
                graph.node(*nid).map(|node| match direction {
                    LayoutDirection::Horizontal => node.size.h,
                    LayoutDirection::Vertical => node.size.w,
                })
            })
            .collect();
        if cross_sizes.len() != n {
            continue;
        }

        // Minimum required span = sum of sizes + MIN_SEP between each pair.
        let total_sizes: f32 = cross_sizes.iter().sum();
        let needed_span = total_sizes + MIN_BRANCH_CROSS_SEP * (n - 1) as f32;
        let available_span = cross_max - cross_min;

        // Assign cross-axis coordinates.
        // - If dagre gave enough space (available >= needed), use even distribution
        //   within [min, max] to preserve dagre's intended spacing.
        // - If dagre's spacing is too tight (available < needed), use cumulative
        //   width-aware allocation centered on the median to prevent overlap.
        if available_span >= needed_span {
            let cross_step = if n > 1 {
                available_span / (n - 1) as f32
            } else {
                0.0
            };
            for (i, (_, nid)) in targets.iter().enumerate() {
                if let Some(pos) = positions.get_mut(nid) {
                    let new_cross = cross_min + cross_step * i as f32;
                    match direction {
                        LayoutDirection::Horizontal => {
                            pos.x = median_main;
                            pos.y = new_cross;
                        }
                        LayoutDirection::Vertical => {
                            pos.y = median_main;
                            pos.x = new_cross;
                        }
                    }
                }
            }
        } else {
            // Cumulative width-aware allocation: stack targets centered on median.
            let cross_center = (cross_min + cross_max) * 0.5;
            let mut cursor = cross_center - needed_span * 0.5;
            for (i, (_, nid)) in targets.iter().enumerate() {
                let new_cross = cursor + cross_sizes[i] * 0.5;
                cursor += cross_sizes[i] + MIN_BRANCH_CROSS_SEP;
                if let Some(pos) = positions.get_mut(nid) {
                    match direction {
                        LayoutDirection::Horizontal => {
                            pos.x = median_main;
                            pos.y = new_cross;
                        }
                        LayoutDirection::Vertical => {
                            pos.y = median_main;
                            pos.x = new_cross;
                        }
                    }
                }
            }
        }
    }
}
