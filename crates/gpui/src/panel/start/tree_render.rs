//! Tree 控件渲染：参数/变量树形编辑。
//!
//! 行模板（顶层项）：`[chevron?] [名称 Input] [类型 Select] [可选][数组] [值 Input?] [删除]`
//! 行模板（子字段）：`[indent]    [名称 Input] [类型 Select] [值 Input]    [删除]`
//!
//! - 所有控件内联在 Tree 行中，无浮层编辑面板
//! - Select 组件用于类型选择（gpui-component Select）
//! - 尽可能消除 Tree/TreeItem 内外边距，充分利用空间

use std::collections::HashMap;

use gpui::{
    div, px, ClickEvent, Context, Entity, InteractiveElement, IntoElement,
    MouseButton, ParentElement, Styled,
};
use gpui_component::button::{Button, ButtonVariants};
use gpui_component::input::Input;
use gpui_component::list::ListItem;
use gpui_component::select::Select;
use gpui_component::switch::Switch;
use gpui_component::tree::{tree, TreeEntry, TreeItem, TreeState};
use gpui_component::{Icon, IconName, Sizable, StyledExt};

use crate::data_type::DataTypeRegistry;
use crate::i18n::{data_type_label, t, Language, TKey};
use crate::theme::Theme;

use super::common::RowInputs;
use super::item::{ItemState, TREE_ENTRY_HEIGHT};
use super::StartPanelView;

/// 添加按钮颜色（紫色 #6366f1）
const ADD_BUTTON_COLOR: gpui::Rgba = gpui::Rgba {
    r: 0x63 as f32 / 255.0,
    g: 0x66 as f32 / 255.0,
    b: 0xf1 as f32 / 255.0,
    a: 1.0,
};

/// 渲染区域 Tree（参数区或变量区）。
#[allow(clippy::too_many_arguments)]
pub(super) fn render_section_tree(
    tree_state: &Entity<TreeState>,
    field_key: &str,
    title: &str,
    add_label: &str,
    _is_variable: bool,
    _registry: &DataTypeRegistry,
    lang: Language,
    theme: Theme,
    entity: &Entity<StartPanelView>,
    states: &[ItemState],
    _cx: &mut Context<StartPanelView>,
) -> impl IntoElement {
    let add_label = add_label.to_string();
    let title = title.to_string();

    let entity_for_add = entity.clone();
    let fk_for_add = field_key.to_string();
    let add_btn_id = if field_key == "variables" {
        "add-var-btn"
    } else {
        "add-param-btn"
    };

    // 预构建每行的输入控件状态
    let mut rows: HashMap<String, RowInputs> = HashMap::new();
    let mut entry_count: usize = 0;
    for (item_idx, st) in states.iter().enumerate() {
        entry_count += 1;
        let type_label = data_type_label(lang, &st.type_value).to_string();
        rows.insert(
            format!("{}:{}", field_key, item_idx),
            RowInputs {
                name: st.name.clone(),
                type_value: st.type_value.clone(),
                type_label,
                type_select: st.type_select.clone(),
                value: st.value.clone(),
                is_optional: st.is_optional,
                is_array: st.is_array,
                item_idx,
                field_idx: None,
            },
        );
        if st.expanded {
            entry_count += st.fields.len();
            for (field_idx, field) in st.fields.iter().enumerate() {
                let ftype_label = data_type_label(lang, &field.type_value).to_string();
                rows.insert(
                    format!("{}:{}:{}", field_key, item_idx, field_idx),
                    RowInputs {
                        name: field.name.clone(),
                        type_value: field.type_value.clone(),
                        type_label: ftype_label,
                        type_select: field.type_select.clone(),
                        value: Some(field.value.clone()),
                        is_optional: false,
                        is_array: false,
                        item_idx,
                        field_idx: Some(field_idx),
                    },
                );
            }
        }
    }

    let tree_height = px(TREE_ENTRY_HEIGHT * entry_count.max(1) as f32);

    let entity_for_row = entity.clone();
    let fk_for_row = field_key.to_string();

    let tree_el = tree(
        tree_state,
        move |ix, entry, _selected, _window, _cx| {
            let id = entry.item().id.to_string();
            let row = match rows.get(&id) {
                Some(r) => r,
                None => return ListItem::new(ix),
            };
            render_tree_row(
                ix,
                entry,
                row,
                &entity_for_row,
                &fk_for_row,
                lang,
                theme,
            )
        },
    )
    .h(tree_height);

    div()
        .flex()
        .flex_col()
        .gap(px(8.0))
        .child(
            div()
                .flex()
                .items_center()
                .justify_between()
                .child(
                    div()
                        .text_size(px(14.0))
                        .font_medium()
                        .text_color(theme.panel_label_text)
                        .child(title),
                )
                .child(
                    Button::new(add_btn_id)
                        .label(add_label)
                        .small()
                        .ghost()
                        .text_color(ADD_BUTTON_COLOR)
                        .on_click(move |_: &ClickEvent, _, cx| {
                            entity_for_add.update(cx, |this, cx| {
                                this.add_item(&fk_for_add, cx);
                            });
                        }),
                ),
        )
        .child(tree_el)
}

