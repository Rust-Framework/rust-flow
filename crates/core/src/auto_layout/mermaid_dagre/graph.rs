//! Internal layout graph for the Mermaid-style dagre pipeline.
//!
//! Self-contained representation independent of [`crate::graph::FlowGraph`].
//! The pipeline operates on node *indices* (`usize`) into a flat `Vec<Node>`,
//! mirroring `@dagrejs/dagre`'s `graphlib` semantics (multigraph + dummy nodes).

use crate::id::NodeId;
use crate::math::Point;

/// A node in the layout graph. Real nodes carry their [`NodeId`]; dummy nodes
/// (inserted by long-edge normalization and edge-label proxies) have `id = None`.
#[derive(Debug, Clone)]
pub struct LayoutNode {
    /// Original [`NodeId`] for real nodes; `None` for virtual/dummy nodes.
    pub id: Option<NodeId>,
    pub width: f32,
    pub height: f32,
    /// Layer index (0 = topmost rank). Assigned by the rank phase.
    pub rank: i32,
    /// Position within a rank. Assigned by the order phase.
    pub order: usize,
    /// Final center x (in rank-axis = Y orientation, i.e. TB internal space).
    pub x: f32,
    /// Final center y.
    pub y: f32,
    pub is_dummy: bool,
    /// Border dummy marker for compound clusters (unused for flat graphs but
    /// kept so the pipeline matches dagre's shape).
    #[allow(dead_code)]
    pub border_left: bool,
    #[allow(dead_code)]
    pub border_right: bool,
}

impl LayoutNode {
    pub fn real(id: NodeId, width: f32, height: f32) -> Self {
        Self {
            id: Some(id),
            width,
            height,
            rank: 0,
            order: 0,
            x: 0.0,
            y: 0.0,
            is_dummy: false,
            border_left: false,
            border_right: false,
        }
    }

    pub fn dummy(rank: i32) -> Self {
        Self {
            id: None,
            width: 0.0,
            height: 0.0,
            rank,
            order: 0,
            x: 0.0,
            y: 0.0,
            is_dummy: true,
            border_left: false,
            border_right: false,
        }
    }
}

/// An edge in the layout graph. Multigraph: parallel edges between the same
/// pair coexist (each gets a distinct `name`).
#[derive(Debug, Clone)]
pub struct LayoutEdge {
    pub from: usize,
    pub to: usize,
    /// Minimum rank span (dagre `minlen`). Doubled during `makeSpaceForEdgeLabels`.
    pub minlen: i32,
    /// Edge weight for barycenter / network-simplex (dagre default 1, Mermaid
    /// main edges 4, feedback 1).
    pub weight: f32,
    /// True if reversed during cycle breaking; `undo` restores direction and
    /// the route point list is flipped.
    pub reversed: bool,
    /// True if this edge was identified as a feedback/back edge and should be
    /// treated specially (lower weight, larger minlen) like Mermaid.
    pub feedback: bool,
    pub label: Option<String>,
    pub label_width: f32,
    pub label_height: f32,
    /// `labelpos` offset rank (rank at which the label proxy node sits).
    pub label_rank: Option<i32>,
    /// Index back into `FlowGraph::edges` for writing routes; `None` for
    /// synthetic edges introduced by normalization.
    pub original_idx: Option<usize>,
    /// Computed orthogonal polyline (world space, center-anchored). Filled by
    /// the routing phase.
    pub points: Vec<Point>,
    /// Label anchor in world space.
    pub label_pos: Option<Point>,
    /// Multigraph discriminator.
    pub name: Option<String>,
}

impl LayoutEdge {
    pub fn new(from: usize, to: usize) -> Self {
        Self {
            from,
            to,
            minlen: 1,
            weight: 1.0,
            reversed: false,
            feedback: false,
            label: None,
            label_width: 0.0,
            label_height: 0.0,
            label_rank: None,
            original_idx: None,
            points: Vec::new(),
            label_pos: None,
            name: None,
        }
    }
}

/// The layout graph. All pipeline phases mutate this in place.
#[derive(Debug, Clone, Default)]
pub struct LayoutGraph {
    pub nodes: Vec<LayoutNode>,
    pub edges: Vec<LayoutEdge>,
    pub ranksep: f32,
    pub nodesep: f32,
    pub edgesep: f32,
    pub marginx: f32,
    pub marginy: f32,
}

impl LayoutGraph {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn add_real_node(&mut self, id: NodeId, width: f32, height: f32) -> usize {
        let idx = self.nodes.len();
        self.nodes.push(LayoutNode::real(id, width, height));
        idx
    }

    pub fn add_dummy(&mut self, rank: i32) -> usize {
        let idx = self.nodes.len();
        self.nodes.push(LayoutNode::dummy(rank));
        idx
    }

    pub fn add_edge(&mut self, from: usize, to: usize) -> usize {
        let idx = self.edges.len();
        self.edges.push(LayoutEdge::new(from, to));
        idx
    }

    /// Out-edges of `v` (excluding reversed feedback edges when `skip_reversed`).
    pub fn out_edges(&self, v: usize) -> impl Iterator<Item = (usize, &LayoutEdge)> {
        self.edges
            .iter()
            .enumerate()
            .filter(move |(_, e)| e.from == v)
    }

    pub fn in_edges(&self, v: usize) -> impl Iterator<Item = (usize, &LayoutEdge)> {
        self.edges
            .iter()
            .enumerate()
            .filter(move |(_, e)| e.to == v)
    }

    /// Predecessor node indices (respecting current direction).
    pub fn preds(&self, v: usize) -> Vec<usize> {
        self.in_edges(v).map(|(_, e)| e.from).collect()
    }

    /// Successor node indices.
    pub fn succs(&self, v: usize) -> Vec<usize> {
        self.out_edges(v).map(|(_, e)| e.to).collect()
    }

    /// Nodes grouped by rank, sorted by `order`.
    pub fn layers(&self) -> Vec<Vec<usize>> {
        let max_rank = self.nodes.iter().map(|n| n.rank).max().unwrap_or(0);
        let mut layers: Vec<Vec<usize>> = vec![Vec::new(); (max_rank + 1) as usize];
        for (i, n) in self.nodes.iter().enumerate() {
            if n.rank >= 0 {
                layers[n.rank as usize].push(i);
            }
        }
        for layer in &mut layers {
            layer.sort_by_key(|&i| self.nodes[i].order);
        }
        layers
    }

    pub fn max_rank(&self) -> i32 {
        self.nodes.iter().map(|n| n.rank).max().unwrap_or(0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn layers_group_by_rank_sorted_by_order() {
        let mut g = LayoutGraph::new();
        let a = g.add_real_node(NodeId::default(), 10.0, 10.0);
        let b = g.add_real_node(NodeId::default(), 10.0, 10.0);
        let c = g.add_real_node(NodeId::default(), 10.0, 10.0);
        g.nodes[a].rank = 0;
        g.nodes[b].rank = 1;
        g.nodes[c].rank = 1;
        g.nodes[b].order = 1;
        g.nodes[c].order = 0;
        let layers = g.layers();
        assert_eq!(layers[0], vec![a]);
        assert_eq!(layers[1], vec![c, b]);
    }
}
