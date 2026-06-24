//! Panel 辅助函数：标签读取、值转换、代码编辑器创建、List 行构建/同步、i18n 映射。

use gpui::{App, AppContext, Context, Entity, Subscription, Window};
use gpui_component::input::{InputEvent, InputState};
use rust_agent_flow::{DropdownOption, FieldSpec, FieldType, ListSpec, Node};
use crate::i18n::{t, Language, TKey};
use crate::node::SharedSyntaxService;

use super::PanelView;

/// 从 node.data 读取 label。
pub(super) fn label_of(node: &Node) -> String {
    node.data
        .get("label")
        .and_then(|v| v.as_str())
        .unwrap_or(&node.kind)
        .to_string()
}

/// JSON Value → String（支持 string/number/bool）。
pub(super) fn value_to_string(v: &serde_json::Value) -> String {
    match v {
        serde_json::Value::String(s) => s.clone(),
        serde_json::Value::Number(n) => n.to_string(),
        serde_json::Value::Bool(b) => b.to_string(),
        serde_json::Value::Null => String::new(),
        _ => v.to_string(),
    }
}

/// 创建代码编辑器 InputState。
///
/// `multi_line=false` 时为单行模式（line_number 自动 false）；
/// `multi_line=true` 时为多行模式（line_number=true, rows=4）。
pub(super) fn new_code_input(
    syntax_service: &SharedSyntaxService,
    default_value: &str,
    placeholder: &str,
    multi_line: bool,
    window: &mut Window,
    cx: &mut App,
) -> Entity<InputState> {
    let language = syntax_service.language_for("rhai");
    cx.new(|cx| {
        let mut state = InputState::new(window, cx)
            .default_value(default_value)
            .placeholder(placeholder);
        if let Some(lang) = language {
            state = state.code_editor(lang);
            if multi_line {
                state = state.multi_line(true).line_number(true).rows(4);
            } else {
                state = state.multi_line(false);
            }
        } else {
            if multi_line {
                state = state.multi_line(true).rows(4);
            } else {
                state = state.multi_line(false);
            }
        }
        state
    })
}

/// 构建 List 字段的初始行。
pub(super) fn build_list_rows(
    field_idx: usize,
    list_spec: &ListSpec,
    default_value: &serde_json::Value,
    syntax_service: &SharedSyntaxService,
    window: &mut Window,
    cx: &mut Context<PanelView>,
    subscriptions: &mut Vec<Subscription>,
) -> Vec<Vec<Entity<InputState>>> {
    let arr = default_value.as_array();
    let mut rows: Vec<Vec<Entity<InputState>>> = Vec::new();
    if let Some(arr) = arr {
        for item in arr {
            let mut row: Vec<Entity<InputState>> = Vec::new();
            for item_field in &list_spec.item_fields {
                let val = item.get(&item_field.key).cloned().unwrap_or_else(|| item_field.default.clone());
                let text = value_to_string(&val);
                let input = match &item_field.field_type {
                    FieldType::CodeEditor => new_code_input(
                        syntax_service,
                        &text,
                        item_field.placeholder.as_deref().unwrap_or(""),
                        false,
                        window,
                        cx,
                    ),
                    FieldType::CodeBlock => new_code_input(
                        syntax_service,
                        &text,
                        item_field.placeholder.as_deref().unwrap_or(""),
                        true,
                        window,
                        cx,
                    ),
                    _ => cx.new(|cx| {
                        InputState::new(window, cx)
                            .default_value(text.as_str())
                            .placeholder(item_field.placeholder.as_deref().unwrap_or(""))
                    }),
                };
                let sub = cx.subscribe_in(&input, window, move |this, _e, ev, _w, cx| {
                    if !this.syncing && matches!(ev, InputEvent::Change) {
                        this.sync_list_to_node(field_idx, cx);
                    }
                });
                subscriptions.push(sub);
                row.push(input);
            }
            rows.push(row);
        }
    }
    rows
}

