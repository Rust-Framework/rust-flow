//! Flow direction — port sides and Mermaid-style subgraph semantics.

use crate::auto_layout::LayoutDirection;
use crate::graph::FlowGraph;
use crate::id::NodeId;
use crate::node_type::{BRANCH, COMMON, HTTP, LOOP, TRIGGER};
use crate::node_sync::sync_all_structured_nodes;
use crate::port::{PortDirection, PortSide};

/// Primary input side: left (LR) or top (TB).
pub fn flow_input_side(direction: LayoutDirection) -> PortSide {
    match direction {
        LayoutDirection::LeftRight => PortSide::Left,
        LayoutDirection::TopBottom => PortSide::Top,
    }
}

/// Primary output side: right (LR) or bottom (TB).
pub fn flow_output_side(direction: LayoutDirection) -> PortSide {
    match direction {
        LayoutDirection::LeftRight => PortSide::Right,
        LayoutDirection::TopBottom => PortSide::Bottom,
    }
}

/// Apply port sides for the whole graph (React Flow `sourcePosition` / `targetPosition`).
pub fn apply_flow_orientation(graph: &mut FlowGraph, direction: LayoutDirection) {
    graph.layout_direction = direction;

    for (_, port) in graph.ports.iter_mut() {
        if let Some(node) = graph.nodes.get(port.node) {
            port.side = port_side_for(&node.node_type, &port.name, port.direction, direction);
        }
    }

    sync_all_structured_nodes(graph);
}

fn port_side_for(
    node_type: &str,
    port_name: &str,
    direction: PortDirection,
    flow: LayoutDirection,
) -> PortSide {
    match node_type {
        LOOP => loop_port_side(port_name, direction, flow),
        BRANCH => branch_port_side(port_name, direction, flow),
        TRIGGER => {
            if direction == PortDirection::Output {
                flow_output_side(flow)
            } else {
                flow_input_side(flow)
            }
        }
        COMMON | HTTP => {
            if direction == PortDirection::Input {
                flow_input_side(flow)
            } else {
                flow_output_side(flow)
            }
        }
        _ => {
            if direction == PortDirection::Input {
                flow_input_side(flow)
            } else {
                flow_output_side(flow)
            }
        }
    }
}

/// Loop shell: main flow in/out on header; continue back on left footer; body down.
fn loop_port_side(port_name: &str, direction: PortDirection, flow: LayoutDirection) -> PortSide {
    match (port_name, direction) {
        ("body", PortDirection::Output) => PortSide::Bottom,
        ("continue", PortDirection::Input) => PortSide::Left,
        ("in", PortDirection::Input) => PortSide::Left,
        ("out", PortDirection::Output) => PortSide::Right,
        (_, PortDirection::Input) => flow_input_side(flow),
        (_, PortDirection::Output) => flow_output_side(flow),
    }
}

fn branch_port_side(_port_name: &str, direction: PortDirection, _flow: LayoutDirection) -> PortSide {
    match direction {
        PortDirection::Input => flow_input_side(_flow),
        PortDirection::Output => PortSide::Right,
    }
}

/// Nodes inside a loop body (excluded from the main Dagre graph, laid out TB).
pub fn collect_loop_body_nodes(graph: &FlowGraph, loop_id: NodeId) -> Vec<NodeId> {
    let body_port = graph
        .ports
        .iter()
        .find(|(_, p)| p.node == loop_id && p.name == "body" && p.is_output())
        .map(|(id, _)| id);

    let start = body_port.and_then(|bp| {
        graph
            .edges
            .iter()
            .find(|e| e.from_port == bp)
            .and_then(|e| graph.ports.get(e.to_port).map(|p| p.node))
    });

    let Some(start) = start else {
        return Vec::new();
    };

    let mut region = std::collections::HashSet::from([start]);
    let mut stack = vec![start];

    while let Some(n) = stack.pop() {
        for edge in &graph.edges {
            let from = match graph.ports.get(edge.from_port) {
                Some(p) => p,
                None => continue,
            };
            let to = match graph.ports.get(edge.to_port) {
                Some(p) => p,
                None => continue,
            };

            if from.node == n {
                if to.node == loop_id {
                    continue;
                }
                if !region.contains(&to.node) {
                    region.insert(to.node);
                    stack.push(to.node);
                }
            } else if to.node == n && from.node != loop_id && !region.contains(&from.node) {
                region.insert(from.node);
                stack.push(from.node);
            }
        }
    }

    region.into_iter().collect()
}

pub fn loop_container_overlap_allowed(graph: &FlowGraph, a: NodeId, b: NodeId) -> bool {
    for (loop_id, node) in graph.nodes.iter() {
        if node.node_type != LOOP {
            continue;
        }
        let children = collect_loop_body_nodes(graph, loop_id);
        if (a == loop_id && children.contains(&b)) || (b == loop_id && children.contains(&a)) {
            return true;
        }
    }
    false
}

pub fn all_loop_body_nodes(graph: &FlowGraph) -> std::collections::HashSet<NodeId> {
    let mut set = std::collections::HashSet::new();
    for (id, node) in graph.nodes.iter() {
        if node.node_type == LOOP {
            for child in collect_loop_body_nodes(graph, id) {
                set.insert(child);
            }
        }
    }
    set
}
