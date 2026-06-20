use std::collections::HashMap;

use crate::edge::{EdgeShape, EdgeStroke, FlowEdge};
use crate::geometry::{
    arrival_side, build_edge_path_with_route, departure_side, edge_path_from_dagre,
    label_pos_from_dagre, EdgePath,
};
use crate::graph::FlowGraph;
use crate::id::{NodeId, PortId};
use crate::math::Point;
use crate::node::ResolvedNode;
use crate::port::PortSide;
use crate::viewport::Viewport;

#[derive(Debug, Clone)]
pub struct ResolvedEdge {
    pub path: EdgePath,
    pub stroke: EdgeStroke,
    pub from: Point,
    pub to: Point,
    pub from_side: PortSide,
    pub to_side: PortSide,
    pub label: Option<String>,
    /// Label anchor in viewport-local screen space (Dagre label position).
    pub label_pos: Option<Point>,
}

#[derive(Debug, Clone, Default)]
pub struct SceneFrame {
    pub zoom: f32,
    pub nodes: Vec<ResolvedNode>,
    pub node_index: HashMap<NodeId, usize>,
    pub edges: Vec<ResolvedEdge>,
    pub preview: Option<ResolvedEdge>,
}

impl SceneFrame {
    pub fn resolve(graph: &FlowGraph, viewport: &Viewport) -> Self {
        let mut frame = Self {
            zoom: viewport.zoom,
            ..Self::default()
        };

        for (id, node) in graph.nodes.iter() {
            let idx = frame.nodes.len();
            frame.node_index.insert(id, idx);
            frame.nodes.push(node.resolve(viewport, &graph.ports, graph.layout_direction));
        }

        let route_offsets = if graph.dagre_edge_routes.iter().any(|r| r.is_some()) {
            vec![crate::auto_layout::EdgeRouteOffset::default(); graph.edges.len()]
        } else {
            let feedback = crate::auto_layout::detect_feedback_edges(
                graph,
                &crate::orientation::all_loop_body_nodes(graph),
            );
            crate::auto_layout::compute_edge_route_offsets(
                graph,
                &frame.nodes,
                &frame.node_index,
                &feedback,
            )
        };

        for (edge_idx, edge) in graph.edges.iter().enumerate() {
            if let Some(resolved) = edge.resolve_with_route(
                &frame.nodes,
                &frame.node_index,
                graph,
                route_offsets.get(edge_idx).copied().unwrap_or_default(),
                edge_idx,
                viewport,
            ) {
                frame.edges.push(resolved);
            }
        }

        frame
    }

    pub fn with_preview(
        mut self,
        from: Point,
        from_side: PortSide,
        to: Point,
        to_side: PortSide,
    ) -> Self {
        self.preview = Some(ResolvedEdge {
            path: build_edge_path_with_route(
                from,
                from_side,
                to,
                to_side,
                EdgeShape::SmoothStep,
                crate::auto_layout::EdgeRouteOffset::default(),
                self.zoom,
            ),
            stroke: EdgeStroke::Dashed,
            from,
            to,
            from_side,
            to_side,
            label: None,
            label_pos: None,
        });
        self
    }

    pub fn port_at(&self, graph: &FlowGraph, port_id: PortId) -> Option<(Point, PortSide)> {
        let port = graph.ports.get(port_id)?;
        let idx = *self.node_index.get(&port.node)?;
        let node = &self.nodes[idx];
        let pt = node.port_anchors.get(&port_id).copied()?;
        let side = node.port_sides.get(&port_id).copied()?;
        Some((pt, side))
    }
}

impl FlowEdge {
    pub fn resolve(
        &self,
        nodes: &[ResolvedNode],
        node_index: &HashMap<NodeId, usize>,
        graph: &FlowGraph,
    ) -> Option<ResolvedEdge> {
        self.resolve_with_route(
            nodes,
            node_index,
            graph,
            crate::auto_layout::EdgeRouteOffset::default(),
            0,
            &Viewport::default(),
        )
    }

