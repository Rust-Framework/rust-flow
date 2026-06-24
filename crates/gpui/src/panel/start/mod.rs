//! Start 节点专属属性面板：支持参数和变量的树形编辑。
//!
//! 与通用 [`PanelView`](super::PanelView) 不同，StartPanelView 专为 Start 节点设计：
//! - 接收参数：0-N 个，基础/复杂/动态类型，子字段值只读
//! - 变量定义：0-N 个，基础/复杂/动态类型，子字段值可编辑
//! - 复杂类型：预定义结构，结构不可编辑
//! - 动态类型（DynamicObject）：结构可手动编辑（增删改字段）
//!
//! 文件拆分：
//! - `data_types.rs`：JSON 辅助函数
//! - `item.rs`：单项状态与渲染（基础行 + 复杂/动态树形）
//! - `section.rs`：列表区块渲染（标题 + 项列表 + 添加按钮）
//! - `mod.rs`：面板实体、状态管理、Render 实现

use std::sync::Arc;

use gpui::{
    div, px, App, AppContext, Context, Entity, InteractiveElement, IntoElement, ParentElement,
    Render, ScrollHandle, StatefulInteractiveElement, Styled, Subscription, Window,
};
use gpui_component::input::{Input, InputEvent, InputState};
use gpui_component::{Icon, Sizable, StyledExt};
use rust_agent_flow::Node;

use crate::builtin::common::node_icon;
use crate::data_type::{DataTypeRegistry, SharedDataTypeProvider};
use crate::i18n::{kind_label, t, Language, TKey};
use crate::node::{ActionCallback, IFlowNode, NodeAction, SharedSyntaxService};
use crate::theme::Theme;

pub mod data_types;
pub mod item;
pub mod section;

use data_types::{build_default_item, build_item_for_type};
use item::ItemState;

/// Start 节点属性面板视图。
pub struct StartPanelView {
    pub node: Node,
    pub flow_node: Option<Arc<dyn IFlowNode>>,
    pub theme: Theme,
    pub on_action: Option<ActionCallback>,
    pub syntax_service: SharedSyntaxService,
    pub language: Language,

    /// 数据类型注册表（内置类型 + provider 注入类型）。
    registry: DataTypeRegistry,
    /// 节点名称输入。
    label_input: Entity<InputState>,
    /// 参数项状态列表。
    params_state: Vec<ItemState>,
    /// 变量项状态列表。
    variables_state: Vec<ItemState>,

    /// 同步标记：避免回环。
    syncing: bool,
    /// 内容区滚动句柄。
    scroll_handle: ScrollHandle,
    _subscriptions: Vec<Subscription>,
}

impl StartPanelView {
    /// 创建 StartPanelView 实体。
    pub fn new(
        node: Node,
        flow_node: Option<Arc<dyn IFlowNode>>,
        theme: Theme,
        on_action: Option<ActionCallback>,
        syntax_service: SharedSyntaxService,
        language: Language,
        data_type_provider: Option<SharedDataTypeProvider>,
        window: &mut Window,
        cx: &mut App,
    ) -> Entity<Self> {
        cx.new(|cx| {
            Self::build(
                node,
                flow_node,
                theme,
                on_action,
                syntax_service,
                language,
                data_type_provider,
                window,
                cx,
            )
        })
    }

