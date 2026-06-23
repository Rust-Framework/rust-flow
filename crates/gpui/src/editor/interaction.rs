//! 交互层：交互状态机 + 鼠标事件处理。
//!
//! 交互采用命中测试方案：画布统一处理鼠标事件，用几何命中测试确定点击的
//! 节点/端口，避免在每个节点 div 上绑定闭包（GPUI 的 listener 闭包无法
//! 捕获外部变量如 node_id）。

use gpui::{
    px, Context, MouseButton, MouseDownEvent, MouseMoveEvent, MouseUpEvent, ScrollWheelEvent,
    Window,
};
use rust_agent_flow::{NodeId, PortId, PointF};

use crate::node::NodeAction;

use super::flow_editor::FlowEditorView;
use super::hit_test::HitResult;
use super::viewport;

/// 编辑器交互状态机。
///
/// 任意时刻只处于一种状态，鼠标事件根据当前状态分发处理。
#[derive(Debug, Clone, Default)]
pub enum InteractionState {
    /// 空闲：无交互进行中。
    #[default]
    Idle,
    /// 平移视口：记录鼠标起点（**屏幕坐标**）和视口 offset 起点。
    ///
    /// 使用屏幕坐标而非逻辑坐标，避免平移过程中 viewport.offset 变化
    /// 导致的逻辑坐标反馈抖动（参考 ReactFlow 成熟方案）。
    Panning {
        start_screen: PointF,
        origin: PointF,
    },
    /// 拖拽节点：记录节点 id、鼠标起点（逻辑坐标）、节点 position 起点。
    DraggingNode {
        node_id: NodeId,
        start: PointF,
        node_origin: PointF,
    },
    /// 绘制连线：记录起点节点/端口，current 为当前鼠标位置（逻辑坐标）。
    DrawingEdge {
        from_node: NodeId,
        from_port: PortId,
        current: PointF,
    },
    /// 点击边「+」按钮后：等待用户在浮层中选择节点类型。
    /// `anchor` 为点击时的屏幕坐标，用于浮层定位。
    AddingNodeFromEdge {
        edge_id: rust_agent_flow::EdgeId,
        anchor: PointF,
    },
}

impl FlowEditorView {
    pub(crate) fn on_mouse_down(
        &mut self,
        event: &MouseDownEvent,
        _window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let logical = self.to_logical(event.position);

        match (event.button, self.hit_test(logical)) {
            (MouseButton::Middle, _) => {
                // 中键拖拽：屏幕坐标起点 + 视口 offset 起点
                let start_screen = PointF::new(
                    event.position.x.as_f32(),
                    event.position.y.as_f32(),
                );
                self.interaction = InteractionState::Panning {
                    start_screen,
                    origin: self.viewport.offset,
                };
            }
            (MouseButton::Left, HitResult::DeleteButton(node_id)) => {
                // 点击删除按钮：删除节点（含线性桥接 + 自动重排）
                self.delete_node(node_id, cx);
            }
            (MouseButton::Left, HitResult::ToggleButton(node_id)) => {
                // 点击切换按钮：切换展开/收起状态
                self.handle_node_action(node_id, NodeAction::ToggleCollapse, cx);
            }
            (MouseButton::Left, HitResult::EdgePlusButton(edge_id)) => {
                // 点击边「+」按钮：进入 AddingNodeFromEdge 状态，显示节点选择浮层
                let anchor = PointF::new(
                    event.position.x.as_f32(),
                    event.position.y.as_f32(),
                );
                self.interaction = InteractionState::AddingNodeFromEdge {
                    edge_id,
                    anchor,
                };
            }
            (MouseButton::Left, HitResult::OutPort(node_id, port)) => {
                self.interaction = InteractionState::DrawingEdge {
                    from_node: node_id,
                    from_port: port,
                    current: logical,
                };
            }
            (MouseButton::Left, HitResult::Node(node_id)) => {
                // 始终选中被点击的节点
                self.selected = Some(node_id);
                if self.drag_enabled {
                    // 允许拖拽：进入 DraggingNode 状态
                    let node_origin =
                        self.graph.node(node_id).map(|n| n.position).unwrap_or_default();
                    self.interaction = InteractionState::DraggingNode {
                        node_id,
                        start: logical,
                        node_origin,
                    };
                }
                // 拖拽禁用时仅选中节点，不进入拖拽状态（Idle）
            }
            (MouseButton::Left, HitResult::InPort(_, _)) => {
                // 点击入端口：暂不处理（可作为连线目标）
            }
            (MouseButton::Left, HitResult::Empty) => {
                // 点击空白：若当前在 AddingNodeFromEdge 状态，仅退出浮层（不平移）
                if matches!(self.interaction, InteractionState::AddingNodeFromEdge { .. }) {
                    self.interaction = InteractionState::Idle;
                    cx.notify();
                    return;
                }
                // 否则：左键拖拽空白区域 → 平移画布（屏幕坐标起点）
                let start_screen = PointF::new(
                    event.position.x.as_f32(),
                    event.position.y.as_f32(),
                );
                self.selected = None;
                self.interaction = InteractionState::Panning {
                    start_screen,
                    origin: self.viewport.offset,
                };
            }
            _ => {}
        }
        cx.notify();
    }

