use rust_agent_flow::{NodeId, PortId, Point as CorePoint};
use gpui::{Point, Pixels};

pub type Pix = Pixels;

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum InteractionState {
    Idle,
    DraggingNode {
        node_id: NodeId,
        /// World-space offset from node origin to grab point.
        grab_offset: CorePoint,
    },
    CreatingConnection {
        from_port: PortId,
        current_mouse: Point<Pix>,
    },
    Panning {
        last_mouse: Point<Pix>,
    },
}

impl Default for InteractionState {
    fn default() -> Self {
        Self::Idle
    }
}

impl InteractionState {
    pub fn is_active(&self) -> bool {
        !matches!(self, Self::Idle)
    }
}
