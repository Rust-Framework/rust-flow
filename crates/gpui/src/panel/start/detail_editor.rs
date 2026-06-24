//! 浮层详细编辑面板 + 面板头部渲染。
//!
//! 点击 TreeItem 时，在面板左侧浮出气泡式详情表单，支持编辑：
//! - 名称、描述、类型、是否可选、是否数组/集合、默认值（顶层项）
//! - 名称、类型、值（子字段）
//!
//! 气泡样式：
//! - 无边框，使用阴影营造浮层感
//! - 圆角 + 纯色背景，简洁大气
//! - Y 坐标跟随选中项位置（基于 flat index + 滚动偏移计算）

use gpui::{
    div, px, ClickEvent, Context, Entity, InteractiveElement, IntoElement,
    MouseMoveEvent, MouseButton, ParentElement, ScrollWheelEvent,
    Styled,
};
use gpui_component::button::{Button, ButtonVariants};
use gpui_component::input::Input;
use gpui_component::menu::{DropdownMenu, PopupMenuItem};
use gpui_component::scroll::ScrollableElement;
use gpui_component::switch::Switch;
use gpui_component::{Icon, IconName, Sizable, StyledExt};
use rust_agent_flow::Node;

use crate::data_type::DataTypeRegistry;
use crate::i18n::{data_type_label, kind_label, t, Language, TKey};
use crate::theme::Theme;

use super::item::{FieldState, ItemState, TREE_ENTRY_HEIGHT};
use super::StartPanelView;

/// 面板头部高度（px）：py(12) + 32px 图标 + border_b_1。
const HEADER_HEIGHT: f32 = 57.0;
/// 节点名称区域高度（px）：padding(16) + label(16) + gap(6) + Input(32)。
const NODE_NAME_SECTION_HEIGHT: f32 = 70.0;
/// Tree 标题行高度（px）：title(24) + gap(8)。
const TREE_TITLE_HEIGHT: f32 = 32.0;
/// Tree 区块间距（px）：内容区 gap(16)。
const TREE_SECTION_GAP: f32 = 16.0;

