use slotmap::SlotMap;
use slotmap::Key;

use crate::auto_layout::LayoutDirection;
use crate::edge::FlowEdge;
use crate::id::{NodeId, PortId};
use crate::math::{Point, Size};
use crate::node::FlowNode;
use crate::port::{FlowPort, PortDirection, PortSide};
use crate::schema::builtin_type_registry;

#[derive(Debug, Clone, Default)]
pub struct FlowGraph {
    pub nodes: SlotMap<NodeId, FlowNode>,
    pub ports: SlotMap<PortId, FlowPort>,
    pub edges: Vec<FlowEdge>,
    pub name: String,
    /// Active flow direction for port sides and auto-layout.
    pub layout_direction: LayoutDirection,
    /// Dagre orthogonal routes (world space); populated by Mermaid layout.
    pub dagre_edge_routes: Vec<Option<crate::dagre_route::DagreEdgeRoute>>,
    /// True when the source document is a `mindmap-1.0` tree.
    /// Triggers mind map specific bezier curves during edge resolution.
    pub is_mindmap: bool,
}

impl FlowGraph {
    pub fn new(name: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            ..Default::default()
        }
    }

    pub fn add_node(&mut self, mut node: FlowNode) -> NodeId {
        let id = self.nodes.insert_with_key(|key| {
            node.id = key;
            if node.schema_id.is_empty() {
                node.schema_id = format!("node_{}", key.data().as_ffi());
            }
            node
        });
        id
    }

    pub fn remove_node(&mut self, node_id: NodeId) -> Option<FlowNode> {
        let port_ids: Vec<PortId> = self
            .ports
            .iter()
            .filter(|(_, p)| p.node == node_id)
            .map(|(id, _)| id)
            .collect();

        self.edges.retain(|e| {
            !port_ids.contains(&e.from_port) && !port_ids.contains(&e.to_port)
        });

        for pid in port_ids {
            self.ports.remove(pid);
        }

        self.nodes.remove(node_id)
    }

    pub fn add_port(
        &mut self,
        node_id: NodeId,
        name: impl Into<String>,
        direction: PortDirection,
        side: PortSide,
    ) -> Option<PortId> {
        let node = self.nodes.get_mut(node_id)?;
        let name = name.into();
        let port_id = self.ports.insert_with_key(|key| FlowPort {
            id: key,
            node: node_id,
            name: name.clone(),
            direction,
            side,
        });

        match direction {
            PortDirection::Input => node.inputs.push((name, port_id)),
            PortDirection::Output => node.outputs.push((name, port_id)),
        }

        Some(port_id)
    }

    pub fn add_edge(&mut self, edge: FlowEdge) -> bool {
        if self.ports.contains_key(edge.from_port) && self.ports.contains_key(edge.to_port) {
            self.edges.push(edge);
            true
        } else {
            false
        }
    }

    pub fn select_node(&mut self, node_id: NodeId, additive: bool) {
        if !additive {
            self.clear_selection();
        }
        if let Some(node) = self.nodes.get_mut(node_id) {
            node.selected = true;
        }
    }

    pub fn clear_selection(&mut self) {
        for node in self.nodes.values_mut() {
            node.selected = false;
        }
    }

    pub fn selected_nodes(&self) -> Vec<NodeId> {
        self.nodes
            .iter()
            .filter(|(_, n)| n.selected)
            .map(|(id, _)| id)
            .collect()
    }

    pub fn move_node(&mut self, node_id: NodeId, delta: Point) {
        if let Some(node) = self.nodes.get_mut(node_id) {
            node.position.x += delta.x;
            node.position.y += delta.y;
        }
    }

    /// World-space port center — delegated to the owning node.
    pub fn port_world_position(&self, port_id: PortId) -> Option<Point> {
        let port = self.ports.get(port_id)?;
        let node = self.nodes.get(port.node)?;
        Some(node.port_world_center(port_id, port.side, &self.ports))
    }

    /// Bounding box of all nodes in world space (for fit-to-view).
    pub fn content_bounds(&self) -> Option<(Point, Size)> {
        if self.nodes.is_empty() {
            return None;
        }
        let mut min_x = f32::MAX;
        let mut min_y = f32::MAX;
        let mut max_x = f32::MIN;
        let mut max_y = f32::MIN;
        for (_, node) in self.nodes.iter() {
            min_x = min_x.min(node.position.x);
            min_y = min_y.min(node.position.y);
            max_x = max_x.max(node.position.x + node.size.width);
            max_y = max_y.max(node.position.y + node.size.height);
        }
        Some((
            Point::new(min_x, min_y),
            Size::new(max_x - min_x, max_y - min_y),
        ))
    }

    pub fn stats(&self) -> GraphStats {
        GraphStats {
            node_count: self.nodes.len(),
            edge_count: self.edges.len(),
            selected_count: self.selected_nodes().len(),
        }
    }

    /// Add a typed node using the builtin type registry (data-driven topology).
    pub fn add_typed_node(
        &mut self,
        node_type: &str,
        label: impl Into<String>,
        position: Point,
    ) -> NodeId {
        self.add_typed_node_with_registry(&builtin_type_registry(), node_type, label, position)
    }

    /// Add a typed node from a [`FlowTypeRegistry`] definition.
    pub fn add_typed_node_with_registry(
        &mut self,
        types: &crate::schema::FlowTypeRegistry,
        node_type: &str,
        label: impl Into<String>,
        position: Point,
    ) -> NodeId {
        let label = label.into();
        let def = types.get(node_type);

        let mut node = FlowNode::typed(NodeId::default(), node_type, label, position);
        if let Some(def) = def {
            node.size = def.default_size.to_size();
            node.data = def.default_data.clone();
        }

        let node_id = self.add_node(node);

        if let Some(def) = def {
            for port_def in &def.ports {
                self.add_port(node_id, &port_def.id, port_def.direction, port_def.side);
            }
        }

        node_id
    }
}

#[derive(Debug, Clone, Copy)]
pub struct GraphStats {
    pub node_count: usize,
    pub edge_count: usize,
    pub selected_count: usize,
}

/// Build the demo graph from embedded Flow Schema document (`schemas/demo.flow.json`).
pub fn demo_chain_graph() -> FlowGraph {
    crate::schema::demo_graph_from_document()
}
