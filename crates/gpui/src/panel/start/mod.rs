//! Start 节点专属属性面板：支持参数和变量的树形编辑。
//!
//! 与通用 [`PanelView`](super::PanelView) 不同，StartPanelView 专为 Start 节点设计：
//! - 接收参数：0-N 个，基础/复杂/动态类型，子字段值只读
//! - 变量定义：0-N 个，基础/复杂/动态类型，子字段值可编辑
//! - 复杂类型：预定义结构，结构不可编辑
//! - 动态类型（Dynamic）：结构可手动编辑（增删改字段）
//!
//! 文件拆分：
//! - `data_types.rs`：JSON 辅助函数
//! - `item.rs`：单项状态管理（ItemState + FieldState）
//! - `common.rs`：公共类型（Selection, RowInputs）与辅助函数
//! - `sync.rs`：数据同步逻辑（node.data ↔ 面板状态）
//! - `handlers.rs`：事件处理器（增删改、类型切换等）
//! - `tree_render.rs`：Tree 控件渲染（内联 Input/Dropdown/Input 控件）
//! - `detail_editor.rs`：浮层详细编辑面板 + 头部渲染
//! - `mod.rs`：面板实体定义、构建、Render 实现

use std::sync::Arc;

use gpui::{
    div, px, App, AppContext, Context, Entity, InteractiveElement, IntoElement, ParentElement,
    Render, ScrollHandle, StatefulInteractiveElement, Styled, Subscription, Window,
};
use gpui_component::input::{Input, InputState};
use gpui_component::tree::{TreeEvent, TreeState};
use gpui_component::StyledExt;
use rust_agent_flow::Node;

use crate::data_type::{DataTypeRegistry, SharedDataTypeProvider};
use crate::i18n::{kind_label, t, Language, TKey};
use crate::node::{ActionCallback, IFlowNode, SharedSyntaxService};
use crate::theme::Theme;

pub mod common;
pub mod data_types;
pub mod detail_editor;
pub mod handlers;
pub mod item;
pub mod sync;
pub mod tree_render;

use common::label_of;
use item::ItemState;
use sync::subscribe_item_inputs;
use tree_render::build_section_tree_items;

/// Start 节点属性面板视图。
pub struct StartPanelView {
    pub node: Node,
    pub flow_node: Option<Arc<dyn IFlowNode>>,
    pub theme: Theme,
    pub on_action: Option<ActionCallback>,
    pub syntax_service: SharedSyntaxService,
    pub language: Language,

    /// 数据类型注册表（内置类型 + provider 注入类型）。
    pub(super) registry: DataTypeRegistry,
    /// 节点名称输入。
    pub(super) label_input: Entity<InputState>,
    /// 参数项状态列表。
    pub(super) params_state: Vec<ItemState>,
    /// 变量项状态列表。
    pub(super) variables_state: Vec<ItemState>,
    /// 参数区 Tree 控件状态。
    pub(super) params_tree: Entity<TreeState>,
    /// 变量区 Tree 控件状态。
    pub(super) variables_tree: Entity<TreeState>,
    /// 当前选中项（驱动浮层详细编辑面板）。
    pub(super) selected: Option<common::Selection>,

    /// 同步标记：避免回环。
    pub(super) syncing: bool,
    /// 内容区滚动句柄。
    pub(super) scroll_handle: ScrollHandle,
    pub(super) _subscriptions: Vec<Subscription>,
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

        // 创建 Tree 状态并构建初始 items
        let params_items = build_section_tree_items(&params_state, "params", &registry, cx);
        let variables_items =
            build_section_tree_items(&variables_state, "variables", &registry, cx);
        let params_tree = cx.new(|cx| TreeState::new(cx).items(params_items));
        let variables_tree = cx.new(|cx| TreeState::new(cx).items(variables_items));

        // 观察 Tree 选中变化 → 更新 selected 状态
        subscriptions.push(cx.observe(&params_tree, move |this, _, cx| {
            this.on_tree_selection("params", cx);
        }));
        subscriptions.push(cx.observe(&variables_tree, move |this, _, cx| {
            this.on_tree_selection("variables", cx);
        }));

        // 订阅 TreeEvent → 同步展开状态到 ItemState
        subscriptions.push(cx.subscribe(&params_tree, |this, _state, event: &TreeEvent, cx| {
            this.on_tree_event("params", event, cx);
        }));
        subscriptions.push(cx.subscribe(&variables_tree, |this, _state, event: &TreeEvent, cx| {
            this.on_tree_event("variables", event, cx);
        }));

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
            params_tree,
            variables_tree,
            selected: None,
            syncing: false,
            scroll_handle: ScrollHandle::default(),
            _subscriptions: subscriptions,
        }
    }
}

impl Render for StartPanelView {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = self.theme;
        let lang = self.language;
        let entity = cx.entity();

        let header = detail_editor::render_header(&self.node, lang, &theme);

        let params_title = t(lang, TKey::PanelParams);
        let vars_title = t(lang, TKey::PanelVariables);
        let add_param_label = t(lang, TKey::PanelAddParam);
        let add_var_label = t(lang, TKey::PanelAddVariable);

        // 参数区 Tree
        let params_tree_el = tree_render::render_section_tree(
            &self.params_tree,
            "params",
            params_title,
            add_param_label,
            false,
            &self.registry,
            lang,
            theme,
            &entity,
            &self.params_state,
            cx,
        );

        // 变量区 Tree
        let vars_tree_el = tree_render::render_section_tree(
            &self.variables_tree,
            "variables",
            vars_title,
            add_var_label,
            true,
            &self.registry,
            lang,
            theme,
            &entity,
            &self.variables_state,
            cx,
        );

        // 选中项的编辑表单通过 Popover 浮层显示在选中行左侧

        div()
            .relative()
            .w_full()
            .h_full()
            .bg(theme.panel_bg)
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
                            .p(px(20.0))
                            .gap(px(24.0))
                            // 节点名称区域
                            .child(
                                div()
                                    .flex()
                                    .flex_col()
                                    .gap(px(10.0))
                                    .child(
                                        div()
                                            .text_size(px(14.0))
                                            .font_medium()
                                            .text_color(theme.panel_label_text)
                                            .child(t(lang, TKey::PanelNodeName).to_string()),
                                    )
                                    .child(Input::new(&self.label_input).appearance(true)),
                            )
                            // 输入参数区域
                            .child(params_tree_el)
                            // 变量定义区域
                            .child(vars_tree_el),
                    ),
            )
    }
}
