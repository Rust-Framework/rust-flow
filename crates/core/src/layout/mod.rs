//! Layout abstraction: `LayoutEngine` trait, `LayoutDirection`, `LayoutResult`.
//!
//! The default `DagreLayout` implementation (behind the `dagre` feature) wraps
//! the `mermaid-dagre` crate — a 1:1 Rust port of JS dagre.

#[cfg(feature = "dagre")]
pub mod dagre;

use crate::geometry::PointF;
use crate::graph::{FlowGraph, NodeId};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Layout direction: vertical (top→bottom) or horizontal (left→right).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum LayoutDirection {
    Vertical,
    Horizontal,
}

/// Result of a layout run: a position for each node.
#[derive(Debug, Clone, Default)]
pub struct LayoutResult {
    pub positions: HashMap<NodeId, PointF>,
}

/// A graph layout engine.
pub trait LayoutEngine: Send + Sync {
    fn layout(&self, graph: &FlowGraph, direction: LayoutDirection) -> LayoutResult;
}
