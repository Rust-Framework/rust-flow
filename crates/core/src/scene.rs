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
        } else {
            let path =
                build_edge_path_with_route(from, from_side, to, to_side, self.shape, route, zoom);
            (path, None, from, to, from_side, to_side)
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
