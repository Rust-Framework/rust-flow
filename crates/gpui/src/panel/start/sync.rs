//! 数据同步逻辑：node.data ↔ 面板状态。
//!
//! 包含：
//! - [`StartPanelView::sync_from_node`]：节点数据变化时同步到面板
//! - [`StartPanelView::sync_list`]：同步单个列表（params 或 variables）
//! - [`subscribe_item_inputs`]：为项的所有 InputState 创建订阅
//! - [`StartPanelView::sync_list_to_node`]：面板状态变化时同步到 node.data

use gpui::{Context, Subscription, Window};
use gpui_component::input::InputEvent;
use gpui_component::tree::TreeEvent;

use super::common::parse_tree_item_id;
use super::item::ItemState;
use super::StartPanelView;

impl StartPanelView {
    /// 节点数据变化时同步到面板状态。
    pub fn sync_from_node(&mut self, node: rust_agent_flow::Node, window: &mut Window, cx: &mut Context<Self>) {
        if self.node.id != node.id {
            return;
        }
        if self.node.data == node.data {
            return;
        }
        self.syncing = true;
        self.node = node;

        // 同步 label
        let label = super::common::label_of(&self.node);
        let current_label = self.label_input.read(cx).value().to_string();
        if current_label != label {
            self.label_input.update(cx, |s, cx| {
                s.set_value(label.as_str(), window, cx);
            });
        }

        // 记录同步前的项数量，用于判断是否需要重建 Tree。
        // 仅在数量变化时重建（set_items 会清空选中/展开状态），
        // 数量不变时 sync_list 已逐项同步值，Tree 文本由内联 Input 实时显示。
        let old_params_count = self.params_state.len();
        let old_vars_count = self.variables_state.len();

        // 同步 params
        self.sync_list("params", false, window, cx);
        // 同步 variables
        self.sync_list("variables", true, window, cx);

        self.syncing = false;
        // 仅在项数量变化时重建 Tree，避免编辑过程中选中/展开状态丢失。
        if self.params_state.len() != old_params_count {
            self.rebuild_params_tree(cx);
        }
        if self.variables_state.len() != old_vars_count {
            self.rebuild_variables_tree(cx);
        }
        cx.notify();
    }

    /// 同步单个列表（params 或 variables）。
    pub(super) fn sync_list(
        &mut self,
        field_key: &str,
        is_variable: bool,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let arr = self.node.data.get(field_key).cloned().unwrap_or_default();
        let items: Vec<serde_json::Value> = arr.as_array().cloned().unwrap_or_default();
        let states = if field_key == "params" {
            &mut self.params_state
        } else {
            &mut self.variables_state
        };

        if items.len() == states.len() {
            // 数量一致：逐项同步值
            for (i, item_val) in items.iter().enumerate() {
                states[i].sync_from_value(item_val, is_variable, &self.registry, window, cx);
            }
        } else {
            // 数量变化：重建（需要重建订阅）
            states.clear();
            // 保留 label 订阅 + 2 个 Tree 观察订阅 + 2 个 TreeEvent 订阅（索引 0-4），清除项相关订阅
            self._subscriptions.truncate(5);
            for item_val in &items {
                let st = ItemState::from_value(item_val, is_variable, &self.registry, window, cx);
                subscribe_item_inputs(states, st, field_key, &mut self._subscriptions, window, cx);
            }
            // 重建另一个列表的订阅（因为 truncate 清除了所有项订阅）
            self.rebuild_other_subscriptions(field_key, window, cx);
        }
    }

    /// 重建非当前列表的订阅（sync_list 重建时调用）。
    pub(super) fn rebuild_other_subscriptions(
        &mut self,
        current_field: &str,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if current_field != "params" {
            let params = std::mem::take(&mut self.params_state);
            for st in params {
                subscribe_item_inputs(
                    &mut self.params_state,
                    st,
                    "params",
                    &mut self._subscriptions,
                    window,
                    cx,
                );
            }
        }
        if current_field != "variables" {
            let vars = std::mem::take(&mut self.variables_state);
            for st in vars {
                subscribe_item_inputs(
                    &mut self.variables_state,
                    st,
                    "variables",
                    &mut self._subscriptions,
                    window,
                    cx,
                );
            }
        }
    }

