//! Layout abstraction: `LayoutEngine` trait, `LayoutDirection`, `LayoutResult`.
//!
//! The [`DagreLayout`] implementation wraps the `dagre` crate — a complete
//! Rust port of dagre.js (the same algorithm family used by ReactFlow's
//! official dagre examples).

pub mod dagre;

use crate::geometry::PointF;
use crate::graph::{FlowGraph, NodeId};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Layout direction: vertical (top→bottom) or horizontal (left→ right).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum LayoutDirection {
    Vertical,
    Horizontal,
}

/// Result of a layout run: a position and rank for each node.
#[derive(Debug, Clone, Default)]
pub struct LayoutResult {
    pub positions: HashMap<NodeId, PointF>,
    /// dagre 分配的 rank（层号），用于避障路由。
    /// rank 从 0 开始，沿布局方向递增。
    pub ranks: HashMap<NodeId, i32>,
}

/// A graph layout engine.
pub trait LayoutEngine: Send + Sync {
    fn layout(&self, graph: &FlowGraph, direction: LayoutDirection) -> LayoutResult;
}

pub use dagre::DagreLayout;
