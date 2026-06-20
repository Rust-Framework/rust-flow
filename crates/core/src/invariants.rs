//! Regression checks for scene graph invariants.
//!
//! Run via `cargo test -p rust-agent-flow regression` after changing nodes, ports, or edges.

use crate::geometry::edge_path_endpoints;
use crate::id::PortId;
use crate::math::Point;
use crate::node::ResolvedNode;
use crate::port::PortSide;
use crate::scene::{ResolvedEdge, SceneFrame};

pub const EPS: f32 = 0.05;

#[derive(Debug, Clone)]
pub struct InvariantViolation {
    pub message: String,
}

impl InvariantViolation {
    fn new(message: impl Into<String>) -> Self {
        Self {
            message: message.into(),
        }
    }
}

pub fn near(a: Point, b: Point, eps: f32) -> bool {
    (a.x - b.x).abs() < eps && (a.y - b.y).abs() < eps
}

/// Handle center sits on the node border for the declared side.
pub fn check_port_on_border(node: &ResolvedNode, port_id: PortId, side: PortSide) -> Result<(), InvariantViolation> {
    let local = *node
        .port_local
        .get(&port_id)
        .ok_or_else(|| InvariantViolation::new(format!("missing port_local for {port_id:?}")))?;
    let w = node.screen_size.width;
    let h = node.screen_size.height;

    match side {
        PortSide::Left if local.x.abs() > EPS => {
            return Err(InvariantViolation::new("left port x should be 0"));
        }
        PortSide::Right if (local.x - w).abs() > EPS => {
            return Err(InvariantViolation::new(format!(
                "right port x should be {w}, got {}",
                local.x
            )));
        }
        PortSide::Top if local.y.abs() > EPS => {
            return Err(InvariantViolation::new("top port y should be 0"));
        }
        PortSide::Bottom if (local.y - h).abs() > EPS => {
            return Err(InvariantViolation::new(format!(
                "bottom port y should be {h}, got {}",
                local.y
            )));
        }
        _ => {}
    }
    Ok(())
}

/// `port_anchors` must equal `screen_pos + port_local`.
pub fn check_node_anchors(node: &ResolvedNode) -> Result<(), InvariantViolation> {
    for (pid, local) in &node.port_local {
        let abs = *node
            .port_anchors
            .get(pid)
            .ok_or_else(|| InvariantViolation::new(format!("missing port_anchors for {pid:?}")))?;
        let expected = Point::new(node.screen_pos.x + local.x, node.screen_pos.y + local.y);
        if !near(abs, expected, EPS) {
            return Err(InvariantViolation::new(format!(
                "port {pid:?} anchor mismatch: got ({}, {}), expected ({}, {})",
                abs.x, abs.y, expected.x, expected.y
            )));
        }
        if let Some(side) = node.port_sides.get(pid) {
            check_port_on_border(node, *pid, *side)?;
        }
    }
    Ok(())
}

/// Edge geometry must start/end exactly at resolved handle centers.
pub fn check_edge(edge: &ResolvedEdge) -> Result<(), InvariantViolation> {
    let (start, end) = edge_path_endpoints(&edge.path);
    if !near(start, edge.from, EPS) {
        return Err(InvariantViolation::new(format!(
            "edge start ({}, {}) != from ({}, {})",
            start.x, start.y, edge.from.x, edge.from.y
        )));
    }
    if !near(end, edge.to, EPS) {
        return Err(InvariantViolation::new(format!(
            "edge end ({}, {}) != to ({}, {})",
            end.x, end.y, edge.to.x, edge.to.y
        )));
    }
    Ok(())
}

/// Validate an entire resolved frame (nodes + edges).
pub fn check_frame(frame: &SceneFrame) -> Result<(), InvariantViolation> {
    for node in &frame.nodes {
        check_node_anchors(node)?;
    }
    for edge in &frame.edges {
        check_edge(edge)?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::graph::demo_chain_graph;
    use crate::viewport::Viewport;

    #[test]
    fn invariants_hold_for_demo_default_viewport() {
        let graph = demo_chain_graph();
        let frame = SceneFrame::resolve(&graph, &Viewport::default());
        check_frame(&frame).expect("demo frame invariants");
    }
}
