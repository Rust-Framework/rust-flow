//! Panel 渲染层：易变的 UI 渲染方法（头部、各类字段控件）。

use gpui::{
    div, px, ClickEvent, Context, IntoElement, ParentElement, Styled,
};
use gpui_component::button::{Button, ButtonVariants};
use gpui_component::input::Input;
use gpui_component::menu::{DropdownMenu, PopupMenuItem};
use gpui_component::switch::Switch;
use gpui_component::{Icon, IconName, Sizable, StyledExt};
use rust_agent_flow::{DropdownOption, ListSpec};
use crate::builtin::common::node_icon;
use crate::i18n::{kind_label, t, TKey};
use crate::theme::Theme;

use super::{FieldState, PanelView};
use super::helpers::dropdown_option_label;

impl PanelView {
    /// 渲染面板头部：节点图标（彩色圆角方块）+ 类型标签 + kind 副标题。
    ///
    /// 视觉参考 n8n/Retool 属性面板头部：带彩色背景的图标 + 标题层次感。
    pub(super) fn render_header(&self, theme: &Theme) -> gpui::AnyElement {
        let lang = self.language;
        let kind = &self.node.kind;
        let icon_name = node_icon(kind);
        let kind_label = kind_label(lang, kind);
        let title = format!("{} {}", kind_label, t(lang, TKey::PanelNodeSuffix));

        div()
            .flex()
            .items_center()
            .gap(px(10.0))
            .px(px(16.0))
            .py(px(12.0))
            .border_b_1()
            .border_color(theme.panel_border)
            .bg(theme.node_title_bg)
            .child(
                div()
                    .w(px(32.0))
                    .h(px(32.0))
                    .rounded_md()
                    .bg(theme.toolbar_accent)
                    .flex()
                    .items_center()
                    .justify_center()
                    .child(
                        Icon::new(icon_name)
                            .small()
                            .text_color(theme.toolbar_accent_text),
                    ),
            )
            .child(
                div()
                    .flex()
                    .flex_col()
                    .gap(px(2.0))
                    .child(
                        div()
                            .text_size(px(14.0))
                            .font_semibold()
                            .text_color(theme.panel_title_text)
                            .child(title),
                    )
                    .child(
                        div()
                            .text_size(px(11.0))
                            .text_color(theme.panel_subtext)
                            .child(kind.to_string()),
                    ),
            )
            .into_any_element()
    }

    /// 渲染 Input 类字段（Text/TextArea/CodeEditor/CodeBlock/Number）。
    pub(super) fn render_input_field(
        &self,
        field_idx: usize,
        label: &str,
        height: Option<gpui::Pixels>,
        theme: &Theme,
    ) -> gpui::AnyElement {
        let entity = match &self.field_states[field_idx] {
            FieldState::Input(e) => e,
            _ => return div().into_any_element(),
        };
        let mut input = Input::new(entity).appearance(true);
        if let Some(h) = height {
            input = input.h(h);
        }
        div()
            .flex()
            .flex_col()
            .gap(px(6.0))
            .child(self.render_label(label, theme))
            .child(input)
            .into_any_element()
    }

    /// 渲染 Switch 字段（水平布局：标签左 + 开关右）。
    pub(super) fn render_switch_field(
        &mut self,
        field_idx: usize,
        label: &str,
        theme: &Theme,
        cx: &mut Context<Self>,
    ) -> gpui::AnyElement {
        let checked = match self.field_states[field_idx] {
            FieldState::Switch(b) => b,
            _ => false,
        };
        let id = format!("field-switch-{}", field_idx);
        div()
            .flex()
            .items_center()
            .justify_between()
            .gap(px(8.0))
            .child(self.render_label(label, theme))
            .child(
                Switch::new(id)
                    .checked(checked)
                    .on_click(cx.listener(move |this, val: &bool, _w, cx| {
                        this.set_switch_field(field_idx, *val, cx);
                    })),
            )
            .into_any_element()
    }

