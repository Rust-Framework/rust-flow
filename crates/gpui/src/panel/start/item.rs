//! Start 节点参数/变量项的状态管理。
//!
//! 三种类型形态：
//! - **基础类型**（String/Integer/Float/Boolean/DateTime）：name + type + is_optional + is_array + value
//! - **复杂类型**（provider 提供的自定义类型）：预定义结构，结构只读，值按模式可编辑
//! - **动态类型**（Dynamic）：结构可手动编辑（增删改字段），值按模式可编辑
//!
//! 低代码变量模型规则：
//! - `is_optional=true` 时默认值可省略（UI 提示可选）
//! - `is_array=true` 表示数组/集合类型
//! - 默认值输入控件根据类型变化：Boolean → Switch，其他 → Input

use gpui::{App, AppContext, Entity, SharedString, Window};
use gpui_component::input::InputState;
use gpui_component::select::SelectState;
use gpui_component::tree::{TreeItem, TreeState};
use gpui_component::IndexPath;

use crate::data_type::DataTypeRegistry;

use super::data_types::{
    item_fields, item_is_array, item_is_optional, item_name, item_type, item_value, value_to_string,
};

/// Tree 条目高度（px），用于计算 Tree 容器高度。
/// 配合 `.small()` 尺寸的 Input/Select/Switch 控件 + py(2) 上下内边距。
pub(super) const TREE_ENTRY_HEIGHT: f32 = 36.0;

/// 创建类型选择 SelectState。
///
/// items = 注册表中所有类型名（SharedString），selected_index = 当前类型位置。
fn create_type_select(
    type_names: &[&str],
    current_type: &str,
    window: &mut Window,
    cx: &mut App,
) -> Entity<SelectState<Vec<SharedString>>> {
    let items: Vec<SharedString> = type_names
        .iter()
        .map(|s| SharedString::from(s.to_string()))
        .collect();
    let selected_index = items
        .iter()
        .position(|t| t.as_ref() == current_type)
        .map(|i| IndexPath::default().row(i));
    cx.new(|cx| SelectState::new(items, selected_index, window, cx))
}

/// 同步类型选择 SelectState 的选中值（外部数据变化时调用）。
fn sync_type_select(
    select: &Entity<SelectState<Vec<SharedString>>>,
    current_type: &str,
    window: &mut Window,
    cx: &mut App,
) {
    let value = SharedString::from(current_type.to_string());
    select.update(cx, |state, cx| {
        state.set_selected_value(&value, window, cx);
    });
}

/// 单个子字段的状态。
pub struct FieldState {
    /// 字段名输入（动态类型可编辑，复杂类型只读渲染）。
    pub name: Entity<InputState>,
    /// 字段类型名。
    pub type_value: String,
    /// 字段类型选择 SelectState。
    pub type_select: Entity<SelectState<Vec<SharedString>>>,
    /// 字段值输入（变量模式可编辑，参数模式只读渲染）。
    pub value: Entity<InputState>,
}

/// 单个参数/变量项的编辑状态（低代码变量模型）。
pub struct ItemState {
    /// 名称输入。
    pub name: Entity<InputState>,
    /// 描述输入（可选，用于详情面板）。
    pub description: Entity<InputState>,
    /// 当前类型（下拉值）。
    pub type_value: String,
    /// 类型选择 SelectState。
    pub type_select: Entity<SelectState<Vec<SharedString>>>,
    /// 是否可选（可选时默认值可省略）。
    pub is_optional: bool,
    /// 是否数组/集合。
    pub is_array: bool,
    /// 基础类型的值输入（复杂/动态类型时为 None）。
    pub value: Option<Entity<InputState>>,
    /// 复杂/动态类型的子字段状态。
    pub fields: Vec<FieldState>,
    /// 是否展开显示子字段。
    pub expanded: bool,
    /// 结构类型的 Tree 控件状态（复杂/动态类型时为 Some）。
    pub tree_state: Option<Entity<TreeState>>,
}

