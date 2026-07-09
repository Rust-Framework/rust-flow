//! FlowEditorView：流程编辑器主视图。
//!
//! 实现 GPUI `Render`，持有图模型 + 视口 + 交互状态 + 节点注册表。
//!
//! 交互采用命中测试方案：画布统一处理鼠标事件，用几何命中测试确定点击的
//! 节点/端口，避免在每个节点 div 上绑定闭包（GPUI 的 listener 闭包无法
//! 捕获外部变量如 node_id）。
//!
//! 缩放方案：
//! - **节点**：逐元素手动缩放（`pos * scale`、`size * scale`），因 GPUI
//!   的 div 不支持 CSS transform-scale。
//! - **边**：在逻辑坐标中计算路径几何（含 step gap、smoothstep 圆角），
//!   通过 `PathBuilder::scale` + `translate` 统一变换到屏幕空间。线宽
//!   手动乘以 `scale`。这样所有几何参数随缩放等比变化，避免错位。
//!
//! 本文件仅包含核心结构体定义、构造、布局方法、坐标转换和 Render 实现。
//! 其他逻辑按职责拆分到同目录下的子模块：
//! - [`super::interaction`]：交互状态机 + 鼠标事件处理
//! - [`super::hit_test`]：命中测试
//! - [`super::rendering`]：边/节点/面板渲染
//! - [`super::toolbar`]：工具栏
//! - [`super::grid`]：点阵背景
//! - [`super::ports`]：端口位置计算
//! - [`super::viewport`]：视口数学映射

use std::collections::{HashMap, HashSet};
use std::sync::Arc;

use gpui::{
    div, px, Context, CursorStyle, InteractiveElement, IntoElement, MouseButton, MouseMoveEvent,
    ParentElement, Pixels, Point, Render, Styled, Window,
};
use rust_agent_flow::{EdgeId, EdgeType, FlowGraph, NodeId, PointF, PortSide, Viewport};
use rust_agent_flow::{
    LayoutDirection as CoreLayoutDirection, LayoutEngine, LayoutResult, DagreLayout,
};

use crate::i18n::{Language, TKey, t};
use crate::node::{default_syntax_service, NodeRegistry, SharedSyntaxService};
use crate::data_type::SharedDataTypeProvider;
use crate::panel::PanelEntity;
use crate::theme::Theme;

use super::interaction::InteractionState;
use super::toolbar_ext::SharedToolbarProvider;

/// 布局方向。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LayoutDirection {
    Horizontal,
    Vertical,
}

