use serde::{Deserialize, Serialize};

use crate::id::{NodeId, PortId};

/// Which side of a node a port sits on — determines edge control-point direction.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, Default)]
pub enum PortSide {
    #[default]
    Left,
    Right,
    Top,
    Bottom,
}

/// Data-flow direction of a port.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum PortDirection {
    Input,
    Output,
}

/// A connection handle on a node.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FlowPort {
    pub id: PortId,
    pub node: NodeId,
    pub name: String,
    pub direction: PortDirection,
    pub side: PortSide,
}

impl FlowPort {
    pub fn is_input(&self) -> bool {
        matches!(self.direction, PortDirection::Input)
    }

    pub fn is_output(&self) -> bool {
        matches!(self.direction, PortDirection::Output)
    }
}
