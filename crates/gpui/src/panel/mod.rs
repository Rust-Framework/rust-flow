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
    div, px, App, AppContext, Context, Entity, EventEmitter, InteractiveElement, IntoElement,
    ParentElement, Render, ScrollHandle, Styled, Subscription, Window,
};
use gpui_component::input::{Input, InputEvent, InputState};
use gpui_component::StyledExt;
use rust_agent_flow::Node;

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

    // 通用字段：节点名称
    label_input: Entity<InputState>,

    // Condition 节点专用：条件分支列表
    condition_inputs: Vec<Entity<InputState>>,

    // Loop 节点专用
    loop_expr_input: Entity<InputState>,
    loop_mode: String,

    // 同步标记：避免节点更新时回环触发 on_change
    syncing: bool,

    // 滚动句柄（面板内容可能超长）
    scroll_handle: ScrollHandle,

    _subscriptions: Vec<Subscription>,
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
        window: &mut Window,
        cx: &mut App,
    ) -> Entity<Self> {
        cx.new(|cx| Self::build(node, flow_node, theme, on_action, syntax_service, window, cx))
    }

    fn build(
        node: Node,
        flow_node: Option<Arc<dyn IFlowNode>>,
        theme: Theme,
        on_action: Option<ActionCallback>,
        syntax_service: SharedSyntaxService,
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

        let mut subscriptions = vec![sub_label];
        subscriptions.extend(sub_conds);

        Self {
            node,
            flow_node,
            theme,
            on_action,
            syntax_service,
            label_input,
            condition_inputs,
            loop_expr_input,
            loop_mode,
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
                // 数量变化：重建 condition_inputs
                // 注意：这里不重建 subscription，因为新 InputState 的变化
                // 会在下次 sync 时通过值更新反映。为简化，重建时重新订阅。
                self.condition_inputs.clear();
                for (_id, cond_label) in &conditions {
                    let cond_input = cx.new(|cx| {
                        InputState::new(window, cx)
                            .default_value(cond_label.as_str())
                            .placeholder("条件表达式")
                    });
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

        self.syncing = false;
        cx.notify();
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
        let new_input = cx.new(|cx| {
            InputState::new(window, cx)
                .default_value("")
                .placeholder("条件表达式")
        });
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
}

impl Render for PanelView {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = self.theme;
        let kind = self.node.kind.clone();

        // 面板内容根据节点类型分发
        let content = match kind.as_str() {
            "condition" => self.render_condition_panel(&theme, cx),
            "loop" => self.render_loop_panel(&theme, cx),
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
                .child("Condition 节点（条件分支）"),
        );

        // Kind 信息
        col = col.child(
            div()
                .text_size(px(13.0))
                .text_color(theme.panel_subtext)
                .child(format!("Kind: {}", self.node.kind)),
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
                        .child("节点名称"),
                )
                .child(Input::new(&self.label_input).appearance(true)),
        );

        // 条件分支列表
        col = col.child(
            div()
                .text_size(px(13.0))
                .font_semibold()
                .text_color(theme.panel_label_text)
                .child("条件分支"),
        );

        let n_conditions = self.condition_inputs.len();
        for (i, cond_input) in self.condition_inputs.iter().enumerate() {
            let delete_handler = cx.listener(move |this, _: &gpui::ClickEvent, window, cx| {
                this.delete_branch(i, cx);
                // 触发 sync_from_node 重建 InputState 列表
                // delete_branch 内部已调用 sync_conditions_to_node
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
                            .child(format!("If {}", i + 1)),
                    )
                    .child(div().flex_1().child(Input::new(cond_input).appearance(true)))
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
        let add_handler = cx.listener(|this, _: &gpui::ClickEvent, window, cx| {
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
                .child("+ 添加分支")
                .on_mouse_down(gpui::MouseButton::Left, add_handler),
        );

        // Else 兜底说明
        col = col.child(
            div()
                .text_size(px(12.0))
                .text_color(theme.panel_subtext)
                .child("Else 分支为自动兜底，无需配置条件"),
        );

        col.into_any_element()
    }

    /// 渲染 Loop 节点可编辑面板。
    fn render_loop_panel(
        &mut self,
        theme: &Theme,
        cx: &mut Context<Self>,
    ) -> gpui::AnyElement {
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
                .child("Loop 节点（循环）"),
        );

        // Kind 信息
        col = col.child(
            div()
                .text_size(px(13.0))
                .text_color(theme.panel_subtext)
                .child(format!("Kind: {}", self.node.kind)),
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
                        .child("节点名称"),
                )
                .child(Input::new(&self.label_input).appearance(true)),
        );

        // 循环模式选择器（按钮组）
        col = col.child(
            div()
                .text_size(px(13.0))
                .font_semibold()
                .text_color(theme.panel_label_text)
                .child("循环模式"),
        );

        let modes = [
            ("for_each", "数组循环 (For Each)"),
            ("while", "条件循环 (While)"),
            ("for_loop", "计次循环 (For Loop)"),
            ("batch_parallel", "批量/并行循环"),
        ];

        let mut mode_row = div().flex().flex_col().gap(px(4.0));
        for (mode_key, mode_label) in &modes {
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
            let mode_handler = cx.listener(move |this, _: &gpui::ClickEvent, _window, cx| {
                this.set_loop_mode(&key, cx);
            });
            mode_row = mode_row.child(
                div()
                    .id(("mode", key.as_str()))
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
                        .child("条件表达式 (rhai)"),
                )
                .child(
                    Input::new(&self.loop_expr_input)
                        .appearance(true)
                        .h(px(80.0)),
                ),
        );

        col.into_any_element()
    }

    /// 渲染简单只读面板（用于 Start/End/Action 等无需编辑的节点）。
    fn render_simple_panel(&self, theme: &Theme) -> gpui::AnyElement {
        let label = label_of(&self.node);
        let desc = desc_of(&self.node);
        let mut col = div().flex().flex_col().gap(px(8.0)).p_4().size_full();

        col = col.child(
            div()
                .text_size(px(16.0))
                .font_semibold()
                .text_color(theme.panel_title_text)
                .child(format!("{} 节点", self.node.kind)),
        );

        col = col.child(
            div()
                .text_size(px(13.0))
                .text_color(theme.panel_subtext)
                .child(format!("Kind: {}", self.node.kind)),
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
