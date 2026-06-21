//! 渲染层：边、节点、属性面板的渲染方法。
//!
//! - **边**：canvas paint，逻辑坐标 + PathBuilder 变换到屏幕空间
//! - **节点**：absolute div 在内容层内，`pos × scale` 定位
//! - **面板**：右侧浮动，不受缩放影响

use std::sync::Arc;

use gpui::{canvas, div, px, IntoElement, ParentElement, Point, Styled};
use rust_agent_flow::{EdgeType, PointF, PortSide};

use crate::edge::paint_edge_scaled;
use crate::node::{IFlowNode, NodeView};
use crate::panel::PanelView;

use super::flow_editor::{FlowEditorView, LayoutDirection};
use super::grid::paint_grid;
use super::interaction::InteractionState;
use super::ports::{edge_endpoints, resolve_port};

impl FlowEditorView {
    /// 当前视口缩放比例。
    pub(crate) fn scale(&self) -> f32 {
        self.viewport.scale
    }

    /// 渲染所有边（canvas paint），使用**逻辑坐标** + PathBuilder 变换。
    ///
    /// 边端点通过 [`edge_endpoints`] 计算：
    /// - 有 source_port/target_port → 使用 `IFlowNode::port_position` 自定义位置
    /// - 无 port_id → 回退到 side-based 计算
    pub(crate) fn render_edges(&self) -> impl IntoElement {
        let s = self.scale();
        let (src_side_default, dst_side_default) = self.port_sides();
        let layout = self.layout_direction;
        let registry = self.registry.clone();

        // 收集边端点（逻辑坐标 + side）
        let edges: Vec<(PointF, PointF, PortSide, PortSide, EdgeType)> = self
            .graph
            .edges()
            .map(|edge| {
                let (src, src_side, dst, dst_side) = edge_endpoints(
                    edge,
                    &self.graph,
                    &registry,
                    layout,
                    src_side_default,
                    dst_side_default,
                );
                (src, dst, src_side, dst_side, edge.edge_type)
            })
            .collect();

        let default_edge_type = self.default_edge_type;
        let drawing = match &self.interaction {
            InteractionState::DrawingEdge {
                from_node,
                from_port,
                current,
                ..
            } => self.graph.node(*from_node).map(|n| {
                // 绘制中的边：源端口用实际 port_id，目标用 current 位置
                let (src, src_side) =
                    resolve_port(n, from_port, &registry, layout);
                let dst = *current;
                (src, dst, src_side, dst_side_default, default_edge_type)
            }),
            _ => None,
        };

        let offset_x = self.viewport.offset.x;
        let offset_y = self.viewport.offset.y;
        let show_grid = self.show_grid;

        canvas(
            |bounds, _window, _cx| bounds.size,
            move |bounds, _size, window, _cx| {
                // 总偏移 = viewport.offset + canvas 在窗口中的绝对位置
                let total_offset = Point::new(
                    px(offset_x + bounds.origin.x.as_f32()),
                    px(offset_y + bounds.origin.y.as_f32()),
                );
                // 点阵背景（在边之前绘制，确保边在网格之上）
                if show_grid {
                    paint_grid(bounds, s, total_offset, window);
                }
                for (src, dst, src_side, dst_side, edge_type) in &edges {
                    paint_edge_scaled(
                        *src, *dst, *src_side, *dst_side, *edge_type, s, total_offset, window,
                    );
                }
                if let Some((src, dst, src_side, dst_side, edge_type)) = drawing {
                    paint_edge_scaled(
                        src, dst, src_side, dst_side, edge_type, s, total_offset, window,
                    );
                }
            },
        )
    }

    /// 渲染所有节点（absolute div 在内容层内）。
    ///
    /// 节点最终屏幕坐标 = content_offset + logical_pos × scale
    pub(crate) fn render_nodes(&self) -> Vec<gpui::AnyElement> {
        let selected = self.selected;
        let registry = &self.registry;
        let s = self.scale();
        let layout = match self.layout_direction {
            LayoutDirection::Horizontal => rust_agent_flow::LayoutDirection::Horizontal,
            LayoutDirection::Vertical => rust_agent_flow::LayoutDirection::Vertical,
        };

        self.graph
            .nodes()
            .map(|node| {
                let node_id = node.id;
                let pos = node.position;
                let flow_node = registry.get(&node.kind);
                let is_selected = selected == Some(node_id);

                let view = NodeView::new(node.clone())
                    .with_flow_node_opt(flow_node)
                    .selected(is_selected)
                    .with_scale(s)
                    .with_layout(layout);

                div()
                    .absolute()
                    .left(px(pos.x * s))
                    .top(px(pos.y * s))
                    .child(view)
                    .into_any_element()
            })
            .collect()
    }

    /// 渲染属性面板。
    pub(crate) fn render_panel(&self) -> Option<gpui::AnyElement> {
        let node = self.selected.and_then(|id| self.graph.node(id).cloned())?;
        let flow_node = self.registry.get(&node.kind);
        let panel = PanelView::new(node).with_flow_node_opt(flow_node);
        Some(
            div()
                .absolute()
                .right_0()
                .top_0()
                .bottom_0()
                .child(panel)
                .into_any_element(),
        )
    }
}

// ---- 视图扩展（仅在渲染层使用） ----

impl NodeView {
    pub(crate) fn with_flow_node_opt(mut self, flow_node: Option<Arc<dyn IFlowNode>>) -> Self {
        self.flow_node = flow_node;
        self
    }
}

impl PanelView {
    pub(crate) fn with_flow_node_opt(mut self, flow_node: Option<Arc<dyn IFlowNode>>) -> Self {
        self.flow_node = flow_node;
        self
    }
}
