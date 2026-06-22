//! Loop 节点：循环体，结构化布局。
//!
//! **设计语义**（以纵向布局为主要标准）：
//! - **纵向**：In 顶 / Done 底（主线），LoopBody 右 / LoopIn 左（循环体支线）
//! - **横向**：In 左 / Done 右（主线），LoopBody 右 / LoopIn 左（循环体支线，固定）
//!
//! **外部回环连线**：从 LoopBody 出口 → 外部循环体节点 → 回连到 LoopIn 入口，
//! 循环体支线在两个方向上均从右侧出、向左绕回左侧入。
//!
//! **Done 出口连线上的 Join 标记**：done → next 的连线上，距目标节点 80 单位处
//! 渲染一个小方块汇聚标记，表示循环结束后主线在此汇聚继续。
//!
//! **布局**（纵向，主要标准）：
//! ```text
//!              In
//!               ↓
//! ┌────────────────────────────┐
//! │         ⟳ Loop             │  标题栏 h=36
//! ├────────────────────────────┤
//! │      For each item          │  循环条件区域 h=44
//! └──┬─────────────────────┬───┘
//!    │ LoopIn        LoopBody│
//!    ↑                      ↓ Done
//! ```
//!
//! **布局**（横向）：
//! ```text
//! In ─▶│ ┌────────────────────────────┐ │◀── Done
//!       │ │         ⟳ Loop             │ │
//!       │ ├────────────────────────────┤ │
//!       │ │      For each item          │ │
//!       │ └──┬─────────────────────┬───┘ │
//!       │    │ LoopIn        LoopBody│   │
//!       │    ↑                      ↓    │
//!       └────┘ (向下绕圈回环) ┌────────┘┘
//! ```

use gpui::{div, px, AnyElement, IntoElement, ParentElement, Styled};
use gpui_component::StyledExt;
use rust_agent_flow::{
    LayoutDirection, Node, NodeSchema, PointF, PortDirection, PortId, PortSide, PortSpec, SizeF,
};

use crate::node::{NodeViewCtx, IFlowNode};

use super::common::{desc_of, label_of, make_port, port_sizes, render_simple_panel};

/// 标题栏高度（逻辑坐标）。
const TITLE_H: f32 = 36.0;

/// Loop 节点：循环体，结构化布局。
///
/// - 纵向：标题栏 In(顶) + Done(底)，循环条件区 LoopBody(右) + LoopIn(左)
/// - 横向：标题栏 In(左) + Done(右)，循环条件区 LoopBody(右) + LoopIn(左)（固定）
pub struct LoopNode {
    schema: NodeSchema,
}

impl Default for LoopNode {
    fn default() -> Self {
        Self::new()
    }
}

impl LoopNode {
    pub fn new() -> Self {
        Self {
            schema: NodeSchema::new("loop", "Loop")
                .with_size(SizeF::new(220.0, 80.0)) // 36 + 44
                // 所有端口 side 设为 Auto，实际 side 由 port_position 位置自动推导
                // 主线 In→Done：纵向→上/下，横向→左/右（随方向变化）
                // 循环体支线 LoopBody/LoopIn：两个方向均为 右/左（固定，向下绕圈回环）
                .with_port(PortSpec::new("in", PortDirection::In, PortSide::Auto))
                .with_port(PortSpec::new("done", PortDirection::Out, PortSide::Auto))
                .with_port(PortSpec::new("loop_body", PortDirection::Out, PortSide::Auto))
                .with_port(PortSpec::new("loop_in", PortDirection::In, PortSide::Auto)),
        }
    }
}

impl IFlowNode for LoopNode {
    fn kind(&self) -> &str {
        "loop"
    }

