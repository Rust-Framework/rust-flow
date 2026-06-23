//! Panel 模块：属性面板容器（有状态实体视图）。
//!
//! PanelView 实现 GPUI `Render` trait，持有可编辑字段对应的 `Entity<InputState>`，
//! 支持 Condition/Loop 节点的属性编辑。
//!
//! 事件流：
//! 1. 用户编辑 Input → InputState 发出 `InputEvent::Change`
//! 2. `subscribe_in` 回调触发 → 调用 `on_action(NodeAction::SetData(...))`
//! 3. `FlowEditorView::handle_node_action` 更新 node.data + relayout
//! 4. `FlowEditorView::render` 检测节点数据变化 → `panel_view.update(sync_from_node)`
//! 5. `sync_from_node` 更新 InputState 值（syncing 标记避免回环）

use std::sync::Arc;

use gpui::{
    div, px, App, AppContext, Context, Entity, InteractiveElement, IntoElement,
    ParentElement, Render, ScrollHandle, Styled, Subscription, Window,
};
use gpui_component::input::{Input, InputEvent, InputState};
use gpui_component::StyledExt;
use rust_agent_flow::Node;

use crate::i18n::{t, Language, TKey};
use crate::node::{ActionCallback, IFlowNode, NodeAction, SharedSyntaxService};
use crate::theme::Theme;

/// 属性面板视图：选中节点时右侧显示。
///
/// 有状态实体视图，持有可编辑字段的 `Entity<InputState>`。
pub struct PanelView {
    pub node: Node,
    pub flow_node: Option<Arc<dyn IFlowNode>>,
    pub theme: Theme,
    pub on_action: Option<ActionCallback>,
    /// 语法高亮服务（用于 CodeEditor 语言映射）。
    pub syntax_service: SharedSyntaxService,
    /// 当前 UI 语言。
    pub language: Language,

    // 通用字段：节点名称
    label_input: Entity<InputState>,

    // Condition 节点专用：条件分支列表
    condition_inputs: Vec<Entity<InputState>>,

    // Loop 节点专用
    loop_expr_input: Entity<InputState>,
    loop_mode: String,

    // Start/End/Variable 节点专用：键值行（name/type/value）
    // param_rows: Start 输入参数；variable_rows: Start 变量定义 + Variable 节点；return_rows: End 返回结果
    param_rows: Vec<KvRow>,
    variable_rows: Vec<KvRow>,
    return_rows: Vec<KvRow>,

    // Agent 节点专用
    agent_model_input: Entity<InputState>,
    agent_prompt_input: Entity<InputState>,

    // 同步标记：避免节点更新时回环触发 on_change
    syncing: bool,

    // 滚动句柄（面板内容可能超长，预留用于未来滚动支持）
    #[allow(dead_code)]
    scroll_handle: ScrollHandle,

    _subscriptions: Vec<Subscription>,
}

/// 键值行：name/type/value 三个输入框，用于参数/变量/返回结果编辑。
struct KvRow {
    name: Entity<InputState>,
    kind: Entity<InputState>,
    value: Entity<InputState>,
}

/// 键值行的同步目标：对应 node.data 中的哪个数组键。
#[derive(Clone, Copy)]
enum KvTarget {
    Params,
    Variables,
    Returns,
}

impl KvTarget {
    fn key(self) -> &'static str {
        match self {
            KvTarget::Params => "params",
            KvTarget::Variables => "variables",
            KvTarget::Returns => "returns",
        }
    }
}

impl PanelView {
    /// 创建 PanelView 实体。
    ///
    /// 在 FlowEditorView::render 中调用，选中节点变化时创建新实例。
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

        // 创建 label InputState
        let label_input = cx.new(|cx| {
            InputState::new(window, cx)
                .default_value(label.as_str())
                .placeholder("节点名称")
        });

        // 订阅 label 变化
        let sub_label = cx.subscribe_in(&label_input, window, Self::on_label_change);

        // Condition 节点：创建条件分支 InputState 列表（CodeEditor + rhai 语法高亮）
        let mut condition_inputs: Vec<Entity<InputState>> = Vec::new();
        let mut sub_conds: Vec<Subscription> = Vec::new();
        if node.kind == "condition" {
            let conditions = get_conditions(&node);
            for (_id, cond_label) in &conditions {
                let cond_input = Self::new_rhai_input(
                    &syntax_service,
                    cond_label,
                    "条件表达式",
                    2,
                    false,
                    window,
                    cx,
                );
                let sub = cx.subscribe_in(&cond_input, window, Self::on_condition_change);
                sub_conds.push(sub);
                condition_inputs.push(cond_input);
            }
        }

        // Loop 节点：创建条件表达式 InputState（CodeEditor + rhai 语法高亮 + 行号）
        let loop_expr_input = if node.kind == "loop" {
            let expr = node
                .data
                .get("loop_expr")
                .and_then(|v| v.as_str())
                .unwrap_or("item > 0");
            let input = Self::new_rhai_input(
                &syntax_service,
                expr,
                "rhai 条件表达式",
                4,
                true,
                window,
                cx,
            );
            let _sub = cx.subscribe_in(&input, window, Self::on_loop_expr_change);
            input
        } else {
            // 非 Loop 节点也创建一个空的（避免 Option 复杂性）
            cx.new(|cx| InputState::new(window, cx))
        };

