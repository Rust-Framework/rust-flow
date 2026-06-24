//! Panel 模块：schema 驱动的属性面板（有状态实体视图）。
//!
//! PanelView 实现 GPUI `Render` trait，根据 `flow_node.schema().fields` 自动
//! 生成编辑界面（Text/TextArea/CodeEditor/CodeBlock/Number/Switch/Dropdown/List），
//! 消除 per-kind 面板分发。
//!
//! 事件流：
//! 1. 用户编辑 Input → InputState 发出 `InputEvent::Change`
//! 2. `subscribe_in` 回调触发 → 调用 `on_action(NodeAction::SetData(...))`
//! 3. `FlowEditorView::handle_node_action` 更新 node.data + relayout
//! 4. `FlowEditorView::render` 检测节点数据变化 → `panel_view.update(sync_from_node)`
//! 5. `sync_from_node` 更新 InputState 值（syncing 标记避免回环）
//!
//! 性能优化：
//! - `sync_from_node` 快速路径：若 `node.data` 未变化则直接返回，避免每帧更新所有 InputState
//! - 每个字段更新前比较当前值与新值，仅在实际变化时调用 `set_value`，避免光标跳动
//! - 渲染时仅克隆 `schema.fields`（Vec<FieldSpec>）而非整个 NodeSchema
//!
//! 视觉设计（参考 n8n/Retool/Appsmith 属性面板）：
//! - 头部：节点图标（彩色圆角方块）+ 类型标签 + kind 副标题
//! - 内容区：可纵向滚动，统一字段间距和标签样式
//! - Dropdown：使用 gpui-component Button + DropdownMenu（与工具栏一致）
//! - List：卡片式行容器 + gpui-component Button 添加/删除
//! - Switch：水平布局（标签左 + 开关右）

use std::sync::Arc;

use gpui::{
    div, px, App, AppContext, Context, Entity, InteractiveElement, IntoElement,
    ParentElement, Render, ScrollHandle, StatefulInteractiveElement, Styled, Subscription,
    Window,
};
use gpui_component::input::{Input, InputEvent, InputState};
use gpui_component::StyledExt;
use rust_agent_flow::{FieldSpec, FieldType, Node};
use crate::i18n::{kind_label, t, Language, TKey};
use crate::node::{ActionCallback, IFlowNode, NodeAction, SharedSyntaxService};
use crate::theme::Theme;

mod helpers;
mod render;
pub mod start;
use helpers::*;

/// 属性面板实体：根据节点类型分发到通用面板或专属面板。
///
/// - `Generic`：使用 schema 驱动的 [`PanelView`]（适用于大多数节点）
/// - `Start`：使用 [`start::StartPanelView`]（Start 节点专属，支持参数/变量树形编辑）
#[derive(Clone)]
pub enum PanelEntity {
    Generic(Entity<PanelView>),
    Start(Entity<start::StartPanelView>),
}

impl PanelEntity {
    /// 渲染面板内容为 AnyElement。
    pub fn render_element(self, window: &mut Window, cx: &mut App) -> gpui::AnyElement {
        match self {
            PanelEntity::Generic(entity) => entity.update(cx, |view, cx| {
                view.render(window, cx).into_any_element()
            }),
            PanelEntity::Start(entity) => entity.update(cx, |view, cx| {
                view.render(window, cx).into_any_element()
            }),
        }
    }
}

/// 单个字段的编辑状态。
///
/// 与 `NodeSchema.fields` 一一对应（label 字段单独由 `label_input` 处理）。
enum FieldState {
    /// 文本/代码类字段（Text/TextArea/CodeEditor/CodeBlock/Number）。
    Input(Entity<InputState>),
    /// 布尔开关。
    Switch(bool),
    /// 下拉选择（存储当前值）。
    Dropdown(String),
    /// 动态列表（每行一组 InputState，按 item_fields 顺序排列）。
    List(Vec<Vec<Entity<InputState>>>),
}

/// 属性面板视图：选中节点时右侧显示。
///
/// schema 驱动：根据 `flow_node.schema().fields` 自动生成编辑界面。
pub struct PanelView {
    pub node: Node,
    pub flow_node: Option<Arc<dyn IFlowNode>>,
    pub theme: Theme,
    pub on_action: Option<ActionCallback>,
    pub syntax_service: SharedSyntaxService,
    pub language: Language,

