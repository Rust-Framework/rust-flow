//! IFlowNode 节点扩展接口（策略模式）。
//!
//! 每个 [`NodeKind`] 对应一个 [`IFlowNode`] 实现，提供节点卡片视图和属性面板。
//! 普通节点只需实现 `kind`/`get_view`/`get_panel`/`schema`；
//! 特殊节点（如条件分支、循环）可覆写 `port_position` 精确控制端口位置。

use std::sync::Arc;

use gpui::{AnyElement, App, Window};
use rust_agent_flow::{LayoutDirection, Node, NodeSchema, PortId, PortSpec, PointF, SizeF};

use crate::i18n::Language;
use crate::theme::Theme;

/// 节点动作：节点视图/属性面板向编辑器发出的操作请求。
///
/// 通过 [`NodeViewCtx::on_action`] 回调传递，闭包已捕获 `node_id`，
/// 调用方无需传入节点 ID。
#[derive(Clone, Debug)]
pub enum NodeAction {
    /// 删除此节点。
    Delete,
    /// 切换展开/收起状态。
    ToggleCollapse,
    /// 更新 `node.data[key] = value`。
    SetData(String, serde_json::Value),
}

/// 动作回调类型：闭包已捕获 `node_id`，接收动作 + `&mut App`。
pub type ActionCallback = Arc<dyn Fn(NodeAction, &mut App) + Send + Sync>;

/// 节点渲染上下文，提供给 [`IFlowNode`] 方法使用。
///
/// 持有 GPUI 的 `Window` 和 `App` 引用，供节点实现调用 GPUI API 构建界面。
/// `theme` 提供当前主题颜色，节点渲染应从中取色以支持主题切换。
pub struct NodeViewCtx<'a> {
    pub window: &'a mut Window,
    pub cx: &'a mut App,
    pub selected: bool,
    /// 当前节点是否被鼠标悬停（用于显示删除按钮等 hover 元素）。
    pub hovered: bool,
    /// 当前视口缩放比例，节点内部元素应按此缩放。
    pub scale: f32,
    /// 当前布局方向（横向/纵向），节点渲染端口位置应与此一致。
    pub layout: LayoutDirection,
    /// 当前主题颜色配置。
    pub theme: Theme,
    /// 当前语言（中英文切换）。
    pub language: Language,
    /// 动作回调：节点视图/面板通过此回调向编辑器发送动作。
    /// 闭包已捕获 `node_id`，调用方无需传入。
    pub on_action: Option<ActionCallback>,
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

    /// 返回节点实例的端口列表（可选，支持动态端口）。
    ///
    /// 默认返回 `self.schema().ports.clone()`。
    ///
    /// 特殊节点（如 Condition）可覆写此方法，根据 `node.data` 动态生成端口列表。
    /// 例如 Condition 节点的 if_0, if_1, ... 端口数量随 conditions 数组变化。
    fn ports_for_node(&self, node: &Node) -> Vec<PortSpec> {
        let _ = node;
        self.schema().ports.clone()
    }

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

    /// 节点由内容推导的实际渲染尺寸（可选）。
    ///
    /// 返回值用于同步 `node.size`，确保 dagre 布局、命中测试、回环边边界
    /// 计算使用与实际渲染一致的尺寸。
    ///
    /// **默认**：返回 `node.size`（适用于尺寸固定的节点，如 Action/Start/End/Loop）。
    ///
    /// **结构化节点应覆写此方法**：当节点数据（如 Condition 的 conditions 数量）
    /// 影响实际渲染高度时，返回基于数据推导的尺寸。例如 Condition 节点的
    /// 高度 = `TITLE_H + ITEM_H * n_branches`，随条件项数量变化。
    ///
    /// 宽度通常保持 `node.size.w`（由 schema default_size 或创建时指定），
    /// 仅高度需要根据数据推导。
    fn content_size(&self, node: &Node) -> SizeF {
        node.size
    }

    /// 边「+」按钮是否应放置在目标节点一侧（而非默认的源节点一侧）。
    ///
    /// **默认**：`false`（按钮在源节点出口附近）。
    ///
    /// 某些结构化节点的特定出口端口位置特殊，按钮放在源端会与节点其他端口
    /// 或回环边视觉冲突。此类节点可覆写此方法，按 `source_port` 判断是否
    /// 将按钮放到目标端。
    ///
    /// `source_port` 为边的源端口 ID（如 `"done"`、`"loop_body"`），`None` 表示
    /// 无显式端口（使用默认端口）。
    fn plus_button_at_target(&self, _source_port: Option<&str>) -> bool {
        false
    }
}