    pub fn resolve_with_route(
        &self,
        nodes: &[ResolvedNode],
        node_index: &HashMap<NodeId, usize>,
        graph: &FlowGraph,
        route: crate::auto_layout::EdgeRouteOffset,
        edge_idx: usize,
        viewport: &Viewport,
    ) -> Option<ResolvedEdge> {
        let from_port = graph.ports.get(self.from_port)?;
        let to_port = graph.ports.get(self.to_port)?;
        let from = anchor(nodes, node_index, graph, self.from_port)?;
        let to = anchor(nodes, node_index, graph, self.to_port)?;
        let from_side = from_port.side;
        let to_side = to_port.side;
        let zoom = nodes
            .get(*node_index.get(&from_port.node)?)
            .map(|n| n.zoom)
            .unwrap_or(1.0);

        let dagre = graph.dagre_edge_routes.get(edge_idx).and_then(|r| r.as_ref());

        let (path, label_pos, path_from, path_to, from_side, to_side) = if let Some(dagre) = dagre {
            let path = edge_path_from_dagre(&dagre.points, viewport);
            let (path_from, path_to) = crate::geometry::edge_path_endpoints(&path);
            let label_pos = dagre
                .label_pos
                .map(|p| label_pos_from_dagre(p, viewport));
            let from_side = departure_side(&path).unwrap_or(from_side);
            let to_side = arrival_side(&path).unwrap_or(to_side);
            (path, label_pos, path_from, path_to, from_side, to_side)
        } else if graph.is_mindmap {
            // Mind map: compute anchor positions based on relative node positions,
            // not port sides. This correctly handles bidirectional layout where
            // left children connect to the parent's left side and right children
            // to the parent's right side.
            let (mm_from, mm_to, mm_from_side, mm_to_side) = mindmap_anchors(
                nodes,
                node_index,
                from_port.node,
                to_port.node,
                graph.layout_direction,
            );
            let path = build_mindmap_edge_path(mm_from, mm_to, graph.layout_direction);
            let label_pos = compute_edge_label_pos(&path, mm_from, mm_to, EdgeShape::Bezier);
            (path, label_pos, mm_from, mm_to, mm_from_side, mm_to_side)
        } else {
            let path =
                build_edge_path_with_route(from, from_side, to, to_side, self.shape, route, zoom);
            // Compute label position using React Flow formulas
            let label_pos = compute_edge_label_pos(&path, from, to, self.shape);
            (path, label_pos, from, to, from_side, to_side)
        };

        Some(ResolvedEdge {
            path,
            stroke: self.stroke,
            from: path_from,
            to: path_to,
            from_side,
            to_side,
            label: self.label.clone(),
            label_pos,
        })
    }
}

fn anchor(
    nodes: &[ResolvedNode],
    node_index: &HashMap<NodeId, usize>,
    graph: &FlowGraph,
    port_id: PortId,
) -> Option<Point> {
    let port = graph.ports.get(port_id)?;
    let idx = *node_index.get(&port.node)?;
    nodes[idx].port_anchors.get(&port_id).copied()
}

/// Compute anchor points for mind map edges based on relative node positions.
///
/// Unlike port-based anchoring (which always uses the same side), this function
/// determines the connection sides by comparing the parent and child positions:
/// - LR layout: child left of parent → parent's left, child's right
///              child right of parent → parent's right, child's left
/// - TB layout: parent's bottom, child's top
///
/// This is essential for bidirectional mind map layout where the root has
/// children on both sides.
fn mindmap_anchors(
    nodes: &[ResolvedNode],
    node_index: &HashMap<NodeId, usize>,
    parent_id: NodeId,
    child_id: NodeId,
    direction: crate::auto_layout::LayoutDirection,
) -> (Point, Point, PortSide, PortSide) {
    let parent_idx = *node_index.get(&parent_id).unwrap_or(&0);
    let child_idx = *node_index.get(&child_id).unwrap_or(&0);
    let parent = &nodes[parent_idx];
    let child = &nodes[child_idx];

    let p_pos = parent.screen_pos;
    let p_size = parent.screen_size;
    let c_pos = child.screen_pos;
    let c_size = child.screen_size;

    match direction {
        crate::auto_layout::LayoutDirection::LeftRight => {
            // Determine if child is to the left or right of parent
            let child_center_x = c_pos.x + c_size.width / 2.0;
            let parent_center_x = p_pos.x + p_size.width / 2.0;

            if child_center_x < parent_center_x {
                // Child is to the LEFT of parent
                let from = Point::new(p_pos.x, p_pos.y + p_size.height / 2.0);
                let to = Point::new(c_pos.x + c_size.width, c_pos.y + c_size.height / 2.0);
                (from, to, PortSide::Left, PortSide::Right)
            } else {
                // Child is to the RIGHT of parent
                let from = Point::new(p_pos.x + p_size.width, p_pos.y + p_size.height / 2.0);
                let to = Point::new(c_pos.x, c_pos.y + c_size.height / 2.0);
                (from, to, PortSide::Right, PortSide::Left)
            }
        }
        crate::auto_layout::LayoutDirection::TopBottom => {
            // TB layout: parent's bottom, child's top
            let from = Point::new(p_pos.x + p_size.width / 2.0, p_pos.y + p_size.height);
            let to = Point::new(c_pos.x + c_size.width / 2.0, c_pos.y);
            (from, to, PortSide::Bottom, PortSide::Top)
        }
    }
}

