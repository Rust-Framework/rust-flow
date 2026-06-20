//! Flow Schema — document format, type registry, and conversion utilities.

mod builtin;
mod convert;
mod demo;
mod document;
mod mindmap;
mod mermaid;
mod template;
mod types;

pub use builtin::builtin_type_registry;
pub use demo::{demo_document, demo_document_json, demo_graph_from_document, mindmap_document_json};
pub use convert::{
    document_from_graph, document_needs_layout, document_to_graph, graph_from_document,
    graph_to_document, viewport_from_document, viewport_to_document,
};
pub use mermaid::{
    mermaid_layout_direction, mermaid_to_flow_document,
};
pub use mindmap::{
    document_from_any_json, is_mindmap_json, mindmap_layout_direction_from_json,
    mindmap_to_flow_document, MindMapDocument, MindMapFlatNode, MindMapNode,
    MINDMAP_DOCUMENT_VERSION,
};
pub use document::{
    FlowDocument, FlowDocumentEdge, FlowDocumentNode, FlowDocumentPosition, FlowDocumentViewport,
    FLOW_DOCUMENT_VERSION,
};
pub use template::apply_template;
pub use types::{
    FlowFieldDef, FlowFieldType, FlowNodeTypeDef, FlowPortDef, FlowRenderDef, FlowTypeRegistry,
    FLOW_TYPE_REGISTRY_VERSION,
};

use crate::FlowGraph;

impl FlowGraph {
    /// Export graph to a React Flow–compatible [`FlowDocument`].
    pub fn to_document(&self) -> FlowDocument {
        graph_to_document(self)
    }

    /// Import graph from a [`FlowDocument`] using the type registry for port topology.
    pub fn from_document(doc: &FlowDocument, types: &FlowTypeRegistry) -> Self {
        graph_from_document(doc, types)
    }

    /// Serialize to JSON string.
    pub fn to_json(&self) -> Result<String, serde_json::Error> {
        serde_json::to_string_pretty(&self.to_document())
    }

    /// Deserialize from JSON string.
    pub fn from_json(json: &str, types: &FlowTypeRegistry) -> Result<Self, serde_json::Error> {
        let doc = document_from_any_json(json)?;
        Ok(Self::from_document(&doc, types))
    }
}