impl StartPanelView {
    /// 渲染浮层详细编辑面板（选中项时显示在左侧，气泡样式）。
    ///
    /// Y 坐标跟随选中项位置：基于 flat index 计算项在 Tree 中的位置，
    /// 减去滚动偏移得到视觉位置，使气泡始终对准选中项。
    pub(super) fn render_detail_panel(
        &self,
        entity: &Entity<StartPanelView>,
        lang: Language,
        theme: Theme,
        _cx: &mut Context<Self>,
    ) -> Option<impl IntoElement> {
        let sel = self.selected.as_ref()?;
        let (field_key, item_idx, field_idx) =
            (sel.field_key.clone(), sel.item_idx, sel.field_idx);

        let states = if field_key == "variables" {
            &self.variables_state
        } else {
            &self.params_state
        };

        let item_state = states.get(item_idx)?;

        // 计算选中项在 Tree 中的 flat index
        let flat_idx = calculate_flat_idx(states, item_idx, field_idx);

        // 计算 Tree 顶部在滚动内容中的 Y 偏移
        let tree_top_in_content = if field_key == "params" {
            // 节点名区 + 区间 gap
            NODE_NAME_SECTION_HEIGHT + TREE_SECTION_GAP
        } else {
            // 变量区在参数区下方：节点名 + gap + 参数区(title + entries) + gap
            let params_entries: usize = self
                .params_state
                .iter()
                .map(|st| 1 + if st.expanded { st.fields.len() } else { 0 })
                .sum();
            NODE_NAME_SECTION_HEIGHT
                + TREE_SECTION_GAP
                + TREE_TITLE_HEIGHT
                + params_entries as f32 * TREE_ENTRY_HEIGHT
                + TREE_SECTION_GAP
        };

        // 选中项在滚动内容中的 Y 位置
        let item_y_in_content =
            tree_top_in_content + TREE_TITLE_HEIGHT + flat_idx as f32 * TREE_ENTRY_HEIGHT;

        // 减去滚动偏移得到视觉位置，并限制在 header 下方
        let scroll_y = self.scroll_handle.offset().y.as_f32();
        let panel_y = (HEADER_HEIGHT + item_y_in_content - scroll_y).max(HEADER_HEIGHT + 8.0);

        // 构建编辑器内容
        let editor: gpui::AnyElement = if let Some(fi) = field_idx {
            let field = item_state.fields.get(fi)?;
            render_detail_field_editor(
                field, &field_key, item_idx, fi, item_state.is_array, entity, lang, &theme,
            )
            .into_any_element()
        } else {
            render_detail_item_editor(
                item_state, &field_key, item_idx, &self.registry, entity, lang, &theme,
            )
            .into_any_element()
        };

        let entity_close = entity.clone();
        let fk_close = field_key.clone();

        Some(
            div()
                .absolute()
                .left(px(-272.0))
                .top(px(panel_y))
                .w(px(260.0))
                .max_h(px(420.0))
                // 气泡样式：无边框，纯色背景 + 阴影
                .bg(theme.panel_bg)
                .rounded(px(10.0))
                .shadow_lg()
                .flex()
                .flex_col()
                .overflow_hidden()
                // 拦截事件，防止穿透到画布
                .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
                .on_mouse_down(MouseButton::Middle, |_, _, cx| cx.stop_propagation())
                .on_mouse_down(MouseButton::Right, |_, _, cx| cx.stop_propagation())
                .on_mouse_up(MouseButton::Left, |_, _, cx| cx.stop_propagation())
                .on_mouse_up(MouseButton::Middle, |_, _, cx| cx.stop_propagation())
                .on_mouse_up(MouseButton::Right, |_, _, cx| cx.stop_propagation())
                .on_mouse_move(|_: &MouseMoveEvent, _, cx| cx.stop_propagation())
                .on_scroll_wheel(|_: &ScrollWheelEvent, _, cx| cx.stop_propagation())
                // 头部：强调色背景 + 标题 + 关闭按钮
                .child(
                    div()
                        .flex()
                        .items_center()
                        .justify_between()
                        .px(px(14.0))
                        .py(px(10.0))
                        .bg(theme.toolbar_accent)
                        .child(
                            div()
                                .text_size(px(12.0))
                                .font_semibold()
                                .text_color(theme.toolbar_accent_text)
                                .child(t(lang, TKey::PanelFieldEdit).to_string()),
                        )
                        .child(
                            Button::new("detail-close")
                                .icon(IconName::Close)
                                .xsmall()
                                .ghost()
                                .on_click(move |_: &ClickEvent, _, cx| {
                                    entity_close.update(cx, |this, cx| {
                                        this.selected = None;
                                        this.clear_tree_selection(&fk_close, cx);
                                        cx.notify();
                                    });
                                }),
                        ),
                )
                // 内容区
                .child(
                    div()
                        .flex_1()
                        .overflow_y_scrollbar()
                        .p(px(14.0))
                        .flex()
                        .flex_col()
                        .gap(px(10.0))
                        .child(editor),
                ),
        )
    }
}

/// 计算选中项在 Tree 中的 flat index（扁平索引）。
///
/// 遍历 states 到目标项，累加每项占用的行数（1 + 展开的子字段数）。
/// 如果选中子字段，再加上 1 + field_idx。
fn calculate_flat_idx(states: &[ItemState], item_idx: usize, field_idx: Option<usize>) -> usize {
    let mut idx = 0usize;
    for (i, st) in states.iter().enumerate() {
        if i == item_idx {
            if let Some(fi) = field_idx {
                return idx + 1 + fi;
            }
            return idx;
        }
        idx += 1;
        if st.expanded {
            idx += st.fields.len();
        }
    }
    idx
}

