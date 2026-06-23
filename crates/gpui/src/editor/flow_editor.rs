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
use rust_agent_flow::{EdgeType, FlowGraph, NodeId, PointF, PortId, PortSide, Viewport};
use rust_agent_flow::{
    LayoutDirection as CoreLayoutDirection, LayoutEngine, LayoutResult, DagreLayout,
};

use crate::node::{default_syntax_service, NodeAction, NodeRegistry, SharedSyntaxService};
use crate::panel::PanelView;
use crate::theme::Theme;

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
    /// 当前悬停的节点 ID（用于显示删除按钮等 hover 元素）。
    pub hovered: Option<NodeId>,
    /// 默认边类型（用于 DrawingEdge 临时连线 + 全局切换）。
    pub default_edge_type: EdgeType,
    /// 布局方向（决定边的端口侧：Horizontal=Right/Left, Vertical=Bottom/Top）。
    pub layout_direction: LayoutDirection,
    /// 是否显示点阵背景。
    pub show_grid: bool,
    /// 点阵背景逻辑间距（与节点坐标同一空间），控制点阵密度。
    /// 屏幕间距 = 逻辑间距 × scale，随缩放等比变化。
    pub grid_spacing: f32,
    /// 是否允许拖拽节点（false 时左键点击节点仅选中，不进入拖拽状态）。
    pub drag_enabled: bool,
    /// 当前主题颜色配置。
    pub theme: Theme,
    /// 属性面板视图实体（选中节点时创建，取消选中时销毁）。
    pub panel_view: Option<gpui::Entity<PanelView>>,
    /// 语法高亮服务（扩展点，默认 `DefaultSyntaxService` 将 rhai 映射到 rust 近似高亮）。
    pub syntax_service: SharedSyntaxService,
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
            hovered: None,
            default_edge_type: EdgeType::SmoothStep,
            layout_direction: LayoutDirection::Horizontal,
            show_grid: true,
            grid_spacing: super::grid::DEFAULT_GRID_SPACING,
            drag_enabled: true,
            theme: Theme::light(),
            panel_view: None,
            syntax_service: default_syntax_service(),
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
    /// 使用 [`DagreLayout`]（包装 `dagre` crate，ReactFlow 同款 Sugiyama 算法），
    /// 保持节点拓扑分层结构。切换方向时调用此方法即可重新排版。
    pub(crate) fn relayout(&mut self) {
        // 同步节点尺寸：确保 dagre 使用与实际渲染一致的尺寸（特别是
        // Condition 节点的高度随条件项数量变化）。
        self.sync_node_sizes();

        let dir = match self.layout_direction {
            LayoutDirection::Horizontal => CoreLayoutDirection::Horizontal,
            LayoutDirection::Vertical => CoreLayoutDirection::Vertical,
        };
        let result: LayoutResult = DagreLayout::new().layout(&self.graph, dir);
        for (node_id, pos) in result.positions {
            if let Some(node) = self.graph.node_mut(node_id) {
                node.position = pos;
            }
        }
    }

    /// 同步所有节点的 `size` 为实际渲染尺寸（`IFlowNode::content_size`）。
    ///
    /// 结构化节点（如 Condition）的渲染高度随数据变化，但 `node.size.h`
    /// 可能在创建后未更新。此方法在布局前调用，确保 dagre、命中测试、
    /// 回环边边界计算使用正确的尺寸。
    fn sync_node_sizes(&mut self) {
        let registry = self.registry.clone();
        let ids: Vec<NodeId> = self.graph.node_ids().collect();
        for id in ids {
            let new_size = {
                let node = match self.graph.node(id) {
                    Some(n) => n,
                    None => continue,
                };
                match registry.get(&node.kind) {
                    Some(f) => f.content_size(node),
                    None => continue,
                }
            };
            if let Some(node) = self.graph.node_mut(id) {
                node.size = new_size;
            }
        }
    }

    /// 自动排版：运行 dagre 布局引擎重新排列所有节点，并通知视图刷新。
    ///
    /// 公开 API，供外部（如 demo）在创建编辑器后触发自动排版。
    pub fn auto_layout(&mut self, cx: &mut Context<Self>) {
        self.relayout();
        cx.notify();
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

    /// 设置是否允许拖拽节点。
    pub fn set_drag_enabled(&mut self, enabled: bool, cx: &mut Context<Self>) {
        self.drag_enabled = enabled;
        cx.notify();
    }

    /// 设置点阵背景逻辑间距，控制点阵密度。值越小点越密。
    /// 屏幕间距随缩放等比变化（屏幕间距 = 逻辑间距 × scale）。
    pub fn set_grid_spacing(&mut self, spacing: f32, cx: &mut Context<Self>) {
        self.grid_spacing = spacing.max(8.0);
        cx.notify();
    }

    /// 设置是否显示点阵背景。
    pub fn set_show_grid(&mut self, show: bool, cx: &mut Context<Self>) {
        self.show_grid = show;
        cx.notify();
    }

    /// 切换主题（亮色 ↔ 暗色）。
    pub fn toggle_theme(&mut self, cx: &mut Context<Self>) {
        self.theme = self.theme.toggle();
        cx.notify();
    }

    /// 设置指定主题。
    pub fn set_theme(&mut self, theme: Theme, cx: &mut Context<Self>) {
        self.theme = theme;
        cx.notify();
    }

    /// 注入自定义语法高亮服务（扩展点）。
    ///
    /// 默认使用 [`DefaultSyntaxService`]（rhai → rust 近似高亮）。
    /// 外部 crate 可实现 [`SyntaxService`] trait 提供精确高亮，通过此方法注入。
    pub fn set_syntax_service(&mut self, service: SharedSyntaxService, cx: &mut Context<Self>) {
        self.syntax_service = service;
        // 销毁现有 panel_view，下次 render 时用新服务重建
        self.panel_view = None;
        cx.notify();
    }

    /// 处理节点动作（由 NodeView/PanelView 的回调调用）。
    pub(crate) fn handle_node_action(
        &mut self,
        node_id: NodeId,
        action: NodeAction,
        cx: &mut Context<Self>,
    ) {
        match action {
            NodeAction::Delete => self.delete_node(node_id, cx),
            NodeAction::ToggleCollapse => {
                if let Some(node) = self.graph.node_mut(node_id) {
                    let collapsed = node
                        .data
                        .get("collapsed")
                        .and_then(|v| v.as_bool())
                        .unwrap_or(false);
                    node.data["collapsed"] = serde_json::json!(!collapsed);
                }
                self.relayout();
                cx.notify();
            }
            NodeAction::SetData(key, value) => {
                if let Some(node) = self.graph.node_mut(node_id) {
                    node.data[key] = value;
                }
                self.sync_node_sizes();
                self.relayout();
                cx.notify();
            }
        }
    }

    /// 删除节点：线性桥接 + 级联删边 + 自动重排。
    ///
    /// 桥接策略（行业标准，参考 n8n/ReactFlow）：
    /// - 仅当节点恰好有 1 条入边和 1 条出边时，自动桥接前驱→后继
    /// - 多端口节点（条件/循环）删除时直接删除所有关联边，不做桥接
    pub(crate) fn delete_node(&mut self, node_id: NodeId, cx: &mut Context<Self>) {
        // 收集边信息（避免借用冲突）
        let in_edges: Vec<(NodeId, Option<PortId>, EdgeType)> = self
            .graph
            .in_edges(node_id)
            .map(|e| (e.source, e.source_port.clone(), e.edge_type))
            .collect();
        let out_edges: Vec<(NodeId, Option<PortId>)> = self
            .graph
            .out_edges(node_id)
            .map(|e| (e.target, e.target_port.clone()))
            .collect();

        // 线性桥接：1 入 1 出 → 创建桥接边
        if in_edges.len() == 1 && out_edges.len() == 1 {
            let (src, src_port, edge_type) = &in_edges[0];
            let (dst, dst_port) = &out_edges[0];
            let mut bridge = rust_agent_flow::Edge::new(*src, *dst);
            bridge.source_port = src_port.clone();
            bridge.target_port = dst_port.clone();
            bridge.edge_type = *edge_type;
            self.graph.add_edge(bridge);
        }

        // 删除节点（级联删除所有关联边）
        self.graph.remove_node(node_id);

        // 清理选中/悬停状态
        if self.selected == Some(node_id) {
            self.selected = None;
        }
        if self.hovered == Some(node_id) {
            self.hovered = None;
        }

        // 自动重排
        self.relayout();
        cx.notify();
    }
}

impl Render for FlowEditorView {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let entity = cx.entity();
        let edges = self.render_edges();
        let nodes = self.render_nodes(entity.clone());
        let panel = self.ensure_panel_view(entity, window, cx);
        let toolbar = self.render_toolbar(cx);

        let offset = self.viewport.offset;

        // ====== 外层容器：全屏，处理事件 ======
        // 光标：平移中 → grabbing（ClosedHand），空闲 → grab（OpenHand）
        let is_panning = matches!(self.interaction, InteractionState::Panning { .. });
        let mut container = div()
            .size_full()
            .relative()
            .bg(self.theme.canvas_bg)
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
        if let Some(panel_view) = panel {
            container = container.child(
                div()
                    .absolute()
                    .right_0()
                    .top_0()
                    .bottom_0()
                    .child(panel_view),
            );
        }

        container
    }
}
