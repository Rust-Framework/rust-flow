//! Start 节点参数/变量列表区块渲染。
//!
//! 每个区块包含：区块标题 + 项列表 + 添加按钮。
//! 区块样式与 PanelView 的 List 字段保持一致（卡片式行容器）。

use gpui::{div, px, AnyElement, App, ClickEvent, IntoElement, ParentElement, Styled};
use gpui_component::button::{Button, ButtonVariants};
use gpui_component::{IconName, Sizable, StyledExt};

use crate::data_type::DataTypeRegistry;
use crate::i18n::Language;
use crate::theme::Theme;

use super::item::{render_item, ItemState};

/// 渲染参数/变量列表区块。
///
/// `field_key` 为 "params" 或 "variables"。
/// `title` 为区块标题。
/// `add_label` 为添加按钮文案。
/// `is_variable` 区分参数（false）和变量（true）。
/// `registry` 提供类型元信息。
pub fn render_section(
    states: &[ItemState],
    field_key: &str,
    title: &str,
    add_label: &str,
    is_variable: bool,
    registry: &DataTypeRegistry,
    lang: Language,
    theme: &Theme,
    entity: &gpui::Entity<super::StartPanelView>,
    cx: &mut App,
) -> AnyElement {
    let mut col = div().flex().flex_col().gap(px(8.0));

    // 区块标题
    col = col.child(
        div()
            .flex()
            .items_center()
            .gap(px(6.0))
            .child(
                div()
                    .w(px(3.0))
                    .h(px(14.0))
                    .rounded_full()
                    .bg(theme.toolbar_accent),
            )
            .child(
                div()
                    .text_size(px(13.0))
                    .font_semibold()
                    .text_color(theme.panel_title_text)
                    .child(title.to_string()),
            )
            .child(
                div()
                    .text_size(px(11.0))
                    .text_color(theme.panel_subtext)
                    .child(format!("{}", states.len())),
            ),
    );

    // 项列表
    let mut items_col = div().flex().flex_col().gap(px(6.0));
    for (idx, state) in states.iter().enumerate() {
        items_col = items_col.child(render_item(
            state,
            field_key,
            idx,
            is_variable,
            registry,
            lang,
            theme,
            entity,
            cx,
        ));
    }
    col = col.child(items_col);

    // 添加按钮
    let add_btn_id = format!("add-{}", field_key);
    let entity_clone = entity.clone();
    let fk = field_key.to_string();
    col = col.child(
        Button::new(add_btn_id)
            .label(add_label)
            .icon(IconName::Plus)
            .small()
            .ghost()
            .w_full()
            .on_click(move |_: &ClickEvent, _, cx| {
                entity_clone.update(cx, |this, cx| {
                    this.add_item(&fk, cx);
                });
            }),
    );

    col.into_any_element()
}
