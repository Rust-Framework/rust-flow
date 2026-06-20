//! Mind map document format — tree JSON that AI agents can emit for visualization.

use serde::{Deserialize, Serialize};

use crate::auto_layout::LayoutDirection;
use crate::schema::document::{
    FlowDocument, FlowDocumentEdge, FlowDocumentNode, FlowDocumentPosition,
};

pub const MINDMAP_DOCUMENT_VERSION: &str = "mindmap-1.0";

/// Hierarchical mind map (nested children).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MindMapDocument {
    #[serde(default = "default_mindmap_version")]
    pub version: String,
    #[serde(default)]
    pub title: String,
    /// `LR` (default) or `TB`.
    #[serde(default, rename = "layoutDirection")]
    pub layout_direction: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub root: Option<MindMapNode>,
    /// Flat node list — alternative for AI (`parent` links to parent id).
    #[serde(default)]
    pub nodes: Vec<MindMapFlatNode>,
}

fn default_mindmap_version() -> String {
    MINDMAP_DOCUMENT_VERSION.to_string()
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MindMapNode {
    pub id: String,
    pub label: String,
    #[serde(default)]
    pub children: Vec<MindMapNode>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MindMapFlatNode {
    pub id: String,
    pub label: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub parent: Option<String>,
}

impl MindMapDocument {
    pub fn from_json(json: &str) -> Result<Self, serde_json::Error> {
        serde_json::from_str(json)
    }

    pub fn layout_direction(&self) -> LayoutDirection {
        parse_layout_direction(self.layout_direction.as_deref())
    }
}

pub fn is_mindmap_json(json: &str) -> bool {
    let Ok(value) = serde_json::from_str::<serde_json::Value>(json) else {
        return false;
    };
    value
        .get("version")
        .and_then(|v| v.as_str())
        .map(|v| v.starts_with("mindmap"))
        .unwrap_or(false)
}

/// Parse JSON as either a mind map or a standard [`FlowDocument`].
pub fn document_from_any_json(json: &str) -> Result<FlowDocument, serde_json::Error> {
    if is_mindmap_json(json) {
        let mm = MindMapDocument::from_json(json)?;
        Ok(mindmap_to_flow_document(&mm))
    } else {
        FlowDocument::from_json(json)
    }
}

pub fn mindmap_layout_direction_from_json(json: &str) -> LayoutDirection {
    if !is_mindmap_json(json) {
        return LayoutDirection::LeftRight;
    }
    MindMapDocument::from_json(json)
        .map(|mm| mm.layout_direction())
        .unwrap_or(LayoutDirection::LeftRight)
}

/// Convert a mind map tree into a flow document (common nodes + tree edges).
pub fn mindmap_to_flow_document(mm: &MindMapDocument) -> FlowDocument {
    let mut nodes = Vec::new();
    let mut edges = Vec::new();

    if let Some(root) = &mm.root {
        walk_tree(root, None, &mut nodes, &mut edges);
    } else if !mm.nodes.is_empty() {
        build_from_flat(&mm.nodes, &mut nodes, &mut edges);
    }

    FlowDocument {
        version: MINDMAP_DOCUMENT_VERSION.to_string(),
        name: if mm.title.is_empty() {
            "Mind Map".into()
        } else {
            mm.title.clone()
        },
        nodes,
        edges,
        viewport: None,
    }
}

fn walk_tree(
    node: &MindMapNode,
    parent_id: Option<&str>,
    nodes: &mut Vec<FlowDocumentNode>,
    edges: &mut Vec<FlowDocumentEdge>,
) {
    nodes.push(flow_node_from_mindmap(&node.id, &node.label));
    if let Some(parent) = parent_id {
        edges.push(tree_edge(parent, &node.id));
    }
    for child in &node.children {
        walk_tree(child, Some(&node.id), nodes, edges);
    }
}

fn build_from_flat(
    flat: &[MindMapFlatNode],
    nodes: &mut Vec<FlowDocumentNode>,
    edges: &mut Vec<FlowDocumentEdge>,
) {
    for entry in flat {
        nodes.push(flow_node_from_mindmap(&entry.id, &entry.label));
    }
    for entry in flat {
        if let Some(parent) = &entry.parent {
            edges.push(tree_edge(parent, &entry.id));
        }
    }
}

use crate::node_layout::mindmap_node_size;

fn flow_node_from_mindmap(id: &str, label: &str) -> FlowDocumentNode {
    let size = mindmap_node_size(label);
    FlowDocumentNode {
        id: id.to_string(),
        node_type: "common".into(),
        position: FlowDocumentPosition { x: 0.0, y: 0.0 },
        data: serde_json::json!({ "label": label, "mindmap": true }),
        width: Some(size.width),
        height: Some(size.height),
        selected: None,
        z_index: None,
    }
}

fn tree_edge(source: &str, target: &str) -> FlowDocumentEdge {
    FlowDocumentEdge {
        id: Some(format!("e_{source}_{target}")),
        source: source.to_string(),
        target: target.to_string(),
        source_handle: Some("out".into()),
        target_handle: Some("in".into()),
        label: None,
        data: None,
        shape: None,
        stroke: None,
    }
}

fn parse_layout_direction(raw: Option<&str>) -> LayoutDirection {
    match raw.map(|s| s.to_ascii_uppercase()).as_deref() {
        Some("TB") | Some("TD") | Some("TOPBOTTOM") | Some("VERTICAL") => {
            LayoutDirection::TopBottom
        }
        _ => LayoutDirection::LeftRight,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn nested_mindmap_converts_to_flow() {
        let json = r#"{
            "version": "mindmap-1.0",
            "title": "产品规划",
            "root": {
                "id": "root",
                "label": "中心主题",
                "children": [
                    { "id": "a", "label": "分支 A", "children": [] },
                    { "id": "b", "label": "分支 B", "children": [
                        { "id": "b1", "label": "子节点", "children": [] }
                    ]}
                ]
            }
        }"#;
        let doc = document_from_any_json(json).unwrap();
        assert_eq!(doc.nodes.len(), 4);
        assert_eq!(doc.edges.len(), 3);
        assert!(doc.nodes.iter().all(|n| n.node_type == "common"));
    }

    #[test]
    fn flat_mindmap_converts_to_flow() {
        let json = r#"{
            "version": "mindmap-1.0",
            "title": "Flat",
            "nodes": [
                { "id": "1", "label": "Root" },
                { "id": "2", "label": "Child", "parent": "1" }
            ]
        }"#;
        let doc = document_from_any_json(json).unwrap();
        assert_eq!(doc.nodes.len(), 2);
        assert_eq!(doc.edges.len(), 1);
        assert_eq!(doc.edges[0].source, "1");
        assert_eq!(doc.edges[0].target, "2");
    }
}