    fn build(
        node: Node,
        flow_node: Option<Arc<dyn IFlowNode>>,
        theme: Theme,
        on_action: Option<ActionCallback>,
        syntax_service: SharedSyntaxService,
        language: Language,
        data_type_provider: Option<SharedDataTypeProvider>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Self {
        let registry = DataTypeRegistry::new(data_type_provider);

        let label = label_of(&node);
        let label_placeholder = kind_label(language, &node.kind).to_string();
        let label_input = cx.new(|cx| {
            InputState::new(window, cx)
                .default_value(label.as_str())
                .placeholder(label_placeholder.as_str())
        });
        let sub_label = cx.subscribe_in(&label_input, window, Self::on_label_change);

        let mut subscriptions: Vec<Subscription> = vec![sub_label];

        // 构建参数项状态
        let params_arr = node.data.get("params").cloned().unwrap_or_default();
        let params_items: Vec<serde_json::Value> = params_arr
            .as_array()
            .cloned()
            .unwrap_or_default();
        let mut params_state: Vec<ItemState> = Vec::new();
        for item_val in &params_items {
            let st = ItemState::from_value(item_val, false, &registry, window, cx);
            subscribe_item_inputs(&mut params_state, st, "params", &mut subscriptions, window, cx);
        }

        // 构建变量项状态
        let vars_arr = node.data.get("variables").cloned().unwrap_or_default();
        let vars_items: Vec<serde_json::Value> = vars_arr
            .as_array()
            .cloned()
            .unwrap_or_default();
        let mut variables_state: Vec<ItemState> = Vec::new();
        for item_val in &vars_items {
            let st = ItemState::from_value(item_val, true, &registry, window, cx);
            subscribe_item_inputs(
                &mut variables_state,
                st,
                "variables",
                &mut subscriptions,
                window,
                cx,
            );
        }

        Self {
            node,
            flow_node,
            theme,
            on_action,
            syntax_service,
            language,
            registry,
            label_input,
            params_state,
            variables_state,
            syncing: false,
            scroll_handle: ScrollHandle::default(),
            _subscriptions: subscriptions,
        }
    }

    /// 节点数据变化时同步到面板状态。
    pub fn sync_from_node(&mut self, node: Node, window: &mut Window, cx: &mut Context<Self>) {
        if self.node.id != node.id {
            return;
        }
        if self.node.data == node.data {
            return;
        }
        self.syncing = true;
        self.node = node;

        // 同步 label
        let label = label_of(&self.node);
        let current_label = self.label_input.read(cx).value().to_string();
        if current_label != label {
            self.label_input.update(cx, |s, cx| {
                s.set_value(label.as_str(), window, cx);
            });
        }

        // 同步 params
        self.sync_list("params", false, window, cx);
        // 同步 variables
        self.sync_list("variables", true, window, cx);

        self.syncing = false;
        cx.notify();
    }

    /// 同步单个列表（params 或 variables）。
    fn sync_list(
        &mut self,
        field_key: &str,
        is_variable: bool,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let arr = self.node.data.get(field_key).cloned().unwrap_or_default();
        let items: Vec<serde_json::Value> = arr.as_array().cloned().unwrap_or_default();
        let states = if field_key == "params" {
            &mut self.params_state
        } else {
            &mut self.variables_state
        };

        if items.len() == states.len() {
            // 数量一致：逐项同步值
            for (i, item_val) in items.iter().enumerate() {
                states[i].sync_from_value(item_val, is_variable, &self.registry, window, cx);
            }
        } else {
            // 数量变化：重建（需要重建订阅）
            states.clear();
            // 保留 label 订阅，清除项相关订阅
            self._subscriptions.truncate(1);
            for item_val in &items {
                let st = ItemState::from_value(item_val, is_variable, &self.registry, window, cx);
                subscribe_item_inputs(states, st, field_key, &mut self._subscriptions, window, cx);
            }
            // 重建另一个列表的订阅（因为 truncate 清除了所有项订阅）
            self.rebuild_other_subscriptions(field_key, window, cx);
        }
    }

    /// 重建非当前列表的订阅（sync_list 重建时调用）。
    fn rebuild_other_subscriptions(
        &mut self,
        current_field: &str,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if current_field != "params" {
            let params = std::mem::take(&mut self.params_state);
            for st in params {
                subscribe_item_inputs(
                    &mut self.params_state,
                    st,
                    "params",
                    &mut self._subscriptions,
                    window,
                    cx,
                );
            }
        }
        if current_field != "variables" {
            let vars = std::mem::take(&mut self.variables_state);
            for st in vars {
                subscribe_item_inputs(
                    &mut self.variables_state,
                    st,
                    "variables",
                    &mut self._subscriptions,
                    window,
                    cx,
                );
            }
        }
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

    /// 添加新项（通过 dispatch SetData 触发 sync 重建）。
    pub fn add_item(&mut self, field_key: &str, cx: &mut Context<Self>) {
        if self.syncing {
            return;
        }
        let states = if field_key == "variables" {
            &self.variables_state
        } else {
            &self.params_state
        };
        let mut arr: Vec<serde_json::Value> =
            states.iter().map(|s| s.to_value(cx)).collect();
        arr.push(build_default_item());
        self.dispatch_set_data(field_key, serde_json::json!(arr), cx);
    }

    /// 删除项（通过 dispatch SetData 触发 sync 重建）。
    pub fn delete_item(&mut self, field_key: &str, item_idx: usize, cx: &mut Context<Self>) {
        if self.syncing {
            return;
        }
        let states = if field_key == "variables" {
            &self.variables_state
        } else {
            &self.params_state
        };
        let arr: Vec<serde_json::Value> = states
            .iter()
            .enumerate()
            .filter(|(i, _)| *i != item_idx)
            .map(|(_, s)| s.to_value(cx))
            .collect();
        self.dispatch_set_data(field_key, serde_json::json!(arr), cx);
    }

    /// 切换结构类型项的展开/收起（纯 UI 状态，无需同步到 node.data）。
    pub fn toggle_item_expanded(&mut self, field_key: &str, item_idx: usize, cx: &mut Context<Self>) {
        let states = if field_key == "variables" {
            &mut self.variables_state
        } else {
            &mut self.params_state
        };
        if let Some(st) = states.get_mut(item_idx) {
            st.expanded = !st.expanded;
        }
        cx.notify();
    }

    /// 切换项的数据类型（通过 dispatch SetData 触发 sync 重建）。
    pub fn change_item_type(
        &mut self,
        field_key: &str,
        item_idx: usize,
        new_type: String,
        cx: &mut Context<Self>,
    ) {
        if self.syncing {
            return;
        }
        let states = if field_key == "variables" {
            &self.variables_state
        } else {
            &self.params_state
        };
        let arr: Vec<serde_json::Value> = states
            .iter()
            .enumerate()
            .map(|(i, s)| {
                if i == item_idx {
                    let name = s.name.read(cx).value().to_string();
                    build_item_for_type(&name, &new_type, &self.registry)
                } else {
                    s.to_value(cx)
                }
            })
            .collect();
        self.dispatch_set_data(field_key, serde_json::json!(arr), cx);
    }

    /// 向动态类型项添加字段（通过 dispatch SetData 触发 sync 重建）。
    pub fn add_field(&mut self, field_key: &str, item_idx: usize, cx: &mut Context<Self>) {
        if self.syncing {
            return;
        }
        let states = if field_key == "variables" {
            &self.variables_state
        } else {
            &self.params_state
        };
        let arr: Vec<serde_json::Value> = states
            .iter()
            .enumerate()
            .map(|(i, s)| {
                if i == item_idx {
                    let mut val = s.to_value(cx);
                    if let Some(fields) = val.get_mut("fields").and_then(|f| f.as_array_mut()) {
                        fields.push(serde_json::json!({
                            "name": "",
                            "type": "String",
                            "value": "",
                        }));
                    }
                    val
                } else {
                    s.to_value(cx)
                }
            })
            .collect();
        self.dispatch_set_data(field_key, serde_json::json!(arr), cx);
    }

    /// 删除动态类型项的字段（通过 dispatch SetData 触发 sync 重建）。
    pub fn delete_field(
        &mut self,
        field_key: &str,
        item_idx: usize,
        field_idx: usize,
        cx: &mut Context<Self>,
    ) {
        if self.syncing {
            return;
        }
        let states = if field_key == "variables" {
            &self.variables_state
        } else {
            &self.params_state
        };
        let arr: Vec<serde_json::Value> = states
            .iter()
            .enumerate()
            .map(|(i, s)| {
                if i == item_idx {
                    let mut val = s.to_value(cx);
                    if let Some(fields) = val.get_mut("fields").and_then(|f| f.as_array_mut()) {
                        if field_idx < fields.len() {
                            fields.remove(field_idx);
                        }
                    }
                    val
                } else {
                    s.to_value(cx)
                }
            })
            .collect();
        self.dispatch_set_data(field_key, serde_json::json!(arr), cx);
    }

    /// 切换动态类型字段的类型（通过 dispatch SetData 触发 sync 重建）。
    pub fn change_field_type(
        &mut self,
        field_key: &str,
        item_idx: usize,
        field_idx: usize,
        new_type: String,
        cx: &mut Context<Self>,
    ) {
        if self.syncing {
            return;
        }
        let states = if field_key == "variables" {
            &self.variables_state
        } else {
            &self.params_state
        };
        let arr: Vec<serde_json::Value> = states
            .iter()
            .enumerate()
            .map(|(i, s)| {
                if i == item_idx {
                    let mut val = s.to_value(cx);
                    if let Some(fields) = val.get_mut("fields").and_then(|f| f.as_array_mut()) {
                        if field_idx < fields.len() {
                            if let Some(ftype) = fields[field_idx].get_mut("type") {
                                *ftype = serde_json::json!(new_type);
                            }
                        }
                    }
                    val
                } else {
                    s.to_value(cx)
                }
            })
            .collect();
        self.dispatch_set_data(field_key, serde_json::json!(arr), cx);
    }

    /// 同步列表状态到 node.data（Input 变更时调用）。
    fn sync_list_to_node(&self, field_key: &str, cx: &mut Context<Self>) {
        let states = if field_key == "variables" {
            &self.variables_state
        } else {
            &self.params_state
        };
        let arr: Vec<serde_json::Value> = states.iter().map(|s| s.to_value(cx)).collect();
        self.dispatch_set_data(field_key, serde_json::json!(arr), cx);
    }

    fn dispatch_set_data(&self, key: &str, value: serde_json::Value, cx: &mut Context<Self>) {
        if let Some(on_action) = &self.on_action {
            on_action(NodeAction::SetData(key.to_string(), value), cx);
        }
    }
}

impl Render for StartPanelView {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = self.theme;
        let lang = self.language;
        let entity = cx.entity();

        let header = render_header(&self.node, lang, &theme);

        let params_title = t(lang, TKey::PanelParams);
        let vars_title = t(lang, TKey::PanelVariables);
        let add_param_label = t(lang, TKey::PanelAddParam);
        let add_var_label = t(lang, TKey::PanelAddVariable);

        let params_section = section::render_section(
            &self.params_state,
            "params",
            params_title,
            add_param_label,
            false,
            &self.registry,
            lang,
            &theme,
            &entity,
            cx,
        );

        let vars_section = section::render_section(
            &self.variables_state,
            "variables",
            vars_title,
            add_var_label,
            true,
            &self.registry,
            lang,
            &theme,
            &entity,
            cx,
        );

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
                    .id("start-panel-scroll")
                    .flex_1()
                    .overflow_y_scroll()
                    .track_scroll(&self.scroll_handle)
                    .child(
                        div()
                            .flex()
                            .flex_col()
                            .gap(px(16.0))
                            .p(px(16.0))
                            .child(
                                div()
                                    .flex()
                                    .flex_col()
                                    .gap(px(6.0))
                                    .child(
                                        div()
                                            .text_size(px(12.0))
                                            .font_semibold()
                                            .text_color(theme.panel_label_text)
                                            .child(t(lang, TKey::PanelNodeName).to_string()),
                                    )
                                    .child(Input::new(&self.label_input).appearance(true)),
                            )
                            .child(params_section)
                            .child(vars_section),
                    ),
            )
    }
}

