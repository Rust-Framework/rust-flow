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

    #[test]
    fn load_orchestrator_mindmap_json() {
        let json = include_str!("../../../../schemas/orchestrator.mindmap.json");
        let doc = document_from_any_json(json).unwrap();
        assert!(doc.version.starts_with("mindmap"));
        assert!(doc.nodes.len() >= 7, "should have at least 7 nodes");
    }

    #[test]
    fn load_orchestrator_flowchart_json() {
        let json = include_str!("../../../../schemas/orchestrator.flowchart.json");
        let doc = document_from_any_json(json).unwrap();
        assert!(doc.version.starts_with("flowchart") || doc.version == "1.0");
        assert_eq!(doc.nodes.len(), 9);
        assert_eq!(doc.edges.len(), 11);
        // 验证反馈边存在
        let feedback_edges = doc.edges.iter()
            .filter(|e| e.label.as_deref() == Some("FAIL") || e.label.as_deref() == Some("阻塞项"))
            .count();
        assert_eq!(feedback_edges, 2, "should have 2 feedback edges");
    }

    /// Full pipeline: load flowchart JSON → build graph → layout → resolve scene → check edge paths.
    #[test]
    fn orchestrator_flowchart_full_pipeline() {
        use crate::FlowGraph;
        use crate::SceneFrame;
        use crate::viewport::Viewport;
        use crate::builtin_type_registry;

        let json = include_str!("../../../../schemas/orchestrator.flowchart.json");
        let doc = document_from_any_json(json).unwrap();
        let types = builtin_type_registry();
        let mut graph = FlowGraph::from_document(&doc, &types);

        println!("\n=== Flowchart JSON Pipeline ===");
        println!("version: {}", doc.version);
        println!("is_mindmap: {}", graph.is_mindmap);
        println!("nodes: {}, edges: {}", graph.nodes.len(), graph.edges.len());

        // Apply TB orientation + Mermaid layout
        use crate::{apply_flow_orientation, LayoutDirection};
        apply_flow_orientation(&mut graph, LayoutDirection::TopBottom);
        graph.auto_layout_mermaid(&crate::LayoutOptions::mermaid_flowchart_tb());

        // Check Dagre routes
        let route_count = graph.dagre_edge_routes.iter().filter(|r| r.is_some()).count();
        println!("dagre_edge_routes populated: {}/{}", route_count, graph.dagre_edge_routes.len());

        // Resolve scene frame
        let viewport = Viewport::default();
        let frame = SceneFrame::resolve(&graph, &viewport);

        let mut bezier_count = 0;
        let mut polyline_count = 0;
        for edge in &frame.edges {
            match &edge.path {
                crate::EdgePath::Bezier(_) => bezier_count += 1,
                crate::EdgePath::Polyline(pts) => {
                    polyline_count += 1;
                    println!("  Polyline edge: {} points", pts.len());
                }
                _ => {}
            }
        }

        println!("Edge path types: {} polylines, {} beziers",
            polyline_count, bezier_count);

        // Most edges should be Dagre polylines
        assert!(
            polyline_count >= 8,
            "expected >=8 polyline edges, got {} polylines / {} beziers",
            polyline_count, bezier_count
        );
    }
}