    /// 同步列表状态到 node.data（Input 变更时调用）。
    pub(super) fn sync_list_to_node(&self, field_key: &str, cx: &mut Context<Self>) {
        let states = if field_key == "variables" {
            &self.variables_state
        } else {
            &self.params_state
        };
        let arr: Vec<serde_json::Value> = states.iter().map(|s| s.to_value(cx)).collect();
        self.dispatch_set_data(field_key, serde_json::json!(arr), cx);
    }

    /// TreeEvent 回调：同步展开状态到 ItemState。
    ///
    /// 用户点击 folder 项的展开箭头时，TreeState 内部更新展开状态并发出 TreeEvent。
    /// 此处将展开状态同步回 `ItemState.expanded`，确保重建 Tree 时不丢失展开状态。
    pub(super) fn on_tree_event(&mut self, field_key: &str, event: &TreeEvent, _cx: &mut Context<Self>) {
        if self.syncing {
            return;
        }
        match event {
            TreeEvent::Expanded(id) | TreeEvent::Collapsed(id) => {
                if let Some((item_idx, _)) = parse_tree_item_id(id.as_ref()) {
                    let expanded = matches!(event, TreeEvent::Expanded(_));
                    let states = if field_key == "params" {
                        &mut self.params_state
                    } else {
                        &mut self.variables_state
                    };
                    if let Some(st) = states.get_mut(item_idx) {
                        st.expanded = expanded;
                    }
                }
            }
        }
    }
}

/// 为项的所有 InputState 创建订阅。
///
/// 订阅范围：
/// - 项名称
/// - 项描述（description）
/// - 基础类型值
/// - 子字段名称（动态类型可编辑）
/// - 子字段值（复杂/动态类型）
pub(super) fn subscribe_item_inputs(
    states: &mut Vec<ItemState>,
    st: ItemState,
    field_key: &str,
    subscriptions: &mut Vec<Subscription>,
    window: &mut Window,
    cx: &mut Context<StartPanelView>,
) {
    let fk = field_key.to_string();

    // 订阅 name
    {
        let fk = fk.clone();
        let sub = cx.subscribe_in(&st.name, window, move |this, _e, ev, _w, cx| {
            if !this.syncing && matches!(ev, InputEvent::Change) {
                this.sync_list_to_node(&fk, cx);
            }
        });
        subscriptions.push(sub);
    }

    // 订阅 description
    {
        let fk = fk.clone();
        let sub = cx.subscribe_in(&st.description, window, move |this, _e, ev, _w, cx| {
            if !this.syncing && matches!(ev, InputEvent::Change) {
                this.sync_list_to_node(&fk, cx);
            }
        });
        subscriptions.push(sub);
    }

    // 订阅 value（基础类型）
    if let Some(ref val) = st.value {
        let fk = fk.clone();
        let sub = cx.subscribe_in(val, window, move |this, _e, ev, _w, cx| {
            if !this.syncing && matches!(ev, InputEvent::Change) {
                this.sync_list_to_node(&fk, cx);
            }
        });
        subscriptions.push(sub);
    }

    // 订阅子字段 name 和 value（复杂/动态类型）
    for field in &st.fields {
        // 子字段 name
        let fk_name = fk.clone();
        let sub = cx.subscribe_in(&field.name, window, move |this, _e, ev, _w, cx| {
            if !this.syncing && matches!(ev, InputEvent::Change) {
                this.sync_list_to_node(&fk_name, cx);
            }
        });
        subscriptions.push(sub);

        // 子字段 value
        let fk_val = fk.clone();
        let sub = cx.subscribe_in(&field.value, window, move |this, _e, ev, _w, cx| {
            if !this.syncing && matches!(ev, InputEvent::Change) {
                this.sync_list_to_node(&fk_val, cx);
            }
        });
        subscriptions.push(sub);
    }

    states.push(st);
}