/// 流程编辑器主视图。
pub struct FlowEditorView {
    pub graph: FlowGraph,
    pub viewport: Viewport,
    pub interaction: InteractionState,
    pub registry: Arc<NodeRegistry>,
    pub selected: Option<NodeId>,
    /// 当前悬停的节点 ID（用于显示删除按钮等 hover 元素）。
    pub hovered: Option<NodeId>,
    /// 当前悬停的边「+」按钮对应的边 ID（用于显示手型 cursor + tooltip）。
    pub hovered_plus: Option<EdgeId>,
    /// 默认边类型（用于 DrawingEdge 临时连线 + 全局切换）。
    pub default_edge_type: EdgeType,
    /// 布局方向（决定边的端口侧：Horizontal=Right/Left, Vertical=Bottom/Top）。
    pub layout_direction: LayoutDirection,
    /// 是否显示点阵背景。
    pub show_grid: bool,
    /// 点阵背景逻辑间距（与节点坐标同一空间），控制点阵密度。
    /// 屏幕间距 = 逻辑间距 × scale，随缩放等比变化。
    pub grid_spacing: f32,
    /// 是否允许拖拽节点（false 时左键点击节点仅选中，不进入拖拽状态）。
    pub drag_enabled: bool,
    /// 当前主题颜色配置。
    pub theme: Theme,
    /// 属性面板视图实体（选中节点时创建，取消选中时销毁）。
    pub panel_view: Option<PanelEntity>,
    /// 语法高亮服务（扩展点，默认 `DefaultSyntaxService` 将 rhai 映射到 rust 近似高亮）。
    pub syntax_service: SharedSyntaxService,
    /// 当前 UI 语言（中英文切换）。
    pub language: Language,
    /// 缓存的循环体分组（Loop 节点 → 其循环体节点集合）。
    ///
    /// 由 `loop_body_groups()` BFS 计算得出，在 `relayout` 末尾更新。
    /// 拖动/平移等不改变图结构的交互不会触发更新，避免每帧重复 O(V+E) 遍历。
    /// 供 `render`、`hit_test_edge_plus` 等复用，保证渲染与命中测试一致。
    pub cached_body_groups: HashMap<NodeId, HashSet<NodeId>>,
    /// 缓存的「所有循环体节点」集合（`cached_body_groups` 的扁平化）。
    ///
    /// 在 `relayout` 末尾与 `cached_body_groups` 一同更新，避免 `render_edges`、
    /// `render_nodes`、`render_edge_plus_buttons`、`hit_test_edge_plus` 等方法
    /// 每帧重复 flat_map 收集（每帧原需构建 4-5 次 HashSet）。
    pub cached_all_body_nodes: HashSet<NodeId>,
    /// 缓存的「已隐藏节点」集合（收起的循环体节点）。
    ///
    /// 在 `relayout` 末尾更新。当 Loop 节点 `body_collapsed == true` 时，
    /// 其循环体节点已隐藏，连接到这些节点的边也不渲染。
    pub cached_hidden_nodes: HashSet<NodeId>,
    /// 缓存的边路由路径（EdgeId → waypoints）。
    ///
    /// 在 `relayout` 末尾由 [`super::routing::route_all_edges`] 更新，
    /// `render_edges` 优先使用，`hit_test` 复用。路由失败的边不包含在此
    /// map 中，渲染层回退到几何路径（`EdgeRender::Normal`）。
    /// 拖拽/平移不触发 relayout → 复用缓存，避免每帧 A* 搜索。
    pub cached_edge_routes: HashMap<EdgeId, Vec<PointF>>,
    /// 自定义工具栏扩展（由调用侧通过 `add_toolbar_provider` 注入）。
    ///
    /// 工具项渲染在内置工具栏末尾，以竖线分隔符区隔。
    /// 多次调用 `add_toolbar_provider` 可注入多个 provider，按注入顺序渲染。
    pub custom_toolbar: Vec<SharedToolbarProvider>,
    /// 自定义数据类型提供程序（由调用侧通过 `set_data_type_provider` 注入）。
    ///
    /// 为 Start 节点属性面板提供自定义复杂数据类型。
    /// 不注入时仅有内置类型（String/Integer/Float/Boolean/DateTime/Dynamic）可用。
    pub data_type_provider: Option<SharedDataTypeProvider>,
    /// 属性面板宽度（像素），可通过拖拽分隔条调整。
    pub panel_width: Pixels,
    /// 是否正在拖拽面板分隔条调整宽度。
    pub resizing_panel: bool,
    /// 拖拽起始鼠标 X 坐标（屏幕像素）。
    pub resize_start_x: f32,
    /// 拖拽起始面板宽度（像素）。
    pub resize_start_width: f32,
}

