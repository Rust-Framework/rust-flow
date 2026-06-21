//! Loop 节点：循环体，结构化布局。
//!
//! **设计语义**：
//! - 标题栏左侧居中进 = 循环体开始前的总输入（In）
//! - 标题栏右侧居中出 = 循环结束/跳出后的继续出口（Done）
//! - 循环条件区域右侧居中出 = 循环体出口（LoopBody）
//! - 循环条件区域左侧居中进 = 循环回连入口（LoopIn）
//!
//! **外部回环连线**：从 LoopBody 出口 → 外部循环体节点 → 回连到 LoopIn 入口，
//! 清晰区分"循环内"与"循环后"的路径。
//!
//! **布局**（横向）：
//! ```text
//! ┌────────────────────────────────────┐
//! │  [In]   ⟳ Loop         [Done]      │  标题栏 h=36
//! ├────────────────────────────────────┤
//! │ [LoopIn]  For each item  [LoopBody] │  循环条件区域 h=44
//! └────────────────────────────────────┘
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
/// - 标题栏：In（主输入，左）+ Done（循环结束，右）
/// - 循环条件区域：LoopIn（循环回连，左）+ LoopBody（循环体出口，右）
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
                // 主线 In→Done：横向→左/右，纵向→上/下
                // 循环体支线 LoopBody/LoopIn：横向→上/下，纵向→右/左（垂直于主线）
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
                // 循环体支线：LoopBody 顶 / LoopIn 底（中心 X），绕一圈回环
                container = container.child(make_port(
                    mid_x - port_outer_half,
                    top - port_outer_half,
                    port_outer,
                    port_size,
                    body_ring,
                    body_dot,
                ));
                container = container.child(make_port(
                    mid_x - port_outer_half,
                    bottom - port_outer_half,
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
            // 主线 In→Done：横向左进右出，纵向上进下出
            "in" => match layout {
                LayoutDirection::Horizontal => Some(PointF::new(left, title_mid_y)),
                LayoutDirection::Vertical => Some(PointF::new(mid_x, top)),
            },
            "done" => match layout {
                LayoutDirection::Horizontal => Some(PointF::new(right, title_mid_y)),
                LayoutDirection::Vertical => Some(PointF::new(mid_x, bottom)),
            },
            // 循环体主支线：垂直于主线方向
            // 横向：LoopBody 顶 / LoopIn 底（上→下，绕一圈回环）
            // 纵向：LoopBody 右 / LoopIn 左（右→左，绕一圈回环）
            "loop_body" => match layout {
                LayoutDirection::Horizontal => Some(PointF::new(mid_x, top)),
                LayoutDirection::Vertical => Some(PointF::new(right, body_mid_y)),
            },
            "loop_in" => match layout {
                LayoutDirection::Horizontal => Some(PointF::new(mid_x, bottom)),
                LayoutDirection::Vertical => Some(PointF::new(left, body_mid_y)),
            },
            _ => None,
        }
    }
}
