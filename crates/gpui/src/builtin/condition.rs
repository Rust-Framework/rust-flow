//! Condition 节点：条件分支，结构化布局。
//!
//! **设计语义**：
//! - 标题栏左侧居中进 = 输入端口（IN）
//! - 标题栏右侧居中出 = 兜底/否则出口（Else）
//! - 条件项右侧垂直居中出 = 各个条件分支出口（If / Else If）
//!
//! **数据格式**：
//! ```json
//! {
//!   "label": "Check",
//!   "conditions": [
//!     { "id": "if_0", "label": "amount > 100" },
//!     { "id": "if_1", "label": "user.is_admin" }
//!   ]
//! }
//! ```
//!
//! **布局**（横向）：
//! ```text
//! ┌───────────────────────────────┐
//! │ [In]  ◆ Condition    [Else]   │  标题栏 h=36
//! ├───────────────────────────────┤
//! │  If amount > 100       [if_0] │  条件项 h=36
//! │  If user.is_admin      [if_1] │  条件项 h=36
//! └───────────────────────────────┘
//! ```

use gpui::{div, px, AnyElement, IntoElement, ParentElement, Styled};
use gpui_component::StyledExt;
use rust_agent_flow::{
    Node, NodeSchema, PointF, PortDirection, PortId, PortSide, PortSpec, SizeF,
};

use crate::node::{NodeViewCtx, IFlowNode};

use super::common::{label_of, make_port, port_sizes, render_simple_panel};

/// 标题栏高度（逻辑坐标）。
const TITLE_H: f32 = 36.0;

/// Condition 节点：条件分支，结构化布局。
///
/// - 标题栏：In（左）+ Else（右，兜底出口）
/// - 条件列表：每行一个 If 出口
pub struct ConditionNode {
    schema: NodeSchema,
}

impl Default for ConditionNode {
    fn default() -> Self {
        Self::new()
    }
}

impl ConditionNode {
    pub fn new() -> Self {
        Self {
            schema: NodeSchema::new("condition", "Condition")
                .with_size(SizeF::new(220.0, 108.0)) // 36 + 36*2
                .with_port(PortSpec::new("in", PortDirection::In, PortSide::Left))
                .with_port(PortSpec::new("else", PortDirection::Out, PortSide::Right))
                .with_port(PortSpec::new("if_0", PortDirection::Out, PortSide::Right))
                .with_port(PortSpec::new("if_1", PortDirection::Out, PortSide::Right)),
        }
    }
}

/// 从 node.data 解析条件项列表 `(id, label)`。
///
/// 缺省回退到 2 个默认条件项。
fn get_conditions(node: &Node) -> Vec<(String, String)> {
    node.data
        .get("conditions")
        .and_then(|v| v.as_array())
        .map(|arr| {
            arr.iter()
                .filter_map(|item| {
                    let id = item.get("id")?.as_str()?.to_string();
                    let label = item.get("label")?.as_str()?.to_string();
                    Some((id, label))
                })
                .collect()
        })
        .unwrap_or_else(|| {
            vec![
                ("if_0".to_string(), "condition 0".to_string()),
                ("if_1".to_string(), "condition 1".to_string()),
            ]
        })
}

/// 计算每个条件项的高度（逻辑坐标）。
fn item_height(node: &Node) -> f32 {
    let n = get_conditions(node).len().max(1);
    (node.size.h - TITLE_H) / n as f32
}

impl IFlowNode for ConditionNode {
    fn kind(&self) -> &str {
        "condition"
    }

    fn get_view(&self, node: &Node, ctx: &mut NodeViewCtx) -> AnyElement {
        let s = ctx.scale;
        let w = node.size.w * s;
        let h = node.size.h * s;
        let title_h = TITLE_H * s;
        let item_h = item_height(node) * s;
        let conditions = get_conditions(node);
        let label = label_of(node);

        let (port_size, port_outer, port_outer_half) = port_sizes(s);
        let border_color = if ctx.selected {
            gpui::rgb(0xf97316)
        } else {
            gpui::rgb(0xfdba74)
        };

        // 外层容器
        let mut container = div().relative().w(px(w)).h(px(h));

        // 标题栏（橙色背景）
        container = container.child(
            div()
                .absolute()
                .top_0()
                .left_0()
                .w(px(w))
                .h(px(title_h))
                .bg(gpui::rgb(0xf97316))
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

        // 条件项（浅橙背景）
        for (i, (_id, cond_label)) in conditions.iter().enumerate() {
            let item_top = title_h + item_h * i as f32;
            container = container.child(
                div()
                    .absolute()
                    .left_0()
                    .top(px(item_top))
                    .w(px(w))
                    .h(px(item_h))
                    .bg(gpui::rgb(0xfff7ed))
                    .border_t_1()
                    .border_color(gpui::rgb(0xfed7aa))
                    .flex()
                    .items_center()
                    .px(px(12.0 * s))
                    .child(
                        div()
                            .text_size(px(12.0 * s))
                            .text_color(gpui::rgb(0x9a3412))
                            .child(format!("If {}", cond_label)),
                    ),
            );
        }

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

        // 端口（在边框之后，绘制在边框之上）
        // In 端口（标题栏左侧中心）— 蓝色
        container = container.child(make_port(
            -port_outer_half,
            title_h * 0.5 - port_outer_half,
            port_outer,
            port_size,
            gpui::rgb(0xc7d2fe),
            gpui::rgb(0x6366f1),
        ));

        // Else 端口（标题栏右侧中心）— 灰色（兜底）
        container = container.child(make_port(
            w - port_outer_half,
            title_h * 0.5 - port_outer_half,
            port_outer,
            port_size,
            gpui::rgb(0xe2e8f0),
            gpui::rgb(0x64748b),
        ));

        // if_i 端口（条件项右侧中心）— 橙色
        for (i, _cond) in conditions.iter().enumerate() {
            let port_y = title_h + item_h * (i as f32 + 0.5) - port_outer_half;
            container = container.child(make_port(
                w - port_outer_half,
                port_y,
                port_outer,
                port_size,
                gpui::rgb(0xfde68a),
                gpui::rgb(0xf97316),
            ));
        }

        container.into_any_element()
    }

    fn get_panel(&self, node: &Node, _ctx: &mut NodeViewCtx) -> AnyElement {
        render_simple_panel(node, "Condition 节点（条件分支）")
    }

    fn schema(&self) -> &NodeSchema {
        &self.schema
    }

    fn port_position(&self, node: &Node, port_id: &PortId) -> Option<PointF> {
        let item_h = item_height(node);
        let left = node.position.x;
        let right = node.position.x + node.size.w;
        let title_mid_y = node.position.y + TITLE_H * 0.5;

        match port_id.as_str() {
            "in" => Some(PointF::new(left, title_mid_y)),
            "else" => Some(PointF::new(right, title_mid_y)),
            pid if pid.starts_with("if_") => {
                let idx: usize = pid[3..].parse().ok()?;
                let y = node.position.y + TITLE_H + item_h * (idx as f32 + 0.5);
                Some(PointF::new(right, y))
            }
            _ => None,
        }
    }
}
