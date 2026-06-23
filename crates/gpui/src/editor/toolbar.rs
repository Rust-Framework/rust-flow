//! 工具栏：左下角浮动面板，提供缩放、视图、布局方向、边类型、网格开关、
//! 拖拽开关、主题切换、数据源切换。
//!
//! 使用 gpui-component `Button` + `Tooltip` + `DropdownMenu` 组件，所有按钮
//! 提供 i18n tooltip 提示。工具栏不受视口缩放影响（在内容层之外）。所有颜色
//! 取自 [`Theme`](crate::theme::Theme)，支持亮色/暗色主题切换。

use gpui::{
    div, px, ClickEvent, Context, IntoElement, ParentElement, Styled, Window,
};
use gpui_component::button::{Button, ButtonVariants};
use gpui_component::menu::{DropdownMenu, PopupMenuItem};
use gpui_component::{IconName, Selectable, Sizable};
use rust_agent_flow::{EdgeType, PointF, RectF, SizeF, Viewport};

use crate::i18n::{t, TKey};
use crate::FlowIcon;

use super::data_source::DataSource;
use super::flow_editor::{FlowEditorView, LayoutDirection};
use super::viewport;

impl FlowEditorView {
    /// 放大（以视口可见区域中心为锚点）。
    pub(crate) fn zoom_in(&mut self, window: &Window, cx: &mut Context<Self>) {
        let size = window.viewport_size();
        let center_screen = PointF::new(size.width.as_f32() * 0.5, size.height.as_f32() * 0.5);
        let center_logical = self.viewport.to_logical(center_screen);
        self.viewport = viewport::handle_zoom(self.viewport, center_logical, -60.0);
        cx.notify();
    }

    /// 缩小（以视口可见区域中心为锚点）。
    pub(crate) fn zoom_out(&mut self, window: &Window, cx: &mut Context<Self>) {
        let size = window.viewport_size();
        let center_screen = PointF::new(size.width.as_f32() * 0.5, size.height.as_f32() * 0.5);
        let center_logical = self.viewport.to_logical(center_screen);
        self.viewport = viewport::handle_zoom(self.viewport, center_logical, 60.0);
        cx.notify();
    }

    /// 重置视口（归位 + 100%缩放）。
    pub(crate) fn reset_view(&mut self, cx: &mut Context<Self>) {
        self.viewport = Viewport::default();
        cx.notify();
    }

    /// 适应视图（将所有节点居中显示）。
    pub(crate) fn fit_view(&mut self, cx: &mut Context<Self>) {
        let mut bounds = Option::<RectF>::None;
        for node in self.graph.nodes() {
            let nb = node.bounds();
            bounds = Some(match bounds {
                Some(b) => RectF::new(
                    PointF::new(b.origin.x.min(nb.origin.x), b.origin.y.min(nb.origin.y)),
                    SizeF::new(
                        b.right().max(nb.right()) - b.left().min(nb.left()),
                        b.bottom().max(nb.bottom()) - b.top().min(nb.top()),
                    ),
                ),
                None => nb,
            });
        }
        if let Some(b) = bounds {
            let margin = 60.0;
            self.viewport.offset = PointF::new(margin - b.origin.x, margin - b.origin.y);
            self.viewport.scale = 1.0;
        }
        cx.notify();
    }

    /// 切换拖拽开关。
    pub(crate) fn toggle_drag(&mut self, cx: &mut Context<Self>) {
        self.drag_enabled = !self.drag_enabled;
        cx.notify();
    }

