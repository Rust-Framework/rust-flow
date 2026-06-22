//! Condition 节点：条件分支，结构化布局。
//!
//! **设计语义**：
//! - 标题栏：In 入口 + Else 兜底出口（横向 Else 在右 / 纵向 Else 不在标题栏）
//! - 条件项行：每个 If 条件对应一个出口
//!
//! **横向布局**：左进右出
//! - In 在标题栏左侧中心
//! - Else 在标题栏右侧中心（垂直居中）
//! - if_i 在每个条件项行的右侧中心
//! - 下一个节点左进右出（由 Auto side 自动推导）
//!
//! **纵向布局**：上进下出
//! - In 在标题栏顶部中心
//! - 所有分支出口（else, if_0, if_1, ...）由底部出去，沿宽度均匀分布，不重叠
//! - else 为最左侧出口端点
//! - 下一个节点上进下出（由 Auto side 自动推导）
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
//! **布局**（横向，左进右出）：
//! ```text
//! ┌───────────────────────────────┐
//! │[In]    ◆ Condition    [Else]→ │  标题栏 h=36
//! ├───────────────────────────────┤
//! │  If amount > 100       [if_0]→│  条件项 h=item_h
//! │  If user.is_admin      [if_1]→│  条件项 h=item_h
//! └───────────────────────────────┘
//! ```
//!
//! **布局**（纵向，上进下出）：
//! ```text
//!            In
//!             ↓
//!      ┌─Condition──┐
//!      │ if amount   │
//!      │ if user     │
//!      │ else        │
//!      └─┬──┬──┬────┘
//!        ↓  ↓  ↓     底部均布出口（else 最左, if_0, if_1），不重叠
//! ```

use gpui::{div, px, AnyElement, IntoElement, ParentElement, Styled};
use gpui_component::StyledExt;
use rust_agent_flow::{
    LayoutDirection, Node, NodeSchema, PointF, PortDirection, PortId, PortSide, PortSpec, SizeF,
};

use crate::node::{NodeViewCtx, IFlowNode};

use super::common::{label_of, make_port, port_sizes, render_simple_panel};

/// 标题栏高度（逻辑坐标）。
const TITLE_H: f32 = 36.0;

/// Condition 节点：条件分支，结构化布局。
///
/// - 横向：In 左 / Else 右（标题栏中心），if_i 右（条件行中心）
/// - 纵向：In 上 / 所有分支（else + if_i）下，沿底部均匀分布不重叠，else 最左
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
                .with_size(SizeF::new(220.0, 144.0)) // 36 + 36*3（2 条件 + 1 else，纵向布局）
                // 所有端口 side 设为 Auto，实际 side 由 port_position 位置自动推导
                .with_port(PortSpec::new("in", PortDirection::In, PortSide::Auto))
                .with_port(PortSpec::new("else", PortDirection::Out, PortSide::Auto))
                .with_port(PortSpec::new("if_0", PortDirection::Out, PortSide::Auto))
                .with_port(PortSpec::new("if_1", PortDirection::Out, PortSide::Auto)),
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

/// 分支总数 = 条件项数 + 1（else 兜底）。
fn n_branches(node: &Node) -> usize {
    get_conditions(node).len() + 1
}

