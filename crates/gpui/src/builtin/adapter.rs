//! Data Adapter 节点：数据适配/转换，In + Out 端口，标题栏 + 主体结构。
//!
//! 主体显示适配描述或节点类型标签。
//! 属性面板支持配置适配规则（desc）。

use gpui::{div, px, AnyElement, IntoElement, ParentElement, Styled};
use gpui_component::{Icon, Sizable, StyledExt};
use rust_agent_flow::{
    FieldSpec, FieldType, LayoutDirection, Node, NodeSchema, PortDirection, PortId, PortSide,
    PortSpec, SizeF, PointF,
};

use crate::i18n::TKey;
use crate::node::{NodeViewCtx, IFlowNode};

use super::common::{desc_of, label_of_localized, make_port, node_icon, port_sizes, render_delete_button, TITLE_H, TITLE_ICON_SIZE};

/// 主体高度（逻辑坐标）。
const BODY_H: f32 = 28.0;

/// Data Adapter 节点：数据适配/转换，In + Out 端口。
pub struct AdapterNode {
    schema: NodeSchema,
}

impl Default for AdapterNode {
    fn default() -> Self {
        Self::new()
    }
}

impl AdapterNode {
    pub fn new() -> Self {
        Self {
            schema: NodeSchema::new("adapter", "Data Adapter")
                .with_size(SizeF::new(200.0, TITLE_H + BODY_H))
                .with_port(PortSpec::new("in", PortDirection::In, PortSide::Auto))
                .with_port(PortSpec::new("out", PortDirection::Out, PortSide::Auto))
                .with_field(
                    FieldSpec::new("label", "Label", FieldType::Text)
                        .with_default(serde_json::json!("")),
                )
                .with_field(
                    FieldSpec::new("desc", "Description", FieldType::Text)
                        .with_default(serde_json::json!("")),
                ),
        }
    }
}

impl IFlowNode for AdapterNode {
    fn kind(&self) -> &str {
        "adapter"
    }

    fn get_view(&self, node: &Node, ctx: &mut NodeViewCtx) -> AnyElement {
        let s = ctx.scale;
        let w = node.size.w * s;
        let h = (TITLE_H + BODY_H) * s;
        let title_h = TITLE_H * s;
        let body_h = BODY_H * s;
        let t = &ctx.theme;
        let layout = ctx.layout;
        let lang = ctx.language;

        let label = label_of_localized(node, lang);
        let desc = desc_of(node).unwrap_or_else(|| crate::i18n::t(lang, TKey::DataAdapter).to_string());

        let (port_size, port_outer, port_outer_half) = port_sizes(s);
        let border_color = if ctx.selected {
            t.node_border_selected
        } else {
            t.node_border
        };

        let mut container = div().relative().w(px(w)).h(px(h));

        // 标题栏
        container = container.child(
            div()
                .absolute()
                .top_0()
                .left_0()
                .w(px(w))
                .h(px(title_h))
                .bg(t.node_title_bg)
                .rounded_t_lg()
                .border_1()
                .border_color(border_color)
                .border_b_0()
                .flex()
                .items_center()
                .px(px(12.0 * s))
                .gap(px(6.0 * s))
                .child(
                    Icon::new(node_icon("adapter"))
                        .with_size(px(TITLE_ICON_SIZE * s))
                        .text_color(t.node_title_text),
                )
                .child(
                    div()
                        .text_size(px(14.0 * s))
                        .font_semibold()
                        .text_color(t.node_title_text)
                        .child(label),
                ),
        );

        // 主体
        container = container.child(
            div()
                .absolute()
                .left_0()
                .top(px(title_h))
                .w(px(w))
                .h(px(body_h))
                .bg(t.node_bg)
                .rounded_b_lg()
                .border_1()
                .border_color(border_color)
                .border_t_0()
                .flex()
                .items_center()
                .px(px(12.0 * s))
                .child(
                    div()
                        .text_size(px(12.0 * s))
                        .text_color(t.node_subtext)
                        .child(desc),
                ),
        );

        // 端口：横向布局按节点垂直居中（非标题栏居中）
        let mid_y_node = h * 0.5;
        match layout {
            LayoutDirection::Horizontal => {
                container = container.child(make_port(
                    -port_outer_half,
                    mid_y_node - port_outer_half,
                    port_outer,
                    port_size,
                    t.node_in_ring,
                    t.node_in_dot,
                    t.port_bg,
                ));
                container = container.child(make_port(
                    w - port_outer_half,
                    mid_y_node - port_outer_half,
                    port_outer,
                    port_size,
                    t.node_out_ring,
                    t.node_out_dot,
                    t.port_bg,
                ));
            }
            LayoutDirection::Vertical => {
                let mid_x = w * 0.5;
                container = container.child(make_port(
                    mid_x - port_outer_half,
                    -port_outer_half,
                    port_outer,
                    port_size,
                    t.node_in_ring,
                    t.node_in_dot,
                    t.port_bg,
                ));
                container = container.child(make_port(
                    mid_x - port_outer_half,
                    h - port_outer_half,
                    port_outer,
                    port_size,
                    t.node_out_ring,
                    t.node_out_dot,
                    t.port_bg,
                ));
            }
        }

        if ctx.hovered {
            container = container.child(render_delete_button(node.size.w, s, t));
        }

        container.into_any_element()
    }

    fn get_panel(&self, node: &Node, ctx: &mut NodeViewCtx) -> AnyElement {
        let _ = (node, ctx);
        div().into_any_element()
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
        let mid_x = node.position.x + node.size.w * 0.5;
        let node_mid_y = node.position.y + node.size.h * 0.5;
        let bottom = node.position.y + TITLE_H + BODY_H;

        match port_id.as_str() {
            "in" => match layout {
                LayoutDirection::Horizontal => Some(PointF::new(left, node_mid_y)),
                LayoutDirection::Vertical => Some(PointF::new(mid_x, top)),
            },
            "out" => match layout {
                LayoutDirection::Horizontal => Some(PointF::new(right, node_mid_y)),
                LayoutDirection::Vertical => Some(PointF::new(mid_x, bottom)),
            },
            _ => None,
        }
    }

    fn content_size(&self, node: &Node) -> SizeF {
        SizeF::new(node.size.w, TITLE_H + BODY_H)
    }
}
