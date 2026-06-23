//! Start 节点：流程起点，仅 Out 端口，标题栏 + 主体结构。

use gpui::{div, px, AnyElement, IntoElement, ParentElement, Styled};
use gpui_component::{Icon, Sizable, StyledExt};
use rust_agent_flow::{
    LayoutDirection, Node, NodeSchema, PortDirection, PortId, PortSide, PortSpec, SizeF, PointF,
};

use crate::node::{NodeViewCtx, IFlowNode};

use super::common::{label_of, make_port, node_icon, port_sizes, render_simple_panel, TITLE_ICON_SIZE};

/// 标题栏高度（逻辑坐标）。
const TITLE_H: f32 = 36.0;

/// 主体高度（逻辑坐标）。
const BODY_H: f32 = 20.0;

/// Start 节点：流程起点，仅 Out 端口。
pub struct StartNode {
    schema: NodeSchema,
}

impl Default for StartNode {
    fn default() -> Self {
        Self::new()
    }
}

impl StartNode {
    pub fn new() -> Self {
        Self {
            schema: NodeSchema::new("start", "Start")
                .with_size(SizeF::new(160.0, TITLE_H + BODY_H))
                .with_port(PortSpec::new("out", PortDirection::Out, PortSide::Auto)),
        }
    }
}

impl IFlowNode for StartNode {
    fn kind(&self) -> &str {
        "start"
    }

    fn get_view(&self, node: &Node, ctx: &mut NodeViewCtx) -> AnyElement {
        let s = ctx.scale;
        let w = node.size.w * s;
        let h = (TITLE_H + BODY_H) * s;
        let title_h = TITLE_H * s;
        let body_h = BODY_H * s;
        let t = &ctx.theme;
        let layout = ctx.layout;

        let label = label_of(node);

        let (port_size, port_outer, port_outer_half) = port_sizes(s);
        let border_color = if ctx.selected {
            t.start_border_selected
        } else {
            t.start_border
        };

        // 外层容器
        let mut container = div().relative().w(px(w)).h(px(h));

        // 标题栏（顶部圆角）：图标 + 标签
        container = container.child(
            div()
                .absolute()
                .top_0()
                .left_0()
                .w(px(w))
                .h(px(title_h))
                .bg(t.start_bg)
                .rounded_t_lg()
                .border_1()
                .border_color(border_color)
                .border_b_0()
                .flex()
                .items_center()
                .px(px(12.0 * s))
                .gap(px(6.0 * s))
                .child(
                    Icon::new(node_icon("start"))
                        .with_size(px(TITLE_ICON_SIZE * s))
                        .text_color(t.start_text),
                )
                .child(
                    div()
                        .text_size(px(14.0 * s))
                        .font_semibold()
                        .text_color(t.start_text)
                        .child(label),
                ),
        );

        // 主体（底部圆角）：提示文案
        container = container.child(
            div()
                .absolute()
                .left_0()
                .top(px(title_h))
                .w(px(w))
                .h(px(body_h))
                .bg(t.start_bg)
                .rounded_b_lg()
                .border_1()
                .border_color(border_color)
                .border_t_0()
                .flex()
                .items_center()
                .justify_center()
                .child(
                    div()
                        .text_size(px(11.0 * s))
                        .text_color(t.start_subtext)
                        .child("▶"),
                ),
        );

        // 端口：仅 Out
        let mid_y_title = title_h * 0.5;
        match layout {
            LayoutDirection::Horizontal => {
                // Out 端口（右侧中心）
                container = container.child(make_port(
                    w - port_outer_half,
                    mid_y_title - port_outer_half,
                    port_outer,
                    port_size,
                    t.node_out_ring,
                    t.start_out_dot,
                    t.port_bg,
                ));
            }
            LayoutDirection::Vertical => {
                let mid_x = w * 0.5;
                // Out 端口（底部中心）
                container = container.child(make_port(
                    mid_x - port_outer_half,
                    h - port_outer_half,
                    port_outer,
                    port_size,
                    t.node_out_ring,
                    t.start_out_dot,
                    t.port_bg,
                ));
            }
        }

        container.into_any_element()
    }

    fn get_panel(&self, node: &Node, ctx: &mut NodeViewCtx) -> AnyElement {
        render_simple_panel(node, "Start 节点", &ctx.theme)
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
        let right = node.position.x + node.size.w;
        let mid_x = node.position.x + node.size.w * 0.5;
        let title_mid_y = node.position.y + TITLE_H * 0.5;
        let bottom = node.position.y + TITLE_H + BODY_H;

        match port_id.as_str() {
            "out" => match layout {
                LayoutDirection::Horizontal => Some(PointF::new(right, title_mid_y)),
                LayoutDirection::Vertical => Some(PointF::new(mid_x, bottom)),
            },
            _ => None,
        }
    }

    fn content_size(&self, node: &Node) -> SizeF {
        SizeF::new(node.size.w, TITLE_H + BODY_H)
    }
}