/// 渲染单个 Tree 行（内联编辑模式）。
///
/// 顶层项：`[chevron?] [名称 Input] [类型 Select] [可选][数组] [值 Input?] [删除]`
/// 子字段：`[indent]    [名称 Input] [类型 Select] [值 Input]    [删除]`
#[allow(clippy::too_many_arguments)]
fn render_tree_row(
    ix: usize,
    entry: &TreeEntry,
    row: &RowInputs,
    entity: &Entity<StartPanelView>,
    field_key: &str,
    lang: Language,
    theme: Theme,
) -> ListItem {
    let depth = entry.depth();
    let indent = px(16.0) * depth as f32;
    let is_field = row.field_idx.is_some();

    // chevron（folder 项显示展开/折叠箭头，叶子项留空占位）
    let chevron = if entry.is_folder() {
        Some(
            div()
                .w(px(16.0))
                .h(px(20.0))
                .flex()
                .items_center()
                .justify_center()
                .child(
                    Icon::new(if entry.is_expanded() {
                        IconName::ChevronDown
                    } else {
                        IconName::ChevronRight
                    })
                    .xsmall()
                    .text_color(theme.panel_subtext),
                ),
        )
    } else {
        Some(div().w(px(16.0)))
    };

    // 名称 Input
    let name_input = div()
        .flex_1()
        .min_w(px(60.0))
        .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
        .child(Input::new(&row.name).small().appearance(true));

    // 类型 Select
    let type_select = div()
        .w(px(90.0))
        .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
        .child(Select::new(&row.type_select).small().appearance(true));

    // 可选 / 数组 Switch（仅顶层项）
    let optional_label = t(lang, TKey::PanelParamOptional).to_string();
    let array_label = t(lang, TKey::PanelParamArray).to_string();

    let optional_switch = if !is_field {
        let entity_opt = entity.clone();
        let fk_opt = field_key.to_string();
        let item_idx = row.item_idx;
        Some(
            div()
                .w(px(28.0))
                .flex()
                .items_center()
                .justify_center()
                .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
                .child(
                    Switch::new(("row-opt", ix))
                        .checked(row.is_optional)
                        .small()
                        .tooltip(optional_label.clone())
                        .on_click(move |checked: &bool, _, cx| {
                            entity_opt.update(cx, |this, cx| {
                                this.toggle_item_optional(&fk_opt, item_idx, *checked, cx);
                            });
                        }),
                ),
        )
    } else {
        None
    };

    let array_switch = if !is_field {
        let entity_arr = entity.clone();
        let fk_arr = field_key.to_string();
        let item_idx = row.item_idx;
        Some(
            div()
                .w(px(28.0))
                .flex()
                .items_center()
                .justify_center()
                .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
                .child(
                    Switch::new(("row-arr", ix))
                        .checked(row.is_array)
                        .small()
                        .tooltip(array_label.clone())
                        .on_click(move |checked: &bool, _, cx| {
                            entity_arr.update(cx, |this, cx| {
                                this.toggle_item_array(&fk_arr, item_idx, *checked, cx);
                            });
                        }),
                ),
        )
    } else {
        None
    };

    // 值 Input（基础类型有值，结构类型无）
    let value_input = row.value.as_ref().map(|val| {
        div()
            .flex_1()
            .min_w(px(60.0))
            .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
            .child(Input::new(val).small().appearance(true))
    });

    // 删除按钮
    let entity_del = entity.clone();
    let fk_del = field_key.to_string();
    let item_idx_del = row.item_idx;
    let field_idx_del = row.field_idx;
    let delete_btn = div()
        .w(px(24.0))
        .flex()
        .items_center()
        .justify_center()
        .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
        .child(
            Button::new(("row-del", ix))
                .icon(IconName::Close)
                .xsmall()
                .ghost()
                .on_click(move |_: &ClickEvent, _, cx| {
                    entity_del.update(cx, |this, cx| {
                        if let Some(fi) = field_idx_del {
                            this.delete_field(&fk_del, item_idx_del, fi, cx);
                        } else {
                            this.delete_item(&fk_del, item_idx_del, cx);
                        }
                    });
                }),
        );

    // 行内容：紧凑布局，消除多余边距
    let row_content = div()
        .w_full()
        .h(px(TREE_ENTRY_HEIGHT))
        .pl(indent)
        .pr(px(4.0))
        .flex()
        .items_center()
        .gap(px(4.0))
        .children(chevron)
        .child(name_input)
        .child(type_select)
        .children(optional_switch)
        .children(array_switch)
        .children(value_input)
        .child(delete_btn);

    // ListItem：消除默认内外边距
    ListItem::new(ix).py_0().px_0().child(row_content)
}

