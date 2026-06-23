//! End 节点：流程终点，仅 In 端口，标题栏 + 主体结构。
//!
//! 主体显示「有返回」/「无返回」，取决于 `node.data["returns"]` 数组是否存在且非空。
//! 属性面板支持配置返回结果（name/type/value）。

use gpui::{div, px, AnyElement, IntoElement, ParentElement, Styled};
use gpui_component::{Icon, Sizable, StyledExt};
use rust_agent_flow::{
    FieldSpec, FieldType, LayoutDirection, ListSpec, Node, NodeSchema, PortDirection, PortId,
    PortSide, PortSpec, SizeF, PointF,
};

use crate::i18n::TKey;
use crate::node::{NodeViewCtx, IFlowNode};

use super::common::{label_of_localized, make_port, node_icon, port_sizes, TITLE_ICON_SIZE};

/// 标题栏高度（逻辑坐标）。
const TITLE_H: f32 = 36.0;

/// 主体高度（逻辑坐标）。
const BODY_H: f32 = 20.0;

/// 判断节点是否有返回结果。
fn has_returns(node: &Node) -> bool {
    node.data
        .get("returns")
        .and_then(|v| v.as_array())
        .map(|arr| !arr.is_empty())
        .unwrap_or(false)
}

/// End 节点：流程终点，仅 In 端口。
pub struct EndNode {
    schema: NodeSchema,
}

impl Default for EndNode {
    fn default() -> Self {
        Self::new()
    }
}

impl EndNode {
    pub fn new() -> Self {
        Self {
            schema: NodeSchema::new("end", "End")
                .with_size(SizeF::new(160.0, TITLE_H + BODY_H))
                .with_port(PortSpec::new("in", PortDirection::In, PortSide::Auto))
                .with_field(
                    FieldSpec::new("label", "Label", FieldType::Text)
                        .with_default(serde_json::json!("")),
                )
                .with_field(
                    FieldSpec::new(
                        "returns",
                        "Return Results",
                        FieldType::List(
                            ListSpec::new(vec![
                                FieldSpec::new("name", "Name", FieldType::Text)
                                    .with_default(serde_json::json!("")),
                                FieldSpec::new("type", "Type", FieldType::Text)
                                    .with_default(serde_json::json!("string")),
                            ]),
                        ),
                    )
                    .with_default(serde_json::json!([])),
                ),
        }
    }
}

impl IFlowNode for EndNode {
    fn kind(&self) -> &str {
        "end"
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
        let has_r = has_returns(node);
        let body_text = if has_r {
            crate::i18n::t(lang, TKey::EndHasReturn).to_string()
        } else {
            crate::i18n::t(lang, TKey::EndNoReturn).to_string()
        };

        let (port_size, port_outer, port_outer_half) = port_sizes(s);
        let border_color = if ctx.selected {
            t.node_border_selected
        } else {
            t.node_border
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
                    Icon::new(node_icon("end"))
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

        // 主体（底部圆角）：有返回/无返回提示
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
                .justify_center()
                .child(
                    div()
                        .text_size(px(11.0 * s))
                        .text_color(t.node_subtext)
                        .child(body_text),
                ),
        );

        // 端口：仅 In，横向布局按节点垂直居中（非标题栏居中）
        let mid_y_node = h * 0.5;
        match layout {
            LayoutDirection::Horizontal => {
                // In 端口（左侧中心）
                container = container.child(make_port(
                    -port_outer_half,
                    mid_y_node - port_outer_half,
                    port_outer,
                    port_size,
                    t.node_in_ring,
                    t.node_in_dot,
                    t.port_bg,
                ));
            }
            LayoutDirection::Vertical => {
                let mid_x = w * 0.5;
                // In 端口（顶部中心）
                container = container.child(make_port(
                    mid_x - port_outer_half,
                    -port_outer_half,
                    port_outer,
                    port_size,
                    t.node_in_ring,
                    t.node_in_dot,
                    t.port_bg,
                ));
            }
        }

        container.into_any_element()
    }

    fn get_panel(&self, node: &Node, ctx: &mut NodeViewCtx) -> AnyElement {
        // End 节点面板由 PanelView 处理（支持返回结果编辑）
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
        let mid_x = node.position.x + node.size.w * 0.5;
        let node_mid_y = node.position.y + node.size.h * 0.5;
        let top = node.position.y;

        match port_id.as_str() {
            "in" => match layout {
                LayoutDirection::Horizontal => Some(PointF::new(left, node_mid_y)),
                LayoutDirection::Vertical => Some(PointF::new(mid_x, top)),
            },
            _ => None,
        }
    }

    fn content_size(&self, node: &Node) -> SizeF {
        SizeF::new(node.size.w, TITLE_H + BODY_H)
    }
}