    fn get_view(&self, node: &Node, ctx: &mut NodeViewCtx) -> AnyElement {
        let s = ctx.scale;
        let w = node.size.w * s;
        let h = node.size.h * s;
        let title_h = TITLE_H * s;
        let body_h = (node.size.h - TITLE_H) * s;
        let layout = ctx.layout;

        let label = label_of(node);
        let desc = desc_of(node).unwrap_or_else(|| "For each item".to_string());

        let (port_size, port_outer, port_outer_half) = port_sizes(s);
        let border_color = if ctx.selected {
            gpui::rgb(0x3b82f6)
        } else {
            gpui::rgb(0x93c5fd)
        };

        // 外层容器
        let mut container = div().relative().w(px(w)).h(px(h));

        // 标题栏（蓝色背景）
        container = container.child(
            div()
                .absolute()
                .top_0()
                .left_0()
                .w(px(w))
                .h(px(title_h))
                .bg(gpui::rgb(0x3b82f6))
                .flex()
                .items_center()
                .justify_center()
                .child(
                    div()
                        .text_size(px(14.0 * s))
                        .font_semibold()
                        .text_color(gpui::rgb(0xffffff))
                        .child(label),
                ),
        );

        // 循环条件区域（浅蓝背景）
        container = container.child(
            div()
                .absolute()
                .left_0()
                .top(px(title_h))
                .w(px(w))
                .h(px(body_h))
                .bg(gpui::rgb(0xeff6ff))
                .border_t_1()
                .border_color(gpui::rgb(0xbfdbfe))
                .flex()
                .items_center()
                .justify_center()
                .child(
                    div()
                        .text_size(px(12.0 * s))
                        .text_color(gpui::rgb(0x1e3a8a))
                        .child(desc),
                ),
        );

        // 边框（覆盖整个节点，圆角）
        container = container.child(
            div()
                .absolute()
                .top_0()
                .left_0()
                .w(px(w))
                .h(px(h))
                .border_1()
                .border_color(border_color)
                .rounded_lg(),
        );

        // 端口位置常量（与 port_position 保持一致）
        let left = 0.0f32;
        let right = w;
        let top = 0.0f32;
        let bottom = h;
        let mid_x = w * 0.5;
        let title_mid_y = title_h * 0.5;
        let body_mid_y = title_h + body_h * 0.5;

        // 端口颜色
        let in_ring = gpui::rgb(0xbfdbfe);
        let in_dot = gpui::rgb(0x3b82f6);
        let done_ring = gpui::rgb(0xe2e8f0);
        let done_dot = gpui::rgb(0x64748b);
        let body_ring = gpui::rgb(0xbfdbfe);
        let body_dot = gpui::rgb(0x3b82f6);

        match layout {
            LayoutDirection::Horizontal => {
                // 主线：In 左 / Done 右（标题栏中心 Y）
                container = container.child(make_port(
                    left - port_outer_half,
                    title_mid_y - port_outer_half,
                    port_outer,
                    port_size,
                    in_ring,
                    in_dot,
                ));
                container = container.child(make_port(
                    right - port_outer_half,
                    title_mid_y - port_outer_half,
                    port_outer,
                    port_size,
                    done_ring,
                    done_dot,
                ));
                // 循环体支线：LoopBody 右 / LoopIn 左（循环条件区域中心 Y），向下绕圈回环
                container = container.child(make_port(
                    right - port_outer_half,
                    body_mid_y - port_outer_half,
                    port_outer,
                    port_size,
                    body_ring,
                    body_dot,
                ));
                container = container.child(make_port(
                    left - port_outer_half,
                    body_mid_y - port_outer_half,
                    port_outer,
                    port_size,
                    body_ring,
                    body_dot,
                ));
            }
            LayoutDirection::Vertical => {
                // 主线：In 顶 / Done 底（中心 X）
                container = container.child(make_port(
                    mid_x - port_outer_half,
                    top - port_outer_half,
                    port_outer,
                    port_size,
                    in_ring,
                    in_dot,
                ));
                container = container.child(make_port(
                    mid_x - port_outer_half,
                    bottom - port_outer_half,
                    port_outer,
                    port_size,
                    done_ring,
                    done_dot,
                ));
                // 循环体支线：LoopBody 右 / LoopIn 左（循环条件区域中心 Y），绕一圈回环
                container = container.child(make_port(
                    right - port_outer_half,
                    body_mid_y - port_outer_half,
                    port_outer,
                    port_size,
                    body_ring,
                    body_dot,
                ));
                container = container.child(make_port(
                    left - port_outer_half,
                    body_mid_y - port_outer_half,
                    port_outer,
                    port_size,
                    body_ring,
                    body_dot,
                ));
            }
        }

        container.into_any_element()
    }

    fn get_panel(&self, node: &Node, _ctx: &mut NodeViewCtx) -> AnyElement {
        render_simple_panel(node, "Loop 节点（循环）")
    }

    fn schema(&self) -> &NodeSchema {
        &self.schema
    }

    fn port_position(
        &self,
        node: &Node,
        port_id: &PortId,
        layout: LayoutDirection,
    ) -> Option<PointF> {
        let left = node.position.x;
        let right = node.position.x + node.size.w;
        let top = node.position.y;
        let bottom = node.position.y + node.size.h;
        let mid_x = node.position.x + node.size.w * 0.5;
        let title_mid_y = node.position.y + TITLE_H * 0.5;
        let body_mid_y = node.position.y + TITLE_H + (node.size.h - TITLE_H) * 0.5;

        match port_id.as_str() {
            // 主线 In→Done：纵向上进下出，横向左进右出
            "in" => match layout {
                LayoutDirection::Horizontal => Some(PointF::new(left, title_mid_y)),
                LayoutDirection::Vertical => Some(PointF::new(mid_x, top)),
            },
            "done" => match layout {
                LayoutDirection::Horizontal => Some(PointF::new(right, title_mid_y)),
                LayoutDirection::Vertical => Some(PointF::new(mid_x, bottom)),
            },
            // 循环体主支线：两个方向固定为右出左入，向下绕圈回环
            // 纵向：LoopBody 右 / LoopIn 左
            // 横向：LoopBody 右 / LoopIn 左（与纵向一致）
            "loop_body" => Some(PointF::new(right, body_mid_y)),
            "loop_in" => Some(PointF::new(left, body_mid_y)),
            _ => None,
        }
    }
}
