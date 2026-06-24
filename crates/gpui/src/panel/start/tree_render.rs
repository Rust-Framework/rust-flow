//! Tree 控件渲染：参数/变量树形编辑。
//!
//! 效果图像素级复刻：
//! - 区域标题行：「输入参数」/「变量定义」（粗体）+ 「+ 添加参数」/「+ 添加变量」（紫色链接）
//! - 行模板：`[chevron?] [名称 Input] [类型 Dropdown] [值 Input]`
//! - 选中行：蓝色边框 + 浅蓝背景
//! - 三列等宽布局，紧凑间距
//!
//! 设计要点：
//! - 使用 `entry.depth()` 实现层级缩进（gpui-component Tree 不自动缩进）
//! - folder 项（复杂/动态类型）显示 chevron 箭头，点击切换展开
//! - 三个内联控件（Input/Dropdown/Input）通过 `cx.stop_propagation()` 阻止
//!   事件冒泡到 Tree 的点击处理器，避免点击控件时触发 Tree 展开
//! - 同时在控件区 `on_mouse_down` 中手动调用 `set_selected_index` 触发选中，
//!   使点击输入框/下拉框也能选中当前项（显示详情面板）
//! - 点击 chevron 或行间隙触发 Tree 默认行为（选中 + 展开 folder）

use std::collections::HashMap;

use gpui::{
    div, px, App, ClickEvent, Context, Entity, InteractiveElement, IntoElement,
    MouseButton, ParentElement, prelude::FluentBuilder as _, Styled,
};
use gpui_component::button::{Button, ButtonVariants};
use gpui_component::input::Input;
use gpui_component::list::ListItem;
use gpui_component::menu::{DropdownMenu, PopupMenuItem};
use gpui_component::tree::{tree, TreeEntry, TreeItem, TreeState};
use gpui_component::{Icon, IconName, Sizable, StyledExt};

use crate::data_type::DataTypeRegistry;
use crate::i18n::{data_type_label, Language};
use crate::theme::Theme;

use super::common::RowInputs;
use super::item::{ItemState, TREE_ENTRY_HEIGHT};
use super::StartPanelView;

/// 选中行边框颜色（蓝色 #818cf8）
const SELECTED_BORDER_COLOR: gpui::Rgba = gpui::Rgba { r: 0x81 as f32 / 255.0, g: 0x8c as f32 / 255.0, b: 0xf8 as f32 / 255.0, a: 1.0 };
/// 选中行背景色（浅蓝 #e0e7ff）
const SELECTED_BG_COLOR: gpui::Rgba = gpui::Rgba { r: 0xe0 as f32 / 255.0, g: 0xe7 as f32 / 255.0, b: 0xff as f32 / 255.0, a: 1.0 };
/// 添加按钮颜色（紫色 #6366f1）
const ADD_BUTTON_COLOR: gpui::Rgba = gpui::Rgba { r: 0x63 as f32 / 255.0, g: 0x66 as f32 / 255.0, b: 0xf1 as f32 / 255.0, a: 1.0 };