impl ItemState {
    /// 从 JSON 值构建项状态。
    pub fn from_value(
        item: &serde_json::Value,
        _is_variable: bool,
        registry: &DataTypeRegistry,
        window: &mut Window,
        cx: &mut App,
    ) -> Self {
        let name_text = item_name(item);
        let type_name = item_type(item);
        let is_optional = item_is_optional(item);
        let is_array = item_is_array(item);
        let desc_text = item
            .get("description")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string();
        let has_fields = registry.has_fields(&type_name);
        let type_names: Vec<&str> = registry.type_names();

        let name = cx.new(|cx| {
            InputState::new(window, cx)
                .default_value(name_text.as_str())
                .placeholder("name")
        });
        let description = cx.new(|cx| {
            InputState::new(window, cx)
                .default_value(desc_text.as_str())
                .placeholder("description")
        });
        let type_select = create_type_select(&type_names, &type_name, window, cx);

        let (value, fields) = if has_fields {
            let json_fields = item_fields(item);
            let field_states = build_field_states(&json_fields, &type_names, window, cx);
            (None, field_states)
        } else {
            let v = item_value(item);
            let val = cx.new(|cx| {
                InputState::new(window, cx)
                    .default_value(v.as_str())
                    .placeholder("value")
            });
            (Some(val), Vec::new())
        };

        let tree_state = if has_fields {
            let is_dyn = registry.is_dynamic(&type_name);
            let items = Self::build_tree_items(&fields, is_dyn);
            Some(cx.new(|cx| TreeState::new(cx).items(items)))
        } else {
            None
        };

        Self {
            name,
            description,
            type_value: type_name,
            type_select,
            is_optional,
            is_array,
            value,
            fields,
            expanded: has_fields,
            tree_state,
        }
    }

    /// 将项状态序列化为 JSON 值。
    pub fn to_value(&self, cx: &App) -> serde_json::Value {
        let name = self.name.read(cx).value().to_string();
        let desc = self.description.read(cx).value().to_string();
        let type_name = self.type_value.clone();

        if !self.fields.is_empty() || self.value.is_none() {
            let fields: Vec<serde_json::Value> = self
                .fields
                .iter()
                .map(|f| {
                    serde_json::json!({
                        "name": f.name.read(cx).value().to_string(),
                        "type": f.type_value,
                        "value": f.value.read(cx).value().to_string(),
                    })
                })
                .collect();
            serde_json::json!({
                "name": name,
                "description": desc,
                "type": type_name,
                "is_optional": self.is_optional,
                "is_array": self.is_array,
                "fields": fields,
            })
        } else {
            let val = self
                .value
                .as_ref()
                .map(|e| e.read(cx).value().to_string())
                .unwrap_or_default();
            serde_json::json!({
                "name": name,
                "description": desc,
                "type": type_name,
                "is_optional": self.is_optional,
                "is_array": self.is_array,
                "value": val,
            })
        }
    }

    /// 构建子字段的 TreeItem 列表。
    fn build_tree_items(fields: &[FieldState], is_dynamic: bool) -> Vec<TreeItem> {
        let mut items: Vec<TreeItem> = fields
            .iter()
            .enumerate()
            .map(|(fi, _)| TreeItem::new(format!("field-{fi}"), format!("field-{fi}")))
            .collect();
        if is_dynamic {
            items.push(TreeItem::new("addfield", "+ Add Field"));
        }
        items
    }

    /// 重建 Tree 控件数据（字段增删或类型切换时调用）。
    pub fn rebuild_tree(&mut self, is_dynamic: bool, cx: &mut App) {
        if let Some(tree_state) = &self.tree_state {
            let items = Self::build_tree_items(&self.fields, is_dynamic);
            tree_state.update(cx, |state, cx| {
                state.set_items(items, cx);
            });
        }
    }

