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

use std::sync::Arc;

use gpui::{
    div, px, App, AppContext, Context, Entity, InteractiveElement, IntoElement, ParentElement,
    Render, ScrollHandle, Styled, Subscription, Window,
};
use gpui_component::input::{Input, InputEvent, InputState};
use gpui_component::{Icon, IconName, Sizable, StyledExt};
use rust_agent_flow::{
    DropdownOption, FieldSpec, FieldType, ListSpec, Node,
};
use gpui_component::switch::Switch;

use crate::i18n::{t, Language, TKey};
use crate::node::{ActionCallback, IFlowNode, NodeAction, SharedSyntaxService};
use crate::theme::Theme;

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

    #[allow(dead_code)]
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
        let label_input = cx.new(|cx| {
            InputState::new(window, cx)
                .default_value(label.as_str())
                .placeholder("Label")
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
    pub fn sync_from_node(&mut self, node: Node, window: &mut Window, cx: &mut Context<Self>) {
        if self.node.id != node.id {
            return;
        }
        self.syncing = true;
        self.node = node;

        // 同步 label
        let label = label_of(&self.node);
        self.label_input.update(cx, |s, cx| {
            s.set_value(label.as_str(), window, cx);
        });

        // 同步每个 field_state
        if let Some(ref fn_) = self.flow_node {
            let schema = fn_.schema().clone();
            for (i, field) in schema.fields.iter().enumerate() {
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
                        entity.update(cx, |s, cx| s.set_value(text.as_str(), window, cx));
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
        let content = self.render_schema_panel(&theme, cx);

        div()
            .w(px(300.0))
            .h_full()
            .bg(theme.panel_bg)
            .border_l_1()
            .border_color(theme.panel_border)
            .flex()
            .flex_col()
            .child(content)
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
            .gap(px(10.0))
            .p_3()
            .size_full()
            .overflow_hidden();

        // 标题：节点类型 i18n 标签 + "节点"
        let kind_label = kind_label_str(lang, &kind);
        let title = format!("{} {}", kind_label, t(lang, TKey::PanelNodeSuffix));
        col = col.child(
            div()
                .text_size(px(15.0))
                .font_semibold()
                .text_color(theme.panel_title_text)
                .child(title),
        );

        // 节点名称（label 字段）
        col = col.child(
            div()
                .flex()
                .flex_col()
                .gap(px(4.0))
                .child(
                    div()
                        .text_size(px(12.0))
                        .font_semibold()
                        .text_color(theme.panel_label_text)
                        .child(t(lang, TKey::PanelNodeName).to_string()),
                )
                .child(Input::new(&self.label_input).appearance(true)),
        );

        // 按 schema.fields 渲染每个字段（跳过 label）
        if let Some(ref fn_) = self.flow_node {
            let schema = fn_.schema().clone();
            for (i, field) in schema.fields.iter().enumerate() {
                if field.key == "label" {
                    continue;
                }
                if i >= self.field_states.len() {
                    break;
                }
                let label = field_label(lang, &kind, &field.key, &field.label);
                col = col.child(self.render_field(i, field, &label, theme, cx));
            }
        }

        col.into_any_element()
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
            FieldType::Text | FieldType::Number => self.render_input_field(field_idx, label, None, theme),
            FieldType::TextArea => self.render_input_field(field_idx, label, Some(px(80.0)), theme),
            FieldType::CodeEditor => self.render_input_field(field_idx, label, None, theme),
            FieldType::CodeBlock => self.render_input_field(field_idx, label, Some(px(120.0)), theme),
            FieldType::Switch => self.render_switch_field(field_idx, label, theme, cx),
            FieldType::Dropdown(options) => self.render_dropdown_field(field_idx, label, options, theme, cx),
            FieldType::List(list_spec) => self.render_list_field(field_idx, label, list_spec, theme, cx),
        }
    }

    /// 渲染 Input 类字段（Text/TextArea/CodeEditor/CodeBlock/Number）。
    fn render_input_field(
        &self,
        field_idx: usize,
        label: &str,
        height: Option<gpui::Pixels>,
        theme: &Theme,
    ) -> gpui::AnyElement {
        let entity = match &self.field_states[field_idx] {
            FieldState::Input(e) => e,
            _ => return div().into_any_element(),
        };
        let mut input = Input::new(entity).appearance(true);
        if let Some(h) = height {
            input = input.h(h);
        }
        div()
            .flex()
            .flex_col()
            .gap(px(4.0))
            .child(
                div()
                    .text_size(px(12.0))
                    .font_semibold()
                    .text_color(theme.panel_label_text)
                    .child(label.to_string()),
            )
            .child(input)
            .into_any_element()
    }

    /// 渲染 Switch 字段。
    fn render_switch_field(
        &mut self,
        field_idx: usize,
        label: &str,
        theme: &Theme,
        cx: &mut Context<Self>,
    ) -> gpui::AnyElement {
        let checked = match self.field_states[field_idx] {
            FieldState::Switch(b) => b,
            _ => false,
        };
        let id = format!("field-switch-{}", field_idx);
        div()
            .flex()
            .items_center()
            .justify_between()
            .gap(px(8.0))
            .child(
                div()
                    .text_size(px(12.0))
                    .font_semibold()
                    .text_color(theme.panel_label_text)
                    .child(label.to_string()),
            )
            .child(
                Switch::new(id)
                    .checked(checked)
                    .on_click(cx.listener(move |this, val: &bool, _w, cx| {
                        this.set_switch_field(field_idx, *val, cx);
                    })),
            )
            .into_any_element()
    }

    /// 渲染 Dropdown 字段（按钮组形式）。
    fn render_dropdown_field(
        &mut self,
        field_idx: usize,
        label: &str,
        options: &[DropdownOption],
        theme: &Theme,
        cx: &mut Context<Self>,
    ) -> gpui::AnyElement {
        let current = match &self.field_states[field_idx] {
            FieldState::Dropdown(s) => s.clone(),
            _ => String::new(),
        };
        let lang = self.language;

        let mut col = div().flex().flex_col().gap(px(4.0));
        col = col.child(
            div()
                .text_size(px(12.0))
                .font_semibold()
                .text_color(theme.panel_label_text)
                .child(label.to_string()),
        );

        let mut btn_row = div().flex().flex_col().gap(px(3.0));
        for (idx, opt) in options.iter().enumerate() {
            let is_active = current == opt.value;
            let bg = if is_active {
                theme.panel_label_text
            } else {
                theme.panel_bg
            };
            let text_color = if is_active {
                theme.panel_bg
            } else {
                theme.panel_label_text
            };
            let opt_label = dropdown_option_label(lang, &kind_label_str(lang, &self.node.kind), &opt);
            let val = opt.value.clone();
            let handler = cx.listener(move |this, _: &gpui::MouseDownEvent, _w, cx| {
                this.set_dropdown_field(field_idx, &val, cx);
            });
            btn_row = btn_row.child(
                div()
                    .id(("dropdown-opt", field_idx * 100 + idx))
                    .px(px(8.0))
                    .py(px(5.0))
                    .rounded_md()
                    .bg(bg)
                    .border_1()
                    .border_color(theme.panel_border)
                    .text_size(px(12.0))
                    .text_color(text_color)
                    .child(opt_label)
                    .on_mouse_down(gpui::MouseButton::Left, handler),
            );
        }
        col = col.child(btn_row);
        col.into_any_element()
    }

    /// 渲染 List 字段（动态行 + 添加/删除按钮）。
    fn render_list_field(
        &mut self,
        field_idx: usize,
        label: &str,
        list_spec: &ListSpec,
        theme: &Theme,
        cx: &mut Context<Self>,
    ) -> gpui::AnyElement {
        let lang = self.language;
        let rows = match &self.field_states[field_idx] {
            FieldState::List(r) => r,
            _ => return div().into_any_element(),
        };

        let mut col = div().flex().flex_col().gap(px(6.0));
        col = col.child(
            div()
                .text_size(px(12.0))
                .font_semibold()
                .text_color(theme.panel_label_text)
                .child(label.to_string()),
        );

        // 每行渲染 item_fields
        for (row_idx, row) in rows.iter().enumerate() {
            let mut row_div = div().flex().items_center().gap(px(4.0));

            // 序号
            row_div = row_div.child(
                div()
                    .w(px(28.0))
                    .text_size(px(11.0))
                    .text_color(theme.panel_subtext)
                    .child(format!("#{}", row_idx + 1)),
            );

            // 每个 item_field 一个 Input
            for (col_idx, _item_field) in list_spec.item_fields.iter().enumerate() {
                if col_idx >= row.len() {
                    break;
                }
                let entity = &row[col_idx];
                let w = if list_spec.item_fields.len() == 1 {
                    None
                } else if col_idx == 0 {
                    Some(px(70.0))
                } else {
                    Some(px(80.0))
                };
                let mut input_div = div();
                if let Some(w) = w {
                    input_div = input_div.w(w);
                } else {
                    input_div = input_div.flex_1();
                }
                row_div = row_div.child(input_div.child(Input::new(entity).appearance(true)));
            }

            // 删除按钮
            let del_handler = cx.listener(move |this, _: &gpui::MouseDownEvent, _w, cx| {
                this.delete_list_item(field_idx, row_idx, cx);
            });
            row_div = row_div.child(
                div()
                    .id(("del-list", field_idx * 1000 + row_idx))
                    .w(px(22.0))
                    .h(px(22.0))
                    .flex()
                    .items_center()
                    .justify_center()
                    .rounded_md()
                    .bg(theme.delete_btn_bg)
                    .child(
                        Icon::new(IconName::Close)
                            .xsmall()
                            .text_color(theme.delete_btn_text),
                    )
                    .on_mouse_down(gpui::MouseButton::Left, del_handler),
            );

            col = col.child(row_div);
        }

        // 添加按钮
        let add_label = t(lang, TKey::PanelAddBranch);
        let add_handler = cx.listener(move |this, _: &gpui::MouseDownEvent, w, cx| {
            this.add_list_item(field_idx, w, cx);
        });
        col = col.child(
            div()
                .id(("add-list", field_idx))
                .px(px(8.0))
                .py(px(5.0))
                .rounded_md()
                .bg(theme.toggle_btn_bg)
                .text_size(px(12.0))
                .text_color(theme.toggle_btn_text)
                .text_center()
                .child(format!("{} +", add_label))
                .on_mouse_down(gpui::MouseButton::Left, add_handler),
        );

        col.into_any_element()
    }
}

// ====== 辅助函数 ======

/// 从 node.data 读取 label。
fn label_of(node: &Node) -> String {
    node.data
        .get("label")
        .and_then(|v| v.as_str())
        .unwrap_or(&node.kind)
        .to_string()
}

/// JSON Value → String（支持 string/number/bool）。
fn value_to_string(v: &serde_json::Value) -> String {
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
fn new_code_input(
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
fn build_list_rows(
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
fn sync_list_rows(
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
        // 数量一致：仅更新值
        for (i, item) in arr.iter().enumerate() {
            for (col, item_field) in list_spec.item_fields.iter().enumerate() {
                if col >= rows[i].len() {
                    continue;
                }
                let val = item.get(&item_field.key).cloned().unwrap_or_else(|| item_field.default.clone());
                let text = value_to_string(&val);
                rows[i][col].update(cx, |s, cx| s.set_value(text.as_str(), window, cx));
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

/// 节点类型 → i18n 标签字符串。
fn kind_label_str(lang: Language, kind: &str) -> &'static str {
    match kind {
        "start" => t(lang, TKey::Start),
        "end" => t(lang, TKey::End),
        "action" => t(lang, TKey::Action),
        "condition" => t(lang, TKey::Condition),
        "loop" => t(lang, TKey::Loop),
        "variable" => t(lang, TKey::Variable),
        "adapter" => t(lang, TKey::DataAdapter),
        "agent" => t(lang, TKey::Agent),
        _ => "",
    }
}

/// 字段标签 i18n 映射：(kind, field_key) → TKey → 本地化文案。
fn field_label(lang: Language, kind: &str, field_key: &str, fallback: &str) -> String {
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
        _ => return fallback.to_string(),
    };
    t(lang, tkey).to_string()
}

/// Dropdown 选项标签 i18n 映射。
fn dropdown_option_label(lang: Language, _kind: &str, opt: &DropdownOption) -> String {
    // Loop 模式特殊映射
    match opt.value.as_str() {
        "for_each" => t(lang, TKey::LoopForEach).to_string(),
        "while" => t(lang, TKey::LoopWhile).to_string(),
        "for_loop" => t(lang, TKey::LoopForLoop).to_string(),
        "batch_parallel" => t(lang, TKey::LoopParallel).to_string(),
        _ => opt.label.clone(),
    }
}
