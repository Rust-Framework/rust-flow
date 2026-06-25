//! 面板头部渲染。

use gpui::{div, px, IntoElement, ParentElement, Styled};
use gpui_component::{Icon, IconName, Sizable, StyledExt};
use rust_agent_flow::Node;

use crate::i18n::{kind_label, t, Language, TKey};
use crate::theme::Theme;

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
