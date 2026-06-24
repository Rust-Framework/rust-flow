//! Start 节点参数/变量项的状态与渲染。
//!
//! 三种类型形态：
//! - **基础类型**（Boolean/String/Number/DateTime）：name + type + value，单行紧凑布局
//! - **复杂类型**（DataModel 等）：预定义结构，结构只读，值按模式可编辑
//! - **动态类型**（DynamicObject）：结构可手动编辑（增删改字段），值按模式可编辑
//!
//! 子字段值编辑规则：
//! - 参数模式（is_variable=false）：子字段值只读
//! - 变量模式（is_variable=true）：子字段值可编辑

use gpui::{
    div, px, AnyElement, App, AppContext, ClickEvent, Entity, IntoElement, ParentElement, Styled,
    Window,
};
use gpui_component::button::{Button, ButtonVariants};
use gpui_component::input::{Input, InputState};
use gpui_component::menu::{DropdownMenu, PopupMenuItem};
use gpui_component::{IconName, Sizable};

use crate::data_type::DataTypeRegistry;
use crate::i18n::{t, Language, TKey};
use crate::theme::Theme;

use super::data_types::{item_fields, item_name, item_type, item_value, value_to_string};
use super::StartPanelView;

/// 单个子字段的状态。
pub struct FieldState {
    /// 字段名输入（动态类型可编辑，复杂类型只读渲染）。
    pub name: Entity<InputState>,
    /// 字段类型名。
    pub type_value: String,
    /// 字段值输入（变量模式可编辑，参数模式只读渲染）。
    pub value: Entity<InputState>,
}

/// 单个参数/变量项的编辑状态。
pub struct ItemState {
    /// 名称输入。
    pub name: Entity<InputState>,
    /// 当前类型（下拉值）。
    pub type_value: String,
    /// 基础类型的值输入（复杂/动态类型时为 None）。
    pub value: Option<Entity<InputState>>,
    /// 复杂/动态类型的子字段状态。
    pub fields: Vec<FieldState>,
    /// 是否展开显示子字段。
    pub expanded: bool,
}