/// 渲染区域 Tree（参数区或变量区）。
///
/// 包含：区域标题（粗体 + 添加按钮） + 行列表（三列式布局）。
#[allow(clippy::too_many_arguments)]
pub(super) fn render_section_tree(
    tree_state: &Entity<TreeState>,
    field_key: &str,
    title: &str,
    add_label: &str,
    _is_variable: bool,
    registry: &DataTypeRegistry,
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

    // 类型列表（所有行共享，用于下拉菜单）
    let type_names: Vec<String> = registry
        .type_names()
        .into_iter()
        .map(|s| s.to_string())
        .collect();

    // 预构建每行的输入控件状态（Entity 句柄）
    let mut rows: HashMap<String, RowInputs> = HashMap::new();
    let mut entry_count: usize = 0;
    for (item_idx, st) in states.iter().enumerate() {
        entry_count += 1; // 顶层项
        let type_label = data_type_label(lang, &st.type_value).to_string();
        rows.insert(
            format!("{}:{}", field_key, item_idx),
            RowInputs {
                name: st.name.clone(),
                type_value: st.type_value.clone(),
                type_label,
                value: st.value.clone(),
                item_idx,
                field_idx: None,
            },
        );
        // 展开项的子字段
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
                        value: Some(field.value.clone()),
                        item_idx,
                        field_idx: Some(field_idx),
                    },
                );
            }
        }
    }

    // Tree 容器高度：至少一行高度（空列表时保留可见区域 + 添加按钮可点）。
    let tree_height = px(TREE_ENTRY_HEIGHT * entry_count.max(1) as f32);

    // 闭包共享数据
    let entity_for_row = entity.clone();
    let fk_for_row = field_key.to_string();
    let tree_state_for_row = tree_state.clone();

    // 获取当前选中索引用于高亮（传递给render_tree_row闭包）
    let _selected_idx = tree_state.read(_cx).selected_index();

    let tree_el = tree(
        tree_state,
        move |ix, entry, selected, _window, _cx| {
            let id = entry.item().id.to_string();
            let row = match rows.get(&id) {
                Some(r) => r,
                None => return ListItem::new(ix),
            };
            render_tree_row(
                ix,
                entry,
                row,
                &type_names,
                &entity_for_row,
                &fk_for_row,
                lang,
                theme,
                &tree_state_for_row,
                selected,
            )
        },
    )
    .h(tree_height);

    div()
        .flex()
        .flex_col()
        .gap(px(12.0))
        // 标题行：左侧粗体标题 + 右侧添加按钮
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
        // 行列表容器
        .child(
            div()
                .flex()
                .flex_col()
                .gap(px(8.0))
                .child(tree_el),
        )
}

