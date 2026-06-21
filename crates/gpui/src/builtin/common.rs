//! 内置节点共享辅助函数：label/desc 提取 + 简单属性面板 + 端口渲染。

use gpui::{div, px, AnyElement, IntoElement, ParentElement, Styled};
use gpui_component::StyledExt;
use rust_agent_flow::Node;

/// 从 node.data 取 label，缺省回退到 kind。
pub(crate) fn label_of(node: &Node) -> String {
    node.data
        .get("label")
        .and_then(|v| v.as_str())
        .unwrap_or(&node.kind)
        .to_string()
}

/// 从 node.data 取 desc。
pub(crate) fn desc_of(node: &Node) -> Option<String> {
    node.data
        .get("desc")
        .and_then(|v| v.as_str())
        .map(|s| s.to_string())
}

/// 渲染简单属性面板：显示 kind + label + desc。
pub(crate) fn render_simple_panel(node: &Node, kind_label: &str) -> AnyElement {
    let label = label_of(node);
    let desc = desc_of(node);
    let mut col = div().flex().flex_col().gap(px(8.0)).p_4();

    col = col.child(
        div()
            .text_size(px(16.0))
            .font_semibold()
            .text_color(gpui::rgb(0x1e293b))
            .child(kind_label.to_string()),
    );
    col = col.child(
        div()
            .text_size(px(13.0))
            .text_color(gpui::rgb(0x64748b))
            .child(format!("Kind: {}", node.kind)),
    );
    col = col.child(
        div()
            .text_size(px(13.0))
            .text_color(gpui::rgb(0x1e293b))
            .child(format!("Label: {}", label)),
    );
    if let Some(desc) = desc {
        col = col.child(
            div()
                .text_size(px(13.0))
                .text_color(gpui::rgb(0x64748b))
                .child(format!("Desc: {}", desc)),
        );
    }

    col.into_any_element()
}

/// 渲染端口圆圈（用于结构化节点的多端口布局）。
///
/// 端口为白底圆环 + 彩色圆点，位于 `(left, top)`（相对父容器左上角，屏幕坐标）。
/// `port_outer` = 外圆直径，`port_size` = 内圆点直径。
pub(crate) fn make_port(
    left: f32,
    top: f32,
    port_outer: f32,
    port_size: f32,
    ring_color: gpui::Rgba,
    dot_color: gpui::Rgba,
) -> AnyElement {
    div()
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
            div()
                .w(px(port_size))
                .h(px(port_size))
                .rounded_full()
                .bg(dot_color),
        )
        .into_any_element()
}

/// 端口尺寸计算（随缩放）。
///
/// 返回 `(port_size, port_outer, port_outer_half)`。
pub(crate) fn port_sizes(scale: f32) -> (f32, f32, f32) {
    let port_size = 6.0 * scale;
    let port_outer = (port_size + 4.0) * scale;
    let port_outer_half = port_outer * 0.5;
    (port_size, port_outer, port_outer_half)
}
