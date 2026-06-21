//! NodeView：节点渲染组件（无状态，RenderOnce）。
//!
//! 持有节点数据和可选的 IFlowNode 实现，调用 `get_view` 获取卡片内容。
//! 位置定位和事件绑定由 FlowEditorView 的外层 div 负责。

use std::sync::Arc;

use gpui::{AnyElement, App, IntoElement, ParentElement, RenderOnce, Styled, Window, px};
use gpui_component::StyledExt;
use rust_agent_flow::{LayoutDirection, Node};

use super::{IFlowNode, NodeViewCtx};

/// 节点视觉配置：供 [`render_node_card`] 使用，描述节点卡片的外观和端口。
#[derive(Clone)]
pub struct NodeVisual {
    /// 主标签（如 "Start"、"Planner"）。
    pub label: String,
    /// 可选副标题（如 "规划下一步动作"）。
    pub desc: Option<String>,
    /// 背景色。
    pub bg: gpui::Rgba,
    /// 边框色（未选中）。
    pub border: gpui::Rgba,
    /// 选中边框色。
    pub border_selected: gpui::Rgba,
    /// 标签文字色。
    pub text: gpui::Rgba,
    /// 副标题文字色。
    pub subtext: gpui::Rgba,
    /// 是否显示入端口。
    pub show_in: bool,
    /// 是否显示出端口。
    pub show_out: bool,
    /// 入端口圆点色。
    pub in_color: gpui::Rgba,
    /// 出端口圆点色。
    pub out_color: gpui::Rgba,
    /// 是否为药丸形（圆角更大，用于 Start/End）。
    pub pill: bool,
}

impl NodeVisual {
    /// 创建默认白色卡片配置（Action 风格）。
    pub fn card(label: impl Into<String>) -> Self {
        Self {
            label: label.into(),
            desc: None,
            bg: gpui::rgb(0xffffff),
            border: gpui::rgb(0xe2e8f0),
            border_selected: gpui::rgb(0x6366f1),
            text: gpui::rgb(0x1e293b),
            subtext: gpui::rgb(0x64748b),
            show_in: true,
            show_out: true,
            in_color: gpui::rgb(0x6366f1),
            out_color: gpui::rgb(0x22c55e),
            pill: false,
        }
    }
}

/// 渲染节点卡片（body + 端口）。
///
/// **z-index 关键**：端口作为 body 的**后续兄弟**（而非 body 的子元素），
/// 确保端口绘制在节点边框之上，避免边框穿过端口圆圈。
///
/// - `w` / `h`：屏幕像素尺寸（已乘 scale）
/// - `scale`：视口缩放
/// - `vertical`：垂直布局（端口在 top/bottom）
/// - `selected`：是否选中
pub fn render_node_card(
    visual: &NodeVisual,
    w: f32,
    h: f32,
    scale: f32,
    vertical: bool,
    selected: bool,
) -> AnyElement {
    let s = scale;

    // 端口尺寸（随缩放）
    let port_size = 6.0 * s;
    let port_outer = (port_size + 4.0) * s;
    let port_outer_half = port_outer * 0.5;

    let font_size = 14.0 * s;
    let desc_size = 11.0 * s;

    let border_color = if selected {
        visual.border_selected
    } else {
        visual.border
    };

    // 端口位置：圆心在节点边缘上（半内半外）
    let (in_port_left, in_port_top, out_port_left, out_port_top) = if vertical {
        (
            w * 0.5 - port_outer_half,
            -port_outer_half,
            w * 0.5 - port_outer_half,
            h - port_outer_half,
        )
    } else {
        (
            -port_outer_half,
            h * 0.5 - port_outer_half,
            w - port_outer_half,
            h * 0.5 - port_outer_half,
        )
    };

    // 构造端口 div 的辅助闭包
    let make_port = |left: f32, top: f32, ring_color: gpui::Rgba, dot_color: gpui::Rgba| {
        gpui::div()
            .absolute()
            .left(px(left))
            .top(px(top))
            .w(px(port_outer))
            .h(px(port_outer))
            .rounded_full()
            .bg(gpui::white())
            .border_1()
            .border_color(ring_color)
            .flex()
            .items_center()
            .justify_center()
            .child(
                gpui::div()
                    .w(px(port_size))
                    .h(px(port_size))
                    .rounded_full()
                    .bg(dot_color),
            )
    };

    // 外层容器：relative，body 和端口都是其子元素。
    // 端口在 body 之后 → 绘制在边框之上。
    let mut container = gpui::div()
        .relative()
        .w(px(w))
        .h(px(h));

    // Body（带边框和背景）
    let mut body = gpui::div()
        .absolute()
        .top_0()
        .left_0()
        .w(px(w))
        .h(px(h))
        .bg(visual.bg)
        .border_1()
        .border_color(border_color)
        .shadow_lg()
        .flex()
        .items_center()
        .justify_center();

    if visual.pill {
        body = body.rounded_full();
    } else {
        body = body.rounded_lg();
    }

    // 内容：标签 + 可选副标题
    let mut content = gpui::div().flex().flex_col().items_center().gap(px(2.0 * s));
    content = content.child(
        gpui::div()
            .text_size(px(font_size))
            .font_semibold()
            .text_color(visual.text)
            .child(visual.label.clone()),
    );
    if let Some(desc) = &visual.desc {
        content = content.child(
            gpui::div()
                .text_size(px(desc_size))
                .text_color(visual.subtext)
                .child(desc.clone()),
        );
    }
    body = body.child(content);

    container = container.child(body);

    // 端口（在 body 之后，绘制在边框之上）
    if visual.show_in {
        container = container.child(make_port(
            in_port_left,
            in_port_top,
            gpui::rgb(0xc7d2fe),
            visual.in_color,
        ));
    }
    if visual.show_out {
        container = container.child(make_port(
            out_port_left,
            out_port_top,
            gpui::rgb(0xbbf7d0),
            visual.out_color,
        ));
    }

    container.into_any_element()
}