impl FlowEditorView {
    pub fn new(graph: FlowGraph, _cx: &mut Context<Self>) -> Self {
        let mut registry = NodeRegistry::new();
        crate::builtin::register_all(&mut registry);
        Self {
            graph,
            viewport: Viewport::default(),
            interaction: InteractionState::default(),
            registry: Arc::new(registry),
            selected: None,
            hovered: None,
            hovered_plus: None,
            default_edge_type: EdgeType::SmoothStep,
            layout_direction: LayoutDirection::Horizontal,
            show_grid: true,
            grid_spacing: super::grid::DEFAULT_GRID_SPACING,
            drag_enabled: true,
            theme: Theme::light(),
            panel_view: None,
            syntax_service: default_syntax_service(),
            language: Language::default(),
            cached_body_groups: HashMap::new(),
            cached_all_body_nodes: HashSet::new(),
            cached_hidden_nodes: HashSet::new(),
            cached_edge_routes: HashMap::new(),
            custom_toolbar: Vec::new(),
            data_type_provider: None,
            panel_width: px(320.0),
            resizing_panel: false,
            resize_start_x: 0.0,
            resize_start_width: 0.0,
        }
    }

    /// 屏幕坐标（GPUI Point<Pixels>）→ 逻辑坐标（PointF）。
    pub(crate) fn to_logical(&self, p: Point<Pixels>) -> PointF {
        self.viewport.to_logical(PointF::new(p.x.as_f32(), p.y.as_f32()))
    }

    /// 根据布局方向返回 (源端口侧, 目标端口侧)。
    pub(crate) fn port_sides(&self) -> (PortSide, PortSide) {
        match self.layout_direction {
            LayoutDirection::Horizontal => (PortSide::Right, PortSide::Left),
            LayoutDirection::Vertical => (PortSide::Bottom, PortSide::Top),
        }
    }

    /// 运行布局引擎，按当前布局方向重新排列所有节点位置。
    ///
    /// 使用 [`DagreLayout`]（包装 `dagre` crate，ReactFlow 同款 Sugiyama 算法），
    /// 保持节点拓扑分层结构。切换方向时调用此方法即可重新排版。
    pub(crate) fn relayout(&mut self) {
        // 同步节点尺寸：确保 dagre 使用与实际渲染一致的尺寸（特别是
        // Condition 节点的高度随条件项数量变化）。
        self.sync_node_sizes();

        let dir = match self.layout_direction {
            LayoutDirection::Horizontal => CoreLayoutDirection::Horizontal,
            LayoutDirection::Vertical => CoreLayoutDirection::Vertical,
        };
        let result: LayoutResult = DagreLayout::new().layout(&self.graph, dir);
        for (node_id, pos) in &result.positions {
            if let Some(node) = self.graph.node_mut(*node_id) {
                node.position = *pos;
            }
        }

        // 更新缓存的循环体分组：图结构/布局变化后重新计算。
        self.cached_body_groups = self.graph.loop_body_groups();

        // 派生缓存：所有循环体节点（扁平化）+ 已隐藏节点（收起的循环体）。
        // 避免每帧在 render_edges/render_nodes/render_edge_plus_buttons/
        // hit_test_edge_plus 中重复构建（原每帧 4-5 次 HashSet 收集）。
        self.cached_all_body_nodes = self
            .cached_body_groups
            .values()
            .flat_map(|s| s.iter().copied())
            .collect();
        self.cached_hidden_nodes = HashSet::new();
        for (loop_node, body_nodes) in &self.cached_body_groups {
            if let Some(ln) = self.graph.node(*loop_node) {
                let body_collapsed = ln
                    .data
                    .get("body_collapsed")
                    .and_then(|v| v.as_bool())
                    .unwrap_or(false);
                if body_collapsed {
                    self.cached_hidden_nodes.extend(body_nodes.iter().copied());
                }
            }
        }

        // 计算边路由缓存：为所有非 LoopBack 边计算避障路径。
        // 与 cached_body_groups/cached_all_body_nodes/cached_hidden_nodes 一同更新，
        // 保证渲染与命中测试使用同一份路由数据。路由失败的边不写入缓存，
        // 渲染层回退到 ReactFlow 几何路径。
        self.reroute_edges();
    }