/// Compute edge label position using React Flow formulas (non-Dagre paths).
fn compute_edge_label_pos(
    path: &EdgePath,
    from: Point,
    to: Point,
    shape: EdgeShape,
) -> Option<Point> {
    use crate::geometry::{get_bezier_edge_center, get_edge_center, get_smooth_step_label_center};
    match (path, shape) {
        (EdgePath::Bezier(b), EdgeShape::Bezier) => {
            let (lx, ly, _, _) = get_bezier_edge_center(b.from, b.cp1, b.cp2, b.to);
            Some(Point::new(lx, ly))
        }
        (EdgePath::SmoothStep { start, segments }, EdgeShape::SmoothStep) => {
            // Reconstruct points from segments for longest-segment lookup
            let mut pts = vec![*start];
            for seg in segments {
                match seg {
                    crate::geometry::SmoothStepSegment::LineTo(p) => pts.push(*p),
                    crate::geometry::SmoothStepSegment::QuadTo { to, .. } => pts.push(*to),
                }
            }
            get_smooth_step_label_center(&pts).map(|(x, y)| Point::new(x, y))
        }
        (EdgePath::Polyline(pts), EdgeShape::Straight) => {
            if pts.len() >= 2 {
                let (lx, ly, _, _) = get_edge_center(pts[0], *pts.last().unwrap());
                Some(Point::new(lx, ly))
            } else {
                None
            }
        }
        _ => {
            // Fallback: geometric center
            let (lx, ly, _, _) = get_edge_center(from, to);
            Some(Point::new(lx, ly))
        }
    }
}

/// Build an edge path using mind map specific bezier curves.
///
/// For LR layout: horizontal cubic bezier (mind-elixir `main()` style).
/// For TB layout: vertical cubic bezier.
fn build_mindmap_edge_path(
    from: Point,
    to: Point,
    direction: crate::auto_layout::LayoutDirection,
) -> EdgePath {
    use crate::geometry::{mindmap_lr_bezier, mindmap_tb_bezier, BezierPath};

    let bezier: BezierPath = match direction {
        crate::auto_layout::LayoutDirection::LeftRight => {
            mindmap_lr_bezier(from, to, 0.5)
        }
        crate::auto_layout::LayoutDirection::TopBottom => {
            mindmap_tb_bezier(from, to, 0.5)
        }
    };
    EdgePath::Bezier(bezier)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::graph::demo_chain_graph;
    use crate::port::PortSide;

    #[test]
    fn handles_on_edge_centers() {
        let graph = demo_chain_graph();
        let frame = SceneFrame::resolve(&graph, &Viewport::default());
        for node in &frame.nodes {
            let h = node.screen_size.height;
            let w = node.screen_size.width;
            for (_, pid) in &node.inputs {
                let side = node.port_sides[pid];
                let local = node.port_local[pid];
                match side {
                    PortSide::Left => {
                        assert!(local.x.abs() < 0.01);
                        assert!(local.y > 0.0 && local.y < h);
                    }
                    PortSide::Right => {
                        assert!((local.x - w).abs() < 0.01);
                        assert!(local.y > 0.0 && local.y < h);
                    }
                    PortSide::Top => {
                        assert!(local.y.abs() < 0.01);
                        assert!(local.x > 0.0 && local.x < w);
                    }
                    PortSide::Bottom => {
                        assert!((local.y - h).abs() < 0.01);
                        assert!(local.x > 0.0 && local.x < w);
                    }
                }
            }
            for (_, pid) in &node.outputs {
                let side = node.port_sides[pid];
                let local = node.port_local[pid];
                match side {
                    PortSide::Left => {
                        assert!(local.x.abs() < 0.01);
                        assert!(local.y > 0.0 && local.y < h);
                    }
                    PortSide::Right => {
                        assert!((local.x - w).abs() < 0.01);
                        assert!(local.y > 0.0 && local.y < h);
                    }
                    PortSide::Top => {
                        assert!(local.y.abs() < 0.01);
                        assert!(local.x > 0.0 && local.x < w);
                    }
                    PortSide::Bottom => {
                        assert!((local.y - h).abs() < 0.01);
                        assert!(local.x > 0.0 && local.x < w);
                    }
                }
            }
        }
    }
}
