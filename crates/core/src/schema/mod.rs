//! Schema data standard: declarative node definitions used to match `IFlowNode`
//! implementations by `kind` (strategy pattern) and to declare port specs.

use crate::geometry::SizeF;
use crate::graph::{NodeKind, PortDirection, PortId, PortSide};
use serde::{Deserialize, Serialize};

/// Specification of a single port on a node schema.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PortSpec {
    pub id: PortId,
    pub direction: PortDirection,
    /// `Auto` lets the framework compute the side dynamically.
    #[serde(default)]
    pub side: PortSide,
    pub label: Option<String>,
}

impl PortSpec {
    pub fn new(id: impl Into<PortId>, direction: PortDirection, side: PortSide) -> Self {
        Self {
            id: id.into(),
            direction,
            side,
            label: None,
        }
    }

    pub fn with_label(mut self, label: impl Into<String>) -> Self {
        self.label = Some(label.into());
        self
    }
}

/// Declarative schema for a node kind.
///
/// Built-in kinds cover turing-complete control flow:
/// `start` / `end` (sequence), `condition` (branch), `loop` (iteration).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NodeSchema {
    pub kind: NodeKind,
    pub label: String,
    pub ports: Vec<PortSpec>,
    pub default_size: SizeF,
}

impl NodeSchema {
    pub fn new(kind: impl Into<NodeKind>, label: impl Into<String>) -> Self {
        Self {
            kind: kind.into(),
            label: label.into(),
            ports: Vec::new(),
            default_size: SizeF::new(180.0, 80.0),
        }
    }

    pub fn with_port(mut self, port: PortSpec) -> Self {
        self.ports.push(port);
        self
    }

    pub fn with_size(mut self, size: SizeF) -> Self {
        self.default_size = size;
        self
    }

    /// Ports of the given direction.
    pub fn ports_by_direction(&self, dir: PortDirection) -> impl Iterator<Item = &PortSpec> {
        self.ports.iter().filter(move |p| p.direction == dir)
    }
}