/// 渲染顶层项详细编辑器。
///
/// 字段：名称、描述、类型、可选、数组、默认值、删除按钮。
#[allow(clippy::too_many_arguments)]
fn render_detail_item_editor(
    st: &ItemState,
    field_key: &str,
    item_idx: usize,
    registry: &DataTypeRegistry,
    entity: &Entity<StartPanelView>,
    lang: Language,
    theme: &Theme,
) -> impl IntoElement {
    let type_names: Vec<String> = registry
        .type_names()
        .into_iter()
        .map(|s| s.to_string())
        .collect();

    let name_label = t(lang, TKey::PanelFieldName).to_string();
    let desc_label = t(lang, TKey::PanelFieldDescription).to_string();
    let type_label = t(lang, TKey::PanelFieldType).to_string();
    let optional_label = t(lang, TKey::PanelParamOptional).to_string();
    let array_label = t(lang, TKey::PanelParamArray).to_string();
    let value_label = t(lang, TKey::PanelParamValue).to_string();
    let delete_label = t(lang, TKey::PanelDeleteRow).to_string();

    let entity_del = entity.clone();
    let fk_del = field_key.to_string();

    let mut col = div().flex().flex_col().gap(px(10.0));

    // 名称
    col = col.child(render_field_row(&name_label, theme, Input::new(&st.name).small().appearance(true)));

    // 描述
    col = col.child(render_field_row(&desc_label, theme, Input::new(&st.description).small().appearance(true)));

    // 类型下拉
    let entity_type = entity.clone();
    let fk_type = field_key.to_string();
    let current_type = st.type_value.clone();
    let current_type_label = data_type_label(lang, &current_type).to_string();
    let type_btn = Button::new(("detail-type", item_idx))
        .label(current_type_label)
        .icon(IconName::ChevronDown)
        .small()
        .secondary()
        .w_full()
        .dropdown_menu(move |menu, _w, _cx| {
            let mut menu = menu;
            for ty in &type_names {
                let ty_val = ty.clone();
                let is_checked = ty_val == current_type;
                let entity = entity_type.clone();
                let fk = fk_type.clone();
                menu = menu.item(
                    PopupMenuItem::new(data_type_label(lang, &ty_val).to_string())
                        .checked(is_checked)
                        .on_click(move |_, _, cx| {
                            entity.update(cx, |this, cx| {
                                this.change_item_type(&fk, item_idx, ty_val.clone(), cx);
                            });
                        }),
                );
            }
            menu
        });
    col = col.child(render_field_row(&type_label, theme, type_btn));

    // Optional / Array 开关
    let entity_opt = entity.clone();
    let fk_opt = field_key.to_string();
    let entity_arr = entity.clone();
    let fk_arr = field_key.to_string();
    col = col.child(
        div()
            .flex()
            .gap(px(16.0))
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap(px(8.0))
                    .child(
                        div()
                            .text_size(px(11.0))
                            .text_color(theme.panel_subtext)
                            .child(optional_label),
                    )
                    .child(
                        Switch::new(("detail-opt", item_idx))
                            .checked(st.is_optional)
                            .small()
                            .on_click(move |checked: &bool, _, cx| {
                                entity_opt.update(cx, |this, cx| {
                                    this.toggle_item_optional(&fk_opt, item_idx, *checked, cx);
                                });
                            }),
                    ),
            )
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap(px(8.0))
                    .child(
                        div()
                            .text_size(px(11.0))
                            .text_color(theme.panel_subtext)
                            .child(array_label),
                    )
                    .child(
                        Switch::new(("detail-arr", item_idx))
                            .checked(st.is_array)
                            .small()
                            .on_click(move |checked: &bool, _, cx| {
                                entity_arr.update(cx, |this, cx| {
                                    this.toggle_item_array(&fk_arr, item_idx, *checked, cx);
                                });
                            }),
                    ),
            ),
    );

    // 值（基础类型）
    if let Some(ref val_input) = st.value {
        col = col.child(render_field_row(&value_label, theme, Input::new(val_input).small().appearance(true)));
    }

    // 删除按钮
    col = col.child(
        div().pt(px(4.0)).child(
            Button::new(("detail-del", item_idx))
                .label(delete_label)
                .icon(IconName::Close)
                .small()
                .danger()
                .w_full()
                .on_click(move |_: &ClickEvent, _, cx| {
                    entity_del.update(cx, |this, cx| {
                        this.delete_item(&fk_del, item_idx, cx);
                        this.selected = None;
                        cx.notify();
                    });
                }),
        ),
    );

    col
}