    /// 重新计算边路由缓存（不调 dagre 布局与后处理）。
    ///
    /// 拖动节点后调用：节点位置已变但图结构未变，无需重算 dagre 分层与 9 步
    /// 后处理，只需用新位置重新路由所有边。比 `relayout` 轻量（跳过 dagre）。
    /// 拖动期间受影响的边用几何路径跟随（见 `render_edges` 的 dragging_node
    /// 逻辑），松手后由此方法用最终位置重新计算避障路由。
    pub(crate) fn reroute_edges(&mut self) {
        let (src_side_default, dst_side_default) = self.port_sides();
        self.cached_edge_routes = super::routing::route_all_edges(
            &self.graph,
            &self.registry,
            &self.cached_all_body_nodes,
            &self.cached_body_groups,
            &self.cached_hidden_nodes,
            self.layout_direction,
            src_side_default,
            dst_side_default,
        );
    }

    /// 同步所有节点的 `size` 为实际渲染尺寸（`IFlowNode::content_size`）。
    ///
    /// 结构化节点（如 Condition）的渲染高度随数据变化，但 `node.size.h`
    /// 可能在创建后未更新。此方法在布局前调用，确保 dagre、命中测试、
    /// 回环边边界计算使用正确的尺寸。
    pub(crate) fn sync_node_sizes(&mut self) {
        let registry = self.registry.clone();
        let ids: Vec<NodeId> = self.graph.node_ids().collect();
        for id in ids {
            let new_size = {
                let node = match self.graph.node(id) {
                    Some(n) => n,
                    None => continue,
                };
                match registry.get(&node.kind) {
                    Some(f) => f.content_size(node),
                    None => continue,
                }
            };
            if let Some(node) = self.graph.node_mut(id) {
                node.size = new_size;
            }
        }
    }

    /// 仅检查并更新单个节点的渲染尺寸，返回尺寸是否发生变化。
    ///
    /// 用于 `SetData` 路径：避免每次按键都遍历所有节点（`sync_node_sizes`）
    /// 和运行 dagre 布局（`relayout`）。只有结构化节点（如 Condition 的
    /// conditions 数量变化）才会真正改变尺寸触发重排。
    pub(crate) fn update_node_size_if_changed(&mut self, node_id: NodeId) -> bool {
        let (kind, old_size) = match self.graph.node(node_id) {
            Some(n) => (n.kind.clone(), n.size),
            None => return false,
        };
        let flow_node = match self.registry.get(&kind) {
            Some(f) => f,
            None => return false,
        };
        let new_size = match self.graph.node(node_id) {
            Some(n) => flow_node.content_size(n),
            None => return false,
        };
        if new_size != old_size {
            if let Some(node) = self.graph.node_mut(node_id) {
                node.size = new_size;
            }
            true
        } else {
            false
        }
    }

    /// 自动排版：运行 dagre 布局引擎重新排列所有节点，并通知视图刷新。
    ///
    /// 公开 API，供外部（如 demo）在创建编辑器后触发自动排版。
    pub fn auto_layout(&mut self, cx: &mut Context<Self>) {
        self.relayout();
        cx.notify();
    }

    /// 切换布局方向并重新排版节点位置。
    pub(crate) fn set_layout_direction(&mut self, dir: LayoutDirection, cx: &mut Context<Self>) {
        if self.layout_direction == dir {
            return;
        }
        self.layout_direction = dir;
        self.relayout();
        cx.notify();
    }

    /// 设置是否允许拖拽节点。
    pub fn set_drag_enabled(&mut self, enabled: bool, cx: &mut Context<Self>) {
        self.drag_enabled = enabled;
        cx.notify();
    }

    /// 设置点阵背景逻辑间距，控制点阵密度。值越小点越密。
    /// 屏幕间距随缩放等比变化（屏幕间距 = 逻辑间距 × scale）。
    pub fn set_grid_spacing(&mut self, spacing: f32, cx: &mut Context<Self>) {
        self.grid_spacing = spacing.max(8.0);
        cx.notify();
    }

    /// 设置是否显示点阵背景。
    pub fn set_show_grid(&mut self, show: bool, cx: &mut Context<Self>) {
        self.show_grid = show;
        cx.notify();
    }

