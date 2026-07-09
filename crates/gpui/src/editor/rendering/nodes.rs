//! 节点渲染层：absolute div 在内容层内定位。
//!
//! 易变模块：节点视觉外观、选中/悬停态、动作回调闭包均在此调整。

use std::sync::Arc;

use gpui::{div, px, App, AppContext, Entity, IntoElement, ParentElement, Styled};

use crate::node::{ActionCallback, IFlowNode, NodeAction, NodeView};

use super::super::flow_editor::{FlowEditorView, LayoutDirection};

impl FlowEditorView {
    /// 渲染所有节点（absolute div 在内容层内）。
    ///
    /// 节点最终屏幕坐标 = content_offset + logical_pos × scale
    ///
    /// 为每个节点创建动作回调闭包，捕获 `node_id` 和 `entity`，
    /// 通过 `cx.update_entity` 调用 `handle_node_action`。
    ///
    /// 循环体节点和隐藏节点信息从 `cached_all_body_nodes`/`cached_hidden_nodes`
    /// 读取（在 `relayout` 末尾更新），无需调用方传入。
    pub(crate) fn render_nodes(
        &self,
        entity: Entity<Self>,
    ) -> Vec<gpui::AnyElement> {
        let selected = self.selected;
        let registry = &self.registry;
        let s = self.scale();
        let layout = match self.layout_direction {
            LayoutDirection::Horizontal => rust_agent_flow::LayoutDirection::Horizontal,
            LayoutDirection::Vertical => rust_agent_flow::LayoutDirection::Vertical,
        };
        let theme = self.theme;
        let hovered = self.hovered;

        // 使用缓存的派生集合，避免每帧重复 flat_map 收集。
        let all_body_nodes = &self.cached_all_body_nodes;
        let hidden_nodes = &self.cached_hidden_nodes;

        self.graph
            .nodes()
            .map(|node| {
                let node_id = node.id;
                let pos = node.position;
                let flow_node = registry.get(&node.kind);
                let is_selected = selected == Some(node_id);
                let is_body = all_body_nodes.contains(&node_id);
                let is_hovered = hovered == Some(node_id);

                // 被收起的循环体节点：不渲染（返回空 div 占位，保持布局位置）
                if hidden_nodes.contains(&node_id) {
                    return div()
                        .absolute()
                        .left(px(pos.x * s))
                        .top(px(pos.y * s))
                        .into_any_element();
                }

                // body 节点处于纵向子流中（align_loop_body_target 纵向堆叠），
                // 使用 Vertical 布局上下文让节点 port_position 回调返回 Top/Bottom 端口。
                let effective_layout = if is_body {
                    rust_agent_flow::LayoutDirection::Vertical
                } else {
                    layout
                };

                // 创建动作回调：闭包捕获 node_id 和 entity
                let on_action: ActionCallback = {
                    let entity = entity.clone();
                    Arc::new(move |action: NodeAction, cx: &mut App| {
                        cx.update_entity(&entity, |view: &mut FlowEditorView, cx| {
                            view.handle_node_action(node_id, action, cx);
                        });
                    })
                };

                let view = NodeView::new(node.clone())
                    .with_flow_node_opt(flow_node)
                    .selected(is_selected)
                    .with_scale(s)
                    .with_layout(effective_layout)
                    .with_body_mode(is_body)
                    .with_theme(theme)
                    .with_hovered(is_hovered)
                    .with_on_action(Some(on_action))
                    .with_language(self.language);

                div()
                    .absolute()
                    .left(px(pos.x * s))
                    .top(px(pos.y * s))
                    .child(view)
                    .into_any_element()
            })
            .collect()
    }
}

// ---- 视图扩展（仅在渲染层使用） ----

impl NodeView {
    pub(crate) fn with_flow_node_opt(mut self, flow_node: Option<Arc<dyn IFlowNode>>) -> Self {
        self.flow_node = flow_node;
        self
    }
}
