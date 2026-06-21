//! NodeView：节点渲染组件（无状态，RenderOnce）。
//!
//! 持有节点数据和可选的 IFlowNode 实现，调用 `get_view` 获取卡片内容。
//! 位置定位和事件绑定由 FlowEditorView 的外层 div 负责。

use std::sync::Arc;

use gpui::{AnyElement, App, IntoElement, ParentElement, RenderOnce, Styled, Window};
use rust_agent_flow::Node;

use super::{IFlowNode, NodeViewCtx};

/// 节点视图组件。
///
/// `flow_node` 为 `None` 时使用 fallback 渲染（显示 kind 文字），
/// 用于未注册的节点 kind（Phase 2 demo 阶段）。
pub struct NodeView {
    pub node: Node,
    pub flow_node: Option<Arc<dyn IFlowNode>>,
    pub selected: bool,
}

impl NodeView {
    pub fn new(node: Node) -> Self {
        Self {
            node,
            flow_node: None,
            selected: false,
        }
    }

    pub fn with_flow_node(mut self, flow_node: Arc<dyn IFlowNode>) -> Self {
        self.flow_node = Some(flow_node);
        self
    }

    pub fn selected(mut self, selected: bool) -> Self {
        self.selected = selected;
        self
    }
}

impl RenderOnce for NodeView {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        if let Some(flow_node) = self.flow_node {
            let mut ctx = NodeViewCtx {
                window,
                cx,
                selected: self.selected,
            };
            flow_node.get_view(&self.node, &mut ctx)
        } else {
            // Fallback：未注册的 kind，显示 kind 文字。
            self.render_fallback()
        }
    }
}

impl IntoElement for NodeView {
    type Element = gpui::Component<Self>;
    fn into_element(self) -> Self::Element {
        gpui::Component::new(self)
    }
}

impl NodeView {
    fn render_fallback(self) -> AnyElement {
        let label = self.node.kind.clone();
        let border_color = if self.selected {
            gpui::rgb(0x3b82f6)
        } else {
            gpui::rgb(0x9ca3af)
        };

        gpui::div()
            .bg(gpui::white())
            .border_1()
            .border_color(border_color)
            .rounded_md()
            .shadow_sm()
            .px_3()
            .py_2()
            .child(label)
            .into_any_element()
    }
}
