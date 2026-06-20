//! Convert between runtime [`FlowGraph`] and [`FlowDocument`].

use std::collections::HashMap;

use slotmap::Key;

use crate::edge::FlowEdge;
use crate::graph::FlowGraph;
use crate::id::{NodeId, PortId};
use crate::math::Point;
use crate::node::FlowNode;
use crate::node_sync::sync_all_structured_nodes;
use crate::schema::document::{
    FlowDocument, FlowDocumentEdge, FlowDocumentNode, FlowDocumentPosition, FlowDocumentViewport,
    FLOW_DOCUMENT_VERSION,
};
use crate::schema::types::FlowTypeRegistry;

/// Export runtime graph to document.
pub fn graph_to_document(graph: &FlowGraph) -> FlowDocument {
    let mut nodes = Vec::new();
    for (_, node) in graph.nodes.iter() {
        let mut data = node.data.clone();
        if let Some(obj) = data.as_object_mut() {
            obj.insert(
                "label".to_string(),
                serde_json::Value::String(node.label.clone()),
            );
        } else if data.is_null() {
            data = serde_json::json!({ "label": node.label });
        }

        nodes.push(FlowDocumentNode {
            id: node.schema_id.clone(),
            node_type: node.node_type.clone(),
            position: FlowDocumentPosition {
                x: node.position.x,
                y: node.position.y,
            },
            data,
            width: Some(node.size.width),
            height: Some(node.size.height),
            selected: Some(node.selected),
            z_index: Some(node.z_order),
        });
    }

    let id_to_schema: HashMap<NodeId, String> = graph
        .nodes
        .iter()
        .map(|(id, n)| (id, n.schema_id.clone()))
        .collect();

    let port_to_handle: HashMap<PortId, (String, String)> = graph
        .ports
        .iter()
        .map(|(pid, port)| {
            let node_id = id_to_schema
                .get(&port.node)
                .cloned()
                .unwrap_or_else(|| format!("node_{}", port.node.data().as_ffi()));
            (pid, (node_id, port.name.clone()))
        })
        .collect();

    let edges: Vec<FlowDocumentEdge> = graph
        .edges
        .iter()
        .enumerate()
        .filter_map(|(idx, edge)| {
            let (source, source_handle) = port_to_handle.get(&edge.from_port)?;
            let (target, target_handle) = port_to_handle.get(&edge.to_port)?;
            Some(FlowDocumentEdge {
                id: Some(format!("edge_{idx}")),
                source: source.clone(),
                target: target.clone(),
                source_handle: Some(source_handle.clone()),
                target_handle: Some(target_handle.clone()),
                label: edge.label.clone(),
                data: None,
                shape: Some(edge.shape),
                stroke: Some(edge.stroke),
            })
        })
        .collect();

    FlowDocument {
        version: FLOW_DOCUMENT_VERSION.to_string(),
        name: graph.name.clone(),
        nodes,
        edges,
        viewport: None,
    }
}

/// True when every node is at the origin — typical for schema files that expect auto-layout.
pub fn document_needs_layout(doc: &FlowDocument) -> bool {
    !doc.nodes.is_empty()
        && doc
            .nodes
            .iter()
            .all(|n| n.position.x.abs() < 0.01 && n.position.y.abs() < 0.01)
}