/// 渲染面板头部（与 PanelView 头部风格一致）。
fn render_header(node: &Node, lang: Language, theme: &Theme) -> gpui::AnyElement {
    let kind = &node.kind;
    let icon_name = node_icon(kind);
    let kind_lbl = kind_label(lang, kind);
    let title = format!("{} {}", kind_lbl, t(lang, TKey::PanelNodeSuffix));

    div()
        .flex()
        .items_center()
        .gap(px(10.0))
        .px(px(16.0))
        .py(px(12.0))
        .border_b_1()
        .border_color(theme.panel_border)
        .bg(theme.node_title_bg)
        .child(
            div()
                .w(px(32.0))
                .h(px(32.0))
                .rounded_md()
                .bg(theme.toolbar_accent)
                .flex()
                .items_center()
                .justify_center()
                .child(
                    Icon::new(icon_name)
                        .small()
                        .text_color(theme.toolbar_accent_text),
                ),
        )
        .child(
            div()
                .flex()
                .flex_col()
                .gap(px(2.0))
                .child(
                    div()
                        .text_size(px(14.0))
                        .font_semibold()
                        .text_color(theme.panel_title_text)
                        .child(title),
                )
                .child(
                    div()
                        .text_size(px(11.0))
                        .text_color(theme.panel_subtext)
                        .child(kind.to_string()),
                ),
        )
        .into_any_element()
}