/// 计算每个分支行的高度（逻辑坐标）。
///
/// - 横向：可用高度均分给条件项行（不含 else，else 在标题栏）
/// - 纵向：可用高度均分给所有分支行（含 else）
fn item_height(node: &Node, layout: LayoutDirection) -> f32 {
    let n_cond = get_conditions(node).len();
    let n = match layout {
        LayoutDirection::Horizontal => n_cond.max(1),
        LayoutDirection::Vertical => n_branches(node).max(1),
    };
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
        let conditions = get_conditions(node);
        let n_cond = conditions.len();
        let n_br = n_branches(node);
        let item_h = item_height(node, ctx.layout) * s;
        let label = label_of(node);
        let layout = ctx.layout;

        let (port_size, port_outer, port_outer_half) = port_sizes(s);
        let border_color = if ctx.selected {
            gpui::rgb(0xf97316)
        } else {
            gpui::rgb(0xfdba74)
        };

        // 端口颜色
        let in_ring = gpui::rgb(0xc7d2fe);
        let in_dot = gpui::rgb(0x6366f1);
        let if_ring = gpui::rgb(0xfde68a);
        let if_dot = gpui::rgb(0xf97316);
        let else_ring = gpui::rgb(0xe2e8f0);
        let else_dot = gpui::rgb(0x64748b);

        // 外层容器（不使用 overflow_hidden，避免裁剪半外露的端口圆圈）
        let mut container = div().relative().w(px(w)).h(px(h));

        // 标题栏（橙色背景，顶部圆角对齐容器圆角）
        container = container.child(
            div()
                .absolute()
                .top_0()
                .left_0()
                .w(px(w))
                .h(px(title_h))
                .bg(gpui::rgb(0xf97316))
                .rounded_t_lg()
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

        // 条件项行（浅橙背景）
        for (i, (_id, cond_label)) in conditions.iter().enumerate() {
            let item_top = title_h + item_h * i as f32;
            // 横向布局：最后一行条件项需要底部圆角（else 在标题栏，不占行）
            let is_last_row = matches!(layout, LayoutDirection::Horizontal)
                && i == n_cond - 1;
            let mut row = div()
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
                );
            if is_last_row {
                row = row.rounded_b_lg();
            }
            container = container.child(row);
        }

        // 纵向布局时渲染 Else 兜底行（横向时 Else 在标题栏，无需单独行）
        if matches!(layout, LayoutDirection::Vertical) {
            let else_top = title_h + item_h * n_cond as f32;
            container = container.child(
                div()
                    .absolute()
                    .left_0()
                    .top(px(else_top))
                    .w(px(w))
                    .h(px(item_h))
                    .bg(gpui::rgb(0xffedd5))
                    .border_t_1()
                    .border_color(gpui::rgb(0xfed7aa))
                    .rounded_b_lg()
                    .flex()
                    .items_center()
                    .px(px(12.0 * s))
                    .child(
                        div()
                            .text_size(px(12.0 * s))
                            .font_semibold()
                            .text_color(gpui::rgb(0x9a3412))
                            .child("Else"),
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

        // 端口位置（与 port_position 保持一致）
        let mid_x = w * 0.5;
        match layout {
            LayoutDirection::Horizontal => {
                // In 端口（标题栏左侧中心）— 靛蓝色
                container = container.child(make_port(
                    -port_outer_half,
                    title_h * 0.5 - port_outer_half,
                    port_outer,
                    port_size,
                    in_ring,
                    in_dot,
                ));

                // Else 端口（标题栏右侧中心）— 灰色（兜底）
                container = container.child(make_port(
                    w - port_outer_half,
                    title_h * 0.5 - port_outer_half,
                    port_outer,
                    port_size,
                    else_ring,
                    else_dot,
                ));

                // if_i 端口（条件项右侧中心）— 橙色
                for (i, _cond) in conditions.iter().enumerate() {
                    let port_y = title_h + item_h * (i as f32 + 0.5) - port_outer_half;
                    container = container.child(make_port(
                        w - port_outer_half,
                        port_y,
                        port_outer,
                        port_size,
                        if_ring,
                        if_dot,
                    ));
                }
            }
            LayoutDirection::Vertical => {
                // In 端口（标题栏顶部中心）— 靛蓝色
                container = container.child(make_port(
                    mid_x - port_outer_half,
                    -port_outer_half,
                    port_outer,
                    port_size,
                    in_ring,
                    in_dot,
                ));

                // 底部均匀分布出口：else 最左（index 0），if_i 依次向右
                // else 端口 — 灰色
                let else_t = 0.5 / n_br as f32;
                let else_x = w * else_t - port_outer_half;
                container = container.child(make_port(
                    else_x,
                    h - port_outer_half,
                    port_outer,
                    port_size,
                    else_ring,
                    else_dot,
                ));

                // if_i 端口 — 橙色
                for (i, _cond) in conditions.iter().enumerate() {
                    let t = (i as f32 + 1.5) / n_br as f32;
                    let port_x = w * t - port_outer_half;
                    container = container.child(make_port(
                        port_x,
                        h - port_outer_half,
                        port_outer,
                        port_size,
                        if_ring,
                        if_dot,
                    ));
                }
            }
        }

        container.into_any_element()
    }

    fn get_panel(&self, node: &Node, _ctx: &mut NodeViewCtx) -> AnyElement {
        render_simple_panel(node, "Condition 节点（条件分支）")
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
        let n_br = n_branches(node);
        let item_h = item_height(node, layout);
        let left = node.position.x;
        let right = node.position.x + node.size.w;
        let top = node.position.y;
        let bottom = node.position.y + node.size.h;
        let mid_x = node.position.x + node.size.w * 0.5;
        let title_mid_y = node.position.y + TITLE_H * 0.5;

        match port_id.as_str() {
            // In：横向左侧中心 / 纵向顶部中心
            "in" => match layout {
                LayoutDirection::Horizontal => Some(PointF::new(left, title_mid_y)),
                LayoutDirection::Vertical => Some(PointF::new(mid_x, top)),
            },
            // Else 兜底出口：
            // 横向 → 标题栏右侧中心（垂直居中）
            // 纵向 → 底部最左侧出口端点
            "else" => match layout {
                LayoutDirection::Horizontal => Some(PointF::new(right, title_mid_y)),
                LayoutDirection::Vertical => {
                    let t = 0.5 / n_br as f32;
                    let x = left + node.size.w * t;
                    Some(PointF::new(x, bottom))
                }
            },
            // if_i 条件出口：
            // 横向 → 条件行右侧中心
            // 纵向 → 底部均匀分布（else 之后，index = i + 1）
            pid if pid.starts_with("if_") => {
                let idx: usize = pid[3..].parse().ok()?;
                match layout {
                    LayoutDirection::Horizontal => {
                        let y = node.position.y + TITLE_H + item_h * (idx as f32 + 0.5);
                        Some(PointF::new(right, y))
                    }
                    LayoutDirection::Vertical => {
                        let t = (idx as f32 + 1.5) / n_br as f32;
                        let x = left + node.size.w * t;
                        Some(PointF::new(x, bottom))
                    }
                }
            }
            _ => None,
        }
    }
}