    /// 节点名称（label 字段，所有节点通用，单独处理）。
    label_input: Entity<InputState>,

    /// 字段编辑状态（与 schema.fields 对齐，跳过 label）。
    field_states: Vec<FieldState>,

    /// 同步标记：避免节点更新时回环触发 on_change。
    syncing: bool,

    /// 内容区滚动句柄（支持属性面板纵向滚动）。
    scroll_handle: ScrollHandle,

    _subscriptions: Vec<Subscription>,
}

impl PanelView {
    /// 创建 PanelView 实体。
    pub fn new(
        node: Node,
        flow_node: Option<Arc<dyn IFlowNode>>,
        theme: Theme,
        on_action: Option<ActionCallback>,
        syntax_service: SharedSyntaxService,
        language: Language,
        window: &mut Window,
        cx: &mut App,
    ) -> Entity<Self> {
        cx.new(|cx| Self::build(node, flow_node, theme, on_action, syntax_service, language, window, cx))
    }

    fn build(
        node: Node,
        flow_node: Option<Arc<dyn IFlowNode>>,
        theme: Theme,
        on_action: Option<ActionCallback>,
        syntax_service: SharedSyntaxService,
        language: Language,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Self {
        let label = label_of(&node);
        let label_placeholder = kind_label(language, &node.kind).to_string();
        let label_input = cx.new(|cx| {
            InputState::new(window, cx)
                .default_value(label.as_str())
                .placeholder(label_placeholder.as_str())
        });
        let sub_label = cx.subscribe_in(&label_input, window, Self::on_label_change);

        // 按 schema.fields 构建 field_states（跳过 label，由 label_input 处理）
        let mut field_states: Vec<FieldState> = Vec::new();
        let mut subscriptions: Vec<Subscription> = vec![sub_label];

        if let Some(ref fn_) = flow_node {
            let schema = fn_.schema();
            for (idx, field) in schema.fields.iter().enumerate() {
                if field.key == "label" {
                    field_states.push(FieldState::Switch(false)); // 占位，不会被渲染
                    continue;
                }
                let default_value = node
                    .data
                    .get(&field.key)
                    .cloned()
                    .unwrap_or_else(|| field.default.clone());
                let state = Self::build_field_state(
                    idx,
                    field,
                    &default_value,
                    &syntax_service,
                    window,
                    cx,
                    &mut subscriptions,
                );
                field_states.push(state);
            }
        }

        Self {
            node,
            flow_node,
            theme,
            on_action,
            syntax_service,
            language,
            label_input,
            field_states,
            syncing: false,
            scroll_handle: ScrollHandle::default(),
            _subscriptions: subscriptions,
        }
    }

    /// 为单个 FieldSpec 创建 FieldState，并订阅 Input 变化。
    fn build_field_state(
        field_idx: usize,
        field: &FieldSpec,
        default_value: &serde_json::Value,
        syntax_service: &SharedSyntaxService,
        window: &mut Window,
        cx: &mut Context<Self>,
        subscriptions: &mut Vec<Subscription>,
    ) -> FieldState {
        match &field.field_type {
            FieldType::Text | FieldType::Number => {
                let text = value_to_string(default_value);
                let input = cx.new(|cx| {
                    InputState::new(window, cx)
                        .default_value(text.as_str())
                        .placeholder(field.placeholder.as_deref().unwrap_or(""))
                });
                subscriptions.push(cx.subscribe_in(&input, window, move |this, _e, ev, _w, cx| {
                    this.on_input_field_change(field_idx, ev, cx);
                }));
                FieldState::Input(input)
            }
            FieldType::TextArea => {
                let text = value_to_string(default_value);
                let input = cx.new(|cx| {
                    InputState::new(window, cx)
                        .default_value(text.as_str())
                        .placeholder(field.placeholder.as_deref().unwrap_or(""))
                        .multi_line(true)
                        .rows(4)
                });
                subscriptions.push(cx.subscribe_in(&input, window, move |this, _e, ev, _w, cx| {
                    this.on_input_field_change(field_idx, ev, cx);
                }));
                FieldState::Input(input)
            }
            FieldType::CodeEditor => {
                // 单行代码编辑器：code_editor(lang).multi_line(false)，行号自动隐藏
                let text = value_to_string(default_value);
                let input = new_code_input(
                    syntax_service,
                    &text,
                    field.placeholder.as_deref().unwrap_or(""),
                    false,
                    window,
                    cx,
                );
                subscriptions.push(cx.subscribe_in(&input, window, move |this, _e, ev, _w, cx| {
                    this.on_input_field_change(field_idx, ev, cx);
                }));
                FieldState::Input(input)
            }
            FieldType::CodeBlock => {
                // 多行代码编辑器：code_editor(lang).line_number(true).rows(4)
                let text = value_to_string(default_value);
                let input = new_code_input(
                    syntax_service,
                    &text,
                    field.placeholder.as_deref().unwrap_or(""),
                    true,
                    window,
                    cx,
                );
                subscriptions.push(cx.subscribe_in(&input, window, move |this, _e, ev, _w, cx| {
                    this.on_input_field_change(field_idx, ev, cx);
                }));
                FieldState::Input(input)
            }
            FieldType::Switch => {
                FieldState::Switch(default_value.as_bool().unwrap_or(false))
            }
            FieldType::Dropdown(_) => {
                let val = default_value
                    .as_str()
                    .unwrap_or("")
                    .to_string();
                FieldState::Dropdown(val)
            }
            FieldType::List(list_spec) => {
                let rows = build_list_rows(
                    field_idx,
                    list_spec,
                    default_value,
                    syntax_service,
                    window,
                    cx,
                    subscriptions,
                );
                FieldState::List(rows)
            }
        }
    }

    /// 节点数据变化时，从 node 同步到字段状态（避免回环）。
    ///
    /// 性能优化：
    /// 1. 快速路径：若 `node.data` 与当前完全一致，直接返回（避免每帧无意义更新）
    /// 2. 逐字段比较：仅在实际值变化时调用 `set_value`，避免光标跳动和不必要重绘
    pub fn sync_from_node(&mut self, node: Node, window: &mut Window, cx: &mut Context<Self>) {
        if self.node.id != node.id {
            return;
        }
        // 快速路径：数据完全一致时跳过所有更新（ensure_panel_view 每帧调用此方法）
        if self.node.data == node.data {
            return;
        }
        self.syncing = true;
        self.node = node;

        // 同步 label（仅在实际变化时更新，避免光标跳动）
        let label = label_of(&self.node);
        let current_label = self.label_input.read(cx).value().to_string();
        if current_label != label {
            self.label_input.update(cx, |s, cx| {
                s.set_value(label.as_str(), window, cx);
            });
        }

        // 同步每个 field_state（仅克隆 fields，避免持有 flow_node 借用）
        if let Some(ref fn_) = self.flow_node {
            let fields = fn_.schema().fields.clone();
            for (i, field) in fields.iter().enumerate() {
                if field.key == "label" {
                    continue;
                }
                if i >= self.field_states.len() {
                    break;
                }
                let value = self
                    .node
                    .data
                    .get(&field.key)
                    .cloned()
                    .unwrap_or_else(|| field.default.clone());
                match &mut self.field_states[i] {
                    FieldState::Input(entity) => {
                        let text = value_to_string(&value);
                        let current = entity.read(cx).value().to_string();
                        if current != text {
                            entity.update(cx, |s, cx| s.set_value(text.as_str(), window, cx));
                        }
                    }
                    FieldState::Switch(b) => {
                        *b = value.as_bool().unwrap_or(false);
                    }
                    FieldState::Dropdown(s) => {
                        if let Some(str_val) = value.as_str() {
                            *s = str_val.to_string();
                        }
                    }
                    FieldState::List(rows) => {
                        sync_list_rows(
                            rows,
                            &value,
                            field,
                            &self.syntax_service,
                            window,
                            cx,
                        );
                    }
                }
            }
        }

        self.syncing = false;
        cx.notify();
    }

    // ====== 事件回调 ======

    fn on_label_change(
        &mut self,
        _state: &Entity<InputState>,
        event: &InputEvent,
        _window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self.syncing || !matches!(event, InputEvent::Change) {
            return;
        }
        let value = self.label_input.read(cx).value().to_string();
        self.dispatch_set_data("label", serde_json::json!(value), cx);
    }

    /// 通用 Input 字段变化回调（Text/TextArea/CodeEditor/CodeBlock/Number）。
    fn on_input_field_change(
        &mut self,
        field_idx: usize,
        event: &InputEvent,
        cx: &mut Context<Self>,
    ) {
        if self.syncing || !matches!(event, InputEvent::Change) {
            return;
        }
        let schema = match self.flow_node.as_ref() {
            Some(fn_) => fn_.schema(),
            None => return,
        };
        if field_idx >= schema.fields.len() {
            return;
        }
        let key = schema.fields[field_idx].key.clone();
        let value = match &self.field_states[field_idx] {
            FieldState::Input(entity) => {
                let text = entity.read(cx).value().to_string();
                serde_json::json!(text)
            }
            _ => return,
        };
        self.dispatch_set_data(&key, value, cx);
    }

    /// Switch 字段变化。
    fn set_switch_field(&mut self, field_idx: usize, new_val: bool, cx: &mut Context<Self>) {
        if self.syncing {
            return;
        }
        if let Some(FieldState::Switch(b)) = self.field_states.get_mut(field_idx) {
            *b = new_val;
        }
        let schema = match self.flow_node.as_ref() {
            Some(fn_) => fn_.schema(),
            None => return,
        };
        if field_idx >= schema.fields.len() {
            return;
        }
        let key = schema.fields[field_idx].key.clone();
        self.dispatch_set_data(&key, serde_json::json!(new_val), cx);
    }

    /// Dropdown 字段变化。
    fn set_dropdown_field(&mut self, field_idx: usize, new_val: &str, cx: &mut Context<Self>) {
        if self.syncing {
            return;
        }
        if let Some(FieldState::Dropdown(s)) = self.field_states.get_mut(field_idx) {
            *s = new_val.to_string();
        }
        let schema = match self.flow_node.as_ref() {
            Some(fn_) => fn_.schema(),
            None => return,
        };
        if field_idx >= schema.fields.len() {
            return;
        }
        let key = schema.fields[field_idx].key.clone();
        self.dispatch_set_data(&key, serde_json::json!(new_val), cx);
    }

    /// List 字段：添加行。
    fn add_list_item(&mut self, field_idx: usize, window: &mut Window, cx: &mut Context<Self>) {
        if self.syncing {
            return;
        }
        let schema = match self.flow_node.as_ref() {
            Some(fn_) => fn_.schema(),
            None => return,
        };
        if field_idx >= schema.fields.len() {
            return;
        }
        let field = &schema.fields[field_idx];
        let list_spec = match &field.field_type {
            FieldType::List(ls) => ls.clone(),
            _ => return,
        };
        // 创建新行（每个 item_field 一个空 InputState）
        let mut new_row: Vec<Entity<InputState>> = Vec::new();
        for item_field in &list_spec.item_fields {
            let default_text = value_to_string(&item_field.default);
            let input = match &item_field.field_type {
                FieldType::CodeEditor => new_code_input(
                    &self.syntax_service,
                    &default_text,
                    item_field.placeholder.as_deref().unwrap_or(""),
                    false,
                    window,
                    cx,
                ),
                FieldType::CodeBlock => new_code_input(
                    &self.syntax_service,
                    &default_text,
                    item_field.placeholder.as_deref().unwrap_or(""),
                    true,
                    window,
                    cx,
                ),
                _ => cx.new(|cx| {
                    InputState::new(window, cx)
                        .default_value(default_text.as_str())
                        .placeholder(item_field.placeholder.as_deref().unwrap_or(""))
                }),
            };
            // 订阅行内 Input 变化
            let sub = cx.subscribe_in(&input, window, move |this, _e, ev, _w, cx| {
                if !this.syncing && matches!(ev, InputEvent::Change) {
                    this.sync_list_to_node(field_idx, cx);
                }
            });
            self._subscriptions.push(sub);
            new_row.push(input);
        }
        if let Some(FieldState::List(rows)) = self.field_states.get_mut(field_idx) {
            rows.push(new_row);
        }
        self.sync_list_to_node(field_idx, cx);
    }

    /// List 字段：删除行。
    fn delete_list_item(&mut self, field_idx: usize, row_idx: usize, cx: &mut Context<Self>) {
        if self.syncing {
            return;
        }
        if let Some(FieldState::List(rows)) = self.field_states.get_mut(field_idx) {
            if row_idx < rows.len() {
                rows.remove(row_idx);
            }
        }
        self.sync_list_to_node(field_idx, cx);
    }

    /// List 字段：同步到 node.data。
    fn sync_list_to_node(&self, field_idx: usize, cx: &mut Context<Self>) {
        let schema = match self.flow_node.as_ref() {
            Some(fn_) => fn_.schema(),
            None => return,
        };
        if field_idx >= schema.fields.len() {
            return;
        }
        let field = &schema.fields[field_idx];
        let list_spec = match &field.field_type {
            FieldType::List(ls) => ls,
            _ => return,
        };
        let rows = match &self.field_states[field_idx] {
            FieldState::List(r) => r,
            _ => return,
        };
        let arr: Vec<serde_json::Value> = rows
            .iter()
            .map(|row| {
                let mut obj = serde_json::Map::new();
                for (col, item_field) in list_spec.item_fields.iter().enumerate() {
                    let val = if col < row.len() {
                        row[col].read(cx).value().to_string()
                    } else {
                        String::new()
                    };
                    obj.insert(item_field.key.clone(), serde_json::json!(val));
                }
                serde_json::Value::Object(obj)
            })
            .collect();
        self.dispatch_set_data(&field.key, serde_json::json!(arr), cx);
    }

    fn dispatch_set_data(&self, key: &str, value: serde_json::Value, cx: &mut Context<Self>) {
        if let Some(on_action) = &self.on_action {
            on_action(NodeAction::SetData(key.to_string(), value), cx);
        }
    }
}

impl Render for PanelView {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = self.theme;
        let header = self.render_header(&theme);
        let content = self.render_schema_panel(&theme, cx);

