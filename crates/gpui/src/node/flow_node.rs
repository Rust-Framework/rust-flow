//! IFlowNode 节点扩展接口（策略模式）。
//!
//! 每个 [`NodeKind`] 对应一个 [`IFlowNode`] 实现，提供节点卡片视图和属性面板。
//! 普通节点只需实现 `kind`/`get_view`/`get_panel`/`schema`；
//! 特殊节点（如条件分支、循环）可覆写 `port_position` 精确控制端口位置。

use gpui::{AnyElement, App, Window};
use rust_agent_flow::{LayoutDirection, Node, NodeSchema, PortId, PointF};

/// 节点渲染上下文，提供给 [`IFlowNode`] 方法使用。
///
/// 持有 GPUI 的 `Window` 和 `App` 引用，供节点实现调用 GPUI API 构建界面。
pub struct NodeViewCtx<'a> {
    pub window: &'a mut Window,
    pub cx: &'a mut App,
    pub selected: bool,
    /// 当前视口缩放比例，节点内部元素应按此缩放。
    pub scale: f32,
    /// 当前布局方向（横向/纵向），节点渲染端口位置应与此一致。
    pub layout: LayoutDirection,
}

/// 节点扩展接口（策略模式）。
///
/// 按 `kind` 匹配实现，提供：
/// - [`get_view`](Self::get_view)：画布上的节点卡片布局
/// - [`get_panel`](Self::get_panel)：选中时右侧属性面板布局
/// - [`schema`](Self::schema)：端口定义、默认尺寸
/// - [`port_position`](Self::port_position)：自定义端口位置（可选）
pub trait IFlowNode: Send + Sync {
    /// 节点 kind 标识，用于注册表匹配。
    fn kind(&self) -> &str;

    /// 节点卡片布局界面（画布上显示的节点主体）。
    fn get_view(&self, node: &Node, ctx: &mut NodeViewCtx) -> AnyElement;

    /// 选中时右侧属性面板布局界面。
    fn get_panel(&self, node: &Node, ctx: &mut NodeViewCtx) -> AnyElement;

    /// 节点 Schema（端口定义、默认尺寸等）。
    fn schema(&self) -> &NodeSchema;

    /// 自定义端口位置计算（可选，不依赖渲染上下文）。
    ///
    /// 返回端口圆心在**逻辑坐标**（节点 position 为左上角原点的绝对坐标）下的位置。
    /// 默认返回 `None`，表示用框架统一算法（按 side 计算节点边缘中点）。
    ///
    /// 特殊节点（如条件分支的多出口、循环节点的循环回环端口）覆写此方法
    /// 以精确控制每个端口的位置，使连线端点与视觉端口对齐。
    ///
    /// **方向感知**：`layout` 参数指示当前布局方向（Horizontal/Vertical），
    /// 端口位置应根据方向调整。例如 Condition 节点的 In 端口在横向布局下
    /// 位于左侧，在纵向布局下位于顶部。
    ///
    /// 位置计算应基于 `node.position` + `node.size` + 端口在节点内的相对偏移。
    fn port_position(
        &self,
        _node: &Node,
        _port_id: &PortId,
        _layout: LayoutDirection,
    ) -> Option<PointF> {
        None
    }
}