/// 为项的所有 InputState 创建订阅。
///
/// 订阅范围：
/// - 项名称
/// - 基础类型值
/// - 子字段名称（动态类型可编辑）
/// - 子字段值（复杂/动态类型）
fn subscribe_item_inputs(
    states: &mut Vec<ItemState>,
    st: ItemState,
    field_key: &str,
    subscriptions: &mut Vec<Subscription>,
    window: &mut Window,
    cx: &mut Context<StartPanelView>,
) {
    let fk = field_key.to_string();

    // 订阅 name
    {
        let fk = fk.clone();
        let sub = cx.subscribe_in(&st.name, window, move |this, _e, ev, _w, cx| {
            if !this.syncing && matches!(ev, InputEvent::Change) {
                this.sync_list_to_node(&fk, cx);
            }
        });
        subscriptions.push(sub);
    }

    // 订阅 value（基础类型）
    if let Some(ref val) = st.value {
        let fk = fk.clone();
        let sub = cx.subscribe_in(val, window, move |this, _e, ev, _w, cx| {
            if !this.syncing && matches!(ev, InputEvent::Change) {
                this.sync_list_to_node(&fk, cx);
            }
        });
        subscriptions.push(sub);
    }

    // 订阅子字段 name 和 value（复杂/动态类型）
    for field in &st.fields {
        // 子字段 name
        let fk_name = fk.clone();
        let sub = cx.subscribe_in(&field.name, window, move |this, _e, ev, _w, cx| {
            if !this.syncing && matches!(ev, InputEvent::Change) {
                this.sync_list_to_node(&fk_name, cx);
            }
        });
        subscriptions.push(sub);

        // 子字段 value
        let fk_val = fk.clone();
        let sub = cx.subscribe_in(&field.value, window, move |this, _e, ev, _w, cx| {
            if !this.syncing && matches!(ev, InputEvent::Change) {
                this.sync_list_to_node(&fk_val, cx);
            }
        });
        subscriptions.push(sub);
    }

    states.push(st);
}

/// 从 node.data 读取 label。
fn label_of(node: &Node) -> String {
    node.data
        .get("label")
        .and_then(|v| v.as_str())
        .unwrap_or(&node.kind)
        .to_string()
}
