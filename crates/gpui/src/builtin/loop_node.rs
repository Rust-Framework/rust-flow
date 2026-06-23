//! Loop 节点：循环体，结构化布局。
//!
//! **设计语义**（两种布局一致）：
//! - 主线：纵向 In 顶 / Done 底；横向 In 左 / Done 右
//! - 循环体支线：LoopBody 始终右出，LoopIn 始终左进（两种布局一致）
//!
//! **外部回环连线**：从 LoopBody 出口 → 外部循环体节点 → 回连到 LoopIn 入口。
//! 循环体节点始终纵向编排（上进下出），无论主布局方向。
//! 回环边从 body 节点底部出，向下绕过 body 组合边界，左进 loop_in。
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
//!    │ LoopIn        LoopBody│──→ (body 节点，上进下出)
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
//!       │    │ LoopIn        LoopBody│──→ (body 节点，上进下出)
//!       │    ↑                      │
//!       │    │           回环边从 body 底部出，向下绕过，左进 loop_in
//!       └────┘
//! ```

use gpui::{div, px, AnyElement, IntoElement, ParentElement, Styled};
use gpui_component::{Icon, Sizable, StyledExt};
use rust_agent_flow::{
    DropdownOption, FieldSpec, FieldType, LayoutDirection, Node, NodeSchema, PointF,
    PortDirection, PortId, PortSide, PortSpec, SizeF,
};

use crate::node::{NodeViewCtx, IFlowNode};

use super::common::{
    desc_of, label_of, make_port, node_icon, port_sizes, render_delete_button, render_simple_panel,
    render_toggle_button, TITLE_ICON_SIZE,
};

/// 标题栏高度（逻辑坐标）。
const TITLE_H: f32 = 36.0;

/// 循环条件区域高度（逻辑坐标，固定值，不随节点尺寸拉伸）。
///
/// 节点总高度 = `TITLE_H + BODY_H`（由内容推导，非输入）。
const BODY_H: f32 = 44.0;

/// 判断循环体是否被收起（隐藏循环体节点）。
///
/// Loop 节点本身始终完整显示，收起的是外部循环体节点。
fn is_body_collapsed(node: &Node) -> bool {
    node.data
        .get("body_collapsed")
        .and_then(|v| v.as_bool())
        .unwrap_or(false)
}

