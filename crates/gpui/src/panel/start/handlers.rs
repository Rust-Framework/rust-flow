//! 事件处理器：用户交互回调。
//!
//! 包含：
//! - label 变更
//! - Tree 选中变化
//! - 增删项/字段
//! - 类型切换
//! - optional/array 标志切换
//! - 值设置
//! - Tree 重建

use gpui::{Context, Entity};

use gpui_component::input::InputEvent;
use gpui_component::tree::TreeState;

use crate::node::NodeAction;

use super::common::parse_tree_item_id;
use super::common::Selection;
use super::StartPanelView;

impl StartPanelView {
    /// label 输入变更回调。
    pub(super) fn on_label_change(
        &mut self,
        _state: &gpui::Entity<gpui_component::input::InputState>,
        event: &InputEvent,
        _window: &mut gpui::Window,
        cx: &mut Context<Self>,
    ) {
        if self.syncing || !matches!(event, InputEvent::Change) {
            return;
        }
        let value = self.label_input.read(cx).value().to_string();
        self.dispatch_set_data("label", serde_json::json!(value), cx);
    }

    /// Tree 选中变化 → 更新 selected 状态。
    ///
    /// TreeState 的 selected_index 是扁平索引（包含展开的子项），
    /// 需要通过 entry 的 item.id 反查得到 item_idx 和 field_idx。
    pub(super) fn on_tree_selection(&mut self, field_key: &str, cx: &mut Context<Self>) {
        if self.syncing {
            return;
        }
        let tree = if field_key == "params" {
            &self.params_tree
        } else {
            &self.variables_tree
        };
        let parsed = tree.read(cx).selected_entry().and_then(|entry| {
            let id = entry.item().id.as_ref();
            parse_tree_item_id(id)
        });

        match parsed {
            Some((item_idx, field_idx)) => {
                self.selected = Some(Selection {
                    field_key: field_key.to_string(),
                    item_idx,
                    field_idx,
                });
            }
            None => {
                self.selected = None;
            }
        }
        cx.notify();
    }

    /// 添加新项（通过 dispatch SetData 触发 sync 重建）。
    pub fn add_item(&mut self, field_key: &str, cx: &mut Context<Self>) {
        if self.syncing {
            return;
        }
        let states = if field_key == "variables" {
            &self.variables_state
        } else {
            &self.params_state
        };
        let mut arr: Vec<serde_json::Value> =
            states.iter().map(|s| s.to_value(cx)).collect();
        arr.push(super::data_types::build_default_item());
        self.dispatch_set_data(field_key, serde_json::json!(arr), cx);
    }

    /// 删除项（通过 dispatch SetData 触发 sync 重建）。
    pub fn delete_item(&mut self, field_key: &str, item_idx: usize, cx: &mut Context<Self>) {
        if self.syncing {
            return;
        }
        let states = if field_key == "variables" {
            &self.variables_state
        } else {
            &self.params_state
        };
        let arr: Vec<serde_json::Value> = states
            .iter()
            .enumerate()
            .filter(|(i, _)| *i != item_idx)
            .map(|(_, s)| s.to_value(cx))
            .collect();
        self.dispatch_set_data(field_key, serde_json::json!(arr), cx);
    }

    /// 切换项的数据类型（通过 dispatch SetData 触发 sync 重建）。
    ///
    /// 保留原 is_optional/is_array 状态（类型切换不丢失标志位）。
    pub fn change_item_type(
        &mut self,
        field_key: &str,
        item_idx: usize,
        new_type: String,
        cx: &mut Context<Self>,
    ) {
        if self.syncing {
            return;
        }
        let states = if field_key == "variables" {
            &self.variables_state
        } else {
            &self.params_state
        };
        let arr: Vec<serde_json::Value> = states
            .iter()
            .enumerate()
            .map(|(i, s)| {
                if i == item_idx {
                    let name = s.name.read(cx).value().to_string();
                    super::data_types::build_item_for_type(
                        &name,
                        &new_type,
                        &self.registry,
                        s.is_optional,
                        s.is_array,
                    )
                } else {
                    s.to_value(cx)
                }
            })
            .collect();
        self.dispatch_set_data(field_key, serde_json::json!(arr), cx);
    }

    /// 切换项的 is_optional 标志（通过 dispatch SetData 触发 sync 重建）。
    pub fn toggle_item_optional(
        &mut self,
        field_key: &str,
        item_idx: usize,
        checked: bool,
        cx: &mut Context<Self>,
    ) {
        if self.syncing {
            return;
        }
        let states = if field_key == "variables" {
            &self.variables_state
        } else {
            &self.params_state
        };
        let arr: Vec<serde_json::Value> = states
            .iter()
            .enumerate()
            .map(|(i, s)| {
                if i == item_idx {
                    let mut val = s.to_value(cx);
                    if let Some(obj) = val.as_object_mut() {
                        obj.insert("is_optional".to_string(), serde_json::json!(checked));
                    }
                    val
                } else {
                    s.to_value(cx)
                }
            })
            .collect();
        self.dispatch_set_data(field_key, serde_json::json!(arr), cx);
    }

    /// 切换项的 is_array 标志（通过 dispatch SetData 触发 sync 重建）。
    pub fn toggle_item_array(
        &mut self,
        field_key: &str,
        item_idx: usize,
        checked: bool,
        cx: &mut Context<Self>,
    ) {
        if self.syncing {
            return;
        }
        let states = if field_key == "variables" {
            &self.variables_state
        } else {
            &self.params_state
        };
        let arr: Vec<serde_json::Value> = states
            .iter()
            .enumerate()
            .map(|(i, s)| {
                if i == item_idx {
                    let mut val = s.to_value(cx);
                    if let Some(obj) = val.as_object_mut() {
                        obj.insert("is_array".to_string(), serde_json::json!(checked));
                    }
                    val
                } else {
                    s.to_value(cx)
                }
            })
            .collect();
        self.dispatch_set_data(field_key, serde_json::json!(arr), cx);
    }

