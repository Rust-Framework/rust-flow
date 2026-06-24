//! `rust-agent-flow-gpui` — GPUI 渲染层。
//!
//! 提供 [`FlowEditorView`] 及 [`IFlowNode`] 扩展接口，基于 `rust-agent-flow` core 层
//! 实现类 ReactFlow 的可视化流程设计器。

pub mod assets;
pub mod builtin;
pub mod editor;
pub mod edge;
pub mod i18n;
pub mod node;
pub mod panel;
pub mod theme;

pub use assets::{CombinedAssets, FlowIcon};
pub use editor::{FlowEditorView, SharedToolbarProvider, ToolbarCtx, ToolbarProvider};
pub use edge::EdgeView;
pub use i18n::{Language, TKey, t};
pub use node::{IFlowNode, NodeRegistry, NodeView, NodeViewCtx};
pub use theme::Theme;

/// 初始化 GPUI 组件库（必须在打开窗口前调用）。
///
/// 封装 `gpui_component::init`，调用方只需一次调用。
pub fn init(cx: &mut gpui::App) {
    gpui_component::init(cx);
}
