//! Node type catalog — declarative definitions for data-driven rendering.

use std::collections::HashMap;

use serde::{Deserialize, Serialize};

use crate::math::Size;
use crate::port::{PortDirection, PortSide};

pub const FLOW_TYPE_REGISTRY_VERSION: &str = "1.0";

/// Registry of node type definitions (the "type schema").
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FlowTypeRegistry {
    #[serde(default = "default_registry_version")]
    pub version: String,
    #[serde(default)]
    pub types: HashMap<String, FlowNodeTypeDef>,
}

fn default_registry_version() -> String {
    FLOW_TYPE_REGISTRY_VERSION.to_string()
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FlowNodeTypeDef {
    /// Display name for palettes and panels.
    pub label: String,
    #[serde(default)]
    pub category: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    #[serde(default)]
    pub default_data: serde_json::Value,
    pub default_size: FlowSizeDef,
    #[serde(default)]
    pub ports: Vec<FlowPortDef>,
    #[serde(default)]
    pub fields: Vec<FlowFieldDef>,
    pub render: FlowRenderDef,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FlowSizeDef {
    pub width: f32,
    pub height: f32,
}

impl FlowSizeDef {
    pub fn to_size(&self) -> Size {
        Size::new(self.width, self.height)
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FlowPortDef {
    pub id: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub label: Option<String>,
    pub direction: PortDirection,
    pub side: PortSide,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FlowFieldDef {
    pub key: String,
    pub label: String,
    #[serde(rename = "type", default)]
    pub field_type: FlowFieldType,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub default: Option<serde_json::Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub placeholder: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "lowercase")]
pub enum FlowFieldType {
    #[default]
    Text,
    Number,
    Expression,
    /// Read-only section header in the property panel.
    Section,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FlowRenderDef {
    /// Accent bar color (hex `#RRGGBB` or `#RRGGBBAA`).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub accent: Option<String>,
    /// Title template — supports `{{label}}`, `{{data.key}}`.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub title: Option<String>,
    /// Subtitle template.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub subtitle: Option<String>,
    /// Additional body lines (templates).
    #[serde(default)]
    pub body: Vec<String>,
    /// Footer hint (e.g. port legend).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub footer: Option<String>,
}

impl FlowTypeRegistry {
    pub fn new() -> Self {
        Self {
            version: FLOW_TYPE_REGISTRY_VERSION.to_string(),
            types: HashMap::new(),
        }
    }

    pub fn register(&mut self, type_id: impl Into<String>, def: FlowNodeTypeDef) {
        self.types.insert(type_id.into(), def);
    }

    pub fn get(&self, type_id: &str) -> Option<&FlowNodeTypeDef> {
        self.types.get(type_id)
    }

    pub fn from_json(json: &str) -> Result<Self, serde_json::Error> {
        serde_json::from_str(json)
    }

    pub fn to_json(&self) -> Result<String, serde_json::Error> {
        serde_json::to_string_pretty(self)
    }
}