        let loop_mode = node
            .data
            .get("loop_mode")
            .and_then(|v| v.as_str())
            .unwrap_or("for_each")
            .to_string();

        // Start 节点：输入参数 + 变量定义
        let mut param_rows: Vec<KvRow> = Vec::new();
        let mut variable_rows: Vec<KvRow> = Vec::new();
        let mut return_rows: Vec<KvRow> = Vec::new();
        let mut sub_kv: Vec<Subscription> = Vec::new();

        if node.kind == "start" {
            let params = get_kv_list(&node, "params");
            for (n, k, v) in &params {
                let row = Self::new_kv_row(n, k, v, window, cx, &mut sub_kv, KvTarget::Params);
                param_rows.push(row);
            }
            let vars = get_kv_list(&node, "variables");
            for (n, k, v) in &vars {
                let row = Self::new_kv_row(n, k, v, window, cx, &mut sub_kv, KvTarget::Variables);
                variable_rows.push(row);
            }
        }

        // End 节点：返回结果
        if node.kind == "end" {
            let returns = get_kv_list(&node, "returns");
            for (n, k, v) in &returns {
                let row = Self::new_kv_row(n, k, v, window, cx, &mut sub_kv, KvTarget::Returns);
                return_rows.push(row);
            }
        }

        // Variable 节点：变量定义
        if node.kind == "variable" {
            let vars = get_kv_list(&node, "variables");
            for (n, k, v) in &vars {
                let row = Self::new_kv_row(n, k, v, window, cx, &mut sub_kv, KvTarget::Variables);
                variable_rows.push(row);
            }
        }

        // Agent 节点：模型 + 系统提示词
        let agent_model_input = if node.kind == "agent" {
            let model = node
                .data
                .get("model")
                .and_then(|v| v.as_str())
                .unwrap_or("");
            let input = cx.new(|cx| {
                InputState::new(window, cx)
                    .default_value(model)
                    .placeholder("model")
            });
            let _sub = cx.subscribe_in(&input, window, Self::on_agent_model_change);
            input
        } else {
            cx.new(|cx| InputState::new(window, cx))
        };

        let agent_prompt_input = if node.kind == "agent" {
            let prompt = node
                .data
                .get("prompt")
                .and_then(|v| v.as_str())
                .unwrap_or("");
            let input = cx.new(|cx| {
                InputState::new(window, cx)
                    .default_value(prompt)
                    .placeholder("system prompt")
                    .multi_line(true)
                    .rows(4)
            });
            let _sub = cx.subscribe_in(&input, window, Self::on_agent_prompt_change);
            input
        } else {
            cx.new(|cx| InputState::new(window, cx))
        };

        let mut subscriptions = vec![sub_label];
        subscriptions.extend(sub_conds);
        subscriptions.extend(sub_kv);

