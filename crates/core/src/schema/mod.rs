//! Schema data standard: declarative node definitions used to match `IFlowNode`
//! implementations by `kind` (strategy pattern) and to declare port specs and
//! field definitions.
//!
//! ## FieldSpec 驱动的属性面板
//!
//! `NodeSchema.fields` 描述 `node.data` 的字段结构，属性面板根据字段类型
//! 自动生成编辑界面（Text/TextArea/CodeEditor/CodeBlock/Number/Switch/
//! Dropdown/List），消除 per-kind 面板分发。
//!
//! ## FlowDocument 数据协议
//!
//! `FlowDocument` 是流程图的序列化协议（JSON），包含元数据 + 节点 + 边，
//! 支持 `FlowGraph::from_document` / `to_document` 互转。

use crate::geometry::{PointF, SizeF};
use crate::graph::{EdgeType, NodeKind, PortDirection, PortId, PortSide};
use serde::{Deserialize, Serialize};

/// Specification of a single port on a node schema.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PortSpec {
    pub id: PortId,
    pub direction: PortDirection,
    /// `Auto` lets the framework compute the side dynamically.
    #[serde(default)]
    pub side: PortSide,
    pub label: Option<String>,
}

impl PortSpec {
    pub fn new(id: impl Into<PortId>, direction: PortDirection, side: PortSide) -> Self {
        Self {
            id: id.into(),
            direction,
            side,
            label: None,
        }
    }

    pub fn with_label(mut self, label: impl Into<String>) -> Self {
        self.label = Some(label.into());
        self
    }
}

// ====== 字段类型定义 ======

/// 字段类型：驱动属性面板渲染对应的编辑控件。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum FieldType {
    /// 单行文本（普通 Input）。
    Text,
    /// 多行文本（TextArea，rows=4）。
    TextArea,
    /// 单行代码编辑器（表达式，无行号、无左边距，禁止换行）。
    CodeEditor,
    /// 多行代码编辑器（带行号、自动缩进）。
    CodeBlock,
    /// 数字输入。
    Number,
    /// 布尔开关（Switch）。
    Switch,
    /// 枚举下拉选择。
    Dropdown(Vec<DropdownOption>),
    /// 动态列表（条件分支/参数/变量等，可增删条目）。
    List(ListSpec),
}

/// Dropdown 选项。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DropdownOption {
    /// 存储到 node.data 的值。
    pub value: String,
    /// 显示标签（由 gpui 层做 i18n 映射）。
    pub label: String,
}

impl DropdownOption {
    pub fn new(value: impl Into<String>, label: impl Into<String>) -> Self {
        Self {
            value: value.into(),
            label: label.into(),
        }
    }
}

/// 动态列表字段规格。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ListSpec {
    /// 每个条目的字段定义（如 name/type/value）。
    pub item_fields: Vec<FieldSpec>,
    /// 最小条数（如 Condition 的 Else 兜底）。
    #[serde(default)]
    pub min_items: usize,
}

impl ListSpec {
    pub fn new(item_fields: Vec<FieldSpec>) -> Self {
        Self {
            item_fields,
            min_items: 0,
        }
    }

    pub fn with_min_items(mut self, n: usize) -> Self {
        self.min_items = n;
        self
    }
}

/// 字段规格：描述 node.data 中的一个字段。
///
/// label 由 gpui 层根据 (kind, key) 做 i18n 映射，core 层仅存描述性默认标签。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FieldSpec {
    /// node.data 中的键名。
    pub key: String,
    /// 默认标签（gpui 层可覆盖为 i18n 文案）。
    pub label: String,
    /// 字段类型。
    pub field_type: FieldType,
    /// 默认值（创建节点时填入 node.data）。
    pub default: serde_json::Value,
    /// 占位符（gpui 层可覆盖为 i18n 文案）。
    #[serde(default)]
    pub placeholder: Option<String>,
}

impl FieldSpec {
    pub fn new(key: impl Into<String>, label: impl Into<String>, field_type: FieldType) -> Self {
        Self {
            key: key.into(),
            label: label.into(),
            field_type,
            default: serde_json::Value::Null,
            placeholder: None,
        }
    }

    pub fn with_default(mut self, default: serde_json::Value) -> Self {
        self.default = default;
        self
    }

    pub fn with_placeholder(mut self, placeholder: impl Into<String>) -> Self {
        self.placeholder = Some(placeholder.into());
        self
    }
}

// ====== NodeSchema ======

/// Declarative schema for a node kind.
///
/// Built-in kinds cover turing-complete control flow:
/// `start` / `end` (sequence), `condition` (branch), `loop` (iteration).
/// `fields` 描述节点业务数据结构，驱动属性面板自动生成。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NodeSchema {
    pub kind: NodeKind,
    pub label: String,
    pub ports: Vec<PortSpec>,
    pub default_size: SizeF,
    /// 字段定义：驱动属性面板渲染。
    #[serde(default)]
    pub fields: Vec<FieldSpec>,
}

