//! IFlowNode 节点扩展接口（策略模式）。
//!
//! 每个 [`NodeKind`] 对应一个 [`IFlowNode`] 实现，提供节点卡片视图和属性面板。
//! 普通节点只需实现 `kind`/`get_view`/`get_panel`/`schema`；
//! 特殊节点（如条件分支）可覆写 `resolve_port` 精确控制端口位置。

use gpui::{AnyElement, App, Window};
use rust_agent_flow::{Node, NodeSchema, PortId, PointF, RectF};

/// 节点渲染上下文，提供给 [`IFlowNode`] 方法使用。
///
/// 持有 GPUI 的 `Window` 和 `App` 引用，供节点实现调用 GPUI API 构建界面。
pub struct NodeViewCtx<'a> {
    pub window: &'a mut Window,
    pub cx: &'a mut App,
    pub selected: bool,
}

/// 节点扩展接口（策略模式）。
///
/// 按 `kind` 匹配实现，提供：
/// - [`get_view`](Self::get_view)：画布上的节点卡片布局
/// - [`get_panel`](Self::get_panel)：选中时右侧属性面板布局
/// - [`schema`](Self::schema)：端口定义、默认尺寸
/// - [`resolve_port`](Self::resolve_port)：自定义端口位置（可选）
pub trait IFlowNode: Send + Sync {
    /// 节点 kind 标识，用于注册表匹配。
    fn kind(&self) -> &str;

    /// 节点卡片布局界面（画布上显示的节点主体）。
    fn get_view(&self, node: &Node, ctx: &mut NodeViewCtx) -> AnyElement;

    /// 选中时右侧属性面板布局界面。
    fn get_panel(&self, node: &Node, ctx: &mut NodeViewCtx) -> AnyElement;

    /// 节点 Schema（端口定义、默认尺寸等）。
    fn schema(&self) -> &NodeSchema;

    /// 自定义端口位置计算（可选）。
    ///
    /// 默认返回 `None`，表示用框架统一算法（[`resolve_endpoints`]）。
    /// 特殊节点（如条件分支）覆写此方法以精确控制端口位置。
    ///
    /// [`resolve_endpoints`]: rust_agent_flow::resolve_endpoints
    fn resolve_port(&self, _port: &PortId, _bounds: RectF, _ctx: &mut NodeViewCtx) -> Option<PointF> {
        None
    }
}