        Self {
            node,
            flow_node,
            theme,
            on_action,
            syntax_service,
            language,
            label_input,
            condition_inputs,
            loop_expr_input,
            loop_mode,
            param_rows,
            variable_rows,
            return_rows,
            agent_model_input,
            agent_prompt_input,
            syncing: false,
            scroll_handle: ScrollHandle::default(),
            _subscriptions: subscriptions,
        }
    }

    /// 创建 rhai CodeEditor InputState。
    ///
    /// 若语法服务支持 rhai，使用 `code_editor` 模式（语法高亮 + 自动缩进）；
    /// 否则回退到普通 `multi_line` Input。
    ///
    /// `rows` 控制可见行数，`line_number` 控制是否显示行号
    ///（Condition 条件项单行关闭行号，Loop 表达式多行开启行号）。
    fn new_rhai_input(
        syntax_service: &SharedSyntaxService,
        default_value: &str,
        placeholder: &str,
        rows: usize,
        line_number: bool,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Entity<InputState> {
        let language = syntax_service.language_for("rhai");
        cx.new(|cx| {
            let mut state = InputState::new(window, cx)
                .default_value(default_value)
                .placeholder(placeholder);
            if let Some(lang) = language {
                state = state.code_editor(lang).line_number(line_number).rows(rows);
            } else {
                state = state.multi_line(true).rows(rows);
            }
            state
        })
    }

    /// 创建一个键值行（name/type/value），并订阅变化事件。
    fn new_kv_row(
        name: &str,
        kind: &str,
        value: &str,
        window: &mut Window,
        cx: &mut Context<Self>,
        subs: &mut Vec<Subscription>,
        target: KvTarget,
    ) -> KvRow {
        let name_input = cx.new(|cx| {
            InputState::new(window, cx)
                .default_value(name)
                .placeholder("name")
        });
        let kind_input = cx.new(|cx| {
            InputState::new(window, cx)
                .default_value(kind)
                .placeholder("type")
        });
        let value_input = cx.new(|cx| {
            InputState::new(window, cx)
                .default_value(value)
                .placeholder("value")
        });
        subs.push(cx.subscribe_in(&name_input, window, move |this, _, e, w, cx| {
            this.on_kv_change(target, e, w, cx);
        }));
        subs.push(cx.subscribe_in(&kind_input, window, move |this, _, e, w, cx| {
            this.on_kv_change(target, e, w, cx);
        }));
        subs.push(cx.subscribe_in(&value_input, window, move |this, _, e, w, cx| {
            this.on_kv_change(target, e, w, cx);
        }));
        KvRow {
            name: name_input,
            kind: kind_input,
            value: value_input,
        }
    }

    /// 节点数据变化时，从 node 同步到 InputState（避免回环）。
    ///
    /// 在 FlowEditorView::render 中调用，当 panel 对应的节点数据被外部修改时同步。
    pub fn sync_from_node(&mut self, node: Node, window: &mut Window, cx: &mut Context<Self>) {
        if self.node.id != node.id {
            // 节点 ID 变了，说明选中了不同节点，不应调用 sync，应由 new 重建
            return;
        }

        self.syncing = true;
        self.node = node;

        // 同步 label
        let label = label_of(&self.node);
        self.label_input.update(cx, |s, cx| {
            s.set_value(label.as_str(), window, cx);
        });

        // 同步条件分支（数量变化时重建 InputState 列表）
        if self.node.kind == "condition" {
            let conditions = get_conditions(&self.node);
            // 如果数量一致，仅更新值；否则重建
            if conditions.len() == self.condition_inputs.len() {
                for (i, (_id, cond_label)) in conditions.iter().enumerate() {
                    self.condition_inputs[i].update(cx, |s, cx| {
                        s.set_value(cond_label.as_str(), window, cx);
                    });
                }
            } else {
                // 数量变化：重建 condition_inputs（保持 CodeEditor 模式）
                self.condition_inputs.clear();
                for (_id, cond_label) in &conditions {
                    let cond_input = Self::new_rhai_input(
                        &self.syntax_service,
                        cond_label,
                        "条件表达式",
                        2,
                        false,
                        window,
                        cx,
                    );
                    let _sub = cx.subscribe_in(&cond_input, window, Self::on_condition_change);
                    self.condition_inputs.push(cond_input);
                }
            }
        }

        // 同步 Loop 表达式
        if self.node.kind == "loop" {
            let expr = self
                .node
                .data
                .get("loop_expr")
                .and_then(|v| v.as_str())
                .unwrap_or("");
            self.loop_expr_input.update(cx, |s, cx| {
                s.set_value(expr, window, cx);
            });
            self.loop_mode = self
                .node
                .data
                .get("loop_mode")
                .and_then(|v| v.as_str())
                .unwrap_or("for_each")
                .to_string();
        }

        // 同步 Start 参数/变量、End 返回、Variable 变量
        if matches!(self.node.kind.as_str(), "start" | "end" | "variable") {
            self.sync_kv_rows(KvTarget::Params, window, cx);
            self.sync_kv_rows(KvTarget::Variables, window, cx);
            self.sync_kv_rows(KvTarget::Returns, window, cx);
        }

        // 同步 Agent 模型/提示词
        if self.node.kind == "agent" {
            let model = self
                .node
                .data
                .get("model")
                .and_then(|v| v.as_str())
                .unwrap_or("");
            self.agent_model_input.update(cx, |s, cx| {
                s.set_value(model, window, cx);
            });
            let prompt = self
                .node
                .data
                .get("prompt")
                .and_then(|v| v.as_str())
                .unwrap_or("");
            self.agent_prompt_input.update(cx, |s, cx| {
                s.set_value(prompt, window, cx);
            });
        }

        self.syncing = false;
        cx.notify();
    }

    /// 同步指定目标的键值行（数量变化时重建）。
    fn sync_kv_rows(
        &mut self,
        target: KvTarget,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let rows = match target {
            KvTarget::Params => &mut self.param_rows,
            KvTarget::Variables => &mut self.variable_rows,
            KvTarget::Returns => &mut self.return_rows,
        };

        // 仅当节点类型匹配该目标时同步
        let should_sync = match target {
            KvTarget::Params => self.node.kind == "start",
            KvTarget::Variables => matches!(self.node.kind.as_str(), "start" | "variable"),
            KvTarget::Returns => self.node.kind == "end",
        };
        if !should_sync {
            return;
        }

        let kv_list = get_kv_list(&self.node, target.key());
        if kv_list.len() == rows.len() {
            for (i, (n, k, v)) in kv_list.iter().enumerate() {
                rows[i].name.update(cx, |s, cx| s.set_value(n.as_str(), window, cx));
                rows[i].kind.update(cx, |s, cx| s.set_value(k.as_str(), window, cx));
                rows[i].value.update(cx, |s, cx| s.set_value(v.as_str(), window, cx));
            }
        } else {
            rows.clear();
            let mut subs: Vec<Subscription> = Vec::new();
            for (n, k, v) in &kv_list {
                let row = Self::new_kv_row(n, k, v, window, cx, &mut subs, target);
                rows.push(row);
            }
            self._subscriptions.extend(subs);
        }
    }

    // ====== 事件回调 ======

    fn on_label_change(
        &mut self,
        _state: &Entity<InputState>,
        _event: &InputEvent,
        _window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self.syncing {
            return;
        }
        if !matches!(_event, InputEvent::Change) {
            return;
        }
        let value = self.label_input.read(cx).value().to_string();
        if let Some(on_action) = &self.on_action {
            on_action(
                NodeAction::SetData("label".into(), serde_json::json!(value)),
                cx,
            );
        }
    }

    fn on_condition_change(
        &mut self,
        _state: &Entity<InputState>,
        _event: &InputEvent,
        _window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self.syncing {
            return;
        }
        if !matches!(_event, InputEvent::Change) {
            return;
        }
        self.sync_conditions_to_node(cx);
    }

    fn on_loop_expr_change(
        &mut self,
        _state: &Entity<InputState>,
        _event: &InputEvent,
        _window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self.syncing {
            return;
        }
        if !matches!(_event, InputEvent::Change) {
            return;
        }
        let value = self.loop_expr_input.read(cx).value().to_string();
        if let Some(on_action) = &self.on_action {
            on_action(
                NodeAction::SetData("loop_expr".into(), serde_json::json!(value)),
                cx,
            );
        }
    }

    /// 键值行变化回调：同步到 node.data。
    fn on_kv_change(
        &mut self,
        target: KvTarget,
        _event: &InputEvent,
        _window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self.syncing {
            return;
        }
        self.sync_kv_to_node(target, cx);
    }

    /// Agent 模型变化回调。
    fn on_agent_model_change(
        &mut self,
        _state: &Entity<InputState>,
        _event: &InputEvent,
        _window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self.syncing || !matches!(_event, InputEvent::Change) {
            return;
        }
        let value = self.agent_model_input.read(cx).value().to_string();
        if let Some(on_action) = &self.on_action {
            on_action(
                NodeAction::SetData("model".into(), serde_json::json!(value)),
                cx,
            );
        }
    }

    /// Agent 系统提示词变化回调。
    fn on_agent_prompt_change(
        &mut self,
        _state: &Entity<InputState>,
        _event: &InputEvent,
        _window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self.syncing || !matches!(_event, InputEvent::Change) {
            return;
        }
        let value = self.agent_prompt_input.read(cx).value().to_string();
        if let Some(on_action) = &self.on_action {
            on_action(
                NodeAction::SetData("prompt".into(), serde_json::json!(value)),
                cx,
            );
        }
    }

    // ====== 条件分支操作 ======

    fn sync_conditions_to_node(&self, cx: &mut Context<Self>) {
        let conditions: Vec<serde_json::Value> = self
            .condition_inputs
            .iter()
            .enumerate()
            .map(|(i, input)| {
                let label = input.read(cx).value().to_string();
                serde_json::json!({ "id": format!("if_{}", i), "label": label })
            })
            .collect();
        if let Some(on_action) = &self.on_action {
            on_action(
                NodeAction::SetData("conditions".into(), serde_json::json!(conditions)),
                cx,
            );
        }
    }

    /// 添加条件分支。
    pub fn add_branch(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let new_input = Self::new_rhai_input(
            &self.syntax_service,
            "",
            "条件表达式",
            2,
            false,
            window,
            cx,
        );
        let _sub = cx.subscribe_in(&new_input, window, Self::on_condition_change);
        self.condition_inputs.push(new_input);
        self.sync_conditions_to_node(cx);
    }

    /// 删除条件分支。
    pub fn delete_branch(&mut self, idx: usize, cx: &mut Context<Self>) {
        if idx < self.condition_inputs.len() {
            self.condition_inputs.remove(idx);
            self.sync_conditions_to_node(cx);
        }
    }

    /// 设置循环模式。
    pub fn set_loop_mode(&mut self, mode: &str, cx: &mut Context<Self>) {
        self.loop_mode = mode.to_string();
        if let Some(on_action) = &self.on_action {
            on_action(
                NodeAction::SetData("loop_mode".into(), serde_json::json!(mode)),
                cx,
            );
        }
    }

    // ====== 键值行操作（参数/变量/返回） ======

    /// 将键值行同步到 node.data。
    fn sync_kv_to_node(&self, target: KvTarget, cx: &mut Context<Self>) {
        let rows = match target {
            KvTarget::Params => &self.param_rows,
            KvTarget::Variables => &self.variable_rows,
            KvTarget::Returns => &self.return_rows,
        };
        let arr: Vec<serde_json::Value> = rows
            .iter()
            .map(|r| {
                serde_json::json!({
                    "name": r.name.read(cx).value().to_string(),
                    "type": r.kind.read(cx).value().to_string(),
                    "value": r.value.read(cx).value().to_string(),
                })
            })
            .collect();
        if let Some(on_action) = &self.on_action {
            on_action(
                NodeAction::SetData(target.key().into(), serde_json::json!(arr)),
                cx,
            );
        }
    }

    /// 添加键值行。
    fn add_kv(&mut self, target: KvTarget, window: &mut Window, cx: &mut Context<Self>) {
        let mut subs: Vec<Subscription> = Vec::new();
        let row = Self::new_kv_row("", "", "", window, cx, &mut subs, target);
        self._subscriptions.extend(subs);
        match target {
            KvTarget::Params => self.param_rows.push(row),
            KvTarget::Variables => self.variable_rows.push(row),
            KvTarget::Returns => self.return_rows.push(row),
        }
        self.sync_kv_to_node(target, cx);
    }

    /// 删除键值行。
    fn delete_kv(&mut self, target: KvTarget, idx: usize, cx: &mut Context<Self>) {
        let rows = match target {
            KvTarget::Params => &mut self.param_rows,
            KvTarget::Variables => &mut self.variable_rows,
            KvTarget::Returns => &mut self.return_rows,
        };
        if idx < rows.len() {
            rows.remove(idx);
            self.sync_kv_to_node(target, cx);
        }
    }
}

