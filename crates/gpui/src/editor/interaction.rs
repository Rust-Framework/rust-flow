//! 交互状态机：覆盖平移、拖拽节点、绘制连线四种交互状态。

use rust_agent_flow::{NodeId, PortId, PointF};

/// 编辑器交互状态机。
///
/// 任意时刻只处于一种状态，鼠标事件根据当前状态分发处理。
#[derive(Debug, Clone)]
pub enum InteractionState {
    /// 空闲：无交互进行中。
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
}

impl Default for InteractionState {
    fn default() -> Self {
        Self::Idle
    }
}
