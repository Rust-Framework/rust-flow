//! Condition 节点：条件分支，结构化布局。
//!
//! **设计语义**：
//! - 标题栏：In 入口（仅输入，无输出）
//! - 条件项行：if_0, if_1, ..., else（else 为最后一行兜底）
//!
//! **横向布局**：左进右出
//! - In 在标题栏左侧中心
//! - if_i 在每个条件项行的右侧中心
//! - else 在最后一行（兜底）的右侧中心
//!
//! **纵向布局**：上进下出
//! - In 在标题栏顶部中心
//! - 所有分支出口（if_0, if_1, ..., else）由底部出去，沿宽度均匀分布，不重叠
//! - else 为最右侧出口端点（兜底，排在所有 if 之后）
//!
//! **内部布局独立性**：
//! - 条件项行高固定为 `ITEM_H`（不随节点尺寸拉伸）
//! - 节点总高度 = `TITLE_H + ITEM_H * n_branches`（由内容推导，非输入）
//! - 端口位置基于固定 `ITEM_H` 计算，与 `node.size.h` 无关
//! - 排版引擎（dagre）通过 `nodesep`/`ranksep` + 节点尺寸自然完成分支对齐
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
//! │[In]        ◆ Condition        │  标题栏 h=TITLE_H
//! ├───────────────────────────────┤
//! │  If amount > 100       [if_0]→│  条件项 h=ITEM_H
//! │  If user.is_admin      [if_1]→│  条件项 h=ITEM_H
//! │  Else                  [else]→│  兜底行 h=ITEM_H
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
//!        ↓  ↓  ↓     底部均布出口（if_0, if_1, else 最右），不重叠
//! ```

use gpui::{div, px, AnyElement, IntoElement, ParentElement, Styled};
use gpui_component::{Icon, Sizable, StyledExt};
use rust_agent_flow::{
    FieldSpec, FieldType, LayoutDirection, ListSpec, Node, NodeSchema, PointF, PortDirection,
    PortId, PortSide, PortSpec, SizeF,
};

use crate::i18n::TKey;
use crate::node::{NodeViewCtx, IFlowNode};

use super::common::{
    label_of_localized, make_port, node_icon, port_sizes, render_delete_button, render_simple_panel,
    render_toggle_button, TITLE_H, TITLE_ICON_SIZE,
};

/// 条件项行高（逻辑坐标，固定值，不随节点尺寸拉伸）。
///
/// 这是节点内部布局的基本单位：所有条件项行、端口位置均基于此常量计算，
/// 与 `node.size.h` 无关。节点总高度由内容推导：`TITLE_H + ITEM_H * n_branches`。
const ITEM_H: f32 = 36.0;

/// 判断节点是否处于收起状态。
fn is_collapsed(node: &Node) -> bool {
    node.data
        .get("collapsed")
        .and_then(|v| v.as_bool())
        .unwrap_or(false)
}

/// Condition 节点：条件分支，结构化布局。
///
/// - 横向：In 左（标题栏），if_i 右（条件行），else 右（最后一行兜底）
/// - 纵向：In 上（标题栏），所有分支（if_i + else）下，沿底部均匀分布，else 最右
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
        // 默认 2 条件 + 1 else：TITLE_H + ITEM_H * 3 = 36 + 108 = 144
        Self {
            schema: NodeSchema::new("condition", "Condition")
                .with_size(SizeF::new(220.0, TITLE_H + ITEM_H * 3.0))
                // 所有端口 side 设为 Auto，实际 side 由 port_position 位置自动推导
                .with_port(PortSpec::new("in", PortDirection::In, PortSide::Auto))
                .with_port(PortSpec::new("else", PortDirection::Out, PortSide::Auto))
                .with_port(PortSpec::new("if_0", PortDirection::Out, PortSide::Auto))
                .with_port(PortSpec::new("if_1", PortDirection::Out, PortSide::Auto))
                .with_field(
                    FieldSpec::new("label", "Label", FieldType::Text)
                        .with_default(serde_json::json!("")),
                )
                .with_field(
                    FieldSpec::new(
                        "conditions",
                        "Conditions",
                        FieldType::List(
                            ListSpec::new(vec![
                                FieldSpec::new("id", "ID", FieldType::Text)
                                    .with_default(serde_json::json!("")),
                                FieldSpec::new("label", "Expression", FieldType::CodeEditor)
                                    .with_default(serde_json::json!("")),
                            ]),
                        ),
                    )
                    .with_default(serde_json::json!([
                        { "id": "if_0", "label": "" },
                        { "id": "if_1", "label": "" }
                    ])),
                ),
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
                ("if_0".to_string(), String::new()),
                ("if_1".to_string(), String::new()),
            ]
        })
}

