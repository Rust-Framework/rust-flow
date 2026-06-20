use crate::graph::FlowGraph;
use crate::id::{NodeId, PortId};
use crate::math::Point;
use crate::viewport::Viewport;

pub const PORT_HIT_RADIUS: f32 = 12.0;

#[derive(Debug, Clone, Copy)]
pub struct HitNode {
    pub id: NodeId,
    pub z_order: u32,
}

pub fn hit_node_at(graph: &FlowGraph, world: Point) -> Option<NodeId> {
    graph
        .nodes
        .iter()
        .filter_map(|(id, node)| {
            let w = node.size.width;
            let nh = node.size.height;
            let in_bounds = world.x >= node.position.x
                && world.x <= node.position.x + w
                && world.y >= node.position.y
                && world.y <= node.position.y + nh;
            if in_bounds {
                Some(HitNode {
                    id,
                    z_order: node.z_order,
                })
            } else {
                None
            }
        })
        .max_by_key(|h| h.z_order)
        .map(|h| h.id)
}

pub fn hit_port_at(graph: &FlowGraph, vp: &Viewport, local: Point) -> Option<PortId> {
    let frame = crate::scene::SceneFrame::resolve(graph, vp);
    let hit_r = PORT_HIT_RADIUS * vp.zoom;
    for (pid, anchor) in frame.nodes.iter().flat_map(|n| n.port_anchors.iter()) {
        if (local.x - anchor.x).abs() < hit_r && (local.y - anchor.y).abs() < hit_r {
            return Some(*pid);
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::graph::demo_chain_graph;

    #[test]
    fn hit_node_finds_center() {
        let graph = demo_chain_graph();
        let first = graph.nodes.iter().next().unwrap();
        let center = Point::new(
            first.1.position.x + 50.0,
            first.1.position.y + 30.0,
        );
        assert_eq!(hit_node_at(&graph, center), Some(first.0));
    }
}