/// Import document into runtime graph.
pub fn graph_from_document(doc: &FlowDocument, types: &FlowTypeRegistry) -> FlowGraph {
    let mut graph = FlowGraph::new(doc.name.clone());
    graph.is_mindmap = doc.version.starts_with("mindmap");
    let mut node_map: HashMap<String, NodeId> = HashMap::new();
    let mut port_map: HashMap<(String, String), PortId> = HashMap::new();

    for doc_node in &doc.nodes {
        let node_id = graph.add_node(document_node_to_flow(doc_node, types));
        node_map.insert(doc_node.id.clone(), node_id);
    }

    sync_all_structured_nodes(&mut graph);

    for (schema_id, node_id) in &node_map {
        let type_def = types.get(
            graph
                .nodes
                .get(*node_id)
                .map(|n| n.node_type.as_str())
                .unwrap_or(""),
        );
        if let Some(def) = type_def {
            let has_ports = graph
                .ports
                .iter()
                .any(|(_, p)| p.node == *node_id);
            if !has_ports {
                for port_def in &def.ports {
                    if let Some(pid) = graph.add_port(
                        *node_id,
                        &port_def.id,
                        port_def.direction,
                        port_def.side,
                    ) {
                        port_map.insert((schema_id.clone(), port_def.id.clone()), pid);
                    }
                }
            }
        }
        if let Some(node) = graph.nodes.get(*node_id) {
            for (name, pid) in node.inputs.iter().chain(node.outputs.iter()) {
                port_map.insert((schema_id.clone(), name.clone()), *pid);
            }
        }
    }

    for doc_edge in &doc.edges {
        let source_handle = doc_edge.source_handle.as_deref().unwrap_or("out");
        let target_handle = doc_edge.target_handle.as_deref().unwrap_or("in");

        let from_port = port_map.get(&(doc_edge.source.clone(), source_handle.to_string()));
        let to_port = port_map.get(&(doc_edge.target.clone(), target_handle.to_string()));

        if let (Some(from), Some(to)) = (from_port, to_port) {
            let mut edge = FlowEdge::new(*from, *to);
            edge.label = doc_edge.label.clone();
            if let Some(shape) = doc_edge.shape {
                edge.shape = shape;
            }
            if let Some(stroke) = doc_edge.stroke {
                edge.stroke = stroke;
            }
            graph.add_edge(edge);
        }
    }

    graph
}

pub fn document_to_graph(doc: &FlowDocument, types: &FlowTypeRegistry) -> FlowGraph {
    graph_from_document(doc, types)
}

pub fn document_from_graph(graph: &FlowGraph) -> FlowDocument {
    graph_to_document(graph)
}

fn document_node_to_flow(doc_node: &FlowDocumentNode, types: &FlowTypeRegistry) -> FlowNode {
    let label = extract_label(doc_node);
    let type_def = types.get(&doc_node.node_type);

    let size = if let (Some(w), Some(h)) = (doc_node.width, doc_node.height) {
        crate::math::Size::new(w, h)
    } else if let Some(def) = type_def {
        def.default_size.to_size()
    } else {
        crate::math::Size::new(200.0, crate::layout::VISUAL_HEIGHT)
    };

    let data = merge_data(doc_node, type_def);

    FlowNode {
        id: NodeId::default(),
        schema_id: doc_node.id.clone(),
        position: Point::new(doc_node.position.x, doc_node.position.y),
        size,
        node_type: doc_node.node_type.clone(),
        label,
        data,
        inputs: Vec::new(),
        outputs: Vec::new(),
        selected: doc_node.selected.unwrap_or(false),
        z_order: doc_node.z_index.unwrap_or(0),
    }
}

fn extract_label(doc_node: &FlowDocumentNode) -> String {
    if let Some(s) = doc_node.data.get("label").and_then(|v| v.as_str()) {
        return s.to_string();
    }
    doc_node
        .data
        .as_str()
        .map(|s| s.to_string())
        .unwrap_or_else(|| doc_node.node_type.clone())
}

fn merge_data(
    doc_node: &FlowDocumentNode,
    type_def: Option<&crate::schema::types::FlowNodeTypeDef>,
) -> serde_json::Value {
    let mut data = if let Some(def) = type_def {
        def.default_data.clone()
    } else {
        serde_json::json!({})
    };

    if let Some(patch) = doc_node.data.as_object() {
        if let Some(base) = data.as_object_mut() {
            for (k, v) in patch {
                if k != "label" {
                    base.insert(k.clone(), v.clone());
                }
            }
            serde_json::Value::Object(base.clone())
        } else {
            let mut merged = patch.clone();
            merged.remove("label");
            serde_json::Value::Object(merged)
        }
    } else {
        data
    }
}

/// Apply viewport from document.
pub fn viewport_from_document(vp: &FlowDocumentViewport) -> crate::viewport::Viewport {
    let mut viewport = crate::viewport::Viewport::new(vp.zoom);
    viewport.pan = Point::new(vp.x, vp.y);
    viewport
}

pub fn viewport_to_document(vp: &crate::viewport::Viewport) -> FlowDocumentViewport {
    FlowDocumentViewport {
        x: vp.pan.x,
        y: vp.pan.y,
        zoom: vp.zoom,
    }
}