    /// 渲染 Dropdown 字段（使用 gpui-component Button + DropdownMenu）。
    ///
    /// 与工具栏风格一致：secondary 按钮 + 下拉菜单 + checked 标记，
    /// 替代原来的自定义 div 按钮组，视觉更专业。
    pub(super) fn render_dropdown_field(
        &mut self,
        field_idx: usize,
        label: &str,
        options: &[DropdownOption],
        theme: &Theme,
        cx: &mut Context<Self>,
    ) -> gpui::AnyElement {
        let current = match &self.field_states[field_idx] {
            FieldState::Dropdown(s) => s.clone(),
            _ => String::new(),
        };
        let lang = self.language;
        let kind_str = kind_label(lang, &self.node.kind);
        let entity = cx.entity();

        // 查找当前选中项的标签
        let current_label = options
            .iter()
            .find(|opt| opt.value == current)
            .map(|opt| dropdown_option_label(lang, kind_str, opt))
            .unwrap_or_else(|| current.clone());

        let btn_id = format!("field-dropdown-{}", field_idx);
        // 克隆 options 供 move 闭包使用
        let options_owned: Vec<DropdownOption> = options.to_vec();

        let mut col = div().flex().flex_col().gap(px(6.0));
        col = col.child(self.render_label(label, theme));

        col = col.child(
            Button::new(btn_id)
                .label(current_label)
                .icon(IconName::ChevronDown)
                .small()
                .secondary()
                .w_full()
                .dropdown_menu(move |menu, _window, _cx| {
                    let mut menu = menu;
                    for opt in &options_owned {
                        let item_label = dropdown_option_label(lang, kind_str, opt);
                        let val = opt.value.clone();
                        let is_checked = val == current;
                        let entity = entity.clone();
                        menu = menu.item(
                            PopupMenuItem::new(item_label)
                                .checked(is_checked)
                                .on_click(move |_, _, cx| {
                                    entity.update(cx, |this, cx| {
                                        this.set_dropdown_field(field_idx, &val, cx);
                                    });
                                }),
                        );
                    }
                    menu
                }),
        );
        col.into_any_element()
    }

    /// 渲染 List 字段（卡片式行 + gpui-component Button 添加/删除）。
    ///
    /// 视觉改进：
    /// - 每行用卡片式容器（背景色 + 圆角 + 边框），层次感更强
    /// - Input 使用 flex_1 自适应宽度，不再固定 70px/80px
    /// - 删除/添加按钮使用 gpui-component Button，风格统一
    pub(super) fn render_list_field(
        &mut self,
        field_idx: usize,
        label: &str,
        list_spec: &ListSpec,
        theme: &Theme,
        cx: &mut Context<Self>,
    ) -> gpui::AnyElement {
        let lang = self.language;
        let rows = match &self.field_states[field_idx] {
            FieldState::List(r) => r,
            _ => return div().into_any_element(),
        };

        let mut col = div().flex().flex_col().gap(px(8.0));
        col = col.child(self.render_label(label, theme));

        // 行容器
        let mut rows_col = div().flex().flex_col().gap(px(6.0));
        for (row_idx, row) in rows.iter().enumerate() {
            let mut row_div = div()
                .flex()
                .items_center()
                .gap(px(6.0))
                .p(px(6.0))
                .rounded_md()
                .bg(theme.node_title_bg)
                .border_1()
                .border_color(theme.panel_border);

            // 序号
            row_div = row_div.child(
                div()
                    .w(px(20.0))
                    .flex()
                    .flex_shrink_0()
                    .items_center()
                    .justify_center()
                    .text_size(px(11.0))
                    .text_color(theme.panel_subtext)
                    .child(format!("{}", row_idx + 1)),
            );

            // 每个 item_field 一个 Input（flex 自适应宽度）
            for (col_idx, _item_field) in list_spec.item_fields.iter().enumerate() {
                if col_idx >= row.len() {
                    break;
                }
                let entity = &row[col_idx];
                row_div = row_div
                    .child(div().flex_1().child(Input::new(entity).appearance(true)));
            }

            // 删除按钮（使用 gpui-component Button）
            let del_btn_id = format!("del-list-{}-{}", field_idx, row_idx);
            row_div = row_div.child(
                Button::new(del_btn_id)
                    .icon(IconName::Close)
                    .xsmall()
                    .ghost()
                    .on_click(cx.listener(move |this, _: &ClickEvent, _w, cx| {
                        this.delete_list_item(field_idx, row_idx, cx);
                    })),
            );

            rows_col = rows_col.child(row_div);
        }
        col = col.child(rows_col);

        // 添加按钮（使用 gpui-component Button）
        let add_label = t(lang, TKey::PanelAddBranch);
        let add_btn_id = format!("add-list-{}", field_idx);
        col = col.child(
            Button::new(add_btn_id)
                .label(format!("{} +", add_label))
                .icon(IconName::Plus)
                .small()
                .ghost()
                .w_full()
                .on_click(cx.listener(move |this, _: &ClickEvent, w, cx| {
                    this.add_list_item(field_idx, w, cx);
                })),
        );

        col.into_any_element()
    }
}