impl Render for PanelView {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = self.theme;
        let kind = self.node.kind.clone();

        // 面板内容根据节点类型分发
        let content = match kind.as_str() {
            "condition" => self.render_condition_panel(&theme, cx),
            "loop" => self.render_loop_panel(&theme, cx),
            "start" => self.render_start_panel(&theme, cx),
            "end" => self.render_end_panel(&theme, cx),
            "variable" => self.render_variable_panel(&theme, cx),
            "agent" => self.render_agent_panel(&theme, cx),
            _ => self.render_simple_panel(&theme),
        };

        // 外层容器：固定宽度，全高，面板背景色，左边框
        div()
            .w(px(320.0))
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
    /// 渲染 Condition 节点可编辑面板。
    fn render_condition_panel(
        &mut self,
        theme: &Theme,
        cx: &mut Context<Self>,
    ) -> gpui::AnyElement {
        let lang = self.language;
        let mut col = div()
            .flex()
            .flex_col()
            .gap(px(12.0))
            .p_4()
            .size_full()
            .overflow_hidden();

        // 标题
        col = col.child(
            div()
                .text_size(px(16.0))
                .font_semibold()
                .text_color(theme.panel_title_text)
                .child(t(lang, TKey::PanelConditionTitle).to_string()),
        );

        // Kind 信息
        col = col.child(
            div()
                .text_size(px(13.0))
                .text_color(theme.panel_subtext)
                .child(format!("{}: {}", t(lang, TKey::PanelKind), self.node.kind)),
        );

        // 节点名称
        col = col.child(
            div()
                .flex()
                .flex_col()
                .gap(px(4.0))
                .child(
                    div()
                        .text_size(px(13.0))
                        .font_semibold()
                        .text_color(theme.panel_label_text)
                        .child(t(lang, TKey::PanelNodeName).to_string()),
                )
                .child(Input::new(&self.label_input).appearance(true)),
        );

        // 条件分支列表
        col = col.child(
            div()
                .text_size(px(13.0))
                .font_semibold()
                .text_color(theme.panel_label_text)
                .child(t(lang, TKey::PanelConditions).to_string()),
        );

        for (i, cond_input) in self.condition_inputs.iter().enumerate() {
            let delete_handler = cx.listener(move |this, _: &gpui::MouseDownEvent, _window, cx| {
                this.delete_branch(i, cx);
            });

            col = col.child(
                div()
                    .flex()
                    .items_center()
                    .gap(px(8.0))
                    .child(
                        div()
                            .text_size(px(12.0))
                            .text_color(theme.panel_subtext)
                            .w(px(40.0))
                            .child(format!("{} {}", t(lang, TKey::If), i + 1)),
                    )
                    .child(div().flex_1().child(Input::new(cond_input).appearance(true).h(px(56.0))))
                    .child(
                        div()
                            .id(("delete_branch", i))
                            .px(px(6.0))
                            .py(px(4.0))
                            .rounded_md()
                            .bg(theme.delete_btn_bg)
                            .text_size(px(12.0))
                            .text_color(theme.delete_btn_text)
                            .child("×")
                            .on_mouse_down(gpui::MouseButton::Left, delete_handler),
                    ),
            );
        }

        // 添加分支按钮
        let add_handler = cx.listener(|this, _: &gpui::MouseDownEvent, window, cx| {
            this.add_branch(window, cx);
        });
        col = col.child(
            div()
                .id("add_branch")
                .px(px(8.0))
                .py(px(6.0))
                .rounded_md()
                .bg(theme.toggle_btn_bg)
                .text_size(px(12.0))
                .text_color(theme.toggle_btn_text)
                .text_center()
                .child(t(lang, TKey::PanelAddBranch).to_string())
                .on_mouse_down(gpui::MouseButton::Left, add_handler),
        );

        // Else 兜底说明
        col = col.child(
            div()
                .text_size(px(12.0))
                .text_color(theme.panel_subtext)
                .child(t(lang, TKey::PanelElseHint).to_string()),
        );

        col.into_any_element()
    }