    /// 渲染工具栏：左下角横向浮动面板。
    ///
    /// 使用 gpui-component Button 组件，所有按钮提供 i18n tooltip。
    /// 边类型/点阵密度/数据源使用 DropdownMenu 下拉菜单。
    pub(crate) fn render_toolbar(&self, cx: &Context<Self>) -> impl IntoElement {
        let theme = self.theme;
        let lang = self.language;
        let scale_pct = (self.viewport.scale * 100.0) as i32;
        let entity = cx.entity();
        let layout_direction = self.layout_direction;
        let show_grid = self.show_grid;
        let drag_enabled = self.drag_enabled;
        let is_dark = theme.is_dark;
        let grid_spacing = self.grid_spacing;
        let default_edge_type = self.default_edge_type;
        let data_source = self.data_source;

        div()
            .absolute()
            .left_3()
            .bottom_3()
            .flex()
            .flex_row()
            .items_center()
            .gap_1()
            .rounded_lg()
            .bg(theme.toolbar_bg)
            .border_1()
            .border_color(theme.toolbar_border)
            .shadow_lg()
            .p_1()
            // ====== 缩放组：放大 + 百分比 + 缩小 ======
            .child(
                Button::new("tb-zoom-in")
                    .icon(IconName::Plus)
                    .small()
                    .ghost()
                    .tooltip(t(lang, TKey::TbZoomIn))
                    .on_click(cx.listener(|this, _: &ClickEvent, window, cx| {
                        this.zoom_in(window, cx);
                    })),
            )
            .child(
                div()
                    .w(px(40.0))
                    .h(px(28.0))
                    .flex()
                    .items_center()
                    .justify_center()
                    .text_xs()
                    .text_color(theme.toolbar_subtext)
                    .child(format!("{}%", scale_pct)),
            )
            .child(
                Button::new("tb-zoom-out")
                    .icon(IconName::Minus)
                    .small()
                    .ghost()
                    .tooltip(t(lang, TKey::TbZoomOut))
                    .on_click(cx.listener(|this, _: &ClickEvent, window, cx| {
                        this.zoom_out(window, cx);
                    })),
            )
            // ====== 视图组：适应 + 重置 ======
            .child(
                Button::new("tb-fit")
                    .icon(FlowIcon::Screen)
                    .small()
                    .ghost()
                    .tooltip(t(lang, TKey::TbFitView))
                    .on_click(cx.listener(|this, _: &ClickEvent, _, cx| {
                        this.fit_view(cx);
                    })),
            )
            .child(
                Button::new("tb-reset")
                    .icon(IconName::Undo)
                    .small()
                    .ghost()
                    .tooltip(t(lang, TKey::TbResetView))
                    .on_click(cx.listener(|this, _: &ClickEvent, _, cx| {
                        this.reset_view(cx);
                    })),
            )
            // ====== 布局方向组 ======
            .child(
                Button::new("tb-dir-h")
                    .icon(FlowIcon::Horizontal)
                    .small()
                    .ghost()
                    .selected(layout_direction == LayoutDirection::Horizontal)
                    .tooltip(t(lang, TKey::TbLayoutHorizontal))
                    .on_click(cx.listener(|this, _: &ClickEvent, _, cx| {
                        this.set_layout_direction(LayoutDirection::Horizontal, cx);
                    })),
            )
            .child(
                Button::new("tb-dir-v")
                    .icon(FlowIcon::Vertical)
                    .small()
                    .ghost()
                    .selected(layout_direction == LayoutDirection::Vertical)
                    .tooltip(t(lang, TKey::TbLayoutVertical))
                    .on_click(cx.listener(|this, _: &ClickEvent, _, cx| {
                        this.set_layout_direction(LayoutDirection::Vertical, cx);
                    })),
            )
            // ====== 边类型 Dropdown ======
            .child(
                Button::new("tb-edge-type")
                    .icon(IconName::Network)
                    .small()
                    .ghost()
                    .tooltip(t(lang, TKey::TbEdgeType))
                    .dropdown_menu({
                        let entity = entity.clone();
                        move |menu, _window, _cx| {
                            let variants = [
                                (EdgeType::Bezier, TKey::EdgeBezier),
                                (EdgeType::Straight, TKey::EdgeStraight),
                                (EdgeType::Step, TKey::EdgeStep),
                                (EdgeType::SmoothStep, TKey::EdgeSmoothStep),
                            ];
                            let mut menu = menu;
                            for (et, key) in variants {
                                let label = t(lang, key);
                                let entity = entity.clone();
                                menu = menu.item(
                                    PopupMenuItem::new(label)
                                        .checked(et == default_edge_type)
                                        .on_click(move |_, _, cx| {
                                            entity.update(cx, |this, cx| {
                                                this.default_edge_type = et;
                                                for edge in this.graph.edges_mut() {
                                                    edge.edge_type = et;
                                                }
                                                cx.notify();
                                            });
                                        }),
                                );
                            }
                            menu
                        }
                    }),
            )
            // ====== 点阵背景 Dropdown（开关 + 密度合一） ======
            .child(
                Button::new("tb-grid")
                    .icon(FlowIcon::Grip)
                    .small()
                    .ghost()
                    .selected(show_grid)
                    .tooltip(t(lang, TKey::TbToggleGrid))
                    .dropdown_menu({
                        let entity = entity.clone();
                        move |menu, _window, _cx| {
                            // 禁用 + 3 档密度
                            let variants: [(Option<f32>, TKey); 4] = [
                                (None, TKey::GridDensityDisabled),
                                (Some(20.0), TKey::GridDensityCompact),
                                (Some(28.0), TKey::GridDensityNormal),
                                (Some(40.0), TKey::GridDensitySparse),
                            ];
                            let mut menu = menu;
                            for (spacing_opt, key) in variants {
                                let label = t(lang, key);
                                let entity = entity.clone();
                                let checked = match spacing_opt {
                                    None => !show_grid,
                                    Some(sp) => show_grid && (grid_spacing as i32) == (sp as i32),
                                };
                                menu = menu.item(
                                    PopupMenuItem::new(label)
                                        .checked(checked)
                                        .on_click(move |_, _, cx| {
                                            entity.update(cx, |this, cx| {
                                                match spacing_opt {
                                                    None => {
                                                        this.show_grid = false;
                                                    }
                                                    Some(sp) => {
                                                        this.show_grid = true;
                                                        this.grid_spacing = sp.max(8.0);
                                                    }
                                                }
                                                cx.notify();
                                            });
                                        }),
                                );
                            }
                            menu
                        }
                    }),
            )
            // ====== 拖拽开关 ======
            .child(
                Button::new("tb-drag")
                    .icon(FlowIcon::Drag)
                    .small()
                    .ghost()
                    .selected(drag_enabled)
                    .tooltip(t(lang, TKey::TbToggleDrag))
                    .on_click(cx.listener(|this, _: &ClickEvent, _, cx| {
                        this.toggle_drag(cx);
                    })),
            )
            // ====== 主题切换 ======
            .child(
                Button::new("tb-theme")
                    .icon(if is_dark { IconName::Sun } else { IconName::Moon })
                    .small()
                    .ghost()
                    .tooltip(t(lang, TKey::TbToggleTheme))
                    .on_click(cx.listener(|this, _: &ClickEvent, _, cx| {
                        this.toggle_theme(cx);
                    })),
            )
            // ====== 语言切换 ======
            .child(
                Button::new("tb-lang")
                    .label(if lang.is_zh() { "En" } else { "中" })
                    .small()
                    .ghost()
                    .tooltip(t(lang, TKey::TbToggleLanguage))
                    .on_click(cx.listener(|this, _: &ClickEvent, _, cx| {
                        this.toggle_language(cx);
                    })),
            )
            // ====== 数据源 Dropdown ======
            .child(
                Button::new("tb-data-source")
                    .icon(IconName::ALargeSmall)
                    .small()
                    .ghost()
                    .tooltip(t(lang, TKey::TbDataSource))
                    .dropdown_menu({
                        let entity = entity.clone();
                        move |menu, _window, _cx| {
                            let mut menu = menu;
                            for &ds in DataSource::all() {
                                let label = t(lang, ds.label_key());
                                let entity = entity.clone();
                                menu = menu.item(
                                    PopupMenuItem::new(label)
                                        .checked(ds == data_source)
                                        .on_click(move |_, _, cx| {
                                            entity.update(cx, |this, cx| {
                                                this.set_data_source(ds, cx);
                                            });
                                        }),
                                );
                            }
                            menu
                        }
                    }),
            )
    }
}
