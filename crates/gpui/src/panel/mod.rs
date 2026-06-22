//! Panel 模块：属性面板容器。
//!
//! Phase 2 提供空壳，Phase 3 接入 IFlowNode::get_panel。

use std::sync::Arc;

use gpui::{AnyElement, App, IntoElement, ParentElement, RenderOnce, Styled, Window};
use rust_agent_flow::Node;

use crate::node::{IFlowNode, NodeViewCtx};
use crate::theme::Theme;

/// 属性面板视图：选中节点时右侧显示。
pub struct PanelView {
    pub node: Node,
    pub flow_node: Option<Arc<dyn IFlowNode>>,
    pub theme: Theme,
}

impl PanelView {
    pub fn new(node: Node) -> Self {
        Self {
            node,
            flow_node: None,
            theme: Theme::default(),
        }
    }

    pub fn with_flow_node(mut self, flow_node: Arc<dyn IFlowNode>) -> Self {
        self.flow_node = Some(flow_node);
        self
    }

    pub fn with_theme(mut self, theme: Theme) -> Self {
        self.theme = theme;
        self
    }
}

impl RenderOnce for PanelView {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let theme = self.theme;
        let content: AnyElement = if let Some(flow_node) = self.flow_node {
            let mut ctx = NodeViewCtx {
                window,
                cx,
                selected: true,
                scale: 1.0,
                layout: rust_agent_flow::LayoutDirection::Horizontal,
                theme,
            };
            flow_node.get_panel(&self.node, &mut ctx)
        } else {
            // Fallback：显示节点 data
            gpui::div()
                .p_4()
                .child(format!("节点: {}\n数据: {}", self.node.kind, self.node.data))
                .into_any_element()
        };

        gpui::div()
            .w_80()
            .h_full()
            .bg(theme.panel_bg)
            .border_l_1()
            .border_color(theme.panel_border)
            .child(content)
    }
}

impl IntoElement for PanelView {
    type Element = gpui::Component<Self>;
    fn into_element(self) -> Self::Element {
        gpui::Component::new(self)
    }
}