    /// 渲染 Loop 节点可编辑面板。
    fn render_loop_panel(
        &mut self,
        theme: &Theme,
        cx: &mut Context<Self>,
    ) -> gpui::AnyElement {
        let lang = self.language;
        let mut col = div()
            .flex()
            .flex_col()
            .gap(px(12.0))
            .p_4()
            .size_full()
            .overflow_hidden();

        // 标题
        col = col.child(
            div()
                .text_size(px(16.0))
                .font_semibold()
                .text_color(theme.panel_title_text)
                .child(t(lang, TKey::PanelLoopTitle).to_string()),
        );

        // Kind 信息
        col = col.child(
            div()
                .text_size(px(13.0))
                .text_color(theme.panel_subtext)
                .child(format!("{}: {}", t(lang, TKey::PanelKind), self.node.kind)),
        );

        // 节点名称
        col = col.child(
            div()
                .flex()
                .flex_col()
                .gap(px(4.0))
                .child(
                    div()
                        .text_size(px(13.0))
                        .font_semibold()
                        .text_color(theme.panel_label_text)
                        .child(t(lang, TKey::PanelNodeName).to_string()),
                )
                .child(Input::new(&self.label_input).appearance(true)),
        );

        // 循环模式选择器（按钮组）
        col = col.child(
            div()
                .text_size(px(13.0))
                .font_semibold()
                .text_color(theme.panel_label_text)
                .child(t(lang, TKey::PanelLoopMode).to_string()),
        );

        let modes = [
            ("for_each", t(lang, TKey::LoopForEach)),
            ("while", t(lang, TKey::LoopWhile)),
            ("for_loop", t(lang, TKey::LoopForLoop)),
            ("batch_parallel", t(lang, TKey::LoopParallel)),
        ];

        let mut mode_row = div().flex().flex_col().gap(px(4.0));
        for (idx, (mode_key, mode_label)) in modes.iter().enumerate() {
            let is_active = self.loop_mode == *mode_key;
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
            let key = mode_key.to_string();
            let mode_handler = cx.listener(move |this, _: &gpui::MouseDownEvent, _window, cx| {
                this.set_loop_mode(&key, cx);
            });
            mode_row = mode_row.child(
                div()
                    .id(("mode", idx))
                    .px(px(8.0))
                    .py(px(6.0))
                    .rounded_md()
                    .bg(bg)
                    .border_1()
                    .border_color(theme.panel_border)
                    .text_size(px(12.0))
                    .text_color(text_color)
                    .child(*mode_label)
                    .on_mouse_down(gpui::MouseButton::Left, mode_handler),
            );
        }
        col = col.child(mode_row);

        // 条件表达式（rhai 嵌入语法）
        col = col.child(
            div()
                .flex()
                .flex_col()
                .gap(px(4.0))
                .child(
                    div()
                        .text_size(px(13.0))
                        .font_semibold()
                        .text_color(theme.panel_label_text)
                        .child(t(lang, TKey::PanelLoopExpr).to_string()),
                )
                .child(
                    Input::new(&self.loop_expr_input)
                        .appearance(true)
                        .h(px(120.0)),
                ),
        );

        col.into_any_element()
    }

