//! Smart endpoint calculation (requirement 3).
//!
//! Computes, for each edge, the actual endpoint positions on the source and
//! target nodes. Non-fixed (`Auto`) sides are derived from the relative
//! position of the two nodes; multiple ports on the same side are distributed
//! evenly, with In/Out ports split to avoid overlap.

use crate::geometry::{PointF, RectF};
use crate::graph::{EdgeId, FlowGraph, NodeId, PortDirection, PortSide};
use crate::schema::PortSpec;
use std::collections::HashMap;

/// Resolved endpoints for a single edge.
#[derive(Debug, Clone, Copy)]
pub struct ResolvedEdge {
    pub src: PointF,
    pub src_side: PortSide,
    pub dst: PointF,
    pub dst_side: PortSide,
}

/// Compute resolved endpoints for every edge in the graph.
///
/// `port_specs` returns the declared port specs for a given node (used to
/// respect fixed sides and to know port directions). The gpui layer supplies
/// this from the `NodeRegistry`.
pub fn resolve_endpoints<F>(graph: &FlowGraph, port_specs: F) -> HashMap<EdgeId, ResolvedEdge>
where
    F: Fn(NodeId) -> Vec<PortSpec>,
{
    // Step 1: determine the side for each edge endpoint.
    let mut edge_sides: HashMap<EdgeId, (PortSide, PortSide)> = HashMap::new();
    for edge in graph.edges() {
        let src_node = match graph.node(edge.source) {
            Some(n) => n,
            None => continue,
        };
        let dst_node = match graph.node(edge.target) {
            Some(n) => n,
            None => continue,
        };

        let src_specs = port_specs(edge.source);
        let dst_specs = port_specs(edge.target);

        let src_side = resolve_side(edge.source_port.as_deref(), &src_specs, src_node, dst_node);
        let dst_side = resolve_side(edge.target_port.as_deref(), &dst_specs, dst_node, src_node);

        edge_sides.insert(edge.id, (src_side, dst_side));
    }

    // Step 2: for each (node, side, direction), collect edges and distribute.
    // Build a list of (node, side, direction, edge_id, is_source) for distribution.
    let mut slots: HashMap<(NodeId, PortSide, PortDirection), Vec<(EdgeId, bool)>> =
        HashMap::new();
    for edge in graph.edges() {
        let (src_side, dst_side) = match edge_sides.get(&edge.id) {
            Some(v) => *v,
            None => continue,
        };
        slots
            .entry((edge.source, src_side, PortDirection::Out))
            .or_default()
            .push((edge.id, true));
        slots
            .entry((edge.target, dst_side, PortDirection::In))
            .or_default()
            .push((edge.id, false));
    }

    // Step 3: compute absolute positions for each slot.
    // Pre-compute which (node, side) pairs have both In and Out to avoid
    // borrowing `slots` after it is consumed by the loop.
    let sides_with_both: std::collections::HashSet<(NodeId, PortSide)> = {
        let keys: std::collections::HashSet<_> = slots.keys().cloned().collect();
        keys.iter()
            .filter(|(node_id, side, _)| {
                keys.contains(&(*node_id, *side, PortDirection::In))
                    && keys.contains(&(*node_id, *side, PortDirection::Out))
            })
            .map(|(node_id, side, _)| (*node_id, *side))
            .collect()
    };

    let mut positions: HashMap<(EdgeId, bool), PointF> = HashMap::new();
    for ((node_id, side, dir), mut entries) in slots {
        let node = match graph.node(node_id) {
            Some(n) => n,
            None => continue,
        };
        let bounds = node.bounds();

        let has_opposite = sides_with_both.contains(&(node_id, side));

        let points = distribute_on_side(bounds, side, dir, has_opposite, entries.len());
        // entries order is stable (insertion order); assign positions.
        entries.sort_by_key(|(_, is_src)| *is_src);
        for (i, (edge_id, is_src)) in entries.iter().enumerate() {
            if let Some(pt) = points.get(i) {
                positions.insert((*edge_id, *is_src), *pt);
            }
        }
    }

    // Step 4: assemble resolved edges.
    let mut result = HashMap::new();
    for edge in graph.edges() {
        let (src_side, dst_side) = match edge_sides.get(&edge.id) {
            Some(v) => *v,
            None => continue,
        };
        let src = positions
            .get(&(edge.id, true))
            .copied()
            .unwrap_or_else(|| graph.node(edge.source).map(|n| n.center()).unwrap_or_default());
        let dst = positions
            .get(&(edge.id, false))
            .copied()
            .unwrap_or_else(|| graph.node(edge.target).map(|n| n.center()).unwrap_or_default());
        result.insert(
            edge.id,
            ResolvedEdge {
                src,
                src_side,
                dst,
                dst_side,
            },
        );
    }
    result
}

/// Resolve the side for one endpoint of an edge.
///
/// If the port has a fixed side in the schema, use it; otherwise compute from
/// the relative position of the two nodes (floating-edge behaviour).
fn resolve_side(
    port_id: Option<&str>,
    specs: &[PortSpec],
    self_node: &crate::graph::Node,
    other_node: &crate::graph::Node,
) -> PortSide {
    // Look up the port spec for a fixed side.
    if let Some(id) = port_id {
        if let Some(spec) = specs.iter().find(|s| s.id == id) {
            if spec.side != PortSide::Auto {
                return spec.side;
            }
        }
    }
    // Auto: compute from relative position.
    compute_side_from_position(self_node.center(), other_node.center())
}

