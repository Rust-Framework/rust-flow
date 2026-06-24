//! `rust-agent-flow` — framework-agnostic workflow graph model, geometry and layout.
//!
//! This crate has **no GPUI dependency**. It provides:
//! - [`graph::FlowGraph`] — nodes + edges with stable slotmap keys.
//! - [`schema`] — declarative node schema (`NodeSchema`, `PortSpec`).
//! - [`geometry`] — point/rect types, edge path algorithms (bezier / straight /
//!   step / smoothstep), smart endpoint calculation, hit-testing.
//! - [`layout`] — `LayoutEngine` trait + `DagreLayout` (wraps the `dagre` crate).
//! - [`Viewport`] — pan/zoom transform math.

pub mod geometry;
pub mod graph;
pub mod layout;
pub mod schema;
pub mod viewport;

pub use geometry::{PointF, RectF, SizeF};
pub use graph::{Edge, EdgeId, EdgeKind, EdgeType, FlowGraph, Node, NodeData, NodeId, NodeKind, PortDirection, PortId, PortSide};
pub use layout::{DagreLayout, LayoutDirection, LayoutEngine, LayoutResult};
pub use schema::{
    DropdownOption, EdgeDef, FieldSpec, FieldType, FlowDocument, FlowMetadata, ListSpec,
    NodeDef, NodeSchema, PortSpec,
};
pub use viewport::Viewport;

// Re-export geometry algorithms for convenient access from the gpui layer.
pub use geometry::edge_path::{
    bezier_path, loop_back_path, round_corners, route_with_channels, smoothstep_path, step_path,
    straight_path,
};
pub use geometry::hit_test::{point_in_rect, point_to_polyline_distance};
pub use geometry::port_calc::{resolve_endpoints, ResolvedEdge};