    /// 切换主题（亮色 ↔ 暗色）。
    ///
    /// 同时同步 gpui-component 全局主题，使 Button/DropdownMenu 等组件的
    /// 图标/文字颜色跟随亮暗切换，避免暗色背景下图标不可见。
    pub fn toggle_theme(&mut self, cx: &mut Context<Self>) {
        self.theme = self.theme.toggle();
        let mode = if self.theme.is_dark {
            gpui_component::ThemeMode::Dark
        } else {
            gpui_component::ThemeMode::Light
        };
        gpui_component::Theme::change(mode, None, cx);
        // 同步主题到已构建的面板：面板内部存有 theme 快照，必须显式通知
        if let Some(panel) = &self.panel_view {
            panel.set_theme(self.theme, cx);
        }
        cx.refresh_windows();
        cx.notify();
    }

    /// 设置指定主题。
    ///
    /// 同时同步 gpui-component 全局主题（见 [`toggle_theme`]）。
    pub fn set_theme(&mut self, theme: Theme, cx: &mut Context<Self>) {
        self.theme = theme;
        let mode = if self.theme.is_dark {
            gpui_component::ThemeMode::Dark
        } else {
            gpui_component::ThemeMode::Light
        };
        gpui_component::Theme::change(mode, None, cx);
        // 同步主题到已构建的面板：面板内部存有 theme 快照，必须显式通知
        if let Some(panel) = &self.panel_view {
            panel.set_theme(self.theme, cx);
        }
        cx.refresh_windows();
        cx.notify();
    }

    /// 注入自定义语法高亮服务（扩展点）。
    ///
    /// 默认使用 [`DefaultSyntaxService`]（rhai → rust 近似高亮）。
    /// 外部 crate 可实现 [`SyntaxService`] trait 提供精确高亮，通过此方法注入。
    pub fn set_syntax_service(&mut self, service: SharedSyntaxService, cx: &mut Context<Self>) {
        self.syntax_service = service;
        // 销毁现有 panel_view，下次 render 时用新服务重建
        self.panel_view = None;
        cx.notify();
    }

    /// 注入自定义工具栏扩展（扩展点）。
    ///
    /// 工具项渲染在内置工具栏末尾，以竖线分隔符区隔。
    /// 多次调用可注入多个 provider，按注入顺序渲染。
    pub fn add_toolbar_provider(
        &mut self,
        provider: SharedToolbarProvider,
        cx: &mut Context<Self>,
    ) {
        self.custom_toolbar.push(provider);
        cx.notify();
    }

    /// 注入自定义数据类型提供程序（扩展点）。
    ///
    /// 为 Start 节点属性面板提供自定义复杂数据类型。
    /// 不注入时仅有内置类型（String/Integer/Float/Boolean/DateTime/Dynamic）可用。
    /// 注入后销毁现有 panel_view，下次 render 时用新类型重建。
    pub fn set_data_type_provider(
        &mut self,
        provider: SharedDataTypeProvider,
        cx: &mut Context<Self>,
    ) {
        self.data_type_provider = Some(provider);
        self.panel_view = None;
        cx.notify();
    }

    /// 切换 UI 语言（中英文）。
    pub fn set_language(&mut self, language: Language, cx: &mut Context<Self>) {
        self.language = language;
        // 销毁现有 panel_view，下次 render 时用新语言重建
        self.panel_view = None;
        cx.notify();
    }

    /// 切换语言（在中/英之间 toggle）。
    pub fn toggle_language(&mut self, cx: &mut Context<Self>) {
        self.set_language(self.language.toggle(), cx);
    }

