//! 属性面板生命周期：同步面板视图与选中节点。
//!
//! 易变模块：面板功能演进、PanelView 构造参数变化均在此调整。

use std::sync::Arc;

use gpui::{App, AppContext, Entity, Window};

use crate::node::{ActionCallback, NodeAction};
use crate::panel::{PanelEntity, PanelView};
use crate::panel::start::StartPanelView;

use super::super::flow_editor::FlowEditorView;

impl FlowEditorView {
    /// 确保属性面板视图与选中节点同步。
    ///
    /// 选中节点变化时创建新面板，节点数据变化时同步更新。
    /// 返回 `Option<PanelEntity>` 供 render 方法作为 child 添加。
    pub(crate) fn ensure_panel_view(
        &mut self,
        entity: Entity<Self>,
        window: &mut Window,
        cx: &mut gpui::Context<Self>,
    ) -> Option<PanelEntity> {
        // 选中节点为 None 时，清理 panel_view
        let selected_id = match self.selected {
            Some(id) => id,
            None => {
                self.panel_view = None;
                return None;
            }
        };

        let node = self.graph.node(selected_id).cloned()?;
        // 如果节点已被删除（graph 中不存在），清理 panel_view
        if self.graph.node(selected_id).is_none() {
            self.panel_view = None;
            return None;
        }

        // 检查是否需要重建面板（选中节点变化或 panel_view 为空）
        let need_rebuild = self
            .panel_view
            .as_ref()
            .map(|pv| match pv {
                PanelEntity::Generic(e) => e.read(cx).node.id != selected_id,
                PanelEntity::Start(e) => e.read(cx).node.id != selected_id,
            })
            .unwrap_or(true);

        if need_rebuild {
            let node_id = node.id;
            let flow_node = self.registry.get(&node.kind);

            // 创建动作回调：闭包捕获 node_id 和 entity
            let on_action: ActionCallback = {
                let entity = entity.clone();
                Arc::new(move |action: NodeAction, cx: &mut App| {
                    cx.update_entity(&entity, |view: &mut FlowEditorView, cx| {
                        view.handle_node_action(node_id, action, cx);
                    });
                })
            };

            // 根据节点类型分发到对应面板
            self.panel_view = if node.kind == "start" {
                Some(PanelEntity::Start(StartPanelView::new(
                    node,
                    flow_node,
                    self.theme,
                    Some(on_action),
                    self.syntax_service.clone(),
                    self.language,
                    self.data_type_provider.clone(),
                    window,
                    cx,
                )))
            } else {
                Some(PanelEntity::Generic(PanelView::new(
                    node,
                    flow_node,
                    self.theme,
                    Some(on_action),
                    self.syntax_service.clone(),
                    self.language,
                    window,
                    cx,
                )))
            };
        } else {
            // 同步节点数据到现有面板
            match &self.panel_view {
                Some(PanelEntity::Generic(pv)) => {
                    pv.update(cx, |view, cx| {
                        view.sync_from_node(node, window, cx);
                    });
                }
                Some(PanelEntity::Start(pv)) => {
                    pv.update(cx, |view, cx| {
                        view.sync_from_node(node, window, cx);
                    });
                }
                None => {}
            }
        }

        self.panel_view.clone()
    }
}