/// 渲染子字段详细编辑器。
///
/// 字段：名称、类型（只读）、值、删除按钮。
#[allow(clippy::too_many_arguments)]
fn render_detail_field_editor(
    field: &FieldState,
    field_key: &str,
    item_idx: usize,
    field_idx: usize,
    _is_array: bool,
    entity: &Entity<StartPanelView>,
    lang: Language,
    theme: &Theme,
) -> impl IntoElement {
    let name_label = t(lang, TKey::PanelFieldName).to_string();
    let type_label = t(lang, TKey::PanelFieldType).to_string();
    let value_label = t(lang, TKey::PanelParamValue).to_string();
    let delete_label = t(lang, TKey::PanelDeleteRow).to_string();

    let entity_del = entity.clone();
    let fk_del = field_key.to_string();

    let mut col = div().flex().flex_col().gap(px(10.0));

    // 名称
    col = col.child(render_field_row(&name_label, theme, Input::new(&field.name).small().appearance(true)));

    // 类型（只读显示）
    col = col.child(
        div()
            .flex()
            .flex_col()
            .gap(px(4.0))
            .child(
                div()
                    .text_size(px(11.0))
                    .text_color(theme.panel_subtext)
                    .child(type_label),
            )
            .child(
                div()
                    .text_size(px(13.0))
                    .text_color(theme.panel_label_text)
                    .child(field.type_value.clone()),
            ),
    );

    // 值
    col = col.child(render_field_row(&value_label, theme, Input::new(&field.value).small().appearance(true)));

    // 删除字段按钮
    col = col.child(
        div().pt(px(4.0)).child(
            Button::new(("detail-field-del", field_idx))
                .label(delete_label)
                .icon(IconName::Close)
                .small()
                .danger()
                .w_full()
                .on_click(move |_: &ClickEvent, _, cx| {
                    entity_del.update(cx, |this, cx| {
                        this.delete_field(&fk_del, item_idx, field_idx, cx);
                        this.selected = None;
                        cx.notify();
                    });
                }),
        ),
    );

    col
}

/// 渲染标签 + 控件的表单行。
fn render_field_row(label: &str, theme: &Theme, control: impl IntoElement) -> impl IntoElement {
    div()
        .flex()
        .flex_col()
        .gap(px(4.0))
        .child(
            div()
                .text_size(px(11.0))
                .text_color(theme.panel_subtext)
                .child(label.to_string()),
        )
        .child(control)
}

/// 渲染面板头部（与效果图一致：图标 + 标题 + 副标题 + 底部分割线）。
pub(super) fn render_header(node: &Node, lang: Language, theme: &Theme) -> gpui::AnyElement {
    let kind = &node.kind;
    let kind_lbl = kind_label(lang, kind);
    let title = format!("{} {}", kind_lbl, t(lang, TKey::PanelNodeSuffix));

    div()
        .flex()
        .items_center()
        .gap(px(12.0))
        .px(px(20.0))
        .py(px(16.0))
        .border_b_1()
        .border_color(theme.panel_border)
        .bg(theme.panel_bg)
        .child(
            div()
                .w(px(40.0))
                .h(px(40.0))
                .rounded_xl()
                .bg(gpui::rgb(0x6366f1))
                .flex()
                .items_center()
                .justify_center()
                .child(
                    Icon::new(IconName::Play)
                        .with_size(px(20.0))
                        .text_color(gpui::rgb(0xffffff)),
                ),
        )
        .child(
            div()
                .flex()
                .flex_col()
                .gap(px(4.0))
                .child(
                    div()
                        .text_size(px(16.0))
                        .font_semibold()
                        .text_color(theme.panel_title_text)
                        .child(title),
                )
                .child(
                    div()
                        .text_size(px(13.0))
                        .text_color(theme.panel_subtext)
                        .child(kind.to_string()),
                ),
        )
        .into_any_element()
}