/// 同步 List 行（数量一致仅更新值，否则重建）。
pub(super) fn sync_list_rows(
    rows: &mut Vec<Vec<Entity<InputState>>>,
    value: &serde_json::Value,
    field: &FieldSpec,
    syntax_service: &SharedSyntaxService,
    window: &mut Window,
    cx: &mut Context<PanelView>,
) {
    let list_spec = match &field.field_type {
        FieldType::List(ls) => ls,
        _ => return,
    };
    let arr: Vec<&serde_json::Value> = value.as_array().map(|a| a.iter().collect()).unwrap_or_default();

    if arr.len() == rows.len() {
        // 数量一致：仅更新值（比较后更新，避免不必要重绘）
        for (i, item) in arr.iter().enumerate() {
            for (col, item_field) in list_spec.item_fields.iter().enumerate() {
                if col >= rows[i].len() {
                    continue;
                }
                let val = item.get(&item_field.key).cloned().unwrap_or_else(|| item_field.default.clone());
                let text = value_to_string(&val);
                let current = rows[i][col].read(cx).value().to_string();
                if current != text {
                    rows[i][col].update(cx, |s, cx| s.set_value(text.as_str(), window, cx));
                }
            }
        }
    } else {
        // 数量变化：重建行（注意：重建会丢失订阅，但 sync 期间 syncing=true 不会触发回调）
        rows.clear();
        for item in &arr {
            let mut row: Vec<Entity<InputState>> = Vec::new();
            for item_field in &list_spec.item_fields {
                let val = item.get(&item_field.key).cloned().unwrap_or_else(|| item_field.default.clone());
                let text = value_to_string(&val);
                let input = match &item_field.field_type {
                    FieldType::CodeEditor => new_code_input(
                        syntax_service,
                        &text,
                        item_field.placeholder.as_deref().unwrap_or(""),
                        false,
                        window,
                        cx,
                    ),
                    FieldType::CodeBlock => new_code_input(
                        syntax_service,
                        &text,
                        item_field.placeholder.as_deref().unwrap_or(""),
                        true,
                        window,
                        cx,
                    ),
                    _ => cx.new(|cx| {
                        InputState::new(window, cx)
                            .default_value(text.as_str())
                            .placeholder(item_field.placeholder.as_deref().unwrap_or(""))
                    }),
                };
                row.push(input);
            }
            rows.push(row);
        }
    }
}

/// 字段标签 i18n 映射：(kind, field_key) → TKey → 本地化文案。
pub(super) fn field_label(lang: Language, kind: &str, field_key: &str, fallback: &str) -> String {
    let tkey = match (kind, field_key) {
        ("condition", "conditions") => TKey::PanelConditions,
        ("loop", "loop_mode") => TKey::PanelLoopMode,
        ("loop", "loop_expr") => TKey::PanelLoopExpr,
        ("start", "params") => TKey::PanelParams,
        ("start", "variables") => TKey::PanelVariables,
        ("end", "returns") => TKey::PanelReturns,
        ("agent", "model") => TKey::PanelAgentModel,
        ("agent", "prompt") => TKey::PanelAgentPrompt,
        ("variable", "variables") => TKey::PanelVariables,
        // desc 字段（action/adapter 共用）
        ("action", "desc") | ("adapter", "desc") => TKey::PanelDesc,
        _ => return fallback.to_string(),
    };
    t(lang, tkey).to_string()
}

/// Dropdown 选项标签 i18n 映射。
pub(super) fn dropdown_option_label(lang: Language, _kind: &str, opt: &DropdownOption) -> String {
    // Loop 模式特殊映射
    match opt.value.as_str() {
        "for_each" => t(lang, TKey::LoopForEach).to_string(),
        "while" => t(lang, TKey::LoopWhile).to_string(),
        "for_loop" => t(lang, TKey::LoopForLoop).to_string(),
        "batch_parallel" => t(lang, TKey::LoopParallel).to_string(),
        _ => opt.label.clone(),
    }
}
