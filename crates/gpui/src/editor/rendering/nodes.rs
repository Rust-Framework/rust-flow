//! 节点渲染层：absolute div 在内容层内定位。
//!
//! 易变模块：节点视觉外观、选中/悬停态、动作回调闭包均在此调整。

use std::collections::{HashMap, HashSet};
use std::sync::Arc;

use gpui::{div, px, App, AppContext, Entity, IntoElement, ParentElement, Styled};
use rust_agent_flow::NodeId;

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
    /// `body_groups` 由调用方（`render`）计算一次并传入，避免与 `render_edges`
    /// 重复执行 BFS 遍历（O(V+E)）。
    pub(crate) fn render_nodes(
        &self,
        entity: Entity<Self>,
        body_groups: &HashMap<NodeId, HashSet<NodeId>>,
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

        let all_body_nodes: HashSet<NodeId> =
            body_groups.values().flat_map(|s| s.iter().copied()).collect();

        // 收集被收起的循环体节点：当 Loop 节点的 body_collapsed == true 时，
        // 其循环体节点不渲染（隐藏），但保留拓扑边。
        let mut hidden_nodes: HashSet<NodeId> = HashSet::new();
        for (loop_node, body_nodes) in body_groups {
            if let Some(ln) = self.graph.node(*loop_node) {
                let body_collapsed = ln
                    .data
                    .get("body_collapsed")
                    .and_then(|v| v.as_bool())
                    .unwrap_or(false);
                if body_collapsed {
                    hidden_nodes.extend(body_nodes.iter().copied());
                }
            }
        }

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
                    .with_layout(layout)
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