    /// 渲染简单只读面板（用于 Start/End/Action 等无需编辑的节点）。
    fn render_simple_panel(&self, theme: &Theme) -> gpui::AnyElement {
        let lang = self.language;
        let label = label_of(&self.node);
        let desc = desc_of(&self.node);
        let mut col = div().flex().flex_col().gap(px(8.0)).p_4().size_full();

        // 标题：类型 + "节点"
        let kind_label = match self.node.kind.as_str() {
            "start" => t(lang, TKey::Start),
            "end" => t(lang, TKey::End),
            "action" => t(lang, TKey::Action),
            _ => self.node.kind.as_str(),
        };
        let title = format!("{} {}", kind_label, t(lang, TKey::PanelNodeSuffix));
        col = col.child(
            div()
                .text_size(px(16.0))
                .font_semibold()
                .text_color(theme.panel_title_text)
                .child(title),
        );

        col = col.child(
            div()
                .text_size(px(13.0))
                .text_color(theme.panel_subtext)
                .child(format!("{}: {}", t(lang, TKey::PanelKind), self.node.kind)),
        );

        col = col.child(
            div()
                .text_size(px(13.0))
                .text_color(theme.panel_label_text)
                .child(format!("{}: {}", t(lang, TKey::PanelLabel), label)),
        );

        if let Some(desc) = desc {
            col = col.child(
                div()
                    .text_size(px(13.0))
                    .text_color(theme.panel_subtext)
                    .child(format!("{}: {}", t(lang, TKey::PanelDesc), desc)),
            );
        }

        col.into_any_element()
    }

