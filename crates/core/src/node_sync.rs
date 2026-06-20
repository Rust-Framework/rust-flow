//! Sync ports and dimensions for structured node types.

use crate::graph::FlowGraph;
use crate::id::{NodeId, PortId};
use crate::node_layout::{
    branch_node_size, loop_node_size, mindmap_node_size, parse_branch_items,
};
use crate::node_type::{BRANCH, COMMON, HTTP, LOOP, TRIGGER};
use crate::orientation::{flow_input_side, flow_output_side};
use crate::port::{PortDirection, PortSide};

/// After loading or editing node data, normalize ports and size.
pub fn sync_structured_node(graph: &mut FlowGraph, node_id: NodeId) {
    let node_type = graph.nodes.get(node_id).map(|n| n.node_type.clone());
    let Some(node_type) = node_type else {
        return;
    };
    match node_type.as_str() {
        BRANCH => sync_branch_node(graph, node_id),
        LOOP => sync_loop_node(graph, node_id),
        TRIGGER => sync_trigger_node(graph, node_id),
        HTTP => sync_http_node(graph, node_id),
        COMMON => sync_mindmap_node(graph, node_id),
        _ => {}
    }
}

fn is_mindmap_node(graph: &FlowGraph, node_id: NodeId) -> bool {
    graph
        .nodes
        .get(node_id)
        .and_then(|n| n.data.get("mindmap"))
        .and_then(|v| v.as_bool())
        .unwrap_or(false)
}

fn sync_mindmap_node(graph: &mut FlowGraph, node_id: NodeId) {
    if !is_mindmap_node(graph, node_id) {
        return;
    }
    if let Some(node) = graph.nodes.get_mut(node_id) {
        node.size = mindmap_node_size(&node.label);
    }
}

pub fn sync_all_structured_nodes(graph: &mut FlowGraph) {
    let ids: Vec<NodeId> = graph.nodes.iter().map(|(id, _)| id).collect();
    for id in ids {
        sync_structured_node(graph, id);
    }
}

fn sync_branch_node(graph: &mut FlowGraph, node_id: NodeId) {
    let data = graph.nodes.get(node_id).map(|n| n.data.clone()).unwrap_or_default();
    if let Some(node) = graph.nodes.get_mut(node_id) {
        node.size = branch_node_size(&data);
    }

    let branches = parse_branch_items(&data);
    let want: Vec<String> = branches.iter().map(|b| b.id.clone()).collect();

    remove_outputs_not_in(graph, node_id, &want);

    if graph
        .ports
        .iter()
        .filter(|(_, p)| p.node == node_id && p.direction == PortDirection::Input)
        .next()
        .is_none()
    {
        graph.add_port(
            node_id,
            "in",
            PortDirection::Input,
            flow_input_side(graph.layout_direction),
        );
    }

    for id in want {
        if !has_port(graph, node_id, &id, PortDirection::Output) {
            graph.add_port(node_id, &id, PortDirection::Output, PortSide::Right);
        }
    }
}

fn sync_loop_node(graph: &mut FlowGraph, node_id: NodeId) {
    if let Some(node) = graph.nodes.get_mut(node_id) {
        node.size = loop_node_size();
    }

    let direction = graph.layout_direction;
    let inputs = ["in", "continue"];
    let outputs = ["out", "body"];

    for name in inputs {
        if !has_port(graph, node_id, name, PortDirection::Input) {
            let side = if name == "continue" || name == "in" {
                PortSide::Left
            } else {
                flow_input_side(direction)
            };
            graph.add_port(node_id, name, PortDirection::Input, side);
        }
    }
    for name in outputs {
        if !has_port(graph, node_id, name, PortDirection::Output) {
            let side = if name == "body" {
                PortSide::Bottom
            } else {
                PortSide::Right
            };
            graph.add_port(node_id, name, PortDirection::Output, side);
        }
    }
}

fn sync_trigger_node(graph: &mut FlowGraph, node_id: NodeId) {
    remove_inputs(graph, node_id);
    if !has_port(graph, node_id, "out", PortDirection::Output) {
        graph.add_port(
            node_id,
            "out",
            PortDirection::Output,
            flow_output_side(graph.layout_direction),
        );
    }
}

fn sync_http_node(graph: &mut FlowGraph, node_id: NodeId) {
    if !has_port(graph, node_id, "in", PortDirection::Input) {
        graph.add_port(
            node_id,
            "in",
            PortDirection::Input,
            flow_input_side(graph.layout_direction),
        );
    }
    if !has_port(graph, node_id, "out", PortDirection::Output) {
        graph.add_port(
            node_id,
            "out",
            PortDirection::Output,
            flow_output_side(graph.layout_direction),
        );
    }
}

fn has_port(
    graph: &FlowGraph,
    node_id: NodeId,
    name: &str,
    direction: PortDirection,
) -> bool {
    graph
        .ports
        .iter()
        .any(|(_, p)| p.node == node_id && p.name == name && p.direction == direction)
}

fn remove_outputs_not_in(graph: &mut FlowGraph, node_id: NodeId, keep: &[String]) {
    let to_remove: Vec<PortId> = graph
        .ports
        .iter()
        .filter(|(_, p)| {
            p.node == node_id
                && p.direction == PortDirection::Output
                && !keep.contains(&p.name)
        })
        .map(|(id, _)| id)
        .collect();

    for pid in to_remove {
        remove_port(graph, pid);
    }
}

fn remove_inputs(graph: &mut FlowGraph, node_id: NodeId) {
    let to_remove: Vec<PortId> = graph
        .ports
        .iter()
        .filter(|(_, p)| p.node == node_id && p.direction == PortDirection::Input)
        .map(|(id, _)| id)
        .collect();
    for pid in to_remove {
        remove_port(graph, pid);
    }
}

fn remove_port(graph: &mut FlowGraph, port_id: PortId) {
    graph.edges.retain(|e| e.from_port != port_id && e.to_port != port_id);
    if let Some(port) = graph.ports.remove(port_id) {
        if let Some(node) = graph.nodes.get_mut(port.node) {
            node.inputs.retain(|(_, id)| *id != port_id);
            node.outputs.retain(|(_, id)| *id != port_id);
        }
    }
}

pub fn toggle_branch_collapsed(graph: &mut FlowGraph, node_id: NodeId) {
    if let Some(node) = graph.nodes.get_mut(node_id) {
        if node.node_type != BRANCH {
            return;
        }
        let collapsed = !node
            .data
            .get("collapsed")
            .and_then(|v| v.as_bool())
            .unwrap_or(false);
        if let Some(obj) = node.data.as_object_mut() {
            obj.insert("collapsed".into(), serde_json::json!(collapsed));
        }
    }
    sync_branch_node(graph, node_id);
}