impl NodeSchema {
    pub fn new(kind: impl Into<NodeKind>, label: impl Into<String>) -> Self {
        Self {
            kind: kind.into(),
            label: label.into(),
            ports: Vec::new(),
            default_size: SizeF::new(180.0, 35.0),
            fields: Vec::new(),
        }
    }

    pub fn with_port(mut self, port: PortSpec) -> Self {
        self.ports.push(port);
        self
    }

    pub fn with_size(mut self, size: SizeF) -> Self {
        self.default_size = size;
        self
    }

    pub fn with_field(mut self, field: FieldSpec) -> Self {
        self.fields.push(field);
        self
    }

    /// Ports of the given direction.
    pub fn ports_by_direction(&self, dir: PortDirection) -> impl Iterator<Item = &PortSpec> {
        self.ports.iter().filter(move |p| p.direction == dir)
    }

    /// 构建默认 node.data：遍历 fields，填入每个字段的 default 值。
    pub fn default_data(&self) -> serde_json::Value {
        let mut obj = serde_json::Map::new();
        for field in &self.fields {
            obj.insert(field.key.clone(), field.default.clone());
        }
        serde_json::Value::Object(obj)
    }
}

// ====== FlowDocument 数据协议 ======

/// 流程文档：完整的流程图序列化协议（JSON）。
///
/// 节点用索引引用（而非 slotmap key），保证序列化稳定性。
/// 通过 `FlowGraph::from_document` / `to_document` 与 `FlowGraph` 互转。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FlowDocument {
    /// 协议版本。
    pub version: String,
    /// 流程元数据。
    pub metadata: FlowMetadata,
    /// 节点定义列表。
    pub nodes: Vec<NodeDef>,
    /// 边定义列表（用节点索引引用）。
    pub edges: Vec<EdgeDef>,
}

impl FlowDocument {
    pub fn new(name: impl Into<String>) -> Self {
        Self {
            version: "1.0".to_string(),
            metadata: FlowMetadata {
                name: name.into(),
                description: None,
            },
            nodes: Vec::new(),
            edges: Vec::new(),
        }
    }

    pub fn with_description(mut self, desc: impl Into<String>) -> Self {
        self.metadata.description = Some(desc.into());
        self
    }

    /// 添加节点定义，返回其索引（用于边引用）。
    pub fn add_node(&mut self, node: NodeDef) -> usize {
        let idx = self.nodes.len();
        self.nodes.push(node);
        idx
    }

    /// 添加边定义。
    pub fn add_edge(&mut self, edge: EdgeDef) {
        self.edges.push(edge);
    }
}

/// 流程元数据。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FlowMetadata {
    /// 流程名称。
    pub name: String,
    /// 流程描述。
    #[serde(default)]
    pub description: Option<String>,
}

/// 节点定义（序列化友好，不含 slotmap key）。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NodeDef {
    /// 节点类型（匹配 IFlowNode::kind）。
    pub kind: String,
    /// 业务数据（label/desc/conditions 等）。
    pub data: serde_json::Value,
    /// 节点尺寸（None 时用 schema.default_size）。
    #[serde(default)]
    pub size: Option<SizeF>,
    /// 节点位置（None 时由布局引擎计算）。
    #[serde(default)]
    pub position: Option<PointF>,
}

impl NodeDef {
    pub fn new(kind: impl Into<String>, data: serde_json::Value) -> Self {
        Self {
            kind: kind.into(),
            data,
            size: None,
            position: None,
        }
    }

    pub fn with_size(mut self, size: SizeF) -> Self {
        self.size = Some(size);
        self
    }

    pub fn with_position(mut self, position: PointF) -> Self {
        self.position = Some(position);
        self
    }
}

/// 边定义（用节点索引引用，序列化友好）。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EdgeDef {
    /// 源节点索引（NodeDef 在 nodes 数组中的下标）。
    pub source: usize,
    /// 目标节点索引。
    pub target: usize,
    /// 源端口 ID。
    #[serde(default)]
    pub source_port: Option<String>,
    /// 目标端口 ID。
    #[serde(default)]
    pub target_port: Option<String>,
    /// 边类型（None 时用默认）。
    #[serde(default)]
    pub edge_type: Option<EdgeType>,
}

impl EdgeDef {
    pub fn new(source: usize, target: usize) -> Self {
        Self {
            source,
            target,
            source_port: None,
            target_port: None,
            edge_type: None,
        }
    }

    pub fn with_source_port(mut self, port: impl Into<String>) -> Self {
        self.source_port = Some(port.into());
        self
    }

    pub fn with_target_port(mut self, port: impl Into<String>) -> Self {
        self.target_port = Some(port.into());
        self
    }

    pub fn with_edge_type(mut self, edge_type: EdgeType) -> Self {
        self.edge_type = Some(edge_type);
        self
    }
}