    pub(crate) fn on_mouse_move(
        &mut self,
        event: &MouseMoveEvent,
        _window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        // 预计算屏幕 + 逻辑坐标，避免 match &mut self.interaction 时借用冲突。
        let screen = PointF::new(
            event.position.x.as_f32(),
            event.position.y.as_f32(),
        );
        let logical = self.to_logical(event.position);

        match &mut self.interaction {
            InteractionState::Panning { start_screen, origin } => {
                // 纯屏幕空间 delta 平移（ReactFlow 成熟方案）：
                // new_offset = origin + (current_screen - start_screen)
                // 避免逻辑坐标反馈抖动，1:1 跟随鼠标。
                self.viewport.offset =
                    viewport::handle_pan(*origin, *start_screen, screen);
                cx.notify();
            }
            InteractionState::DraggingNode {
                node_id,
                start,
                node_origin,
            } => {
                let node_id = *node_id;
                let start = *start;
                let node_origin = *node_origin;
                if let Some(node) = self.graph.node_mut(node_id) {
                    node.position = PointF::new(
                        node_origin.x + (logical.x - start.x),
                        node_origin.y + (logical.y - start.y),
                    );
                }
                cx.notify();
            }
            InteractionState::DrawingEdge { current: cur, .. } => {
                *cur = logical;
                cx.notify();
            }
            InteractionState::AddingNodeFromEdge { .. } => {
                // 浮层显示期间不追踪悬停，保持浮层稳定
            }
            InteractionState::Idle => {
                // 悬停追踪：hit test → 更新 hovered → 通知视图刷新
                // 用于显示/隐藏删除按钮等 hover 元素
                let hit = self.hit_test(logical);
                let new_hovered = match &hit {
                    HitResult::Node(id)
                    | HitResult::DeleteButton(id)
                    | HitResult::ToggleButton(id) => Some(*id),
                    HitResult::OutPort(id, _) | HitResult::InPort(id, _) => Some(*id),
                    HitResult::EdgePlusButton(_) | HitResult::Empty => None,
                };
                if new_hovered != self.hovered {
                    self.hovered = new_hovered;
                    cx.notify();
                }
            }
        }
    }

    pub(crate) fn on_mouse_up(
        &mut self,
        event: &MouseUpEvent,
        _window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let logical = self.to_logical(event.position);

        match &self.interaction {
            InteractionState::DrawingEdge { from_node, from_port, .. } => {
                let from_node = *from_node;
                let from_port = from_port.clone();
                // 命中测试目标入端口
                if let HitResult::InPort(to_node, to_port) = self.hit_test(logical) {
                    if from_node != to_node {
                        let mut edge = rust_agent_flow::Edge::new(from_node, to_node);
                        edge.source_port = Some(from_port);
                        edge.target_port = Some(to_port);
                        self.graph.add_edge(edge);
                    }
                }
                self.interaction = InteractionState::Idle;
            }
            InteractionState::DraggingNode { .. } | InteractionState::Panning { .. } => {
                self.interaction = InteractionState::Idle;
            }
            InteractionState::AddingNodeFromEdge { .. } | InteractionState::Idle => {}
        }
        cx.notify();
    }

    pub(crate) fn on_scroll(
        &mut self,
        event: &ScrollWheelEvent,
        _window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let mouse_logical = self.to_logical(event.position);
        // GPUI 滚轮 delta：向上为负（放大），向下为正（缩小）
        let delta = event.delta.pixel_delta(px(20.0)).y.as_f32();
        self.viewport = viewport::handle_zoom(self.viewport, mouse_logical, delta);
        cx.notify();
    }
}