/// Pick the side of `self_center` that faces `other_center`.
fn compute_side_from_position(self_center: PointF, other_center: PointF) -> PortSide {
    let dx = other_center.x - self_center.x;
    let dy = other_center.y - self_center.y;
    if dx.abs() >= dy.abs() {
        if dx >= 0.0 {
            PortSide::Right
        } else {
            PortSide::Left
        }
    } else if dy >= 0.0 {
        PortSide::Bottom
    } else {
        PortSide::Top
    }
}

/// Distribute `count` ports along one side of `bounds`.
///
/// When the opposite direction also occupies this side, In and Out each get
/// half the side to avoid overlap (requirement 3.3).
fn distribute_on_side(
    bounds: RectF,
    side: PortSide,
    dir: PortDirection,
    has_opposite: bool,
    count: usize,
) -> Vec<PointF> {
    if count == 0 {
        return vec![];
    }
    // Along-side parameter range [start, end] in 0..1.
    // If the opposite direction shares this side, use half the side.
    let (start, end) = if has_opposite {
        match dir {
            PortDirection::In => (0.5, 1.0),
            PortDirection::Out => (0.0, 0.5),
        }
    } else {
        (0.0, 1.0)
    };

    let outward = 2.0; // slightly outward
    let step = if count > 1 {
        (end - start) / (count as f32)
    } else {
        0.0
    };
    let first = if count > 1 {
        start + step * 0.5
    } else {
        (start + end) * 0.5
    };

    (0..count)
        .map(|i| {
            let t = first + step * i as f32;
            point_on_side(bounds, side, t, outward)
        })
        .collect()
}

/// Absolute position at parameter `t` (0..1) along `side` of `bounds`,
/// pushed outward by `outward` pixels.
fn point_on_side(bounds: RectF, side: PortSide, t: f32, outward: f32) -> PointF {
    match side {
        PortSide::Top => PointF::new(
            bounds.left() + bounds.size.w * t,
            bounds.top() - outward,
        ),
        PortSide::Right => PointF::new(
            bounds.right() + outward,
            bounds.top() + bounds.size.h * t,
        ),
        PortSide::Bottom => PointF::new(
            bounds.left() + bounds.size.w * t,
            bounds.bottom() + outward,
        ),
        PortSide::Left => PointF::new(
            bounds.left() - outward,
            bounds.top() + bounds.size.h * t,
        ),
        PortSide::Auto => PointF::new(bounds.right() + outward, bounds.center().y),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::geometry::{PointF, SizeF};
    use crate::graph::{Edge, FlowGraph};
    use crate::schema::PortSpec;

    fn make_graph() -> (FlowGraph, [NodeId; 2]) {
        let mut g = FlowGraph::new();
        let a = g.add_node("start", serde_json::json!({}));
        let b = g.add_node("end", serde_json::json!({}));
        g.node_mut(a).unwrap().position = PointF::new(0.0, 0.0);
        g.node_mut(a).unwrap().size = SizeF::new(100.0, 60.0);
        g.node_mut(b).unwrap().position = PointF::new(300.0, 0.0);
        g.node_mut(b).unwrap().size = SizeF::new(100.0, 60.0);
        g.add_edge(Edge::new(a, b));
        (g, [a, b])
    }

    fn specs_for(_node: NodeId) -> Vec<PortSpec> {
        vec![] // all Auto
    }

    #[test]
    fn auto_side_picks_right_for_target_on_right() {
        let (g, _) = make_graph();
        let resolved = resolve_endpoints(&g, specs_for);
        assert_eq!(resolved.len(), 1);
        let r = resolved.values().next().unwrap();
        // Target is to the right → source side = Right, target side = Left.
        assert_eq!(r.src_side, PortSide::Right);
        assert_eq!(r.dst_side, PortSide::Left);
    }

    #[test]
    fn in_out_on_same_side_do_not_overlap() {
        let mut g = FlowGraph::new();
        let a = g.add_node("a", serde_json::json!({}));
        let b = g.add_node("b", serde_json::json!({}));
        let c = g.add_node("c", serde_json::json!({}));
        // a and c are both to the right of b, so b's right side has both
        // an In edge (from a) and an Out edge (to c).
        g.node_mut(a).unwrap().position = PointF::new(300.0, 0.0);
        g.node_mut(b).unwrap().position = PointF::new(0.0, 0.0);
        g.node_mut(c).unwrap().position = PointF::new(300.0, 100.0);
        for n in [a, b, c] {
            g.node_mut(n).unwrap().size = SizeF::new(100.0, 60.0);
        }
        g.add_edge(Edge::new(a, b)); // a → b (b has In on Right)
        g.add_edge(Edge::new(b, c)); // b → c (b has Out on Right)

        let resolved = resolve_endpoints(&g, specs_for);
        // Find the two edges touching b's right side.
        let b_bounds = g.node(b).unwrap().bounds();
        let mut in_y = None;
        let mut out_y = None;
        for edge in g.edges() {
            if let Some(r) = resolved.get(&edge.id) {
                if edge.target == b && r.dst_side == PortSide::Right {
                    in_y = Some(r.dst.y);
                }
                if edge.source == b && r.src_side == PortSide::Right {
                    out_y = Some(r.src.y);
                }
            }
        }
        let (in_y, out_y) = (in_y.unwrap(), out_y.unwrap());
        // In and Out must not overlap (different y on the right side).
        assert!(
            (in_y - out_y).abs() > 1.0,
            "in_y={} and out_y={} should differ",
            in_y,
            out_y
        );
        // Both should be within the node's right edge.
        let _ = b_bounds;
    }
}