/// 分支总数 = 条件项数 + 1（else 兜底）。
fn n_branches(node: &Node) -> usize {
    get_conditions(node).len() + 1
}

/// 节点由内容推导的高度。
///
/// 横向和纵向一致：`TITLE_H + ITEM_H * n_branches`（else 占一行）。
fn content_height(node: &Node) -> f32 {
    TITLE_H + ITEM_H * n_branches(node) as f32
}

impl IFlowNode for ConditionNode {
    fn kind(&self) -> &str {
        "condition"
    }

    fn get_view(&self, node: &Node, ctx: &mut NodeViewCtx) -> AnyElement {
        let s = ctx.scale;
        let w = node.size.w * s;
        let collapsed = is_collapsed(node);
        let title_h = TITLE_H * s;
        let item_h = ITEM_H * s;
        let conditions = get_conditions(node);
        let n_cond = conditions.len();
        // n_br = n_cond + 1（else 兜底），直接推导避免重复调用 get_conditions
        let n_br = n_cond + 1;
        // 收起状态高度 = TITLE_H + ITEM_H（标题栏 + 主体，跟其他节点一样规格）
        // 展开状态高度 = TITLE_H + ITEM_H * n_br（标题栏 + 条件项行 + else 行）
        let h = if collapsed {
            (TITLE_H + ITEM_H) * s
        } else {
            (TITLE_H + ITEM_H * n_br as f32) * s
        };
        let label = label_of_localized(node, ctx.language);
        let layout = ctx.layout;
        let t = &ctx.theme;

        let (port_size, port_outer, port_outer_half) = port_sizes(s);
        let border_color = if ctx.selected {
            t.node_border_selected
        } else {
            t.node_border
        };

        // 端口颜色
        let in_ring = t.node_in_ring;
        let in_dot = t.node_in_dot;
        let if_ring = t.node_out_ring;
        let if_dot = t.node_out_dot;
        let else_ring = t.node_out_ring;
        let else_dot = t.node_out_dot;

        // 外层容器（不使用 overflow_hidden，避免裁剪半外露的端口圆圈）
        let mut container = div().relative().w(px(w)).h(px(h));

        // ====== 收起状态：标题栏 + 主体（提示文案）+ In/Out 端口 ======
        // 收起时跟其他节点一样规格：标题栏 + 主体，单一出口
        if collapsed {
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
                    .flex()
                    .items_center()
                    .px(px(12.0 * s))
                    .gap(px(6.0 * s))
                    .child(
                        Icon::new(node_icon("condition"))
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

            // 主体（提示文案，底部圆角）— 显示条件数量
            let hint = format!("{} {}", n_cond, crate::i18n::t(ctx.language, TKey::ConditionsCount));
            container = container.child(
                div()
                    .absolute()
                    .left_0()
                    .top(px(title_h))
                    .w(px(w))
                    .h(px(item_h))
                    .bg(t.node_bg)
                    .border_t_1()
                    .border_color(t.node_border)
                    .rounded_b_lg()
                    .flex()
                    .items_center()
                    .justify_center()
                    .child(
                        div()
                            .text_size(px(12.0 * s))
                            .text_color(t.node_subtext)
                            .child(hint),
                    ),
            );

            // 边框
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

            // 端口：In 入口 + 单一 Out 出口（使用 else 端口位置）
            let mid_x = w * 0.5;
            let node_mid_y = h * 0.5;
            match layout {
                LayoutDirection::Horizontal => {
                    // In 端口（节点左侧垂直中心）
                    container = container.child(make_port(
                        -port_outer_half,
                        node_mid_y - port_outer_half,
                        port_outer,
                        port_size,
                        in_ring,
                        in_dot,
                        t.port_bg,
                    ));
                    // Out 端口（节点右侧垂直中心）— 使用 else 颜色
                    container = container.child(make_port(
                        w - port_outer_half,
                        node_mid_y - port_outer_half,
                        port_outer,
                        port_size,
                        else_ring,
                        else_dot,
                        t.port_bg,
                    ));
                }
                LayoutDirection::Vertical => {
                    // In 端口（标题栏顶部中心）
                    container = container.child(make_port(
                        mid_x - port_outer_half,
                        -port_outer_half,
                        port_outer,
                        port_size,
                        in_ring,
                        in_dot,
                        t.port_bg,
                    ));
                    // Out 端口（主体底部中心）— 使用 else 颜色
                    container = container.child(make_port(
                        mid_x - port_outer_half,
                        h - port_outer_half,
                        port_outer,
                        port_size,
                        else_ring,
                        else_dot,
                        t.port_bg,
                    ));
                }
            }

            // toggle 按钮（▷，点击展开）
            container = container.child(render_toggle_button(node.size.w, s, true, t));
            // delete 按钮（hover 时显示）
            if ctx.hovered {
                container = container.child(render_delete_button(node.size.w, s, t));
            }
            return container.into_any_element();
        }

        // ====== 展开状态：标题栏 + 条件项行 + else 行 + 各端口 ======
        // 标题栏（橙色背景，顶部圆角对齐容器圆角）：图标 + 标签 — 仅 In 入口，无出口
        container = container.child(
            div()
                .absolute()
                .top_0()
                .left_0()
                .w(px(w))
                .h(px(title_h))
                .bg(t.node_title_bg)
                .rounded_t_lg()
                .flex()
                .items_center()
                .px(px(12.0 * s))
                .gap(px(6.0 * s))
                .child(
                    Icon::new(node_icon("condition"))
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

        // 条件项行（浅橙背景）— if_0, if_1, ...
        for (i, (_id, cond_label)) in conditions.iter().enumerate() {
            let item_top = title_h + item_h * i as f32;
            let cond_text = if cond_label.is_empty() {
                format!(
                    "{} ({})",
                    crate::i18n::t(ctx.language, TKey::If),
                    crate::i18n::t(ctx.language, TKey::ConditionExprPlaceholder)
                )
            } else {
                format!("{} {}", crate::i18n::t(ctx.language, TKey::If), cond_label)
            };
            let row = div()
                .absolute()
                .left_0()
                .top(px(item_top))
                .w(px(w))
                .h(px(item_h))
                .bg(t.node_bg)
                .border_t_1()
                .border_color(t.node_border)
                .flex()
                .items_center()
                .px(px(12.0 * s))
                .child(
                    div()
                        .text_size(px(12.0 * s))
                        .text_color(t.node_subtext)
                        .child(cond_text),
                );
            container = container.child(row);
        }

        // Else 兜底行（最后一行，浅橙偏暖背景，底部圆角）
        let else_top = title_h + item_h * n_cond as f32;
        container = container.child(
            div()
                .absolute()
                .left_0()
                .top(px(else_top))
                .w(px(w))
                .h(px(item_h))
                .bg(t.node_bg)
                .border_t_1()
                .border_color(t.node_border)
                .rounded_b_lg()
                .flex()
                .items_center()
                .px(px(12.0 * s))
                .child(
                    div()
                        .text_size(px(12.0 * s))
                        .font_semibold()
                        .text_color(t.node_subtext)
                        .child(crate::i18n::t(ctx.language, TKey::Else).to_string()),
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

        // 端口位置（与 port_position 保持一致）
        let mid_x = w * 0.5;
        let node_mid_y = h * 0.5;
        match layout {
            LayoutDirection::Horizontal => {
                // In 端口（节点左侧垂直中心）— 靛蓝色
                container = container.child(make_port(
                    -port_outer_half,
                    node_mid_y - port_outer_half,
                    port_outer,
                    port_size,
                    in_ring,
                    in_dot,
                    t.port_bg,
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
                        t.port_bg,
                    ));
                }

                // else 端口（最后一行右侧中心）— 灰色（兜底）
                let else_port_y = title_h + item_h * (n_cond as f32 + 0.5) - port_outer_half;
                container = container.child(make_port(
                    w - port_outer_half,
                    else_port_y,
                    port_outer,
                    port_size,
                    else_ring,
                    else_dot,
                    t.port_bg,
                ));
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
                    t.port_bg,
                ));

                // 底部均匀分布出口：if_0 最左，..., else 最右
                // if_i 端口 — 橙色
                for (i, _cond) in conditions.iter().enumerate() {
                    let if_t = (i as f32 + 0.5) / n_br as f32;
                    let port_x = w * if_t - port_outer_half;
                    container = container.child(make_port(
                        port_x,
                        h - port_outer_half,
                        port_outer,
                        port_size,
                        if_ring,
                        if_dot,
                        t.port_bg,
                    ));
                }

                // else 端口（最右）— 灰色（兜底）
                let else_t = (n_cond as f32 + 0.5) / n_br as f32;
                let else_x = w * else_t - port_outer_half;
                container = container.child(make_port(
                    else_x,
                    h - port_outer_half,
                    port_outer,
                    port_size,
                    else_ring,
                    else_dot,
                    t.port_bg,
                ));
            }
        }

        // toggle 按钮（▽，点击收起）
        container = container.child(render_toggle_button(node.size.w, s, false, t));
        // hover 时叠加删除按钮
        if ctx.hovered {
            container = container.child(render_delete_button(node.size.w, s, t));
        }

        container.into_any_element()
    }

    fn get_panel(&self, node: &Node, ctx: &mut NodeViewCtx) -> AnyElement {
        render_simple_panel(node, ctx.language, &ctx.theme)
    }

    fn schema(&self) -> &NodeSchema {
        &self.schema
    }

    /// 根据 node.data["conditions"] 动态生成端口列表。
    ///
    /// 端口列表 = in + else + if_0, if_1, ...（按 conditions 数组长度）
    fn ports_for_node(&self, node: &Node) -> Vec<rust_agent_flow::PortSpec> {
        let conditions = get_conditions(node);
        let mut ports = vec![
            PortSpec::new("in", PortDirection::In, PortSide::Auto),
            PortSpec::new("else", PortDirection::Out, PortSide::Auto),
        ];
        for (id, _) in &conditions {
            ports.push(PortSpec::new(
                id.as_str(),
                PortDirection::Out,
                PortSide::Auto,
            ));
        }
        ports
    }

    fn port_position(
        &self,
        node: &Node,
        port_id: &PortId,
        layout: LayoutDirection,
    ) -> Option<(PointF, PortSide)> {
        let collapsed = is_collapsed(node);
        let left = node.position.x;
        let right = node.position.x + node.size.w;
        let top = node.position.y;
        let mid_x = node.position.x + node.size.w * 0.5;
        let node_mid_y = node.position.y + node.size.h * 0.5;

        // ====== 收起状态：In 入口 + 单一 Out 出口（else 位置） ======
        if collapsed {
            let bottom = node.position.y + TITLE_H + ITEM_H;
            return match port_id.as_str() {
                "in" => match layout {
                    LayoutDirection::Horizontal => Some((PointF::new(left, node_mid_y), PortSide::Left)),
                    LayoutDirection::Vertical => Some((PointF::new(mid_x, top), PortSide::Top)),
                },
                _ => match layout {
                    LayoutDirection::Horizontal => Some((PointF::new(right, node_mid_y), PortSide::Right)),
                    LayoutDirection::Vertical => Some((PointF::new(mid_x, bottom), PortSide::Bottom)),
                },
            };
        }

        // ====== 展开状态 ======
        let n_cond = get_conditions(node).len();
        let n_br = n_cond + 1;
        let bottom = node.position.y + TITLE_H + ITEM_H * n_br as f32;

        match port_id.as_str() {
            "in" => match layout {
                LayoutDirection::Horizontal => Some((PointF::new(left, node_mid_y), PortSide::Left)),
                LayoutDirection::Vertical => Some((PointF::new(mid_x, top), PortSide::Top)),
            },
            "else" => match layout {
                LayoutDirection::Horizontal => {
                    let y = node.position.y + TITLE_H + ITEM_H * (n_cond as f32 + 0.5);
                    Some((PointF::new(right, y), PortSide::Right))
                }
                LayoutDirection::Vertical => {
                    let t = (n_cond as f32 + 0.5) / n_br as f32;
                    let x = left + node.size.w * t;
                    Some((PointF::new(x, bottom), PortSide::Bottom))
                }
            },
            pid if pid.starts_with("if_") => {
                let idx: usize = pid[3..].parse().ok()?;
                match layout {
                    LayoutDirection::Horizontal => {
                        let y = node.position.y + TITLE_H + ITEM_H * (idx as f32 + 0.5);
                        Some((PointF::new(right, y), PortSide::Right))
                    }
                    LayoutDirection::Vertical => {
                        let t = (idx as f32 + 0.5) / n_br as f32;
                        let x = left + node.size.w * t;
                        Some((PointF::new(x, bottom), PortSide::Bottom))
                    }
                }
            }
            _ => None,
        }
    }

    /// Condition 节点的实际渲染高度：
    /// - 收起状态：`TITLE_H + ITEM_H`（标题栏 + 主体，跟其他节点一样规格）
    /// - 展开状态：`TITLE_H + ITEM_H * n_branches`（n_branches = 条件数 + 1 个 else）
    ///
    /// 宽度保持 `node.size.w`（由 schema default_size 或创建时指定）。
    fn content_size(&self, node: &Node) -> SizeF {
        let h = if is_collapsed(node) {
            TITLE_H + ITEM_H
        } else {
            content_height(node)
        };
        SizeF::new(node.size.w, h)
    }
}