        div()
            .w(px(300.0))
            .h_full()
            .bg(theme.panel_bg)
            .border_l_1()
            .border_color(theme.panel_border)
            .flex()
            .flex_col()
            .child(header)
            .child(
                div()
                    .id("panel-scroll")
                    .flex_1()
                    .overflow_y_scroll()
                    .track_scroll(&self.scroll_handle)
                    .child(content),
            )
    }
}

impl PanelView {
    /// schema 驱动统一面板渲染。
    fn render_schema_panel(
        &mut self,
        theme: &Theme,
        cx: &mut Context<Self>,
    ) -> gpui::AnyElement {
        let lang = self.language;
        let kind = self.node.kind.clone();

        let mut col = div()
            .flex()
            .flex_col()
            .gap(px(14.0))
            .p(px(16.0));

        // 节点名称（label 字段）
        col = col.child(
            div()
                .flex()
                .flex_col()
                .gap(px(6.0))
                .child(self.render_label(t(lang, TKey::PanelNodeName), theme))
                .child(Input::new(&self.label_input).appearance(true)),
        );

        // 按 schema.fields 渲染每个字段（跳过 label）
        // 克隆 fields 避免 holding borrow on self.flow_node，以便后续 &mut self 调用
        let fields: Vec<FieldSpec> = match &self.flow_node {
            Some(fn_) => fn_.schema().fields.clone(),
            None => Vec::new(),
        };
        for (i, field) in fields.iter().enumerate() {
            if field.key == "label" {
                continue;
            }
            if i >= self.field_states.len() {
                break;
            }
            let label = field_label(lang, &kind, &field.key, &field.label);
            col = col.child(self.render_field(i, field, &label, theme, cx));
        }

        col.into_any_element()
    }

