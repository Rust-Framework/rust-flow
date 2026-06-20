use serde::{Deserialize, Serialize};

use crate::id::PortId;

/// Edge geometry — defaults match React Flow.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
pub enum EdgeShape {
    #[default]
    SmoothStep,
    Bezier,
    Natural,
    Straight,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
pub enum EdgeStroke {
    #[default]
    Solid,
    Dashed,
    Dotted,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FlowEdge {
    pub from_port: PortId,
    pub to_port: PortId,
    #[serde(default)]
    pub shape: EdgeShape,
    #[serde(default)]
    pub stroke: EdgeStroke,
    pub label: Option<String>,
}

impl FlowEdge {
    pub fn new(from_port: PortId, to_port: PortId) -> Self {
        Self {
            from_port,
            to_port,
            shape: EdgeShape::default(),
            stroke: EdgeStroke::default(),
            label: None,
        }
    }
}