/// 构建 Tree items 列表（参数区或变量区）。
pub(super) fn build_section_tree_items(
    states: &[ItemState],
    field_key: &str,
    _registry: &DataTypeRegistry,
    cx: &mut Context<StartPanelView>,
) -> Vec<TreeItem> {
    states
        .iter()
        .enumerate()
        .map(|(item_idx, st)| {
            let id = format!("{}:{}", field_key, item_idx);
            let label = format_item_summary(st, cx);
            let mut item = TreeItem::new(id, label);

            if !st.fields.is_empty() {
                item = item.expanded(st.expanded);
                let children: Vec<TreeItem> = st
                    .fields
                    .iter()
                    .enumerate()
                    .map(|(field_idx, field)| {
                        let child_id = format!("{}:{}:{}", field_key, item_idx, field_idx);
                        let child_label = format_field_summary(field, cx);
                        TreeItem::new(child_id, child_label)
                    })
                    .collect();
                item = item.children(children);
            }

            item
        })
        .collect()
}

fn format_item_summary(st: &ItemState, cx: &mut Context<StartPanelView>) -> String {
    let name = st.name.read(cx).value().to_string();
    let type_str =
        super::common::display_type_with_flags(&st.type_value, st.is_optional, st.is_array);
    if let Some(ref val_input) = st.value {
        let val = val_input.read(cx).value().to_string();
        if val.is_empty() {
            format!("{}: {}", name, type_str)
        } else {
            format!("{}: {} = {}", name, type_str, val)
        }
    } else {
        format!("{}: {}", name, type_str)
    }
}

fn format_field_summary(field: &super::item::FieldState, cx: &mut Context<StartPanelView>) -> String {
    let name = field.name.read(cx).value().to_string();
    let val = field.value.read(cx).value().to_string();
    if val.is_empty() {
        format!("{}: {}", name, field.type_value)
    } else {
        format!("{}: {} = {}", name, field.type_value, val)
    }
}
