//! NodeRegistry：按 kind 注册和查找 IFlowNode 实现。

use std::collections::HashMap;
use std::sync::Arc;

use rust_agent_flow::{NodeId, PortSpec};

use super::IFlowNode;

/// 节点注册表：`NodeKind` → `Arc<dyn IFlowNode>`。
///
/// 在 [`FlowEditorView`] 构造时注册所有内置节点，渲染时按节点 kind 查找。
///
/// [`FlowEditorView`]: crate::FlowEditorView
#[derive(Default)]
pub struct NodeRegistry {
    nodes: HashMap<String, Arc<dyn IFlowNode>>,
}

impl NodeRegistry {
    pub fn new() -> Self {
        Self::default()
    }

    /// 注册一个节点实现。
    pub fn register(&mut self, node: Arc<dyn IFlowNode>) {
        let kind = node.kind().to_string();
        self.nodes.insert(kind, node);
    }

    /// 按 kind 查找节点实现。
    pub fn get(&self, kind: &str) -> Option<Arc<dyn IFlowNode>> {
        self.nodes.get(kind).cloned()
    }

    /// 按 NodeId 查找节点实现（需传入 FlowGraph 引用以获取 kind）。
    ///
    /// 此方法为便捷方法，实际调用方通常先从 graph 获取 node.kind 再调用 get。
    pub fn port_specs_for(&self, kind: &str) -> Vec<PortSpec> {
        self.nodes
            .get(kind)
            .map(|n| n.schema().ports.clone())
            .unwrap_or_default()
    }

    /// 提供给 [`resolve_endpoints`] 的回调：返回指定节点的端口规格列表。
    ///
    /// 调用方需先用 graph 查询节点 kind，再调用此方法。
    ///
    /// [`resolve_endpoints`]: rust_agent_flow::resolve_endpoints
    pub fn specs_fn(&self) -> impl Fn(NodeId) -> Vec<PortSpec> + '_ {
        // 注意：此处无法直接返回闭包查询 graph，因为 registry 不持有 graph。
        // 实际使用时，FlowEditorView 会构造一个捕获 graph 和 registry 的闭包。
        // 此方法保留为 port_specs_for 的别名用途说明。
        |_| Vec::new()
    }
}