    /// 渲染键值表（参数/变量/返回结果通用）。
    fn render_kv_table(
        &mut self,
        theme: &Theme,
        title: &str,
        add_label: &str,
        target: KvTarget,
        cx: &mut Context<Self>,
    ) -> gpui::AnyElement {
        let lang = self.language;
        let rows = match target {
            KvTarget::Params => &self.param_rows,
            KvTarget::Variables => &self.variable_rows,
            KvTarget::Returns => &self.return_rows,
        };

        let mut col = div().flex().flex_col().gap(px(6.0));

        // 表头
        col = col.child(
            div()
                .text_size(px(13.0))
                .font_semibold()
                .text_color(theme.panel_label_text)
                .child(title.to_string()),
        );

        // 表头列名
        col = col.child(
            div()
                .flex()
                .gap(px(4.0))
                .child(
                    div()
                        .w(px(80.0))
                        .text_size(px(11.0))
                        .text_color(theme.panel_subtext)
                        .child(t(lang, TKey::PanelParamName).to_string()),
                )
                .child(
                    div()
                        .w(px(60.0))
                        .text_size(px(11.0))
                        .text_color(theme.panel_subtext)
                        .child(t(lang, TKey::PanelParamType).to_string()),
                )
                .child(
                    div()
                        .flex_1()
                        .text_size(px(11.0))
                        .text_color(theme.panel_subtext)
                        .child(t(lang, TKey::PanelParamValue).to_string()),
                )
                .child(div().w(px(24.0))),
        );

        // 数据行
        for (i, row) in rows.iter().enumerate() {
            let delete_handler =
                cx.listener(move |this, _: &gpui::MouseDownEvent, _window, cx| {
                    this.delete_kv(target, i, cx);
                });
            col = col.child(
                div()
                    .flex()
                    .items_center()
                    .gap(px(4.0))
                    .child(
                        div()
                            .w(px(80.0))
                            .child(Input::new(&row.name).appearance(true)),
                    )
                    .child(
                        div()
                            .w(px(60.0))
                            .child(Input::new(&row.kind).appearance(true)),
                    )
                    .child(
                        div()
                            .flex_1()
                            .child(Input::new(&row.value).appearance(true)),
                    )
                    .child(
                        div()
                            .id(("del_kv", i))
                            .w(px(24.0))
                            .h(px(24.0))
                            .flex()
                            .items_center()
                            .justify_center()
                            .rounded_md()
                            .bg(theme.delete_btn_bg)
                            .text_size(px(14.0))
                            .text_color(theme.delete_btn_text)
                            .child("×")
                            .on_mouse_down(gpui::MouseButton::Left, delete_handler),
                    ),
            );
        }

        // 添加按钮
        let add_handler = cx.listener(move |this, _: &gpui::MouseDownEvent, window, cx| {
            this.add_kv(target, window, cx);
        });
        let add_id = match target {
            KvTarget::Params => "add_kv_params",
            KvTarget::Variables => "add_kv_variables",
            KvTarget::Returns => "add_kv_returns",
        };
        col = col.child(
            div()
                .id(add_id)
                .px(px(8.0))
                .py(px(4.0))
                .rounded_md()
                .bg(theme.toggle_btn_bg)
                .text_size(px(12.0))
                .text_color(theme.toggle_btn_text)
                .text_center()
                .child(add_label.to_string())
                .on_mouse_down(gpui::MouseButton::Left, add_handler),
        );

        col.into_any_element()
    }

    /// 渲染 Start 节点面板：节点名称 + 输入参数表 + 变量定义表。
    fn render_start_panel(
        &mut self,
        theme: &Theme,
        cx: &mut Context<Self>,
    ) -> gpui::AnyElement {
        let lang = self.language;
        let mut col = div()
            .flex()
            .flex_col()
            .gap(px(12.0))
            .p_4()
            .size_full()
            .overflow_hidden();

        // 标题
        col = col.child(
            div()
                .text_size(px(16.0))
                .font_semibold()
                .text_color(theme.panel_title_text)
                .child(t(lang, TKey::PanelStartTitle).to_string()),
        );

        // 节点名称
        col = col.child(
            div()
                .flex()
                .flex_col()
                .gap(px(4.0))
                .child(
                    div()
                        .text_size(px(13.0))
                        .font_semibold()
                        .text_color(theme.panel_label_text)
                        .child(t(lang, TKey::PanelNodeName).to_string()),
                )
                .child(Input::new(&self.label_input).appearance(true)),
        );

        // 输入参数表
        col = col.child(self.render_kv_table(
            theme,
            t(lang, TKey::PanelParams),
            t(lang, TKey::PanelAddParam),
            KvTarget::Params,
            cx,
        ));

        // 变量定义表
        col = col.child(self.render_kv_table(
            theme,
            t(lang, TKey::PanelVariables),
            t(lang, TKey::PanelAddVariable),
            KvTarget::Variables,
            cx,
        ));

        col.into_any_element()
    }

