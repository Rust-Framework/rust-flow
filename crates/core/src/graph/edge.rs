//! Edge types: `Edge`, `EdgeId`, `EdgeType`, `EdgeKind`.

use crate::graph::{NodeId, PortId};
use serde::{Deserialize, Serialize};
use slotmap::new_key_type;

new_key_type! {
    pub struct EdgeId;
}

/// The path algorithm used to render an edge.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, Default)]
pub enum EdgeType {
    /// Cubic Bézier curve (default, free-flowing).
    #[default]
    Bezier,
    /// Straight line between endpoints.
    Straight,
    /// Orthogonal path with sharp 90° corners.
    Step,
    /// Orthogonal path with rounded corners.
    SmoothStep,
}

/// Semantic kind of an edge.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, Default)]
pub enum EdgeKind {
    /// A normal connection.
    #[default]
    Normal,
    /// A loop-back edge (used by loop nodes to return to the loop head).
    LoopBack,
}

/// A directed edge connecting two nodes (optionally via specific ports).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Edge {
    pub id: EdgeId,
    pub source: NodeId,
    pub source_port: Option<PortId>,
    pub target: NodeId,
    pub target_port: Option<PortId>,
    #[serde(default)]
    pub edge_type: EdgeType,
    #[serde(default)]
    pub kind: EdgeKind,
}

impl Edge {
    pub fn new(source: NodeId, target: NodeId) -> Self {
        Self {
            id: EdgeId::default(),
            source,
            source_port: None,
            target,
            target_port: None,
            edge_type: EdgeType::default(),
            kind: EdgeKind::default(),
        }
    }
}