    /// 渲染节点类型选择浮层（仅在 AddingNodeFromEdge 状态下显示）。
    ///
    /// 浮层定位 = anchor + (10, 10) 偏移，列出 6 种可插入节点类型。
    /// 点击某项 → 调用 `insert_node_at_edge` → 退出 AddingNodeFromEdge。
    /// 浮层根 div 拦截鼠标按下事件，防止冒泡到画布 Empty 分支导致浮层关闭。
    fn render_node_picker(&self, cx: &mut Context<Self>) -> Option<impl IntoElement> {
        let InteractionState::AddingNodeFromEdge { edge_id, anchor } = &self.interaction else {
            return None;
        };
        let edge_id = *edge_id;
        let lang = self.language;
        let theme = self.theme;
        let bg = theme.panel_bg;
        let border = theme.panel_border;
        let text_color = theme.panel_label_text;
        let hover_bg = theme.toolbar_hover_bg;
        let subtext = theme.panel_subtext;

        // 6 种可插入节点类型 + 对应 i18n 键
        let kinds: [(&str, TKey); 6] = [
            ("action", TKey::AddNodeAction),
            ("condition", TKey::AddNodeCondition),
            ("loop", TKey::AddNodeLoop),
            ("variable", TKey::AddNodeVariable),
            ("adapter", TKey::AddNodeAdapter),
            ("agent", TKey::AddNodeAgent),
        ];

        Some(
            div()
                .absolute()
                .left(px(anchor.x + 10.0))
                .top(px(anchor.y + 10.0))
                .w(px(160.0))
                .bg(bg)
                .border_1()
                .border_color(border)
                .rounded_md()
                .shadow_lg()
                .p_2()
                .flex()
                .flex_col()
                .gap_1()
                .on_mouse_down(MouseButton::Left, |_, _, cx| {
                    // 拦截浮层内点击，防止冒泡到画布的 Empty 分支导致浮层关闭
                    cx.stop_propagation();
                })
                .child(
                    div()
                        .text_sm()
                        .text_color(subtext)
                        .child(t(lang, TKey::AddNodeTitle).to_string()),
                )
                .children(kinds.iter().enumerate().map(|(idx, &(kind, key))| {
                    div()
                        .id(("node-picker", idx))
                        .px_2()
                        .py_1()
                        .rounded_md()
                        .text_sm()
                        .text_color(text_color)
                        .hover(|s| s.bg(hover_bg))
                        .on_mouse_down(
                            MouseButton::Left,
                            cx.listener(move |this, _, _, cx| {
                                this.insert_node_at_edge(edge_id, kind, cx);
                                this.interaction = InteractionState::Idle;
                                cx.notify();
                            }),
                        )
                        .child(t(lang, key).to_string())
                })),
        )
    }
}

impl Render for FlowEditorView {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let entity = cx.entity();
        // 先调用需要 &mut self 的方法（ensure_panel_view 可能修改 panel_view），
        // 借用释放后再引用 cached_body_groups，避免每帧 clone 整个 HashMap。
        let panel = self.ensure_panel_view(entity.clone(), window, cx);

        // 使用缓存的循环体分组引用，避免每帧 clone。
        // 缓存在 relayout 末尾更新，拖动/平移等不改变图结构的交互不会触发更新。
        let body_groups = &self.cached_body_groups;
        let edges = self.render_edges(body_groups);
        let nodes = self.render_nodes(entity.clone());
        let toolbar = self.render_toolbar(cx);

        let offset = self.viewport.offset;
        let panel_width = self.panel_width;
        let has_panel = panel.is_some();
        let theme = self.theme;