    /// 渲染 End 节点面板：节点名称 + 返回结果表。
    fn render_end_panel(
        &mut self,
        theme: &Theme,
        cx: &mut Context<Self>,
    ) -> gpui::AnyElement {
        let lang = self.language;
        let mut col = div()
            .flex()
            .flex_col()
            .gap(px(12.0))
            .p_4()
            .size_full()
            .overflow_hidden();

        // 标题
        col = col.child(
            div()
                .text_size(px(16.0))
                .font_semibold()
                .text_color(theme.panel_title_text)
                .child(t(lang, TKey::PanelEndTitle).to_string()),
        );

        // 节点名称
        col = col.child(
            div()
                .flex()
                .flex_col()
                .gap(px(4.0))
                .child(
                    div()
                        .text_size(px(13.0))
                        .font_semibold()
                        .text_color(theme.panel_label_text)
                        .child(t(lang, TKey::PanelNodeName).to_string()),
                )
                .child(Input::new(&self.label_input).appearance(true)),
        );

        // 返回结果表
        col = col.child(self.render_kv_table(
            theme,
            t(lang, TKey::PanelReturns),
            t(lang, TKey::PanelAddReturn),
            KvTarget::Returns,
            cx,
        ));

        col.into_any_element()
    }

    /// 渲染 Variable 节点面板：节点名称 + 变量定义表。
    fn render_variable_panel(
        &mut self,
        theme: &Theme,
        cx: &mut Context<Self>,
    ) -> gpui::AnyElement {
        let lang = self.language;
        let mut col = div()
            .flex()
            .flex_col()
            .gap(px(12.0))
            .p_4()
            .size_full()
            .overflow_hidden();

        // 标题
        col = col.child(
            div()
                .text_size(px(16.0))
                .font_semibold()
                .text_color(theme.panel_title_text)
                .child(t(lang, TKey::PanelVariableTitle).to_string()),
        );

        // 节点名称
        col = col.child(
            div()
                .flex()
                .flex_col()
                .gap(px(4.0))
                .child(
                    div()
                        .text_size(px(13.0))
                        .font_semibold()
                        .text_color(theme.panel_label_text)
                        .child(t(lang, TKey::PanelNodeName).to_string()),
                )
                .child(Input::new(&self.label_input).appearance(true)),
        );

        // 变量定义表
        col = col.child(self.render_kv_table(
            theme,
            t(lang, TKey::PanelVariables),
            t(lang, TKey::PanelAddVariable),
            KvTarget::Variables,
            cx,
        ));

        col.into_any_element()
    }

    /// 渲染 Agent 节点面板：节点名称 + 模型 + 系统提示词。
    fn render_agent_panel(
        &mut self,
        theme: &Theme,
        cx: &mut Context<Self>,
    ) -> gpui::AnyElement {
        let _ = cx;
        let lang = self.language;
        let mut col = div()
            .flex()
            .flex_col()
            .gap(px(12.0))
            .p_4()
            .size_full()
            .overflow_hidden();

        // 标题
        col = col.child(
            div()
                .text_size(px(16.0))
                .font_semibold()
                .text_color(theme.panel_title_text)
                .child(t(lang, TKey::PanelAgentTitle).to_string()),
        );

        // 节点名称
        col = col.child(
            div()
                .flex()
                .flex_col()
                .gap(px(4.0))
                .child(
                    div()
                        .text_size(px(13.0))
                        .font_semibold()
                        .text_color(theme.panel_label_text)
                        .child(t(lang, TKey::PanelNodeName).to_string()),
                )
                .child(Input::new(&self.label_input).appearance(true)),
        );

        // 模型
        col = col.child(
            div()
                .flex()
                .flex_col()
                .gap(px(4.0))
                .child(
                    div()
                        .text_size(px(13.0))
                        .font_semibold()
                        .text_color(theme.panel_label_text)
                        .child(t(lang, TKey::PanelAgentModel).to_string()),
                )
                .child(Input::new(&self.agent_model_input).appearance(true)),
        );

        // 系统提示词
        col = col.child(
            div()
                .flex()
                .flex_col()
                .gap(px(4.0))
                .child(
                    div()
                        .text_size(px(13.0))
                        .font_semibold()
                        .text_color(theme.panel_label_text)
                        .child(t(lang, TKey::PanelAgentPrompt).to_string()),
                )
                .child(
                    Input::new(&self.agent_prompt_input)
                        .appearance(true)
                        .h(px(120.0)),
                ),
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

/// 从 node.data 读取 desc。
fn desc_of(node: &Node) -> Option<String> {
    node.data
        .get("desc")
        .and_then(|v| v.as_str())
        .map(|s| s.to_string())
}

/// 从 node.data 解析键值列表 `(name, type, value)`。
///
/// 用于 Start 参数/变量、End 返回结果、Variable 节点变量。
fn get_kv_list(node: &Node, key: &str) -> Vec<(String, String, String)> {
    node.data
        .get(key)
        .and_then(|v| v.as_array())
        .map(|arr| {
            arr.iter()
                .map(|item| {
                    let name = item.get("name").and_then(|v| v.as_str()).unwrap_or("").to_string();
                    let kind = item.get("type").and_then(|v| v.as_str()).unwrap_or("").to_string();
                    let value = item.get("value").and_then(|v| v.as_str()).unwrap_or("").to_string();
                    (name, kind, value)
                })
                .collect()
        })
        .unwrap_or_default()
}

/// 从 node.data 解析条件项列表 `(id, label)`。
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
