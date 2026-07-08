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

/// Which side of a node a port sits on.
///
/// `Auto` is a weak constraint: the framework computes the side based on
/// layout direction. Concrete sides (Top/Right/Bottom/Left) may be either
/// weak or strong, determined by `PortSpec.fixed`:
/// - `fixed = false` (default): weak constraint, side may be overridden
/// - `fixed = true`: strong constraint, side is fixed by the node impl
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default, Serialize, Deserialize)]
pub enum PortSide {
    Top,
    Right,
    Bottom,
    Left,
    /// Weak constraint: framework computes the side from layout direction.
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