/// 节点视图组件。
///
/// `flow_node` 为 `None` 时使用 fallback 渲染（显示 kind 文字），
/// 用于未注册的节点 kind（Phase 2 demo 阶段）。
pub struct NodeView {
    pub node: Node,
    pub flow_node: Option<Arc<dyn IFlowNode>>,
    pub selected: bool,
    /// 缩放比例（来自视口），所有内部元素按此比例缩放。
    pub scale: f32,
    /// 垂直布局（端口在 top/bottom 而非 left/right）。
    pub vertical: bool,
    /// 布局方向。
    pub layout: LayoutDirection,
}

impl NodeView {
    pub fn new(node: Node) -> Self {
        Self {
            node,
            flow_node: None,
            selected: false,
            scale: 1.0,
            vertical: false,
            layout: LayoutDirection::Vertical,
        }
    }

    pub fn with_scale(mut self, scale: f32) -> Self {
        self.scale = scale;
        self
    }

    pub fn with_vertical(mut self, vertical: bool) -> Self {
        self.vertical = vertical;
        self
    }

    pub fn with_layout(mut self, layout: LayoutDirection) -> Self {
        self.layout = layout;
        self.vertical = layout == LayoutDirection::Vertical;
        self
    }

    pub fn with_flow_node(mut self, flow_node: Arc<dyn IFlowNode>) -> Self {
        self.flow_node = Some(flow_node);
        self
    }

    pub fn selected(mut self, selected: bool) -> Self {
        self.selected = selected;
        self
    }
}

impl RenderOnce for NodeView {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        if let Some(flow_node) = self.flow_node {
            let mut ctx = NodeViewCtx {
                window,
                cx,
                selected: self.selected,
                scale: self.scale,
                layout: self.layout,
            };
            flow_node.get_view(&self.node, &mut ctx)
        } else {
            // Fallback：未注册的 kind，显示 kind 文字。
            self.render_fallback()
        }
    }
}

impl IntoElement for NodeView {
    type Element = gpui::Component<Self>;
    fn into_element(self) -> Self::Element {
        gpui::Component::new(self)
    }
}

impl NodeView {
    fn render_fallback(self) -> AnyElement {
        let label = self
            .node
            .data
            .get("label")
            .and_then(|v| v.as_str())
            .unwrap_or(&self.node.kind)
            .to_string();
        let desc = self
            .node
            .data
            .get("desc")
            .and_then(|v| v.as_str())
            .map(|s| s.to_string());

        let visual = NodeVisual {
            label,
            desc,
            bg: gpui::rgb(0xffffff),
            border: gpui::rgb(0xe2e8f0),
            border_selected: gpui::rgb(0x6366f1),
            text: gpui::rgb(0x1e293b),
            subtext: gpui::rgb(0x64748b),
            show_in: true,
            show_out: true,
            in_color: gpui::rgb(0x6366f1),
            out_color: gpui::rgb(0x22c55e),
            pill: false,
        };

        let w = self.node.size.w * self.scale;
        let h = self.node.size.h * self.scale;
        render_node_card(&visual, w, h, self.scale, self.vertical, self.selected)
    }
}
