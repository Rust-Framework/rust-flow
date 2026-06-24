//! 工具栏扩展接口（策略模式）。
//!
//! 调用侧实现 [`ToolbarProvider`] trait，通过
//! [`FlowEditorView::add_toolbar_provider`](super::flow_editor::FlowEditorView::add_toolbar_provider)
//! 注入自定义工具项。工具项渲染在内置工具栏末尾，以竖线分隔符区隔。
//!
//! **设计参考**：与 [`SyntaxService`](crate::node::SyntaxService) 相同的
//! trait + `Arc<dyn Trait>` + setter 注入模式。

use std::sync::Arc;

use gpui::{AnyElement, Entity};

use crate::i18n::Language;
use crate::theme::Theme;

use super::flow_editor::FlowEditorView;

/// 工具栏扩展接口（扩展点）。
///
/// 调用侧实现此 trait，通过
/// [`FlowEditorView::add_toolbar_provider`] 注入。
/// [`ToolbarProvider::render_items`] 返回的元素会追加到内置工具栏末尾。
///
/// **示例**（demo 数据源选择器）：
/// ```ignore
/// use std::sync::{Arc, Mutex};
/// use gpui::AnyElement;
/// use rust_agent_flow_gpui::{ToolbarCtx, ToolbarProvider};
///
/// pub struct DataSourceToolbar {
///     current: Arc<Mutex<MyDataSource>>,
/// }
///
/// impl ToolbarProvider for DataSourceToolbar {
///     fn render_items(&self, ctx: &ToolbarCtx) -> Vec<AnyElement> {
///         // 构建 Button + DropdownMenu，通过 ctx.entity.update() 切换图
///         vec![/* ... */]
///     }
/// }
/// ```
pub trait ToolbarProvider: Send + Sync {
    /// 渲染自定义工具项。
    ///
    /// 返回的元素追加到内置工具之后。每个元素应是自包含的工具项
    ///（Button、DropdownMenu 等），通过 `ctx.entity` 在回调中更新编辑器。
    fn render_items(&self, ctx: &ToolbarCtx) -> Vec<AnyElement>;
}

/// 共享工具栏扩展类型（`Arc<dyn ToolbarProvider>` 的别名）。
pub type SharedToolbarProvider = Arc<dyn ToolbarProvider>;

/// 工具栏渲染上下文，传给 [`ToolbarProvider::render_items`]。
///
/// 持有编辑器实体句柄（用于在回调中 `entity.update(cx, |this, cx| { ... })`）、
/// 当前主题、语言和拖拽开关状态，供 provider 构建风格一致的工具项。
pub struct ToolbarCtx {
    /// 编辑器实体句柄，用于在回调中更新编辑器状态。
    pub entity: Entity<FlowEditorView>,
    /// 当前主题颜色配置。
    pub theme: Theme,
    /// 当前 UI 语言。
    pub language: Language,
    /// 当前是否允许拖拽节点（用于拖拽开关按钮的 selected 态）。
    pub drag_enabled: bool,
}
