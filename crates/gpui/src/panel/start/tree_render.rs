//! Tree 控件渲染：参数/变量树形编辑。
//!
//! 行模板（未选中）：`[chevron?] [文本摘要] [删除]`
//! 行模板（选中）：同上 + 浮层编辑表单（左侧 Popover 风格的 deferred+anchored 覆盖层）
//!
//! - 未选中：紧凑文本摘要（name: type?[] = value），节省空间
//! - 选中：左侧浮层显示完整编辑表单（name/type/optional/array/value/description）
//! - chevron 点击：切换展开/折叠（toggle_item_expand），不选中
//! - 文本点击：选中行（set_selected_index），不切换展开
//! - 删除按钮：stop_propagation，不触发选中

use std::collections::HashMap;

use gpui::{
    anchored, deferred, div, px, Anchor, App, Bounds, ClickEvent, Context, Deferred, Entity,
    InteractiveElement, IntoElement, MouseButton, ParentElement, Pixels, Point,
    Styled, StyledText, Window,
};
use gpui::prelude::FluentBuilder as _;
use gpui_component::button::{Button, ButtonVariants};
use gpui_component::checkbox::Checkbox;
use gpui_component::input::Input;
use gpui_component::list::ListItem;
use gpui_component::select::Select;
use gpui_component::tree::{tree, TreeEntry, TreeItem, TreeState};
use gpui_component::{ElementExt, Icon, IconName, Sizable, StyledExt};

use crate::data_type::DataTypeRegistry;
use crate::i18n::{data_type_label, t, Language, TKey};
use crate::theme::Theme;