        // ====== 画布区域：flex-1，处理画布交互事件 ======
        // 光标：平移中 → grabbing（ClosedHand），悬停「+」按钮 → pointer（PointingHand），
        // 空闲 → grab（OpenHand）
        let is_panning = matches!(self.interaction, InteractionState::Panning { .. });
        let is_on_plus = self.hovered_plus.is_some() && !is_panning;
        let mut canvas = div()
            .flex_1()
            .relative()
            .bg(self.theme.canvas_bg)
            .overflow_hidden()
            .cursor(if is_panning {
                CursorStyle::ClosedHand
            } else if is_on_plus {
                CursorStyle::PointingHand
            } else {
                CursorStyle::OpenHand
            })
            .on_mouse_down(MouseButton::Left, cx.listener(Self::on_mouse_down))
            .on_mouse_down(MouseButton::Middle, cx.listener(Self::on_mouse_down))
            .on_mouse_move(cx.listener(Self::on_mouse_move))
            .on_mouse_up(MouseButton::Left, cx.listener(Self::on_mouse_up))
            .on_mouse_up(MouseButton::Middle, cx.listener(Self::on_mouse_up))
            .on_scroll_wheel(cx.listener(Self::on_scroll));

        // ====== 边（Canvas）：直接放在容器根层级 ======
        canvas = canvas.child(edges);

        // ====== 内容层：仅包含节点，通过 offset + scale 定位 ======
        let mut content = div()
            .absolute()
            .left(px(offset.x))
            .top(px(offset.y));

        for node_el in nodes {
            content = content.child(node_el);
        }

        canvas = canvas.child(content);

        // ====== 边「+」按钮层 ======
        let is_interacting = matches!(
            self.interaction,
            InteractionState::DraggingNode { .. } | InteractionState::Panning { .. }
        );
        if !is_interacting {
            canvas = canvas.child(self.render_edge_plus_buttons());
        }

        // ====== 「+」按钮 tooltip ======
        if !is_interacting {
            if let Some(tooltip) = self.render_plus_tooltip() {
                canvas = canvas.child(tooltip);
            }
        }

        // ====== 工具栏 ======
        canvas = canvas.child(toolbar);

        // ====== 节点选择浮层 ======
        if let Some(picker) = self.render_node_picker(cx) {
            canvas = canvas.child(picker);
        }

        // ====== 外层 flex 容器：画布 + 分隔条 + 属性面板 ======
        // 面板作为画布右侧区域（非浮层），分隔条可拖拽调整面板宽度。
        // 拖拽分隔条时，外层容器接管 mouse_move/mouse_up 事件。
        let mut layout = div()
            .size_full()
            .flex()
            .flex_row()
            .on_mouse_move(cx.listener(move |this, event: &MouseMoveEvent, _, cx| {
                if this.resizing_panel {
                    // 面板在右侧，鼠标左移 → 面板变宽
                    let delta = -(event.position.x.as_f32() - this.resize_start_x);
                    let new_width = this.resize_start_width + delta;
                    this.panel_width = px(new_width.max(200.0).min(600.0));
                    cx.notify();
                }
            }))
            .on_mouse_up(
                MouseButton::Left,
                cx.listener(|this, _: &gpui::MouseUpEvent, _, cx| {
                    if this.resizing_panel {
                        this.resizing_panel = false;
                        cx.notify();
                    }
                }),
            )
            .child(canvas);

        // 分隔条 + 属性面板（仅在有面板时显示）
        if has_panel {
            // 分隔条
            layout = layout.child(
                div()
                    .w(px(4.0))
                    .h_full()
                    .bg(theme.panel_border)
                    .cursor(CursorStyle::ResizeLeftRight)
                    .flex_shrink_0()
                    .id("panel-divider")
                    .on_mouse_down(
                        MouseButton::Left,
                        cx.listener(|this, event: &gpui::MouseDownEvent, _, cx| {
                            this.resizing_panel = true;
                            this.resize_start_x = event.position.x.as_f32();
                            this.resize_start_width = this.panel_width.as_f32();
                            cx.notify();
                        }),
                    ),
            );

            // 属性面板
            if let Some(panel_view) = panel {
                layout = layout.child(
                    div()
                        .w(panel_width)
                        .h_full()
                        .flex_shrink_0()
                        .bg(theme.panel_bg)
                        .border_l_1()
                        .border_color(theme.panel_border)
                        .child(panel_view.render_element(window, cx)),
                );
            }
        }

        layout
    }
}