impl ItemState {
    /// 从 JSON 值构建项状态。
    ///
    /// `registry` 提供类型元信息（分类、字段定义）。
    pub fn from_value(
        item: &serde_json::Value,
        _is_variable: bool,
        registry: &DataTypeRegistry,
        window: &mut Window,
        cx: &mut App,
    ) -> Self {
        let name_text = item_name(item);
        let type_name = item_type(item);
        let has_fields = registry.has_fields(&type_name);

        let name = cx.new(|cx| {
            InputState::new(window, cx)
                .default_value(name_text.as_str())
                .placeholder("name")
        });

        let (value, fields) = if has_fields {
            // 复杂/动态类型：从 JSON 读取字段
            let json_fields = item_fields(item);
            let field_states: Vec<FieldState> = json_fields
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
                    FieldState {
                        name: name_input,
                        type_value: ftype,
                        value: val_input,
                    }
                })
                .collect();
            (None, field_states)
        } else {
            // 基础类型
            let v = item_value(item);
            let val = cx.new(|cx| {
                InputState::new(window, cx)
                    .default_value(v.as_str())
                    .placeholder("value")
            });
            (Some(val), Vec::new())
        };

        Self {
            name,
            type_value: type_name,
            value,
            fields,
            expanded: has_fields,
        }
    }

    /// 将项状态序列化为 JSON 值。
    pub fn to_value(&self, cx: &App) -> serde_json::Value {
        let name = self.name.read(cx).value().to_string();
        let type_name = self.type_value.clone();

        if !self.fields.is_empty() || self.value.is_none() {
            // 复杂/动态类型
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
                "type": type_name,
                "fields": fields,
            })
        } else {
            // 基础类型
            let val = self
                .value
                .as_ref()
                .map(|e| e.read(cx).value().to_string())
                .unwrap_or_default();
            serde_json::json!({
                "name": name,
                "type": type_name,
                "value": val,
            })
        }
    }

    /// 同步项状态从 JSON 值（避免回环）。
    ///
    /// 当类型变化时重建子结构（需要 Window 创建新 InputState）。
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

        let type_name = item_type(item);
        let type_changed = type_name != self.type_value;
        let has_fields = registry.has_fields(&type_name);

        if type_changed {
            self.type_value = type_name.clone();
            let was_structured = self.value.is_none();

            if was_structured && !has_fields {
                // 结构 → 基础
                self.fields.clear();
                self.value = Some(cx.new(|cx| {
                    InputState::new(window, cx).placeholder("value")
                }));
                self.expanded = false;
            } else if !was_structured && has_fields {
                // 基础 → 结构
                self.value = None;
                self.expanded = true;
                // 重建字段
                self.rebuild_fields(item, registry, window, cx);
            } else if was_structured && has_fields {
                // 结构 → 结构（类型切换）：重建字段
                self.rebuild_fields(item, registry, window, cx);
            }
        } else if has_fields {
            // 类型未变但字段可能变化（动态类型增删字段）
            self.sync_fields(item, window, cx);
        }

        // 同步值
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
        registry: &DataTypeRegistry,
        window: &mut Window,
        cx: &mut App,
    ) {
        self.fields.clear();
        let json_fields = item_fields(item);
        for f in &json_fields {
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
            self.fields.push(FieldState {
                name: name_input,
                type_value: ftype,
                value: val_input,
            });
        }
        let _ = registry;
    }

    /// 同步字段值（类型未变，字段数量可能变化）。
    fn sync_fields(&mut self, item: &serde_json::Value, window: &mut Window, cx: &mut App) {
        let json_fields = item_fields(item);

        // 字段数量变化：重建
        if json_fields.len() != self.fields.len() {
            self.fields.clear();
            for f in &json_fields {
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
                self.fields.push(FieldState {
                    name: name_input,
                    type_value: ftype,
                    value: val_input,
                });
            }
            return;
        }

        // 字段数量一致：逐字段同步
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

            // 同步字段名
            let current_name = self.fields[i].name.read(cx).value().to_string();
            if current_name != fname {
                self.fields[i].name.update(cx, |s, cx| {
                    s.set_value(fname.as_str(), window, cx);
                });
            }
            // 同步字段类型
            self.fields[i].type_value = ftype;
            // 同步字段值
            let current_val = self.fields[i].value.read(cx).value().to_string();
            if current_val != fval {
                self.fields[i].value.update(cx, |s, cx| {
                    s.set_value(fval.as_str(), window, cx);
                });
            }
        }
    }
}

