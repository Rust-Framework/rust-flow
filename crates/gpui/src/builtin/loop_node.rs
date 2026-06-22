//! Loop 节点：循环体，结构化布局。
//!
//! **设计语义**（以纵向布局为主要标准）：
//! - **纵向**：In 顶 / Done 底（主线），LoopBody 右 / LoopIn 左（循环体支线）
//! - **横向**：In 左 / Done 右（主线），LoopBody 右 / LoopIn 左（循环体支线，固定）
//!
//! **外部回环连线**：从 LoopBody 出口 → 外部循环体节点 → 回连到 LoopIn 入口，
//! 循环体支线在两个方向上均从右侧出、向左绕回左侧入。
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

/// 循环条件区域高度（逻辑坐标，固定值，不随节点尺寸拉伸）。
///
/// 节点总高度 = `TITLE_H + BODY_H`（由内容推导，非输入）。
const BODY_H: f32 = 44.0;

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
                .with_size(SizeF::new(220.0, TITLE_H + BODY_H))
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
        // 使用固定高度，而非 node.size.h（保证内部布局独立性）
        let h = (TITLE_H + BODY_H) * s;
        let title_h = TITLE_H * s;
        let body_h = BODY_H * s;
        let layout = ctx.layout;
        let t = &ctx.theme;

        let label = label_of(node);
        let desc = desc_of(node).unwrap_or_else(|| "For each item".to_string());

        let (port_size, port_outer, port_outer_half) = port_sizes(s);
        let border_color = if ctx.selected {
            t.loop_border_selected
        } else {
            t.loop_border
        };

        // 外层容器（不使用 overflow_hidden，避免裁剪半外露的端口圆圈）
        let mut container = div().relative().w(px(w)).h(px(h));

        // 标题栏（蓝色背景，顶部圆角对齐容器圆角）
        container = container.child(
            div()
                .absolute()
                .top_0()
                .left_0()
                .w(px(w))
                .h(px(title_h))
                .bg(t.loop_title_bg)
                .rounded_t_lg()
                .flex()
                .items_center()
                .justify_center()
                .child(
                    div()
                        .text_size(px(14.0 * s))
                        .font_semibold()
                        .text_color(t.loop_title_text)
                        .child(label),
                ),
        );

        // 循环条件区域（浅蓝背景，底部圆角对齐容器圆角）
        container = container.child(
            div()
                .absolute()
                .left_0()
                .top(px(title_h))
                .w(px(w))
                .h(px(body_h))
                .bg(t.loop_body_bg)
                .border_t_1()
                .border_color(t.loop_body_border)
                .rounded_b_lg()
                .flex()
                .items_center()
                .justify_center()
                .child(
                    div()
                        .text_size(px(12.0 * s))
                        .text_color(t.loop_body_text)
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
        let in_ring = t.loop_in_ring;
        let in_dot = t.loop_in_dot;
        let done_ring = t.loop_done_ring;
        let done_dot = t.loop_done_dot;
        let body_ring = t.loop_in_ring;
        let body_dot = t.loop_in_dot;
        let port_bg = t.port_bg;

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
                    port_bg,
                ));
                container = container.child(make_port(
                    right - port_outer_half,
                    title_mid_y - port_outer_half,
                    port_outer,
                    port_size,
                    done_ring,
                    done_dot,
                    port_bg,
                ));
                // 循环体支线：LoopBody 右 / LoopIn 左（循环条件区域中心 Y），向下绕圈回环
                container = container.child(make_port(
                    right - port_outer_half,
                    body_mid_y - port_outer_half,
                    port_outer,
                    port_size,
                    body_ring,
                    body_dot,
                    port_bg,
                ));
                container = container.child(make_port(
                    left - port_outer_half,
                    body_mid_y - port_outer_half,
                    port_outer,
                    port_size,
                    body_ring,
                    body_dot,
                    port_bg,
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
                    port_bg,
                ));
                container = container.child(make_port(
                    mid_x - port_outer_half,
                    bottom - port_outer_half,
                    port_outer,
                    port_size,
                    done_ring,
                    done_dot,
                    port_bg,
                ));
                // 循环体支线：LoopBody 底部偏左（w*0.25），LoopIn 左侧
                // LoopBody 与 Done 都在底部，错开位置避免重叠
                let body_port_x = w * 0.25;
                container = container.child(make_port(
                    body_port_x - port_outer_half,
                    bottom - port_outer_half,
                    port_outer,
                    port_size,
                    body_ring,
                    body_dot,
                    port_bg,
                ));
                container = container.child(make_port(
                    left - port_outer_half,
                    body_mid_y - port_outer_half,
                    port_outer,
                    port_size,
                    body_ring,
                    body_dot,
                    port_bg,
                ));
            }
        }

        container.into_any_element()
    }

    fn get_panel(&self, node: &Node, ctx: &mut NodeViewCtx) -> AnyElement {
        render_simple_panel(node, "Loop 节点（循环）", &ctx.theme)
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
        // 使用固定高度，保证端口位置与实际渲染高度一致
        let bottom = node.position.y + TITLE_H + BODY_H;
        let mid_x = node.position.x + node.size.w * 0.5;
        let title_mid_y = node.position.y + TITLE_H * 0.5;
        let body_mid_y = node.position.y + TITLE_H + BODY_H * 0.5;

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
            // 循环体支线：
            // - 纵向：LoopBody 底部偏左（w*0.25），与 Done（mid_x）错开；LoopIn 左侧入
            // - 横向：LoopBody 右侧出；LoopIn 左侧入（回环边从下绕回）
            "loop_body" => match layout {
                LayoutDirection::Horizontal => Some(PointF::new(right, body_mid_y)),
                LayoutDirection::Vertical => {
                    Some(PointF::new(node.position.x + node.size.w * 0.25, bottom))
                }
            },
            "loop_in" => Some(PointF::new(left, body_mid_y)),
            _ => None,
        }
    }
}