/// 根据 `loop_mode` 返回节点循环条件区的默认显示文案。
///
/// 当节点未设置 `desc` 时，用此文案作为循环条件区的提示文字，
/// 直观体现当前循环模式语义。支持中英文 i18n。
fn loop_mode_label(node: &Node, lang: crate::i18n::Language) -> &'static str {
    use crate::i18n::{t, TKey};
    match node.data.get("loop_mode").and_then(|v| v.as_str()) {
        Some("for_each") => t(lang, TKey::LoopForEach),
        Some("for_loop") => t(lang, TKey::LoopForLoop),
        Some("while") => t(lang, TKey::LoopWhile),
        Some("batch_parallel") => t(lang, TKey::LoopParallel),
        _ => t(lang, TKey::LoopForEach),
    }
}

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
                .with_port(PortSpec::new("loop_in", PortDirection::In, PortSide::Auto))
                .with_field(
                    FieldSpec::new("label", "Label", FieldType::Text)
                        .with_default(serde_json::json!("Loop")),
                )
                .with_field(
                    FieldSpec::new(
                        "loop_mode",
                        "Loop Mode",
                        FieldType::Dropdown(vec![
                            DropdownOption::new("for_each", "For each item"),
                            DropdownOption::new("while", "while cond"),
                            DropdownOption::new("for_loop", "for i in 0..n"),
                            DropdownOption::new("batch_parallel", "parallel each"),
                        ]),
                    )
                    .with_default(serde_json::json!("for_each")),
                )
                .with_field(
                    FieldSpec::new("loop_expr", "Condition Expression", FieldType::CodeBlock)
                        .with_default(serde_json::json!("item > 0")),
                ),
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
        // Loop 节点始终完整显示（标题栏 + 循环条件区），收起的是外部循环体节点
        let body_collapsed = is_body_collapsed(node);
        let h = (TITLE_H + BODY_H) * s;
        let title_h = TITLE_H * s;
        let body_h = BODY_H * s;
        let layout = ctx.layout;
        let t = &ctx.theme;

        let label = label_of(node);
        // 循环条件区文案：优先用 node.data["desc"]（用户自定义），否则按 loop_mode 显示模式标签
        let desc = desc_of(node).unwrap_or_else(|| loop_mode_label(node, ctx.language).to_string());

        let (port_size, port_outer, port_outer_half) = port_sizes(s);
        let border_color = if ctx.selected {
            t.node_border_selected
        } else {
            t.node_border
        };

        // 端口颜色
        let in_ring = t.node_in_ring;
        let in_dot = t.node_in_dot;
        let done_ring = t.node_out_ring;
        let done_dot = t.node_out_dot;
        let body_ring = t.node_in_ring;
        let body_dot = t.node_in_dot;
        let port_bg = t.port_bg;

        // 外层容器（不使用 overflow_hidden，避免裁剪半外露的端口圆圈）
        let mut container = div().relative().w(px(w)).h(px(h));

        // ====== 标题栏 + 循环条件区 + 4 端口（Loop 节点始终完整显示） ======
        // 标题栏（蓝色背景，顶部圆角对齐容器圆角）：图标 + 标签
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
                    Icon::new(node_icon("loop"))
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

        // 循环条件区域（浅蓝背景，底部圆角对齐容器圆角）
        container = container.child(
            div()
                .absolute()
                .left_0()
                .top(px(title_h))
                .w(px(w))
                .h(px(body_h))
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
        let node_mid_y = h * 0.5;
        let body_mid_y = title_h + body_h * 0.5;

        match layout {
            LayoutDirection::Horizontal => {
                // 主线：In 左 / Done 右（节点垂直中心 Y）
                container = container.child(make_port(
                    left - port_outer_half,
                    node_mid_y - port_outer_half,
                    port_outer,
                    port_size,
                    in_ring,
                    in_dot,
                    port_bg,
                ));
                container = container.child(make_port(
                    right - port_outer_half,
                    node_mid_y - port_outer_half,
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
                // 循环体支线：LoopBody 右 / LoopIn 左（与横向布局一致）
                // 循环体始终纵向编排（上进下出），loop_body 右出，loop_in 左进
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
        }

        // toggle 按钮：根据 body_collapsed 状态显示图标
        // body_collapsed=true → ChevronRight（循环体已收起，点击展开）
        // body_collapsed=false → ChevronDown（循环体已展开，点击收起）
        container = container.child(render_toggle_button(node.size.w, s, body_collapsed, t));
        // hover 时叠加删除按钮
        if ctx.hovered {
            container = container.child(render_delete_button(node.size.w, s, t));
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
        // Loop 节点始终完整显示，端口位置固定（不随 body_collapsed 变化）
        let left = node.position.x;
        let right = node.position.x + node.size.w;
        let top = node.position.y;
        let mid_x = node.position.x + node.size.w * 0.5;
        let node_mid_y = node.position.y + node.size.h * 0.5;

        // 使用固定高度，保证端口位置与实际渲染高度一致
        let bottom = node.position.y + TITLE_H + BODY_H;
        let body_mid_y = node.position.y + TITLE_H + BODY_H * 0.5;

        match port_id.as_str() {
            // 主线 In→Done：纵向上进下出，横向左进右出（节点垂直中心）
            "in" => match layout {
                LayoutDirection::Horizontal => Some(PointF::new(left, node_mid_y)),
                LayoutDirection::Vertical => Some(PointF::new(mid_x, top)),
            },
            "done" => match layout {
                LayoutDirection::Horizontal => Some(PointF::new(right, node_mid_y)),
                LayoutDirection::Vertical => Some(PointF::new(mid_x, bottom)),
            },
            // 循环体支线：loop_body 始终右出，loop_in 始终左进（两种布局一致）
            // 循环体节点纵向编排（上进下出），回环边从 body 底部向下绕回 loop_in
            "loop_body" => Some(PointF::new(right, body_mid_y)),
            "loop_in" => Some(PointF::new(left, body_mid_y)),
            _ => None,
        }
    }

    /// Loop 节点的实际渲染尺寸：始终为 `TITLE_H + BODY_H`（Loop 节点本身不收起，
    /// 收起的是外部循环体节点）。
    ///
    /// 宽度保持 `node.size.w`（由 schema default_size 或创建时指定）。
    fn content_size(&self, node: &Node) -> SizeF {
        SizeF::new(node.size.w, TITLE_H + BODY_H)
    }
}