/// 渲染单个 Tree 行：`[chevron?] [名称 Input] [类型 Dropdown] [值 Input]`
///
/// 像素级复刻效果图：
/// - 三列式布局：名称输入框 | 类型下拉菜单 | 值输入框
/// - 选中行：蓝色边框（#818cf8）+ 浅蓝背景（#e0e7ff）
/// - `entry.depth()` 控制缩进（depth 0 = 顶层项，depth 1 = 子字段）
/// - folder 项显示 chevron 箭头（向下=展开，向右=收起）
/// - 控件区 `on_mouse_down` 阻止冒泡 + 手动 `set_selected_index` 触发选中
#[allow(clippy::too_many_arguments)]
fn render_tree_row(
    ix: usize,
    entry: &TreeEntry,
    row: &RowInputs,
    type_names: &[String],
    entity: &Entity<StartPanelView>,
    field_key: &str,
    lang: Language,
    theme: Theme,
    tree_state: &Entity<TreeState>,
    selected: bool,
) -> ListItem {
    let depth = entry.depth();
    let indent = px(16.0) * depth as f32;

    // chevron：folder 项显示箭头，非 folder 的顶层项留空位对齐
    let chevron = if entry.is_folder() {
        Some(
            div()
                .w(px(20.0))
                .h(px(32.0))
                .flex()
                .items_center()
                .justify_center()
                .child(
                    Icon::new(if entry.is_expanded() {
                        IconName::ChevronDown
                    } else {
                        IconName::ChevronRight
                    })
                    .small()
                    .text_color(theme.panel_subtext),
                ),
        )
    } else if depth == 0 {
        Some(div().w(px(20.0)))
    } else {
        None
    };

    // 类型下拉按钮 - 效果图样式：圆角 + 背景色
    let entity_type = entity.clone();
    let fk_type = field_key.to_string();
    let current_type = row.type_value.clone();
    let item_idx = row.item_idx;
    let field_idx = row.field_idx;
    let type_names_vec = type_names.to_vec();
    let type_label = row.type_label.clone();

    let type_dropdown = Button::new(("tree-type", ix))
        .label(type_label)
        .icon(IconName::ChevronDown)
        .small()
        .secondary()
        .dropdown_menu(move |menu, _w, _cx| {
            let mut menu = menu;
            for ty in &type_names_vec {
                let ty_val = ty.clone();
                let is_checked = ty_val == current_type;
                let entity = entity_type.clone();
                let fk = fk_type.clone();
                menu = menu.item(
                    PopupMenuItem::new(data_type_label(lang, &ty_val).to_string())
                        .checked(is_checked)
                        .on_click(move |_, _, cx| {
                            entity.update(cx, |this, cx| {
                                if let Some(fi) = field_idx {
                                    this.change_field_type(&fk, item_idx, fi, ty_val.clone(), cx);
                                } else {
                                    this.change_item_type(&fk, item_idx, ty_val.clone(), cx);
                                }
                            });
                        }),
                );
            }
            menu
        });

    // 控件区选中句柄：点击控件时阻止冒泡 + 手动选中当前项
    let tree_state_sel = tree_state.clone();
    let select_idx = ix;

    // 构建行内容容器，根据选中状态添加边框和背景
    let mut row_container = div()
        .w_full()
        .pl(indent)
        .pr(px(16.0))
        .py(px(6.0))
        .rounded_lg();

    // 选中状态：蓝色边框 + 浅蓝背景
    if selected {
        row_container = row_container
            .border_1()
            .border_color(SELECTED_BORDER_COLOR)
            .bg(SELECTED_BG_COLOR);
    }

    ListItem::new(ix)
        .w_full()
        .child(
            row_container
                .child(
                    div()
                        .flex()
                        .items_center()
                        .gap(px(12.0))
                        .children(chevron)
                        // 控件区：阻止冒泡 + 手动选中
                        .child(
                            div()
                                .flex()
                                .flex_1()
                                .items_center()
                                .gap(px(12.0))
                                .on_mouse_down(MouseButton::Left, move |_, _, cx: &mut App| {
                                    cx.stop_propagation();
                                    tree_state_sel.update(cx, |state, cx| {
                                        state.set_selected_index(Some(select_idx), cx);
                                    });
                                })
                                .on_mouse_down(MouseButton::Right, |_, _, cx| cx.stop_propagation())
                                // 名称 Input（固定宽度比例）
                                .child(
                                    div()
                                        .flex_1()
                                        .min_w(px(100.0))
                                        .child(Input::new(&row.name).small().appearance(true)),
                                )
                                // 类型 Dropdown（固定宽度）
                                .child(
                                    div()
                                        .w(px(80.0))
                                        .child(type_dropdown),
                                )
                                // 值 Input（基础类型时显示，flex_1填充剩余空间）
                                .when_some(row.value.as_ref(), |d, val| {
                                    d.child(
                                        div()
                                            .flex_1()
                                            .min_w(px(100.0))
                                            .child(Input::new(val).small().appearance(true)),
                                    )
                                }),
                        ),
                ),
        )
}

/// 构建 Tree items 列表（参数区或变量区）。
///
/// ID 格式：
/// - 顶层项：`"params:0"`、`"variables:1"`
/// - 子字段：`"params:0:0"`、`"variables:1:2"`
///
/// 复杂/动态类型项展开后显示子字段；基础类型项为叶子节点。
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

            // 复杂/动态类型：添加子字段
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

/// 格式化项摘要文本：`name: type` 或 `name: type = value`。
fn format_item_summary(st: &ItemState, cx: &mut Context<StartPanelView>) -> String {
    let name = st.name.read(cx).value().to_string();
    let type_str = super::common::display_type_with_flags(&st.type_value, st.is_optional, st.is_array);
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

/// 格式化子字段摘要文本。
fn format_field_summary(field: &super::item::FieldState, cx: &mut Context<StartPanelView>) -> String {
    let name = field.name.read(cx).value().to_string();
    let val = field.value.read(cx).value().to_string();
    if val.is_empty() {
        format!("{}: {}", name, field.type_value)
    } else {
        format!("{}: {} = {}", name, field.type_value, val)
    }
}
