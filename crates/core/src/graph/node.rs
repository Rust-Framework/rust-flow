//! Node types: `Node`, `NodeId`, `NodeKind`, `NodeData`.

use crate::geometry::{PointF, SizeF};
use serde::{Deserialize, Serialize};
use slotmap::new_key_type;

new_key_type! {
    pub struct NodeId;
}

/// The kind of a node, matched against `IFlowNode` implementations (strategy pattern).
///
/// Stored as a `String` so custom node kinds can be registered without modifying core.
pub type NodeKind = String;

/// Free-form business data carried by a node.
pub type NodeData = serde_json::Value;

/// A node in the flow graph.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Node {
    pub id: NodeId,
    pub kind: NodeKind,
    pub data: NodeData,
    /// Top-left position in logical (document) coordinates.
    pub position: PointF,
    pub size: SizeF,
}

impl Node {
    /// The node's bounding rectangle in logical coordinates.
    pub fn bounds(&self) -> crate::geometry::RectF {
        crate::geometry::RectF::new(self.position, self.size)
    }

    pub fn center(&self) -> PointF {
        self.bounds().center()
    }
}
