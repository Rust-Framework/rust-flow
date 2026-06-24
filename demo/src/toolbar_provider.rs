//! Demo 工具栏扩展：数据源选择器 + 应用控件（拖拽/主题/语言）。
//!
//! 实现 [`ToolbarProvider`] trait，通过 `add_toolbar_provider` 注入到编辑器。
//! - [`DataSourceToolbar`]：数据源下拉菜单，切换时调用 `set_graph` 重建图。
//! - [`AppControlsToolbar`]：拖拽开关、主题切换、语言切换。
//!   这些控件跟随目标系统，由调用侧决定如何呈现，框架仅提供能力方法。

use std::sync::{Arc, Mutex};

use gpui::{AnyElement, IntoElement};
use gpui_component::button::{Button, ButtonVariants};
use gpui_component::menu::{DropdownMenu, PopupMenuItem};
use gpui_component::{IconName, Selectable, Sizable};
use rust_agent_flow_gpui::{FlowIcon, TKey, ToolbarCtx, ToolbarProvider, t};

use crate::data_sources::DemoDataSource;

/// Demo 数据源选择器（ToolbarProvider 实现）。
///
/// 持有当前数据源状态（`Arc<Mutex<DemoDataSource>>`），切换时更新自身状态
/// 并调用 `FlowEditorView::set_graph` 重建图。
pub struct DataSourceToolbar {
    current: Arc<Mutex<DemoDataSource>>,
}

impl DataSourceToolbar {
    pub fn new(initial: DemoDataSource) -> Self {
        Self {
            current: Arc::new(Mutex::new(initial)),
        }
    }
}

impl ToolbarProvider for DataSourceToolbar {
    fn render_items(&self, ctx: &ToolbarCtx) -> Vec<AnyElement> {
        let lang = ctx.language;
        let current = *self.current.lock().unwrap();
        let entity = ctx.entity.clone();
        let current_state = self.current.clone();

        let btn = Button::new("demo-data-source")
            .icon(IconName::ALargeSmall)
            .small()
            .ghost()
            .tooltip(t(lang, TKey::TbDataSource))
            .dropdown_menu(move |menu, _window, _cx| {
                let mut menu = menu;
                for &ds in DemoDataSource::all() {
                    let label = ds.label(lang);
                    let entity = entity.clone();
                    let current_state = current_state.clone();
                    menu = menu.item(
                        PopupMenuItem::new(label)
                            .checked(ds == current)
                            .on_click(move |_, _, cx| {
                                *current_state.lock().unwrap() = ds;
                                let graph = ds.to_graph();
                                entity.update(cx, |this, cx| {
                                    this.set_graph(graph, cx);
                                });
                            }),
                    );
                }
                menu
            })
            .into_any_element();

        vec![btn]
    }
}

/// Demo 应用控件工具栏（拖拽开关、主题切换、语言切换）。
///
/// 这些控件在实际应用中跟随目标系统，由调用侧决定如何呈现。
/// 框架仅提供能力方法（`toggle_drag`/`toggle_theme`/`toggle_language`），
/// 调用侧通过 `ToolbarProvider` 注入 UI。
pub struct AppControlsToolbar;

impl AppControlsToolbar {
    pub fn new() -> Self {
        Self
    }
}

impl Default for AppControlsToolbar {
    fn default() -> Self {
        Self::new()
    }
}

impl ToolbarProvider for AppControlsToolbar {
    fn render_items(&self, ctx: &ToolbarCtx) -> Vec<AnyElement> {
        let lang = ctx.language;
        let is_dark = ctx.theme.is_dark;
        let drag_enabled = ctx.drag_enabled;

        // ====== 拖拽开关 ======
        let drag_entity = ctx.entity.clone();
        let drag_btn = Button::new("app-toggle-drag")
            .icon(FlowIcon::Drag)
            .small()
            .ghost()
            .selected(drag_enabled)
            .tooltip(t(lang, TKey::TbToggleDrag))
            .on_click(move |_, _, cx| {
                drag_entity.update(cx, |this, cx| {
                    this.toggle_drag(cx);
                });
            })
            .into_any_element();

        // ====== 主题切换（暗色时显示太阳=切到亮色，亮色时显示月亮=切到暗色） ======
        let theme_entity = ctx.entity.clone();
        let theme_btn = Button::new("app-toggle-theme")
            .icon(if is_dark { IconName::Sun } else { IconName::Moon })
            .small()
            .ghost()
            .tooltip(t(lang, TKey::TbToggleTheme))
            .on_click(move |_, _, cx| {
                theme_entity.update(cx, |this, cx| {
                    this.toggle_theme(cx);
                });
            })
            .into_any_element();

        // ====== 语言切换 ======
        let lang_entity = ctx.entity.clone();
        let lang_btn = Button::new("app-toggle-language")
            .icon(IconName::Globe)
            .small()
            .ghost()
            .tooltip(t(lang, TKey::TbToggleLanguage))
            .on_click(move |_, _, cx| {
                lang_entity.update(cx, |this, cx| {
                    this.toggle_language(cx);
                });
            })
            .into_any_element();

        vec![drag_btn, theme_btn, lang_btn]
    }
}
