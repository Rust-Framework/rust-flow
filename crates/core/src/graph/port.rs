//! Port identifiers and direction/side enums.

use serde::{Deserialize, Serialize};

/// A port identifier, unique within a node.
pub type PortId = String;

/// Direction of a port: data flows in or out of the node.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum PortDirection {
    In,
    Out,
}

/// Which side of the node a port sits on.
///
/// `Auto` means the framework computes the side dynamically based on the
/// relative position of the connected nodes (floating-edge behaviour).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default, Serialize, Deserialize)]
pub enum PortSide {
    Top,
    Right,
    Bottom,
    Left,
    /// Framework computes the side from relative node positions.
    #[default]
    Auto,
}

impl PortSide {
    /// The opposite side.
    pub fn opposite(self) -> Self {
        match self {
            Self::Top => Self::Bottom,
            Self::Right => Self::Left,
            Self::Bottom => Self::Top,
            Self::Left => Self::Right,
            Self::Auto => Self::Auto,
        }
    }

    /// Whether this side is horizontal (Left/Right).
    pub fn is_horizontal(self) -> bool {
        matches!(self, Self::Left | Self::Right)
    }
}
