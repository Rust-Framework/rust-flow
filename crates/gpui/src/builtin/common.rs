//! 内置节点共享辅助函数：label/desc 提取 + 简单属性面板 + 端口渲染 + 按钮渲染 + 图标映射。
//!
//! 所有节点统一为「标题栏 + 主体」结构：
//! - 标题栏：图标 + 名称 + 操作按钮（删除/展开收起）
//! - 主体：根据节点类型不同（Action 显示 desc，Condition 显示条件项，Loop 显示循环条件）

use gpui::{div, px, AnyElement, IntoElement, ParentElement, Styled};
use gpui_component::{Icon, IconName, Sizable};
use gpui_component::StyledExt;
use rust_agent_flow::Node;

use crate::theme::Theme;

/// 删除按钮尺寸（逻辑坐标，会随缩放自动缩放）。
pub(crate) const DELETE_BTN_SIZE: f32 = 20.0;

/// 展开/收起切换按钮尺寸（逻辑坐标，会随缩放自动缩放）。
pub(crate) const TOGGLE_BTN_SIZE: f32 = 20.0;

/// 标题栏高度（逻辑坐标，用于按钮垂直居中计算）。
pub(crate) const TITLE_H: f32 = 36.0;

/// 按钮距节点右边缘的间距（逻辑坐标）。
pub(crate) const BTN_MARGIN: f32 = 4.0;

/// 标题栏图标尺寸（逻辑坐标）。
pub(crate) const TITLE_ICON_SIZE: f32 = 16.0;

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

/// 节点类型 → 图标映射。
///
/// 返回 `IconName` 用于标题栏左侧图标显示。
pub(crate) fn node_icon(kind: &str) -> IconName {
    match kind {
        "start" => IconName::Play,
        "end" => IconName::CircleCheck,
        "action" => IconName::Cpu,
        "condition" => IconName::Network,
        "loop" => IconName::Redo,
        _ => IconName::Settings,
    }
}

/// 渲染简单属性面板：显示 kind + label + desc。
///
/// 颜色取自 `theme`，支持主题切换。
pub(crate) fn render_simple_panel(node: &Node, kind_label_str: &str, theme: &Theme) -> AnyElement {
    let label = label_of(node);
    let desc = desc_of(node);
    let mut col = div().flex().flex_col().gap(px(8.0)).p_4();

    col = col.child(
        div()
            .text_size(px(16.0))
            .font_semibold()
            .text_color(theme.panel_title_text)
            .child(kind_label_str.to_string()),
    );
    col = col.child(
        div()
            .text_size(px(13.0))
            .text_color(theme.panel_subtext)
            .child(format!("Kind: {}", node.kind)),
    );
    col = col.child(
        div()
            .text_size(px(13.0))
            .text_color(theme.panel_label_text)
            .child(format!("Label: {}", label)),
    );
    if let Some(desc) = desc {
        col = col.child(
            div()
                .text_size(px(13.0))
                .text_color(theme.panel_subtext)
                .child(format!("Desc: {}", desc)),
        );
    }

    col.into_any_element()
}

/// 渲染端口圆圈（用于结构化节点的多端口布局）。
///
/// 端口为 `port_bg` 底色圆环 + 彩色圆点，位于 `(left, top)`（相对父容器左上角，屏幕坐标）。
/// `port_outer` = 外圆直径，`port_size` = 内圆点直径。
pub(crate) fn make_port(
    left: f32,
    top: f32,
    port_outer: f32,
    port_size: f32,
    ring_color: gpui::Rgba,
    dot_color: gpui::Rgba,
    port_bg: gpui::Rgba,
) -> AnyElement {
    div()
        .absolute()
        .left(px(left))
        .top(px(top))
        .w(px(port_outer))
        .h(px(port_outer))
        .rounded_full()
        .bg(port_bg)
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

/// 渲染图标按钮（使用 gpui-component Icon），仅在 hover 时由调用方决定是否渲染。
///
/// 绝对定位在指定位置，尺寸 `btn_size × btn_size`（屏幕坐标）。
///
/// - `left`, `top`: 按钮左上角位置（屏幕坐标，已乘 scale）
/// - `btn_size`: 按钮尺寸（屏幕坐标）
/// - `icon`: 图标名称
/// - `bg`: 背景色
/// - `text_color`: 图标颜色
/// - `scale`: 视口缩放比例（用于图标尺寸计算）
fn render_icon_button(
    left: f32,
    top: f32,
    btn_size: f32,
    icon: IconName,
    bg: gpui::Rgba,
    text_color: gpui::Rgba,
) -> AnyElement {
    let icon_size = btn_size * 0.6;
    div()
        .absolute()
        .left(px(left))
        .top(px(top))
        .w(px(btn_size))
        .h(px(btn_size))
        .rounded_md()
        .bg(bg)
        .flex()
        .items_center()
        .justify_center()
        .child(
            Icon::new(icon)
                .with_size(px(icon_size))
                .text_color(text_color),
        )
        .into_any_element()
}

/// 渲染删除按钮（Delete 图标），仅在 hover 时由调用方决定是否渲染。
///
/// 绝对定位在节点右上角：距右边缘 `BTN_MARGIN`，距顶 `BTN_MARGIN`。
///
/// - `node_w`: 节点逻辑宽度（未乘 scale）
/// - `scale`: 视口缩放比例
pub(crate) fn render_delete_button(node_w: f32, scale: f32, theme: &Theme) -> AnyElement {
    let btn_size = DELETE_BTN_SIZE * scale;
    let margin = BTN_MARGIN * scale;
    let left = node_w * scale - btn_size - margin;
    let top = margin;
    render_icon_button(
        left,
        top,
        btn_size,
        IconName::Delete,
        theme.delete_btn_bg,
        theme.delete_btn_text,
    )
}

/// 渲染展开/收起切换按钮（ChevronDown/ChevronRight 图标）。
///
/// 绝对定位在删除按钮左侧，垂直居中于标题栏。
///
/// - `node_w`: 节点逻辑宽度（未乘 scale）
/// - `scale`: 视口缩放比例
/// - `collapsed`: true 显示 ChevronRight（已收起，点击展开），false 显示 ChevronDown（已展开，点击收起）
pub(crate) fn render_toggle_button(
    node_w: f32,
    scale: f32,
    collapsed: bool,
    theme: &Theme,
) -> AnyElement {
    let btn_size = TOGGLE_BTN_SIZE * scale;
    let margin = BTN_MARGIN * scale;
    // 位于删除按钮左侧（删除按钮宽 + 间距）
    let left = node_w * scale - btn_size - margin - btn_size - margin;
    // 标题栏垂直居中
    let top = (TITLE_H * scale - btn_size) * 0.5;
    let icon = if collapsed {
        IconName::ChevronRight
    } else {
        IconName::ChevronDown
    };
    render_icon_button(
        left,
        top,
        btn_size,
        icon,
        theme.toggle_btn_bg,
        theme.toggle_btn_text,
    )
}


