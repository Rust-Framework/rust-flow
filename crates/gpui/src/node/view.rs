//! NodeView：节点渲染组件（无状态，RenderOnce）。
//!
//! 持有节点数据和可选的 IFlowNode 实现，调用 `get_view` 获取卡片内容。
//! 位置定位和事件绑定由 FlowEditorView 的外层 div 负责。

use std::sync::Arc;

use gpui::{AnyElement, App, IntoElement, ParentElement, RenderOnce, Styled, Window, px};
use gpui_component::StyledExt;
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
        // 优先从 data.label 取标签，否则用 kind。
        let label = self
            .node
            .data
            .get("label")
            .and_then(|v| v.as_str())
            .unwrap_or(&self.node.kind)
            .to_string();
        let border_color = if self.selected {
            gpui::rgb(0x6366f1)
        } else {
            gpui::rgb(0xe2e8f0)
        };
        let w = self.node.size.w;
        let h = self.node.size.h;

        // 现代端点样式参数
        let port_size = 6.0;
        let port_in_color = gpui::rgb(0x6366f1);
        let port_out_color = gpui::rgb(0x22c55e);
        let _port_border = gpui::white();
        let port_outer = port_size + 4.0; // 外环尺寸 10px
        let port_outer_half = port_outer * 0.5; // 5px

        gpui::div()
            .w(px(w))
            .h(px(h))
            .bg(gpui::white())
            .border_1()
            .border_color(border_color)
            .rounded_lg()
            .shadow_lg()
            .flex()
            .items_center()
            .justify_center()
            .relative()
            .overflow_hidden()
            .child(
                gpui::div()
                    .text_sm()
                    .font_bold()
                    .text_color(gpui::rgb(0x1e293b))
                    .child(label),
            )
            // 左侧入端口 — 中心对齐节点左边缘中点 (0, h/2)
            .child(
                gpui::div()
                    .absolute()
                    .left(px(-port_outer_half))
                    .top(px(h * 0.5 - port_outer_half))
                    .w(px(port_outer))
                    .h(px(port_outer))
                    .rounded_full()
                    .bg(gpui::white())
                    .border_1()
                    .border_color(gpui::rgb(0xc7d2fe))
                    .flex()
                    .items_center()
                    .justify_center()
                    .child(
                        gpui::div()
                            .w(px(port_size))
                            .h(px(port_size))
                            .rounded_full()
                            .bg(port_in_color),
                    ),
            )
            // 右侧出端口 — 中心对齐节点右边缘中点 (w, h/2)
            .child(
                gpui::div()
                    .absolute()
                    .left(px(w - port_outer_half))
                    .top(px(h * 0.5 - port_outer_half))
                    .w(px(port_outer))
                    .h(px(port_outer))
                    .rounded_full()
                    .bg(gpui::white())
                    .border_1()
                    .border_color(gpui::rgb(0xbbf7d0))
                    .flex()
                    .items_center()
                    .justify_center()
                    .child(
                        gpui::div()
                            .w(px(port_size))
                            .h(px(port_size))
                            .rounded_full()
                            .bg(port_out_color),
                    ),
            )
            .into_any_element()
    }
}