    /// 同步项状态从 JSON 值（避免回环）。
    pub fn sync_from_value(
        &mut self,
        item: &serde_json::Value,
        _is_variable: bool,
        registry: &DataTypeRegistry,
        window: &mut Window,
        cx: &mut App,
    ) {
        let name_text = item_name(item);
        let current_name = self.name.read(cx).value().to_string();
        if current_name != name_text {
            self.name.update(cx, |s, cx| {
                s.set_value(name_text.as_str(), window, cx);
            });
        }

        // 同步 description
        let desc_text = item
            .get("description")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string();
        let current_desc = self.description.read(cx).value().to_string();
        if current_desc != desc_text {
            self.description.update(cx, |s, cx| {
                s.set_value(desc_text.as_str(), window, cx);
            });
        }

        // 同步 is_optional / is_array
        self.is_optional = item_is_optional(item);
        self.is_array = item_is_array(item);

        let type_name = item_type(item);
        let type_changed = type_name != self.type_value;
        let has_fields = registry.has_fields(&type_name);
        let type_names: Vec<&str> = registry.type_names();

        if type_changed {
            self.type_value = type_name.clone();
            sync_type_select(&self.type_select, &type_name, window, cx);
            let was_structured = self.value.is_none();

            if was_structured && !has_fields {
                self.fields.clear();
                self.value = Some(cx.new(|cx| {
                    InputState::new(window, cx).placeholder("value")
                }));
                self.expanded = false;
                self.tree_state = None;
            } else if !was_structured && has_fields {
                self.value = None;
                self.expanded = true;
                self.rebuild_fields(item, &type_names, window, cx);
                let is_dyn = registry.is_dynamic(&type_name);
                let tree_items = Self::build_tree_items(&self.fields, is_dyn);
                self.tree_state = Some(cx.new(|cx| TreeState::new(cx).items(tree_items)));
            } else if was_structured && has_fields {
                self.rebuild_fields(item, &type_names, window, cx);
                let is_dyn = registry.is_dynamic(&type_name);
                self.rebuild_tree(is_dyn, cx);
            }
        } else if has_fields {
            self.sync_fields(item, registry, &type_names, window, cx);
        }

        if !has_fields {
            let v = item_value(item);
            if let Some(val_entity) = &self.value {
                let current = val_entity.read(cx).value().to_string();
                if current != v {
                    val_entity.update(cx, |s, cx| {
                        s.set_value(v.as_str(), window, cx);
                    });
                }
            }
        }
    }

    /// 重建所有字段（类型切换时）。
    fn rebuild_fields(
        &mut self,
        item: &serde_json::Value,
        type_names: &[&str],
        window: &mut Window,
        cx: &mut App,
    ) {
        self.fields.clear();
        let json_fields = item_fields(item);
        self.fields = build_field_states(&json_fields, type_names, window, cx);
    }

    /// 同步字段值（类型未变，字段数量可能变化）。
    fn sync_fields(
        &mut self,
        item: &serde_json::Value,
        registry: &DataTypeRegistry,
        type_names: &[&str],
        window: &mut Window,
        cx: &mut App,
    ) {
        let json_fields = item_fields(item);

        if json_fields.len() != self.fields.len() {
            self.fields = build_field_states(&json_fields, type_names, window, cx);
        } else {
            for (i, f) in json_fields.iter().enumerate() {
                let fname = f
                    .get("name")
                    .and_then(|v| v.as_str())
                    .unwrap_or("")
                    .to_string();
                let ftype = f
                    .get("type")
                    .and_then(|v| v.as_str())
                    .unwrap_or("String")
                    .to_string();
                let fval = f.get("value").map(value_to_string).unwrap_or_default();

                let current_name = self.fields[i].name.read(cx).value().to_string();
                if current_name != fname {
                    self.fields[i].name.update(cx, |s, cx| {
                        s.set_value(fname.as_str(), window, cx);
                    });
                }
                if self.fields[i].type_value != ftype {
                    self.fields[i].type_value = ftype.clone();
                    sync_type_select(&self.fields[i].type_select, &ftype, window, cx);
                }
                let current_val = self.fields[i].value.read(cx).value().to_string();
                if current_val != fval {
                    self.fields[i].value.update(cx, |s, cx| {
                        s.set_value(fval.as_str(), window, cx);
                    });
                }
            }
        }

        let is_dyn = registry.is_dynamic(&self.type_value);
        self.rebuild_tree(is_dyn, cx);
    }
}

/// 从 JSON 字段数组构建 FieldState 列表。
fn build_field_states(
    json_fields: &[serde_json::Value],
    type_names: &[&str],
    window: &mut Window,
    cx: &mut App,
) -> Vec<FieldState> {
    json_fields
        .iter()
        .map(|f| {
            let fname = f
                .get("name")
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string();
            let ftype = f
                .get("type")
                .and_then(|v| v.as_str())
                .unwrap_or("String")
                .to_string();
            let fval = f.get("value").map(value_to_string).unwrap_or_default();
            let name_input = cx.new(|cx| {
                InputState::new(window, cx)
                    .default_value(fname.as_str())
                    .placeholder("name")
            });
            let val_input = cx.new(|cx| {
                InputState::new(window, cx)
                    .default_value(fval.as_str())
                    .placeholder("value")
            });
            let type_select = create_type_select(type_names, &ftype, window, cx);
            FieldState {
                name: name_input,
                type_value: ftype,
                type_select,
                value: val_input,
            }
        })
        .collect()
}
