//! Flow graph: nodes + edges with stable slotmap keys.

pub mod edge;
pub mod node;
pub mod port;

pub use edge::{Edge, EdgeId, EdgeKind, EdgeType};
pub use node::{Node, NodeData, NodeId, NodeKind};
pub use port::{PortDirection, PortId, PortSide};

use slotmap::SlotMap;

/// The flow graph: a collection of nodes and directed edges.
#[derive(Debug, Default)]
pub struct FlowGraph {
    nodes: SlotMap<NodeId, Node>,
    edges: SlotMap<EdgeId, Edge>,
    /// Monotonic version counter, bumped on any structural change.
    /// Used to invalidate cached geometry (e.g. `PortResolver`).
    version: u64,
}

impl FlowGraph {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn version(&self) -> u64 {
        self.version
    }

    // ---- nodes ----

    pub fn add_node(&mut self, kind: impl Into<NodeKind>, data: NodeData) -> NodeId {
        self.version = self.version.wrapping_add(1);
        self.nodes.insert_with_key(|key| Node {
            id: key,
            kind: kind.into(),
            data,
            position: crate::geometry::PointF::zero(),
            size: crate::geometry::SizeF::new(180.0, 35.0),
        })
    }

    /// 添加节点并指定尺寸（用于 Condition/Loop 等结构化节点）。
    pub fn add_node_with_size(
        &mut self,
        kind: impl Into<NodeKind>,
        data: NodeData,
        size: crate::geometry::SizeF,
    ) -> NodeId {
        self.version = self.version.wrapping_add(1);
        self.nodes.insert_with_key(|key| Node {
            id: key,
            kind: kind.into(),
            data,
            position: crate::geometry::PointF::zero(),
            size,
        })
    }

    pub fn remove_node(&mut self, id: NodeId) -> Option<Node> {
        let node = self.nodes.remove(id)?;
        // Remove all edges referencing this node.
        self.edges.retain(|_, e| e.source != id && e.target != id);
        self.version = self.version.wrapping_add(1);
        Some(node)
    }

    pub fn node(&self, id: NodeId) -> Option<&Node> {
        self.nodes.get(id)
    }

    pub fn node_mut(&mut self, id: NodeId) -> Option<&mut Node> {
        self.version = self.version.wrapping_add(1);
        self.nodes.get_mut(id)
    }

    pub fn nodes(&self) -> impl Iterator<Item = &Node> {
        self.nodes.values()
    }

    pub fn node_ids(&self) -> impl Iterator<Item = NodeId> + '_ {
        self.nodes.keys()
    }

    // ---- edges ----

    pub fn add_edge(&mut self, mut edge: Edge) -> EdgeId {
        self.version = self.version.wrapping_add(1);
        self.edges.insert_with_key(|key| {
            edge.id = key;
            edge
        })
    }

    pub fn remove_edge(&mut self, id: EdgeId) -> Option<Edge> {
        let edge = self.edges.remove(id)?;
        self.version = self.version.wrapping_add(1);
        Some(edge)
    }

    pub fn edge(&self, id: EdgeId) -> Option<&Edge> {
        self.edges.get(id)
    }

    pub fn edges(&self) -> impl Iterator<Item = &Edge> {
        self.edges.values()
    }

    pub fn edges_mut(&mut self) -> impl Iterator<Item = &mut Edge> {
        self.version = self.version.wrapping_add(1);
        self.edges.values_mut()
    }

    pub fn edge_ids(&self) -> impl Iterator<Item = EdgeId> + '_ {
        self.edges.keys()
    }

    /// Outgoing edges from `node`.
    pub fn out_edges(&self, node: NodeId) -> impl Iterator<Item = &Edge> {
        self.edges.values().filter(move |e| e.source == node)
    }

    /// Incoming edges to `node`.
    pub fn in_edges(&self, node: NodeId) -> impl Iterator<Item = &Edge> {
        self.edges.values().filter(move |e| e.target == node)
    }

    /// Collect all Loop nodes and their associated body node groups.
    ///
    /// For each Loop node (identified by having a `loop_body` outgoing edge),
    /// BFS-expand the body group along forward edges, excluding:
    /// - `loop_in` back-edges (`target_port == "loop_in"`)
    /// - Edges back to the Loop node itself (e.g. `done`)
    /// - Edges to `done` targets of the same Loop node (prevents absorbing
    ///   the exit node into the body group)
    ///
    /// This is the single source of truth for loop body group computation,
    /// shared by the dagre layout post-processing and the rendering layer.
    pub fn loop_body_groups(
        &self,
    ) -> std::collections::HashMap<NodeId, std::collections::HashSet<NodeId>> {
        use std::collections::{HashMap, HashSet, VecDeque};

        let mut groups: HashMap<NodeId, HashSet<NodeId>> = HashMap::new();

        // Step 1: Find all loop_body edges, grouped by source (Loop node).
        for edge in self.edges() {
            if edge.source_port.as_deref() == Some("loop_body") {
                groups.entry(edge.source).or_default().insert(edge.target);
            }
        }

        // Step 2: BFS-expand each body group along forward edges.
        for (loop_node, body_nodes) in groups.iter_mut() {
            // Pre-compute done targets of this Loop node to exclude from body group.
            let done_targets: HashSet<NodeId> = self
                .edges()
                .filter(|e| {
                    e.source == *loop_node && e.source_port.as_deref() == Some("done")
                })
                .map(|e| e.target)
                .collect();

            let mut queue: VecDeque<NodeId> = body_nodes.iter().copied().collect();
            while let Some(nid) = queue.pop_front() {
                for edge in self.out_edges(nid) {
                    // Skip back-edges (to loop_in)
                    if edge.target_port.as_deref() == Some("loop_in") {
                        continue;
                    }
                    // Skip edges back to the Loop node (e.g. done)
                    if edge.target == *loop_node {
                        continue;
                    }
                    // Skip edges to done targets (prevents absorbing exit nodes)
                    if done_targets.contains(&edge.target) {
                        continue;
                    }
                    if body_nodes.insert(edge.target) {
                        queue.push_back(edge.target);
                    }
                }
            }
        }

        groups
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn add_remove_node_and_edge() {
        let mut g = FlowGraph::new();
        let a = g.add_node("start", serde_json::json!({}));
        let b = g.add_node("end", serde_json::json!({}));
        let v0 = g.version();
        let e = g.add_edge(Edge::new(a, b));
        assert!(g.version() > v0);
        assert_eq!(g.out_edges(a).count(), 1);
        assert_eq!(g.in_edges(b).count(), 1);

        // Removing a node also removes its edges.
        g.remove_node(a);
        assert_eq!(g.in_edges(b).count(), 0);
        assert!(g.edge(e).is_none());
    }
}
