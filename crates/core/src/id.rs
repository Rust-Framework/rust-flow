use serde::{Deserialize, Serialize};
use slotmap::new_key_type;

new_key_type! {
    /// Stable node identifier.
    pub struct NodeId;
    /// Stable port identifier.
    pub struct PortId;
}

/// Lightweight edge identifier (index into `FlowGraph::edges`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct EdgeId(pub usize);