/// 渲染单个参数/变量项。
///
/// `field_key` 为 "params" 或 "variables"，用于回调标识。
/// `item_idx` 为项在列表中的索引。
/// `is_variable` 区分参数（false）和变量（true），影响子字段值是否可编辑。
/// `registry` 提供类型元信息。
pub fn render_item(
    state: &ItemState,
    field_key: &str,
    item_idx: usize,
    is_variable: bool,
    registry: &DataTypeRegistry,
    lang: Language,
    theme: &Theme,
    entity: &Entity<StartPanelView>,
    cx: &App,
) -> AnyElement {
    let has_fields = registry.has_fields(&state.type_value);
    let is_dynamic = registry.is_dynamic(&state.type_value);
    let is_complex = registry.is_complex(&state.type_value);

    let mut col = div().flex().flex_col().gap(px(4.0));

    // 第一行：序号 + 名称 + 类型下拉 + 展开/收起(结构类型) + 删除
    let mut row1 = div()
        .flex()
        .items_center()
        .gap(px(6.0))
        .p(px(6.0))
        .rounded_md()
        .bg(theme.node_title_bg)
        .border_1()
        .border_color(theme.panel_border);

    row1 = row1.child(
        div()
            .w(px(18.0))
            .flex()
            .flex_shrink_0()
            .items_center()
            .justify_center()
            .text_size(px(11.0))
            .text_color(theme.panel_subtext)
            .child(format!("{}", item_idx + 1)),
    );

    row1 = row1.child(div().flex_1().child(Input::new(&state.name).appearance(true)));

    // 类型下拉按钮
    let type_btn_id = format!("type-{}-{}", field_key, item_idx);
    let current_type = state.type_value.clone();
    // 转为 owned String 避免 registry 引用逃逸到 'static 闭包
    let type_names: Vec<String> = registry.type_names().into_iter().map(|s| s.to_string()).collect();
    // 预计算基础类型列表（用于动态类型字段类型下拉）
    let basic_types: Vec<String> = type_names
        .iter()
        .filter(|t| registry.is_basic(t))
        .cloned()
        .collect();
    let entity_clone = entity.clone();
    let fk = field_key.to_string();

    row1 = row1.child(
        Button::new(type_btn_id)
            .label(current_type.clone())
            .icon(IconName::ChevronDown)
            .xsmall()
            .secondary()
            .w(px(100.0))
            .dropdown_menu(move |menu, _w, _cx| {
                let mut menu = menu;
                for ty in &type_names {
                    let ty_val = ty.to_string();
                    let is_checked = ty_val == current_type;
                    let entity = entity_clone.clone();
                    let fk = fk.clone();
                    menu = menu.item(
                        PopupMenuItem::new(ty_val.clone())
                            .checked(is_checked)
                            .on_click(move |_, _, cx| {
                                entity.update(cx, |this, cx| {
                                    this.change_item_type(&fk, item_idx, ty_val.clone(), cx);
                                });
                            }),
                    );
                }
                menu
            }),
    );

    // 展开/收起按钮（仅结构类型）
    if has_fields {
        let expand_btn_id = format!("expand-{}-{}", field_key, item_idx);
        let icon = if state.expanded {
            IconName::ChevronDown
        } else {
            IconName::ChevronRight
        };
        let entity_clone2 = entity.clone();
        let fk2 = field_key.to_string();
        row1 = row1.child(
            Button::new(expand_btn_id)
                .icon(icon)
                .xsmall()
                .ghost()
                .on_click(move |_: &ClickEvent, _, cx| {
                    entity_clone2.update(cx, |this, cx| {
                        this.toggle_item_expanded(&fk2, item_idx, cx);
                    });
                }),
        );
    }

    // 删除按钮
    let del_btn_id = format!("del-{}-{}", field_key, item_idx);
    let entity_clone3 = entity.clone();
    let fk3 = field_key.to_string();
    row1 = row1.child(
        Button::new(del_btn_id)
            .icon(IconName::Close)
            .xsmall()
            .ghost()
            .on_click(move |_: &ClickEvent, _, cx| {
                entity_clone3.update(cx, |this, cx| {
                    this.delete_item(&fk3, item_idx, cx);
                });
            }),
    );

    col = col.child(row1);

    // 基础类型：第二行显示值输入（全宽）
    if !has_fields {
        if let Some(val_entity) = &state.value {
            let value_label = t(lang, TKey::PanelParamValue);
            col = col.child(
                div()
                    .flex()
                    .flex_col()
                    .gap(px(4.0))
                    .pl(px(30.0))
                    .child(
                        div()
                            .text_size(px(11.0))
                            .text_color(theme.panel_subtext)
                            .child(value_label),
                    )
                    .child(Input::new(val_entity).appearance(true)),
            );
        }
    }

    // 结构类型且展开：树形显示子字段
    if has_fields && state.expanded {
        let mut fields_col = div().flex().flex_col().gap(px(3.0)).pl(px(30.0));

        let field_count = state.fields.len();
        for (fi, field_state) in state.fields.iter().enumerate() {
            let is_last = fi == field_count - 1;
            let tree_icon = if is_last { "└" } else { "├" };

            let mut field_row = div()
                .flex()
                .items_center()
                .gap(px(4.0))
                .py(px(3.0))
                .px(px(6.0))
                .rounded_md()
                .bg(theme.panel_bg)
                .border_1()
                .border_color(theme.panel_border);

            field_row = field_row.child(
                div()
                    .w(px(16.0))
                    .flex()
                    .flex_shrink_0()
                    .items_center()
                    .justify_center()
                    .text_size(px(12.0))
                    .text_color(theme.panel_subtext)
                    .child(tree_icon),
            );

            if is_dynamic {
                // 动态类型：字段名可编辑
                field_row = field_row.child(
                    div()
                        .w(px(60.0))
                        .flex_shrink_0()
                        .child(Input::new(&field_state.name).appearance(true)),
                );

                // 动态类型：字段类型下拉（仅基础类型可选）
                let field_type_btn_id = format!("ftype-{}-{}-{}", field_key, item_idx, fi);
                let current_ftype = field_state.type_value.clone();
                let entity_clone4 = entity.clone();
                let fk4 = field_key.to_string();
                let basic_types_clone = basic_types.clone();
                field_row = field_row.child(
                    Button::new(field_type_btn_id)
                        .label(current_ftype.clone())
                        .icon(IconName::ChevronDown)
                        .xsmall()
                        .ghost()
                        .w(px(80.0))
                        .dropdown_menu(move |menu, _w, _cx| {
                            let mut menu = menu;
                            for ty in &basic_types_clone {
                                let ty_val = ty.to_string();
                                let is_checked = ty_val == current_ftype;
                                let entity = entity_clone4.clone();
                                let fk = fk4.clone();
                                menu = menu.item(
                                    PopupMenuItem::new(ty_val.clone())
                                        .checked(is_checked)
                                        .on_click(move |_, _, cx| {
                                            entity.update(cx, |this, cx| {
                                                this.change_field_type(
                                                    &fk,
                                                    item_idx,
                                                    fi,
                                                    ty_val.clone(),
                                                    cx,
                                                );
                                            });
                                        }),
                                );
                            }
                            menu
                        }),
                );

                // 动态类型：删除字段按钮
                let del_field_btn_id = format!("fdel-{}-{}-{}", field_key, item_idx, fi);
                let entity_clone5 = entity.clone();
                let fk5 = field_key.to_string();
                field_row = field_row.child(
                    Button::new(del_field_btn_id)
                        .icon(IconName::Close)
                        .xsmall()
                        .ghost()
                        .on_click(move |_: &ClickEvent, _, cx| {
                            entity_clone5.update(cx, |this, cx| {
                                this.delete_field(&fk5, item_idx, fi, cx);
                            });
                        }),
                );

                // 字段值（变量模式可编辑）
                if is_variable {
                    field_row = field_row.child(
                        div()
                            .flex_1()
                            .child(Input::new(&field_state.value).appearance(true)),
                    );
                } else {
                    let val = field_state.value.read(cx).value().to_string();
                    field_row = field_row.child(
                        div()
                            .flex_1()
                            .text_size(px(12.0))
                            .text_color(theme.panel_subtext)
                            .child(val),
                    );
                }
            } else {
                // 复杂类型：字段名只读
                field_row = field_row.child(
                    div()
                        .w(px(60.0))
                        .flex_shrink_0()
                        .text_size(px(12.0))
                        .text_color(theme.panel_label_text)
                        .child(field_state.name.read(cx).value().to_string()),
                );

                // 复杂类型：字段类型只读 tag
                field_row = field_row.child(
                    div()
                        .w(px(60.0))
                        .flex_shrink_0()
                        .px(px(4.0))
                        .py(px(2.0))
                        .rounded_sm()
                        .bg(theme.toolbar_toggle_bg)
                        .text_size(px(11.0))
                        .text_color(theme.toolbar_toggle_text)
                        .flex()
                        .items_center()
                        .justify_center()
                        .child(field_state.type_value.clone()),
                );

                // 字段值（变量模式可编辑）
                if is_variable {
                    field_row = field_row.child(
                        div()
                            .flex_1()
                            .child(Input::new(&field_state.value).appearance(true)),
                    );
                } else {
                    let val = field_state.value.read(cx).value().to_string();
                    field_row = field_row.child(
                        div()
                            .flex_1()
                            .text_size(px(12.0))
                            .text_color(theme.panel_subtext)
                            .child(val),
                    );
                }
            }

            fields_col = fields_col.child(field_row);
        }

        // 动态类型：添加字段按钮
        if is_dynamic {
            let add_field_btn_id = format!("fadd-{}-{}", field_key, item_idx);
            let entity_clone6 = entity.clone();
            let fk6 = field_key.to_string();
            fields_col = fields_col.child(
                div().pl(px(22.0)).child(
                    Button::new(add_field_btn_id)
                        .icon(IconName::Plus)
                        .xsmall()
                        .ghost()
                        .on_click(move |_: &ClickEvent, _, cx| {
                            entity_clone6.update(cx, |this, cx| {
                                this.add_field(&fk6, item_idx, cx);
                            });
                        }),
                ),
            );
        }

        let _ = is_complex; // 复杂类型标记已通过 is_dynamic=false 体现
        col = col.child(fields_col);
    }

    col.into_any_element()
}