    /// 设置基础类型项的默认值（Boolean Switch 切换时调用）。
    pub fn set_item_value(
        &mut self,
        field_key: &str,
        item_idx: usize,
        new_value: String,
        cx: &mut Context<Self>,
    ) {
        if self.syncing {
            return;
        }
        let states = if field_key == "variables" {
            &self.variables_state
        } else {
            &self.params_state
        };
        let arr: Vec<serde_json::Value> = states
            .iter()
            .enumerate()
            .map(|(i, s)| {
                if i == item_idx {
                    let mut val = s.to_value(cx);
                    if let Some(obj) = val.as_object_mut() {
                        obj.insert("value".to_string(), serde_json::json!(new_value));
                    }
                    val
                } else {
                    s.to_value(cx)
                }
            })
            .collect();
        self.dispatch_set_data(field_key, serde_json::json!(arr), cx);
    }

    /// 向动态类型项添加字段（通过 dispatch SetData 触发 sync 重建）。
    pub fn add_field(&mut self, field_key: &str, item_idx: usize, cx: &mut Context<Self>) {
        if self.syncing {
            return;
        }
        let states = if field_key == "variables" {
            &self.variables_state
        } else {
            &self.params_state
        };
        let arr: Vec<serde_json::Value> = states
            .iter()
            .enumerate()
            .map(|(i, s)| {
                if i == item_idx {
                    let mut val = s.to_value(cx);
                    if let Some(fields) = val.get_mut("fields").and_then(|f| f.as_array_mut()) {
                        fields.push(serde_json::json!({
                            "name": "",
                            "type": "String",
                            "value": "",
                        }));
                    }
                    val
                } else {
                    s.to_value(cx)
                }
            })
            .collect();
        self.dispatch_set_data(field_key, serde_json::json!(arr), cx);
    }

    /// 删除动态类型项的字段（通过 dispatch SetData 触发 sync 重建）。
    pub fn delete_field(
        &mut self,
        field_key: &str,
        item_idx: usize,
        field_idx: usize,
        cx: &mut Context<Self>,
    ) {
        if self.syncing {
            return;
        }
        let states = if field_key == "variables" {
            &self.variables_state
        } else {
            &self.params_state
        };
        let arr: Vec<serde_json::Value> = states
            .iter()
            .enumerate()
            .map(|(i, s)| {
                if i == item_idx {
                    let mut val = s.to_value(cx);
                    if let Some(fields) = val.get_mut("fields").and_then(|f| f.as_array_mut()) {
                        if field_idx < fields.len() {
                            fields.remove(field_idx);
                        }
                    }
                    val
                } else {
                    s.to_value(cx)
                }
            })
            .collect();
        self.dispatch_set_data(field_key, serde_json::json!(arr), cx);
    }

    /// 切换动态类型字段的类型（通过 dispatch SetData 触发 sync 重建）。
    pub fn change_field_type(
        &mut self,
        field_key: &str,
        item_idx: usize,
        field_idx: usize,
        new_type: String,
        cx: &mut Context<Self>,
    ) {
        if self.syncing {
            return;
        }
        let states = if field_key == "variables" {
            &self.variables_state
        } else {
            &self.params_state
        };
        let arr: Vec<serde_json::Value> = states
            .iter()
            .enumerate()
            .map(|(i, s)| {
                if i == item_idx {
                    let mut val = s.to_value(cx);
                    if let Some(fields) = val.get_mut("fields").and_then(|f| f.as_array_mut()) {
                        if field_idx < fields.len() {
                            if let Some(ftype) = fields[field_idx].get_mut("type") {
                                *ftype = serde_json::json!(new_type);
                            }
                        }
                    }
                    val
                } else {
                    s.to_value(cx)
                }
            })
            .collect();
        self.dispatch_set_data(field_key, serde_json::json!(arr), cx);
    }

    /// 重建参数区 Tree items。
    pub(super) fn rebuild_params_tree(&mut self, cx: &mut Context<Self>) {
        let items = super::tree_render::build_section_tree_items(
            &self.params_state,
            "params",
            &self.registry,
            cx,
        );
        self.params_tree.update(cx, |state, cx| {
            state.set_items(items, cx);
        });
    }

    /// 重建变量区 Tree items。
    pub(super) fn rebuild_variables_tree(&mut self, cx: &mut Context<Self>) {
        let items = super::tree_render::build_section_tree_items(
            &self.variables_state,
            "variables",
            &self.registry,
            cx,
        );
        self.variables_tree.update(cx, |state, cx| {
            state.set_items(items, cx);
        });
    }

    /// 派发 SetData 动作到外部回调。
    pub(super) fn dispatch_set_data(&self, key: &str, value: serde_json::Value, cx: &mut Context<Self>) {
        if let Some(on_action) = &self.on_action {
            on_action(NodeAction::SetData(key.to_string(), value), cx);
        }
    }

    /// 清除指定 Tree 的选中状态。
    pub(super) fn clear_tree_selection(&mut self, field_key: &str, cx: &mut Context<Self>) {
        let tree: &Entity<TreeState> = if field_key == "params" {
            &self.params_tree
        } else {
            &self.variables_tree
        };
        tree.update(cx, |s, cx| {
            s.set_selected_index(None, cx);
        });
    }
}
