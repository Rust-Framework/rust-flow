//! Dagre layout edge routes (Mermaid / dagre-wrapper orthogonal points).

use crate::math::Point;

/// World-space orthogonal polyline from Dagre after layout.
#[derive(Debug, Clone, Default)]
pub struct DagreEdgeRoute {
    pub points: Vec<Point>,
    pub label_pos: Option<Point>,
}
