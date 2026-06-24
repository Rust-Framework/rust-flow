//! Start 节点参数/变量项的状态与渲染。
//!
//! 每个项有两种形态：
//! - 简单类型：name + type(dropdown) + value，单行紧凑布局
//! - 复杂类型：name + type(dropdown)，展开后树形显示子字段（结构只读，值按模式可编辑）
//!
//! 子字段编辑规则：
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

use crate::i18n::{t, Language, TKey};
use crate::theme::Theme;

use super::data_types::{
    all_types, complex_type_fields, is_complex_type, item_fields, item_name, item_type,
    item_value, value_to_string,
};
use super::StartPanelView;

/// 单个参数/变量项的编辑状态。
pub struct ItemState {
    /// 名称输入。
    pub name: Entity<InputState>,
    /// 当前类型（下拉值）。
    pub type_value: String,
    /// 简单类型的值输入（复杂类型时为 None）。
    pub value: Option<Entity<InputState>>,
    /// 复杂类型子字段的值输入（仅变量模式可编辑，参数模式为 None）。
    pub field_values: Vec<Entity<InputState>>,
    /// 复杂类型是否展开显示子字段。
    pub expanded: bool,
}

impl ItemState {
    /// 从 JSON 值构建项状态。
    pub fn from_value(
        item: &serde_json::Value,
        _is_variable: bool,
        window: &mut Window,
        cx: &mut App,
    ) -> Self {
        let name_text = item_name(item);
        let type_name = item_type(item);
        let complex = is_complex_type(&type_name);

        let name = cx.new(|cx| {
            InputState::new(window, cx)
                .default_value(name_text.as_str())
                .placeholder("name")
        });

        let (value, field_values) = if complex {
            let fields = item_fields(item);
            let fvs: Vec<Entity<InputState>> = fields
                .iter()
                .map(|f| {
                    let v = f.get("value").map(value_to_string).unwrap_or_default();
                    cx.new(|cx| {
                        InputState::new(window, cx)
                            .default_value(v.as_str())
                            .placeholder("value")
                    })
                })
                .collect();
            (None, fvs)
        } else {
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
            field_values,
            expanded: complex,
        }
    }

    /// 将项状态序列化为 JSON 值。
    pub fn to_value(&self, cx: &App) -> serde_json::Value {
        let name = self.name.read(cx).value().to_string();
        let type_name = self.type_value.clone();

        if is_complex_type(&type_name) {
            let field_defs = complex_type_fields(&type_name).unwrap_or_default();
            let fields: Vec<serde_json::Value> = field_defs
                .iter()
                .enumerate()
                .map(|(i, def)| {
                    let val = if i < self.field_values.len() {
                        self.field_values[i].read(cx).value().to_string()
                    } else {
                        def.default_value.to_string()
                    };
                    serde_json::json!({
                        "name": def.name,
                        "type": def.field_type,
                        "value": val,
                    })
                })
                .collect();
            serde_json::json!({
                "name": name,
                "type": type_name,
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
        is_variable: bool,
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

        if type_changed {
            // 类型变化：重建数据结构
            self.type_value = type_name.clone();
            let was_complex = self.value.is_none();
            let is_complex = is_complex_type(&type_name);

            if was_complex && !is_complex {
                // 复杂 → 简单
                self.field_values.clear();
                self.value = Some(cx.new(|cx| {
                    InputState::new(window, cx).placeholder("value")
                }));
                self.expanded = false;
            } else if !was_complex && is_complex {
                // 简单 → 复杂
                self.value = None;
                let defs = complex_type_fields(&type_name).unwrap_or_default();
                self.field_values = defs
                    .iter()
                    .map(|def| {
                        cx.new(|cx| {
                            InputState::new(window, cx)
                                .default_value(def.default_value)
                                .placeholder("value")
                        })
                    })
                    .collect();
                self.expanded = true;
            }
        }

        if is_complex_type(&type_name) {
            let fields = item_fields(item);
            for (i, f) in fields.iter().enumerate() {
                if i < self.field_values.len() {
                    let v = f.get("value").map(value_to_string).unwrap_or_default();
                    let current = self.field_values[i].read(cx).value().to_string();
                    if current != v {
                        self.field_values[i].update(cx, |s, cx| {
                            s.set_value(v.as_str(), window, cx);
                        });
                    }
                }
            }
        } else {
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

        let _ = is_variable;
    }
}

/// 渲染单个参数/变量项。
///
/// `field_key` 为 "params" 或 "variables"，用于回调标识。
/// `item_idx` 为项在列表中的索引。
/// `is_variable` 区分参数（false）和变量（true），影响子字段值是否可编辑。
pub fn render_item(
    state: &ItemState,
    field_key: &str,
    item_idx: usize,
    is_variable: bool,
    lang: Language,
    theme: &Theme,
    entity: &Entity<StartPanelView>,
    cx: &App,
) -> AnyElement {
    let complex = is_complex_type(&state.type_value);

    let mut col = div().flex().flex_col().gap(px(4.0));

    // 第一行：序号 + 名称 + 类型下拉 + 展开/收起(复杂) + 删除
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
    let types = all_types();
    let entity_clone = entity.clone();
    let fk = field_key.to_string();

    row1 = row1.child(
        Button::new(type_btn_id)
            .label(current_type.clone())
            .icon(IconName::ChevronDown)
            .xsmall()
            .secondary()
            .w(px(90.0))
            .dropdown_menu(move |menu, _w, _cx| {
                let mut menu = menu;
                for ty in &types {
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

    // 展开/收起按钮（仅复杂类型）
    if complex {
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

    // 简单类型：第二行显示值输入（全宽）
    if !complex {
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

    // 复杂类型且展开：树形显示子字段
    if complex && state.expanded {
        let field_defs = complex_type_fields(&state.type_value).unwrap_or_default();
        let mut fields_col = div().flex().flex_col().gap(px(3.0)).pl(px(30.0));

        for (fi, def) in field_defs.iter().enumerate() {
            let is_last = fi == field_defs.len() - 1;
            let tree_icon = if is_last { "└" } else { "├" };

            let mut field_row = div()
                .flex()
                .items_center()
                .gap(px(6.0))
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

            // 子字段名称（只读）
            field_row = field_row.child(
                div()
                    .w(px(60.0))
                    .flex_shrink_0()
                    .text_size(px(12.0))
                    .text_color(theme.panel_label_text)
                    .child(def.name.to_string()),
            );

            // 子字段类型（只读 tag 样式）
            field_row = field_row.child(
                div()
                    .w(px(50.0))
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
                    .child(def.field_type.to_string()),
            );

            // 子字段值
            if is_variable {
                if fi < state.field_values.len() {
                    field_row = field_row.child(
                        div()
                            .flex_1()
                            .child(Input::new(&state.field_values[fi]).appearance(true)),
                    );
                }
            } else {
                let val = if fi < state.field_values.len() {
                    state.field_values[fi].read(cx).value().to_string()
                } else {
                    def.default_value.to_string()
                };
                field_row = field_row.child(
                    div()
                        .flex_1()
                        .text_size(px(12.0))
                        .text_color(theme.panel_subtext)
                        .child(val),
                );
            }

            fields_col = fields_col.child(field_row);
        }

        col = col.child(fields_col);
    }

    col.into_any_element()
}
