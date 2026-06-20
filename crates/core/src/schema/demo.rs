//! Embedded demo flow document (data-driven).

use crate::schema::document::FlowDocument;
use crate::schema::{builtin_type_registry, graph_from_document};
use crate::FlowGraph;

/// Raw JSON for the builtin demo flow (`schemas/demo.flow.json`).
pub fn demo_document_json() -> &'static str {
    include_str!("../../../../schemas/demo.flow.json")
}

/// Parsed demo document.
pub fn demo_document() -> FlowDocument {
    FlowDocument::from_json(demo_document_json()).expect("valid demo.flow.json")
}

/// Raw JSON for the mind map example (`schemas/mindmap.example.json`).
pub fn mindmap_document_json() -> &'static str {
    include_str!("../../../../schemas/mindmap.example.json")
}

/// Build runtime graph from the embedded demo document and apply auto-layout.
pub fn demo_graph_from_document() -> FlowGraph {
    let types = builtin_type_registry();
    let doc = demo_document();
    let mut graph = graph_from_document(&doc, &types);
    graph.auto_layout_default();
    graph
}