    /// 渲染统一风格的字段标签。
    fn render_label(&self, text: &str, theme: &Theme) -> gpui::AnyElement {
        div()
            .text_size(px(12.0))
            .font_semibold()
            .text_color(theme.panel_label_text)
            .child(text.to_string())
            .into_any_element()
    }

    /// 渲染单个字段（按 FieldType 分发）。
    fn render_field(
        &mut self,
        field_idx: usize,
        field: &FieldSpec,
        label: &str,
        theme: &Theme,
        cx: &mut Context<Self>,
    ) -> gpui::AnyElement {
        match &field.field_type {
            FieldType::Text | FieldType::Number => {
                self.render_input_field(field_idx, label, None, theme)
            }
            FieldType::TextArea => self.render_input_field(field_idx, label, Some(px(80.0)), theme),
            FieldType::CodeEditor => self.render_input_field(field_idx, label, None, theme),
            FieldType::CodeBlock => {
                self.render_input_field(field_idx, label, Some(px(120.0)), theme)
            }
            FieldType::Switch => self.render_switch_field(field_idx, label, theme, cx),
            FieldType::Dropdown(options) => {
                self.render_dropdown_field(field_idx, label, options, theme, cx)
            }
            FieldType::List(list_spec) => {
                self.render_list_field(field_idx, label, list_spec, theme, cx)
            }
        }
    }
}