use super::common::{display_type_with_flags, RowInputs};
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
                description: Some(st.description.clone()),
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
                        description: None,
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
    let tree_state_for_row = tree_state.clone();

    let tree_el = tree(
        tree_state,
        move |ix, entry: &TreeEntry, selected, window, cx| {
            let id = entry.item().id.to_string();
            match rows.get(&id) {
                Some(row) => render_tree_row(
                    ix,
                    entry,
                    row,
                    &entity_for_row,
                    &tree_state_for_row,
                    &fk_for_row,
                    lang,
                    theme,
                    selected,
                    window,
                    cx,
                ),
                None => ListItem::new(ix),
            }
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

/// 行边界状态（用于浮层定位）。
///
/// 首次渲染选中项时，`on_prepaint` 捕获行边界并请求下一帧动画，
/// 捕获后在下一帧渲染浮层。`captured` 标志确保只请求一次动画帧。
#[derive(Default)]
struct RowBoundsState {
    bounds: Bounds<Pixels>,
    captured: bool,
}

/// 渲染单个 Tree 行。
///
/// 未选中：`[chevron?] [文本摘要] [删除]`（紧凑文本，节省空间）
/// 选中：同上 + 左侧浮层显示完整编辑表单（deferred+anchored 覆盖层）
///
/// - chevron 点击：toggle_item_expand 切换展开，stop_propagation 不触发选中
/// - 文本点击：set_selected_index 选中行，stop_propagation 不切换展开
/// - 删除按钮：stop_propagation，不触发选中
#[allow(clippy::too_many_arguments)]
fn render_tree_row(
    ix: usize,
    entry: &TreeEntry,
    row: &RowInputs,
    entity: &Entity<StartPanelView>,
    tree_state: &Entity<TreeState>,
    field_key: &str,
    lang: Language,
    theme: Theme,
    selected: bool,
    window: &mut Window,
    cx: &mut App,
) -> ListItem {
    let depth = entry.depth();
    let indent = px(16.0) * depth as f32;

    // chevron：folder 项的展开/折叠箭头（点击冒泡到 Tree 的 on_entry_click，
    // 由 TreeState 内部处理选中+展开，避免 set_items 清除选中状态）
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

    // 文本摘要：name: type?[] = value（紧凑显示）
    let name_value = row.name.read(cx).value().to_string();
    let type_str = display_type_with_flags(&row.type_value, row.is_optional, row.is_array);
    let summary = if let Some(val_entity) = row.value.as_ref() {
        let val = val_entity.read(cx).value().to_string();
        if val.is_empty() {
            format!("{}: {}", name_value, type_str)
        } else {
            format!("{}: {} = {}", name_value, type_str, val)
        }
    } else {
        format!("{}: {}", name_value, type_str)
    };

    // 文本摘要容器：点击选中行（不切换展开），stop_propagation 阻止 on_entry_click
    let ts_text = tree_state.clone();
    let text_summary = div()
        .flex_1()
        .h_full()
        .flex()
        .items_center()
        .px(px(6.0))
        .text_size(px(13.0))
        .text_color(if selected {
            theme.panel_label_text
        } else {
            theme.panel_subtext
        })
        .when(selected, |this| this.font_weight(gpui::FontWeight::MEDIUM))
        .on_mouse_down(MouseButton::Left, move |_, _, cx| {
            ts_text.update(cx, |state, cx| {
                state.set_selected_index(Some(ix), cx);
            });
            cx.stop_propagation();
        })
        .child(StyledText::new(summary));

    // 删除按钮：stop_propagation，不触发选中
    let entity_del = entity.clone();
    let fk_del = field_key.to_string();
    let item_idx_del = row.item_idx;
    let field_idx_del = row.field_idx;
    let delete_btn = div()
        .w(px(24.0))
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

    // 行内容：紧凑布局
    let mut row_content = div()
        .w_full()
        .h(px(TREE_ENTRY_HEIGHT))
        .py(px(2.0))
        .pl(indent)
        .pr(px(0.0))
        .flex()
        .items_center()
        .gap(px(2.0))
        .children(chevron)
        .child(text_summary)
        .child(delete_btn);

    // 选中时：捕获行边界并渲染左侧浮层编辑表单
    let bounds_key = if field_key == "params" {
        "row-bounds-p"
    } else {
        "row-bounds-v"
    };
    if selected {
        let bounds_state =
            window.use_keyed_state((bounds_key, ix), cx, |_, _| RowBoundsState::default());

        let bs_for_prepaint = bounds_state.clone();
        row_content = row_content.on_prepaint(move |bounds, window, cx| {
            bs_for_prepaint.update(cx, |state, _| {
                let first = !state.captured;
                state.bounds = bounds;
                state.captured = true;
                if first {
                    window.request_animation_frame();
                }
            });
        });

        let bs_read = bounds_state.read(cx);
        if bs_read.captured {
            let bounds = bs_read.bounds;
            let edit_form = build_edit_form(row, entity, tree_state, field_key, lang, theme, ix);
            let ts_dismiss = tree_state.clone();
            let overlay = render_edit_overlay(bounds, edit_form, theme, move |_, _, cx| {
                ts_dismiss.update(cx, |state, cx| {
                    state.set_selected_index(None, cx);
                });
            });
            row_content = row_content.child(overlay);
        }
    } else {
        // 未选中时重置 captured 标志（避免下次选中时使用旧边界）
        let bounds_state =
            window.use_keyed_state((bounds_key, ix), cx, |_, _| RowBoundsState::default());
        bounds_state.update(cx, |state, _| {
            state.captured = false;
        });
    }

    ListItem::new(ix)
        .py_0()
        .px_0()
        .selected(selected)
        .child(row_content)
}

/// 渲染左侧浮层编辑表单（deferred + anchored）。
///
/// 使用 `Anchor::TopRight`：浮层的右上角对齐到行左边界，
/// 使浮层向左展开，出现在选中行的左侧。
fn render_edit_overlay<E, F>(
    row_bounds: Bounds<Pixels>,
    content: E,
    theme: Theme,
    on_dismiss: F,
) -> Deferred
where
    E: IntoElement + 'static,
    F: Fn(&gpui::MouseDownEvent, &mut Window, &mut App) + 'static,
{
    let position: Point<Pixels> = row_bounds.origin;
    let bg = theme.panel_bg;
    let border = theme.panel_border;

    deferred(
        anchored()
            .snap_to_window_with_margin(px(8.))
            .anchor(Anchor::TopRight)
            .position(position)
            .child(
                div()
                    .id("edit-overlay")
                    .occlude()
                    .tab_group()
                    .w(px(320.0))
                    .p(px(12.0))
                    .bg(bg)
                    .border_1()
                    .border_color(border)
                    .rounded_md()
                    .on_mouse_down_out(on_dismiss)
                    .child(content),
            ),
    )
    .with_priority(1)
}

/// 构建完整编辑表单（浮层内容）。
///
/// 顶层项：name / type / optional / array / value / description
/// 子字段：name / type / value
#[allow(clippy::too_many_arguments)]
fn build_edit_form(
    row: &RowInputs,
    entity: &Entity<StartPanelView>,
    _tree_state: &Entity<TreeState>,
    field_key: &str,
    lang: Language,
    theme: Theme,
    ix: usize,
) -> impl IntoElement {
    let is_field = row.field_idx.is_some();
    let label_color = theme.panel_subtext;
    let gap = px(8.0);

    let name_label = t(lang, TKey::PanelParamName).to_string();
    let type_label = t(lang, TKey::PanelParamType).to_string();
    let optional_label = t(lang, TKey::PanelParamOptional).to_string();
    let array_label = t(lang, TKey::PanelParamArray).to_string();
    let desc_label = t(lang, TKey::PanelDesc).to_string();

    // 名称
    let name_field = div()
        .flex()
        .flex_col()
        .gap(px(4.0))
        .child(
            div()
                .text_size(px(11.0))
                .text_color(label_color)
                .child(name_label),
        )
        .child(Input::new(&row.name).small().appearance(true));

    // 类型
    let type_field = div()
        .flex()
        .flex_col()
        .gap(px(4.0))
        .child(
            div()
                .text_size(px(11.0))
                .text_color(label_color)
                .child(type_label),
        )
        .child(Select::new(&row.type_select).small().appearance(true));

    // 可选 / 数组（仅顶层项）
    let flags_row = if !is_field {
        let entity_opt = entity.clone();
        let fk_opt = field_key.to_string();
        let item_idx_opt = row.item_idx;
        let entity_arr = entity.clone();
        let fk_arr = field_key.to_string();
        let item_idx_arr = row.item_idx;
        Some(
            div()
                .flex()
                .gap(px(16.0))
                .child(
                    div()
                        .flex()
                        .items_center()
                        .gap(px(6.0))
                        .child(
                            Checkbox::new(("edit-opt", ix))
                                .checked(row.is_optional)
                                .on_click(move |checked: &bool, _, cx| {
                                    entity_opt.update(cx, |this, cx| {
                                        this.toggle_item_optional(&fk_opt, item_idx_opt, *checked, cx);
                                    });
                                }),
                        )
                        .child(
                            div()
                                .text_size(px(12.0))
                                .text_color(label_color)
                                .child(optional_label),
                        ),
                )
                .child(
                    div()
                        .flex()
                        .items_center()
                        .gap(px(6.0))
                        .child(
                            Checkbox::new(("edit-arr", ix))
                                .checked(row.is_array)
                                .on_click(move |checked: &bool, _, cx| {
                                    entity_arr.update(cx, |this, cx| {
                                        this.toggle_item_array(&fk_arr, item_idx_arr, *checked, cx);
                                    });
                                }),
                        )
                        .child(
                            div()
                                .text_size(px(12.0))
                                .text_color(label_color)
                                .child(array_label),
                        ),
                ),
        )
    } else {
        None
    };

    // 值（基础类型）
    let value_field = row.value.as_ref().map(|val| {
        let value_label = t(lang, TKey::PanelParamValue).to_string();
        div()
            .flex()
            .flex_col()
            .gap(px(4.0))
            .child(
                div()
                    .text_size(px(11.0))
                    .text_color(label_color)
                    .child(value_label),
            )
            .child(Input::new(val).small().appearance(true))
    });

    // 描述（仅顶层项）
    let desc_field = row.description.as_ref().map(|desc| {
        div()
            .flex()
            .flex_col()
            .gap(px(4.0))
            .child(
                div()
                    .text_size(px(11.0))
                    .text_color(label_color)
                    .child(desc_label),
            )
            .child(Input::new(desc).small().appearance(true))
    });

    div()
        .flex()
        .flex_col()
        .gap(gap)
        .child(name_field)
        .child(type_field)
        .children(flags_row)
        .children(value_field)
        .children(desc_field)
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
