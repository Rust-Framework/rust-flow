//! Flow document format — React Flow / xyflow compatible instance schema.

use serde::{Deserialize, Serialize};

use crate::edge::{EdgeShape, EdgeStroke};

pub const FLOW_DOCUMENT_VERSION: &str = "1.0";

/// Top-level flow document (graph instance).
///
/// Field layout follows [React Flow](https://reactflow.dev/api-reference/types/node)
/// and [xyflow](https://github.com/xyflow/xyflow) conventions for interoperability.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FlowDocument {
    /// Flow Schema document version.
    #[serde(default = "default_version")]
    pub version: String,
    /// Human-readable flow name.
    #[serde(default)]
    pub name: String,
    /// Node instances.
    #[serde(default)]
    pub nodes: Vec<FlowDocumentNode>,
    /// Edge instances.
    #[serde(default)]
    pub edges: Vec<FlowDocumentEdge>,
    /// Optional viewport state (pan + zoom).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub viewport: Option<FlowDocumentViewport>,
}

fn default_version() -> String {
    FLOW_DOCUMENT_VERSION.to_string()
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FlowDocumentPosition {
    pub x: f32,
    pub y: f32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FlowDocumentViewport {
    pub x: f32,
    pub y: f32,
    #[serde(default = "default_zoom")]
    pub zoom: f32,
}

fn default_zoom() -> f32 {
    1.0
}

/// A node instance in the document.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FlowDocumentNode {
    pub id: String,
    /// Node type id — matches `FlowTypeRegistry` entry and React Flow `type`.
    #[serde(rename = "type")]
    pub node_type: String,
    pub position: FlowDocumentPosition,
    #[serde(default)]
    pub data: serde_json::Value,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub width: Option<f32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub height: Option<f32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub selected: Option<bool>,
    #[serde(rename = "zIndex", default, skip_serializing_if = "Option::is_none")]
    pub z_index: Option<u32>,
}

/// An edge instance — React Flow `source`/`target` + handle ids.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FlowDocumentEdge {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub id: Option<String>,
    pub source: String,
    pub target: String,
    #[serde(rename = "sourceHandle", skip_serializing_if = "Option::is_none")]
    pub source_handle: Option<String>,
    #[serde(rename = "targetHandle", skip_serializing_if = "Option::is_none")]
    pub target_handle: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub label: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub data: Option<serde_json::Value>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub shape: Option<EdgeShape>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub stroke: Option<EdgeStroke>,
}

impl FlowDocument {
    pub fn new(name: impl Into<String>) -> Self {
        Self {
            version: FLOW_DOCUMENT_VERSION.to_string(),
            name: name.into(),
            nodes: Vec::new(),
            edges: Vec::new(),
            viewport: None,
        }
    }

    pub fn from_json(json: &str) -> Result<Self, serde_json::Error> {
        serde_json::from_str(json)
    }

    pub fn to_json(&self) -> Result<String, serde_json::Error> {
        serde_json::to_string_pretty(self)
    }
}
