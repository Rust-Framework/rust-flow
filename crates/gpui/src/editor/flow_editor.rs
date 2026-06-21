//! FlowEditorView：流程编辑器主视图。
//!
//! 实现 GPUI `Render`，持有图模型 + 视口 + 交互状态 + 节点注册表。
//!
//! 交互采用命中测试方案：画布统一处理鼠标事件，用几何命中测试确定点击的
//! 节点/端口，避免在每个节点 div 上绑定闭包（GPUI 的 listener 闭包无法
//! 捕获外部变量如 node_id）。
//!
//! 缩放方案：
//! - **节点**：逐元素手动缩放（`pos * scale`、`size * scale`），因 GPUI
//!   的 div 不支持 CSS transform-scale。
//! - **边**：在逻辑坐标中计算路径几何（含 step gap、smoothstep 圆角），
//!   通过 `PathBuilder::scale` + `translate` 统一变换到屏幕空间。线宽
//!   手动乘以 `scale`。这样所有几何参数随缩放等比变化，避免错位。
//!
//! 本文件仅包含核心结构体定义、构造、布局方法、坐标转换和 Render 实现。
//! 其他逻辑按职责拆分到同目录下的子模块：
//! - [`super::interaction`]：交互状态机 + 鼠标事件处理
//! - [`super::hit_test`]：命中测试
//! - [`super::rendering`]：边/节点/面板渲染
//! - [`super::toolbar`]：工具栏
//! - [`super::grid`]：点阵背景
//! - [`super::ports`]：端口位置计算
//! - [`super::viewport`]：视口数学映射

use std::sync::Arc;

use gpui::{
    div, px, Context, CursorStyle, InteractiveElement, IntoElement, MouseButton, ParentElement,
    Pixels, Point, Render, Styled, Window,
};
use rust_agent_flow::{EdgeType, FlowGraph, NodeId, PointF, PortSide, Viewport};
use rust_agent_flow::{
    LayoutDirection as CoreLayoutDirection, LayoutEngine, LayoutResult, SimpleLayout,
};

use crate::node::NodeRegistry;

use super::interaction::InteractionState;

/// 布局方向。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LayoutDirection {
    Horizontal,
    Vertical,
}

/// 流程编辑器主视图。
pub struct FlowEditorView {
    pub graph: FlowGraph,
    pub viewport: Viewport,
    pub interaction: InteractionState,
    pub registry: Arc<NodeRegistry>,
    pub selected: Option<NodeId>,
    /// 默认边类型（用于 DrawingEdge 临时连线 + 全局切换）。
    pub default_edge_type: EdgeType,
    /// 布局方向（决定边的端口侧：Horizontal=Right/Left, Vertical=Bottom/Top）。
    pub layout_direction: LayoutDirection,
    /// 是否显示点阵背景。
    pub show_grid: bool,
}

impl FlowEditorView {
    pub fn new(graph: FlowGraph, _cx: &mut Context<Self>) -> Self {
        let mut registry = NodeRegistry::new();
        crate::builtin::register_all(&mut registry);
        Self {
            graph,
            viewport: Viewport::default(),
            interaction: InteractionState::default(),
            registry: Arc::new(registry),
            selected: None,
            default_edge_type: EdgeType::SmoothStep,
            layout_direction: LayoutDirection::Vertical,
            show_grid: true,
        }
    }

    /// 屏幕坐标（GPUI Point<Pixels>）→ 逻辑坐标（PointF）。
    pub(crate) fn to_logical(&self, p: Point<Pixels>) -> PointF {
        self.viewport.to_logical(PointF::new(p.x.as_f32(), p.y.as_f32()))
    }

    /// 根据布局方向返回 (源端口侧, 目标端口侧)。
    pub(crate) fn port_sides(&self) -> (PortSide, PortSide) {
        match self.layout_direction {
            LayoutDirection::Horizontal => (PortSide::Right, PortSide::Left),
            LayoutDirection::Vertical => (PortSide::Bottom, PortSide::Top),
        }
    }

    /// 运行布局引擎，按当前布局方向重新排列所有节点位置。
    ///
    /// 使用内置 [`SimpleLayout`]（无外部依赖），保持节点拓扑分层结构。
    /// 切换方向时调用此方法即可重新排版。
    pub(crate) fn relayout(&mut self) {
        let dir = match self.layout_direction {
            LayoutDirection::Horizontal => CoreLayoutDirection::Horizontal,
            LayoutDirection::Vertical => CoreLayoutDirection::Vertical,
        };
        let result: LayoutResult = SimpleLayout::new().layout(&self.graph, dir);
        for (node_id, pos) in result.positions {
            if let Some(node) = self.graph.node_mut(node_id) {
                node.position = pos;
            }
        }
    }

    /// 切换布局方向并重新排版节点位置。
    pub(crate) fn set_layout_direction(&mut self, dir: LayoutDirection, cx: &mut Context<Self>) {
        if self.layout_direction == dir {
            return;
        }
        self.layout_direction = dir;
        self.relayout();
        cx.notify();
    }
}

impl Render for FlowEditorView {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let edges = self.render_edges();
        let nodes = self.render_nodes();
        let panel = self.render_panel();
        let toolbar = self.render_toolbar(cx);

        let offset = self.viewport.offset;

        // ====== 外层容器：全屏，处理事件 ======
        // 光标：平移中 → grabbing（ClosedHand），空闲 → grab（OpenHand）
        let is_panning = matches!(self.interaction, InteractionState::Panning { .. });
        let mut container = div()
            .size_full()
            .relative()
            .bg(gpui::rgb(0xf8fafc))
            .overflow_hidden()
            .cursor(if is_panning {
                CursorStyle::ClosedHand
            } else {
                CursorStyle::OpenHand
            })
            .on_mouse_down(MouseButton::Left, cx.listener(Self::on_mouse_down))
            .on_mouse_down(MouseButton::Middle, cx.listener(Self::on_mouse_down))
            .on_mouse_move(cx.listener(Self::on_mouse_move))
            .on_mouse_up(MouseButton::Left, cx.listener(Self::on_mouse_up))
            .on_mouse_up(MouseButton::Middle, cx.listener(Self::on_mouse_up))
            .on_scroll_wheel(cx.listener(Self::on_scroll));

        // ====== 边（Canvas）：直接放在容器根层级 ======
        // 边在逻辑坐标中计算路径，paint 时通过 PathBuilder::scale + translate
        // 变换到屏幕空间。translate = viewport.offset + canvas bounds.origin，
        // 确保与节点（通过 div offset + pos×scale 定位）的屏幕坐标一致。
        container = container.child(edges);

        // ====== 内容层：仅包含节点，通过 offset + scale 定位 ======
        // 节点最终屏幕坐标 = container_origin + offset + logical_pos × scale
        let mut content = div()
            .absolute()
            .left(px(offset.x))
            .top(px(offset.y));

        for node_el in nodes {
            content = content.child(node_el);
        }

        container = container.child(content);

        // ====== 工具栏：不受缩放影响 ======
        container = container.child(toolbar);

        // ====== 属性面板：不受缩放影响 ======
        if let Some(panel_el) = panel {
            container = container.child(panel_el);
        }

        container
    }
}
