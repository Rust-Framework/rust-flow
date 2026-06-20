//! Schema round-trip and React Flow compatibility tests.

use rust_agent_flow::{
    builtin_type_registry, demo_chain_graph, graph_from_document, graph_to_document, FlowDocument,
    FlowTypeRegistry,
};

#[test]
fn schema_document_needs_layout_at_origin() {
    use rust_agent_flow::{demo_document, document_needs_layout};

    let doc = demo_document();
    assert!(document_needs_layout(&doc));

    let mut moved = doc.clone();
    moved.nodes[0].position.x = 120.0;
    assert!(!document_needs_layout(&moved));
}

#[test]
fn schema_document_roundtrip() {
    let types = builtin_type_registry();
    let graph = demo_chain_graph();
    let doc = graph_to_document(&graph);
    let restored = graph_from_document(&doc, &types);

    assert_eq!(restored.name, graph.name);
    assert_eq!(restored.nodes.len(), graph.nodes.len());
    assert_eq!(restored.edges.len(), graph.edges.len());
}

#[test]
fn schema_json_roundtrip() {
    let types = builtin_type_registry();
    let graph = demo_chain_graph();
    let json = graph.to_json().expect("serialize");
    let restored = rust_agent_flow::FlowGraph::from_json(&json, &types).expect("deserialize");
    assert_eq!(restored.nodes.len(), graph.nodes.len());
}

#[test]
fn schema_react_flow_edge_fields() {
    let doc = graph_to_document(&demo_chain_graph());
    let edge = doc.edges.first().expect("edge");
    assert!(!edge.source.is_empty());
    assert!(!edge.target.is_empty());
    assert!(edge.source_handle.is_some());
    assert!(edge.target_handle.is_some());
}

#[test]
fn schema_type_registry_json() {
    let types = builtin_type_registry();
    let json = types.to_json().expect("types json");
    let parsed: FlowTypeRegistry = FlowTypeRegistry::from_json(&json).expect("parse");
    assert!(parsed.get("common").is_some());
    assert!(parsed.get("branch").is_some());
    assert!(parsed.get("loop").is_some());
}

#[test]
fn schema_document_from_json_example() {
    let json = rust_agent_flow::demo_document_json();
    let types = builtin_type_registry();
    let doc = FlowDocument::from_json(json).expect("parse demo");
    let graph = graph_from_document(&doc, &types);
    assert_eq!(graph.nodes.len(), 10);
    assert_eq!(graph.edges.len(), 11);
}

#[test]
fn schema_demo_uses_all_node_types() {
    let graph = demo_chain_graph();
    let types: std::collections::HashSet<String> = graph
        .nodes
        .values()
        .map(|n| n.node_type.clone())
        .collect();
    assert!(types.contains("common"));
    assert!(types.contains("branch"));
    assert!(types.contains("trigger"));
    assert!(types.contains("loop"));
}

#[test]
fn schema_demo_closed_loop() {
    let doc = rust_agent_flow::demo_document();
    let has_loop_back = doc.edges.iter().any(|e| {
        e.source == "deduct_stock"
            && e.target == "loop_lines"
            && e.source_handle.as_deref() == Some("out")
            && e.target_handle.as_deref() == Some("continue")
    });
    assert!(has_loop_back, "demo should have loop continue edge");
}

#[test]
fn schema_invariants_after_import() {
    use rust_agent_flow::{check_frame, SceneFrame, Viewport};

    let json = rust_agent_flow::demo_document_json();
    let types = builtin_type_registry();
    let doc = FlowDocument::from_json(json).expect("parse");
    let graph = graph_from_document(&doc, &types);
    let frame = SceneFrame::resolve(&graph, &Viewport::default());
    check_frame(&frame).expect("invariants");
}
