//! 公共类型与辅助函数。
//!
//! 包含：
//! - [`Selection`]：当前选中项（驱动浮层详细编辑面板）
//! - [`RowInputs`]：Tree 行的内联控件输入状态（Entity 句柄）
//! - 辅助函数：ID 解析、类型显示、标签提取等

use gpui::{Entity, SharedString};
use gpui_component::input::InputState;
use gpui_component::select::SelectState;
use rust_agent_flow::Node;

/// 当前选中项（用于行高亮等视觉反馈）。
///
/// `field_idx` 为 `None` 表示选中顶层参数/变量项；
/// 为 `Some(i)` 表示选中结构类型的第 i 个子字段。
#[derive(Clone, Debug)]
#[allow(dead_code)]
pub(crate) struct Selection {
    /// "params" 或 "variables"。
    pub field_key: String,
    /// 项索引（params_state / variables_state 中的位置）。
    pub item_idx: usize,
    /// 子字段索引（None = 顶层项）。
    pub field_idx: Option<usize>,
}

/// Tree 行的内联控件输入状态。
///
/// 在 render 前一次性从 `ItemState`/`FieldState` 提取 `Entity` 句柄，
/// 供 Tree 的 render_item 闭包直接创建 Input/Select/Switch 控件。
/// `Entity` 是引用计数句柄（Clone + 'static），可安全移入渲染闭包。
#[allow(dead_code)]
pub(super) struct RowInputs {
    /// 名称输入句柄。
    pub name: Entity<InputState>,
    /// 当前类型值（用于下拉选中状态判断）。
    pub type_value: String,
    /// 类型显示标签（本地化）。
    pub type_label: String,
    /// 类型选择 SelectState 句柄。
    pub type_select: Entity<SelectState<Vec<SharedString>>>,
    /// 默认值输入句柄（基础类型为 Some；结构类型为 None）。
    pub value: Option<Entity<InputState>>,
    /// 是否可选（仅顶层项）。
    pub is_optional: bool,
    /// 是否数组（仅顶层项）。
    pub is_array: bool,
    /// 项索引（用于回调）。
    pub item_idx: usize,
    /// 子字段索引（子字段时为 Some）。
    pub field_idx: Option<usize>,
}

/// 从 node.data 读取 label。
pub(super) fn label_of(node: &Node) -> String {
    node.data
        .get("label")
        .and_then(|v| v.as_str())
        .unwrap_or(&node.kind)
        .to_string()
}

/// 解析 Tree item ID → (item_idx, field_idx)。
///
/// - `"params:0"` → `(0, None)`
/// - `"params:0:1"` → `(0, Some(1))`
pub(super) fn parse_tree_item_id(id: &str) -> Option<(usize, Option<usize>)> {
    let parts: Vec<&str> = id.split(':').collect();
    if parts.len() < 2 {
        return None;
    }
    // parts[0] = field_key ("params" or "variables")
    // parts[1] = item_idx
    // parts[2] = field_idx (optional)
    let item_idx = parts[1].parse::<usize>().ok()?;
    let field_idx = if parts.len() >= 3 {
        Some(parts[2].parse::<usize>().ok()?)
    } else {
        None
    };
    Some((item_idx, field_idx))
}

/// 显示类型名（含 optional/array 标志）。
pub(super) fn display_type_with_flags(type_value: &str, is_optional: bool, is_array: bool) -> String {
    let mut s = type_value.to_string();
    if is_array {
        s = format!("{}[]", s);
    }
    if is_optional {
        s = format!("{}?", s);
    }
    s
}
