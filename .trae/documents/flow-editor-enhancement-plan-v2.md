# Flow Editor 功能完善实施计划

## 概述

本计划承接已完成的基础工作（schema 扩展、i18n keys 定义、面板消失修复），聚焦剩余 5 项任务的完整实现：
1. 工具栏 tooltip + 本地化 + gpui-component 重构
2. Demo 数据驱动 + 数据源切换
3. 属性面板 schema 驱动重构 + 单行 CodeEditor
4. 连线「+」按钮添加节点
5. 编译与运行验证

## 当前状态分析

### 已完成（不在本计划范围）
- `crates/core/src/schema/mod.rs`：FieldType/FieldSpec/ListSpec/DropdownOption/FlowDocument/NodeDef/EdgeDef 已定义
- 8 个 builtin 节点已声明 `schema.fields`
- `crates/gpui/src/i18n.rs`：Tb*/DataSource*/AddNode*/EdgeType*/GridDensity* keys 已定义（中英文）
- `crates/gpui/src/editor/flow_editor.rs`：面板容器 `id("panel-container")` + 空闭包拦截事件已修复点击消失

### 待完成（本计划范围）

| 文件 | 行数 | 现状 | 需要做的 |
|------|------|------|----------|
| `crates/gpui/src/panel/mod.rs` | 1472 | per-kind 渲染 + per-kind 状态字段 | schema 驱动统一渲染 |
| `crates/gpui/src/editor/toolbar.rs` | 462 | div 手写按钮，无 Tooltip，无 Button | Button+Tooltip+Dropdown 重构 |
| `demo/src/main.rs` | 241 | 硬编码 build_agent_flow | 3 个 FlowDocument 数据源 |
| `crates/gpui/src/editor/rendering.rs` | 444 | 无 plus button 渲染 | 边中点 + 按钮渲染 |
| `crates/gpui/src/editor/hit_test.rs` | 148 | 6 个 HitResult，无边命中 | 增加 EdgePlusButton 变体 |
| `crates/gpui/src/editor/interaction.rs` | 224 | 4 个 InteractionState | 增加 plus button 点击处理 |
| `crates/gpui/src/editor/flow_editor.rs` | 395 | 无 add_node/insert_node | 增加 insert_node_at_edge |

### gpui-component API 关键确认（来自 Phase 1 探索）
- **Button**: `Button::new(id).icon(IconName).tooltip(text).on_click(handler)` — 仅 icon 无 label 即为图标按钮
- **Switch**: `Switch::new(id).checked(bool).on_click(|&bool, _, _|)` — stateless，on_click 给出新状态
- **InputState**: `InputState::new(window, cx).code_editor(lang).multi_line(false)` — 单行模式，line_number 自动 false
- **InputState 事件**: 通过 `EventEmitter<InputEvent>` 发射，用 `cx.subscribe(&entity, handler)` 监听
- **Tooltip**: 仅 Button/Switch/DropdownButton 有 `.tooltip()` 方法，**不能**附加到任意 div
- **DropdownMenu**: `Button.dropdown_menu(|menu, window, cx| menu.menu(label, action))` 或 `.item(PopupMenuItem::new(label).on_click(handler))`
- **可用图标**: Plus/Minus（缩放）、Maximize（适配）、Undo/Redo（重置）、LayoutDashboard（网格）、Sun/Moon（主题）、ChevronDown（下拉）、Delete（删除）、Play（开始）、CircleCheck（结束）、Cpu（动作）、Network（条件）、Redo（循环）、MemoryStick（变量）、Replace（适配器）、Bot（智能体）
- **List 组件**: 是虚拟列表，不适合动态表单行；用 `v_flex().children(rows.iter().map(...))` + 按钮增删

## 实施阶段

### 阶段 1：属性面板 schema 驱动重构（核心，最大工作量）

**目标**：消除 per-kind 渲染分发，用 `schema.fields` 统一驱动面板 UI，支持真实编辑 + 实时同步。

**文件**：`crates/gpui/src/panel/mod.rs`（重写）

#### 1.1 新增 FieldState 枚举与 PanelView 结构调整

```rust
/// 单个字段的编辑状态。
enum FieldState {
    /// 文本/代码类字段（Text/TextArea/CodeEditor/CodeBlock/Number）。
    Input(Entity<InputState>),
    /// 布尔开关。
    Switch(bool),
    /// 下拉选择（存储当前值）。
    Dropdown(String),
    /// 动态列表（每行一组 InputState）。
    List(Vec<Vec<Entity<InputState>>>),
}
```

PanelView 结构体调整：
- **删除** per-kind 字段：`condition_inputs`、`loop_expr_input`、`loop_mode`、`param_rows`、`variable_rows`、`return_rows`、`agent_model_input`、`agent_prompt_input`、`KvRow`、`KvTarget`
- **新增**：`field_states: Vec<FieldState>`（与 `schema.fields` 一一对应）
- **保留**：`label_input: Entity<InputState>`（节点名称，所有节点通用，单独处理）
- **保留**：`node`、`flow_node`、`theme`、`on_action`、`syntax_service`、`language`、`syncing`、`scroll_handle`、`_subscriptions`

#### 1.2 build() 重写：按 schema.fields 创建 FieldState

```rust
fn build(...) -> Self {
    let schema = flow_node.schema();
    let mut field_states = Vec::new();
    for field in &schema.fields {
        let state = match &field.field_type {
            FieldType::Text | FieldType::Number => {
                let input = new_text_input(..., default_value, field.placeholder, false);
                FieldState::Input(input)
            }
            FieldType::TextArea => {
                let input = new_text_input(..., default_value, field.placeholder, true);
                FieldState::Input(input)
            }
            FieldType::CodeEditor => {
                // 单行代码编辑器：code_editor(lang).multi_line(false)
                let input = new_code_input(..., default_value, field.placeholder, false);
                FieldState::Input(input)
            }
            FieldType::CodeBlock => {
                // 多行代码编辑器：code_editor(lang).line_number(true).rows(4)
                let input = new_code_input(..., default_value, field.placeholder, true);
                FieldState::Input(input)
            }
            FieldType::Switch => {
                FieldState::Switch(default_value.as_bool().unwrap_or(false))
            }
            FieldType::Dropdown(options) => {
                let val = default_value.as_str().unwrap_or("").to_string();
                FieldState::Dropdown(val)
            }
            FieldType::List(list_spec) => {
                // 从 node.data[key] 读取数组，每行创建 item_fields 对应的 InputState
                let rows = build_list_rows(..., list_spec);
                FieldState::List(rows)
            }
        };
        field_states.push(state);
    }
    // ... 订阅每个 Input 的 on_change
}
```

**辅助函数**：
- `new_text_input(syntax_service, default_value, placeholder, multi_line, window, cx) -> Entity<InputState>`：普通文本输入
- `new_code_input(syntax_service, default_value, placeholder, multi_line, window, cx) -> Entity<InputState>`：代码编辑器，`code_editor(lang)`，单行时 `.multi_line(false)`，多行时 `.line_number(true).rows(4)`

#### 1.3 统一 render_schema_panel 替换所有 render_*_panel

```rust
fn render_schema_panel(&self, theme: &Theme, cx: &mut Context<Self>) -> AnyElement {
    let lang = self.language;
    let kind = &self.node.kind;
    let schema = self.flow_node.schema();
    
    let mut col = div().flex().flex_col().gap(px(10.0)).p_3().size_full().overflow_y_scroll();
    
    // 标题：节点类型 i18n 标签
    col = col.child(self.render_header(theme));
    
    // 节点名称 Input（label 字段，所有节点通用）
    col = col.child(self.render_label_field(theme, cx));
    
    // 按 schema.fields 渲染每个字段
    for (i, field) in schema.fields.iter().enumerate() {
        if field.key == "label" { continue; } // label 已单独渲染
        let label = field_label(lang, kind, &field.key, &field.label);
        col = col.child(self.render_field(i, field, &label, theme, cx));
    }
    
    col.into_any_element()
}
```

**render_field** 按 FieldType 分发：
- `Text/Number`：`Input::new(&state).placeholder(...)` 单行
- `TextArea`：`Input::new(&state).h(px(80.0))` 多行
- `CodeEditor`：`Input::new(&state)` 单行代码（已通过 InputState 配置）
- `CodeBlock`：`Input::new(&state).h(px(120.0))` 多行代码
- `Switch`：`Switch::new(id).checked(bool).on_click(handler)` + 标签
- `Dropdown(options)`：按钮组或 `Button.dropdown_menu`，选中态高亮
- `List(list_spec)`：`v_flex` 渲染每行 item_fields + 添加/删除按钮

#### 1.4 i18n 标签映射函数

```rust
/// 字段标签 i18n 映射：(kind, field_key) → TKey → 本地化文案。
fn field_label(lang: Language, kind: &str, field_key: &str, fallback: &str) -> String {
    // 已有的 Panel* keys 映射
    let tkey = match (kind, field_key) {
        ("condition", "conditions") => TKey::PanelConditions,
        ("loop", "loop_mode") => TKey::PanelLoopMode,
        ("loop", "loop_expr") => TKey::PanelLoopExpr,
        ("start", "params") => TKey::PanelParams,
        ("start", "variables") => TKey::PanelVariables,
        ("end", "returns") => TKey::PanelReturns,
        ("agent", "model") => TKey::PanelAgentModel,
        ("agent", "prompt") => TKey::PanelAgentPrompt,
        ("loop", "label") | ("condition", "label") | ... => TKey::PanelLabel,
        _ => return fallback.to_string(),
    };
    t(lang, tkey).to_string()
}
```

#### 1.5 sync_from_node 重写

```rust
fn sync_from_node(&mut self, node: &Node, window: &mut Window, cx: &mut Context<Self>) {
    if self.node.id != node.id { return; } // 应由 new 重建
    self.node = node.clone();
    self.syncing = true;
    
    // 同步 label
    let label = label_of(node);
    self.label_input.update(cx, |s, cx| s.set_value(label, window, cx));
    
    // 同步每个 field_state
    let schema = self.flow_node.schema();
    for (i, field) in schema.fields.iter().enumerate() {
        if field.key == "label" { continue; }
        let value = node.data.get(&field.key).cloned().unwrap_or(field.default.clone());
        match &mut self.field_states[i] {
            FieldState::Input(entity) => {
                let text = value_to_string(&value);
                entity.update(cx, |s, cx| s.set_value(text, window, cx));
            }
            FieldState::Switch(b) => { *b = value.as_bool().unwrap_or(false); }
            FieldState::Dropdown(s) => {
                if let Some(str_val) = value.as_str() { *s = str_val.to_string(); }
            }
            FieldState::List(rows) => {
                // 数量一致仅更新值，否则重建
                sync_list_rows(rows, &value, &field.field_type, ...);
            }
        }
    }
    
    self.syncing = false;
    cx.notify();
}
```

#### 1.6 on_field_change 统一回调

```rust
fn on_field_change(&mut self, field_idx: usize, event: &InputEvent, _input: Entity<InputState>, _window: &mut Window, cx: &mut Context<Self>) {
    if self.syncing { return; }
    if !matches!(event, InputEvent::Change) { return; }
    
    let schema = self.flow_node.schema();
    let field = &schema.fields[field_idx];
    let key = field.key.clone();
    
    let value = match &self.field_states[field_idx] {
        FieldState::Input(entity) => {
            let text = entity.read(cx).value().to_string();
            serde_json::json!(text)
        }
        _ => return, // Switch/Dropdown/List 有自己的 handler
    };
    
    if let Some(cb) = &self.on_action {
        (cb)(self.node.id, NodeAction::SetData(key, value), cx);
    }
}
```

**订阅方式**：build() 中为每个 Input Entity 创建订阅，闭包捕获 `field_idx`：
```rust
let sub = cx.subscribe(&input_entity, Self::on_field_change_with_idx(field_idx));
self._subscriptions.push(sub);
```

#### 1.7 Switch/Dropdown/List 专用 handler

- **Switch**：`Switch::new(id).checked(bool).on_click(cx.listener(move |this, &new_val, _, cx| { ... SetData(key, json!(new_val)) ... }))`
- **Dropdown**：按钮组，点击 `set_dropdown(field_idx, value, cx)` → SetData
- **List 添加**：`add_list_item(field_idx, cx)` → push 新行 + sync_list_to_node
- **List 删除**：`delete_list_item(field_idx, row_idx, cx)` → remove + sync_list_to_node
- **sync_list_to_node**：构造 `[{name, type, value}, ...]` 数组 → SetData

#### 1.8 视觉优化（紧凑、高密度、统一）

- 面板宽度：`px(320.0)` → `px(300.0)`（更紧凑）
- padding：`p_4()` → `p_3()`（16px → 12px）
- gap：`px(12.0)` → `px(10.0)`
- 字段标签：`text_size(px(12.0))` + `font_semibold` + `text_color(theme.panel_label_text)`
- Input 高度：Text 单行自适应，TextArea `h(px(80.0))`，CodeBlock `h(px(120.0))`
- List 行：`h(px(28.0))` 紧凑行高，3 列布局 name/type/value + 删除按钮
- 删除按钮：`Icon::new(IconName::Close).xsmall()` 替代 "×" 文字
- 添加按钮：`Button::new(id).small().ghost().label("+ Add").on_click(...)` 或 `Button::new(id).icon(IconName::Plus).small()`

#### 1.9 删除的旧代码

- `KvRow` 结构体
- `KvTarget` 枚举
- `render_condition_panel`、`render_loop_panel`、`render_start_panel`、`render_end_panel`、`render_variable_panel`、`render_agent_panel`、`render_simple_panel`、`render_kv_table`
- `on_condition_change`、`on_loop_expr_change`、`on_kv_change`、`on_agent_model_change`、`on_agent_prompt_change`
- `sync_conditions_to_node`、`sync_kv_to_node`、`sync_kv_rows`
- `add_branch`、`delete_branch`、`set_loop_mode`、`add_kv`、`delete_kv`
- `get_kv_list`、`get_conditions` 辅助函数（移到 panel 内部或保留为通用 list 解析）

---

### 阶段 2：工具栏重构（Button + Tooltip + Dropdown + 数据源切换）

**目标**：所有按钮改用 gpui-component Button，添加 Tooltip（i18n），边类型/网格密度/数据源改用 Dropdown。

**文件**：`crates/gpui/src/editor/toolbar.rs`（重写）

#### 2.1 按钮重构映射

| 旧按钮 | 新实现 | IconName | Tooltip TKey |
|--------|--------|----------|--------------|
| zoom-in "+" | `Button::new("tb-zoom-in").icon(Plus).tooltip(t(lang,TbZoomIn)).on_click(...)` | Plus | TbZoomIn |
| zoom-out "−" | `Button::new("tb-zoom-out").icon(Minus).tooltip(...)` | Minus | TbZoomOut |
| fit "□" | `Button::new("tb-fit").icon(Maximize).tooltip(...)` | Maximize | TbFitView |
| reset "⟳" | `Button::new("tb-reset").icon(Undo).tooltip(...)` | Undo | TbResetView |
| dir-h "↔" | `Button::new("tb-dir-h").icon(ArrowRight).selected(is_h).tooltip(...)` | ArrowRight | TbLayoutHorizontal |
| dir-v "↕" | `Button::new("tb-dir-v").icon(ArrowDown).selected(is_v).tooltip(...)` | ArrowDown | TbLayoutVertical |
| edge-type | `Button::new("tb-edge").label(edge_label).dropdown_menu(...)` | - | TbEdgeType |
| grid "▦" | `Button::new("tb-grid").icon(LayoutDashboard).selected(show_grid).tooltip(...)` | LayoutDashboard | TbToggleGrid |
| grid-density | `Button::new("tb-density").label(density_label).dropdown_menu(...)` | - | TbGridDensity |
| drag "✎" | `Button::new("tb-drag").icon(Edit).selected(drag_enabled).tooltip(...)` | Edit | TbToggleDrag |
| theme "☀/☽" | `Button::new("tb-theme").icon(Sun/Moon).tooltip(...)` | Sun/Moon | TbToggleTheme |
| lang "En/中" | `Button::new("tb-lang").label(label).tooltip(...)` | - | TbToggleLanguage |
| **数据源（新增）** | `Button::new("tb-data-source").icon(Database).label(current_name).dropdown_menu(...)` | Database | TbDataSource |

**注意**：gpui-component 无 `Edit`/`Database` 图标，需确认替代：
- drag：用 `IconName::Pencil` 或 `IconName::Move`（若存在），否则保留 `IconName::Settings`
- data-source：用 `IconName::HardDrive` 或 `IconName::FolderOpen`

#### 2.2 Dropdown 实现（边类型/网格密度/数据源）

```rust
// 边类型 Dropdown
Button::new("tb-edge")
    .label(edge_type_label(lang, self.default_edge_type))
    .small()
    .dropdown_menu(move |menu, window, cx| {
        menu.item(PopupMenuItem::new(t(lang, TKey::EdgeBezier)).on_click(...))
            .item(PopupMenuItem::new(t(lang, TKey::EdgeStraight)).on_click(...))
            .item(PopupMenuItem::new(t(lang, TKey::EdgeStep)).on_click(...))
            .item(PopupMenuItem::new(t(lang, TKey::EdgeSmoothStep)).on_click(...))
    })
```

#### 2.3 数据源切换

FlowEditorView 新增字段：
```rust
pub data_source: DataSource,  // enum { AgentFlow, DataPipeline, SimpleFlow }
```

新增方法：
```rust
pub fn set_data_source(&mut self, ds: DataSource, cx: &mut Context<Self>) {
    self.data_source = ds;
    self.graph = ds.to_graph();  // 从 FlowDocument 构建
    self.selected = None;
    self.panel_view = None;
    self.relayout();
    cx.notify();
}
```

DataSource enum 定义在 `demo` 或 `crates/gpui/src/editor/data_source.rs`，提供 `to_graph() -> FlowGraph`。

Toolbar 中数据源 Dropdown：
```rust
Button::new("tb-data-source")
    .label(t(lang, current_ds_key))
    .dropdown_menu(|menu, _, _| {
        menu.item(PopupMenuItem::new(t(lang, DataSourceAgentFlow)).on_click(|_, _, cx| {
            cx.window_handle().dispatch_action(Box::new(SetDataSource(DataSource::AgentFlow)));
        }))
        // ...
    })
```

**注意**：PopupMenu 的 on_click 在 `&mut App` 上下文中，需要通过 action 或 window entity 引用回 FlowEditorView。具体实现时用 `cx.window_handle().dispatch_action` 或直接捕获 Entity<FlowEditorView>。

---

### 阶段 3：Demo 数据驱动重构

**目标**：3 个 FlowDocument 数据源，支持切换。

**文件**：
- `demo/src/main.rs`（重写）
- 新增 `demo/src/data_sources.rs`（或在 main.rs 内）

#### 3.1 三个数据源

```rust
pub enum DataSource {
    AgentFlow,
    DataPipeline,
    SimpleFlow,
}

impl DataSource {
    pub fn to_document(&self) -> FlowDocument { ... }
    pub fn to_graph(&self) -> FlowGraph {
        FlowGraph::from_document(&self.to_document())
    }
    pub fn label_key(&self) -> TKey { ... }
}
```

**数据源 1：AgentFlow**（迁移现有 build_agent_flow）
- 13 节点：start/variable/agent/planner/adapter/condition/search/notify/tool/loop/process/summarize/end
- 15 边：主流程 + 条件分支 + 循环体 + 汇合

**数据源 2：DataPipeline**（新）
- 8 节点：start/adapter/variable/condition/process/process/adapter/end
- 数据清洗 → 分流 → 处理 → 汇合

**数据源 3：SimpleFlow**（新）
- 4 节点：start/action/action/end
- 简单线性流程

#### 3.2 main.rs 重写

```rust
fn main() {
    gpui_platform::application()
        .with_assets(Assets)
        .run(move |cx: &mut gpui::App| {
            rust_agent_flow_gpui::init(cx);
            cx.spawn(async move |cx| {
                cx.open_window(WindowOptions::default(), |window, cx| {
                    let graph = DataSource::AgentFlow.to_graph();
                    let view = cx.new(|cx| {
                        let mut editor = FlowEditorView::new(graph, cx);
                        editor.set_data_source(DataSource::AgentFlow, cx);
                        editor.auto_layout(cx);
                        editor
                    });
                    cx.new(|cx| gpui_component::Root::new(view, window, cx))
                })
                .expect("Failed to open window");
            })
            .detach();
        });
}
```

---

### 阶段 4：连线「+」按钮添加节点

**目标**：边中点显示「+」按钮，点击弹出节点选择菜单，选择后在边中间插入新节点。

**文件**：
- `crates/gpui/src/editor/hit_test.rs`：增加 `EdgePlusButton(edge_id)` 变体 + 边中点命中测试
- `crates/gpui/src/editor/rendering.rs`：增加 `render_edge_plus_buttons`
- `crates/gpui/src/editor/interaction.rs`：增加 plus button 点击处理
- `crates/gpui/src/editor/flow_editor.rs`：增加 `insert_node_at_edge` 方法

#### 4.1 HitResult 扩展

```rust
pub(crate) enum HitResult {
    Empty,
    Node(NodeId),
    OutPort(NodeId, PortId),
    InPort(NodeId, PortId),
    DeleteButton(NodeId),
    ToggleButton(NodeId),
    EdgePlusButton(EdgeId),  // 新增
}
```

#### 4.2 边中点 + 按钮渲染

在 `rendering.rs` 的 `render_edges` 中，计算每条边的中点，渲染「+」按钮：

```rust
// 在 canvas paint 之后，用 div 渲染 + 按钮（屏幕坐标）
for edge_render in &edge_renders {
    let mid = edge_midpoint(edge_render);  // (src + dst) / 2
    let screen_mid = logical_to_screen(mid, viewport, bounds);
    // 渲染 + 按钮：圆形 + Plus 图标
    plus_buttons.push(render_plus_button(screen_mid, theme));
}
```

**plus button 渲染**：`div().absolute().left(px(screen_x)).top(px(screen_y)).w(px(20.0)).h(px(20.0)).rounded_full().bg(theme.plus_btn_bg).flex().items_center().justify_center().child(Icon::new(IconName::Plus).xsmall())`

**注意**：plus button 需要在 hover 边时才显示，或始终显示。简化方案：始终显示在每条边中点。

#### 4.3 边中点命中测试

在 `hit_test.rs` 中，遍历所有边，计算中点，判断鼠标是否在 plus button 范围内：

```rust
// 在端口/按钮/节点命中测试之前或之后
for edge in self.graph.edges() {
    let (src, dst) = compute_edge_endpoints(...);
    let mid = PointF::new((src.x + dst.x) * 0.5, (src.y + dst.y) * 0.5);
    let screen_mid = self.to_screen(mid);
    let btn_rect = RectF::new(
        PointF::new(screen_mid.x - 12.0, screen_mid.y - 12.0),
        SizeF::new(24.0, 24.0),
    );
    if point_in_rect(screen_pos, btn_rect) {
        return HitResult::EdgePlusButton(edge.id);
    }
}
```

#### 4.4 点击处理 + 节点插入

在 `interaction.rs` 的 `on_mouse_down`：
```rust
(MouseButton::Left, HitResult::EdgePlusButton(edge_id)) => {
    self.show_add_node_menu(edge_id, cx);
}
```

`show_add_node_menu` 弹出 PopupMenu，选择节点类型后调用 `insert_node_at_edge`。

#### 4.5 insert_node_at_edge 方法

```rust
pub(crate) fn insert_node_at_edge(
    &mut self,
    edge_id: EdgeId,
    kind: &str,
    cx: &mut Context<Self>,
) {
    // 1. 获取原边 source/target/ports
    let edge = self.graph.edge(edge_id).cloned();
    let (source_id, target_id, source_port, target_port) = ...;
    
    // 2. 计算插入位置（边中点）
    let mid = edge_midpoint(...);
    
    // 3. 创建新节点（用 schema.default_data 填充 data）
    let schema = self.registry.get(kind).map(|f| f.schema().clone());
    let data = schema.as_ref().map(|s| s.default_data()).unwrap_or(json!({}));
    let size = schema.as_ref().map(|s| s.default_size).unwrap_or(SizeF::new(180.0, 64.0));
    let new_node = Node::new(kind, data).with_size(size).with_position(mid);
    let new_id = self.graph.add_node(new_node);
    
    // 4. 删除原边
    self.graph.remove_edge(edge_id);
    
    // 5. 创建两条新边：source→new, new→target
    let mut e1 = Edge::new(source_id, new_id);
    e1.source_port = source_port;
    e1.target_port = Some("in".to_string());
    e1.edge_type = self.default_edge_type;
    self.graph.add_edge(e1);
    
    let mut e2 = Edge::new(new_id, target_id);
    e2.source_port = Some("out".to_string());
    e2.target_port = target_port;
    e2.edge_type = self.default_edge_type;
    self.graph.add_edge(e2);
    
    // 6. 选中新节点 + relayout
    self.selected = Some(new_id);
    self.relayout();
    cx.notify();
}
```

#### 4.6 节点选择菜单

用 PopupMenu 列出可添加的节点类型：
```rust
PopupMenu::build(window, cx, |menu, _, _| {
    menu.item(PopupMenuItem::new(t(lang, AddNodeAction)).on_click(|_, _, cx| {
        // dispatch InsertNode { edge_id, kind: "action" }
    }))
    .item(PopupMenuItem::new(t(lang, AddNodeCondition)).on_click(...))
    .item(PopupMenuItem::new(t(lang, AddNodeLoop)).on_click(...))
    .item(PopupMenuItem::new(t(lang, AddNodeVariable)).on_click(...))
    .item(PopupMenuItem::new(t(lang, AddNodeAdapter)).on_click(...))
    .item(PopupMenuItem::new(t(lang, AddNodeAgent)).on_click(...))
})
```

**注意**：PopupMenu 的 on_click 在 `&mut App` 上下文，需要通过 action 机制或捕获 Entity 回调到 FlowEditorView。具体实现用 `cx.dispatch_action(Box::new(InsertNodeAction { edge_id, kind }))`，在 FlowEditorView 注册 action handler。

---

### 阶段 5：编译与运行验证

#### 5.1 编译验证
```bash
cargo build --release
```
预期：无编译错误，可能有少量 unused warning（可接受）。

#### 5.2 运行验证
```bash
cargo run --release
```
验证项：
- [ ] 工具栏所有按钮有 tooltip 提示（中英文切换）
- [ ] 边类型/网格密度/数据源 Dropdown 正常工作
- [ ] 3 个数据源可切换，切换后流程图正确渲染
- [ ] 点击节点显示属性面板，面板不消失
- [ ] 属性面板可编辑各字段，编辑后节点实时更新
- [ ] CodeEditor 字段为单行，无行号
- [ ] Switch/Dropdown/List 字段正常工作
- [ ] 边中点显示「+」按钮
- [ ] 点击「+」弹出节点选择菜单
- [ ] 选择节点类型后在边中间插入新节点
- [ ] 插入后原边删除，两条新边正确连接

## 假设与决策

1. **PanelView 完全重写**：per-kind 逻辑全部删除，统一为 schema 驱动。保留 `label_input` 单独处理（所有节点通用）。
2. **单行 CodeEditor**：`InputState::new(...).code_editor(lang).multi_line(false)`，line_number 自动 false（无需显式调用 `.line_number(false)`，且调用会 panic）。
3. **List 动态行**：用 `v_flex().children()` + Button 增删，不用 gpui-component List（虚拟列表不适合表单）。
4. **Tooltip 仅用 Button.tooltip()**：不尝试给 div 加 tooltip（API 不支持）。
5. **数据源切换**：定义 DataSource enum，每个变体返回 FlowDocument，通过 `FlowGraph::from_document` 转换。
6. **边「+」按钮始终显示**：简化实现，不做 hover 检测（避免边命中测试复杂度）。
7. **PopupMenu 回调**：通过 action 机制或捕获 Entity 引用回调 FlowEditorView（实现时确认最佳方式）。
8. **图标替代**：drag 用 `Pencil`（若无则 `Settings`），data-source 用 `HardDrive`（若无则 `FolderOpen`）。
9. **i18n 标签映射**：`field_label(lang, kind, key, fallback)` 函数，已有 Panel* keys 优先映射，无映射则用 fallback。
10. **保留 sync_from_node + syncing 防回环机制**：schema 驱动不改变事件流。

## 文件变更清单

| 文件 | 操作 | 说明 |
|------|------|------|
| `crates/gpui/src/panel/mod.rs` | 重写 | schema 驱动统一渲染 |
| `crates/gpui/src/editor/toolbar.rs` | 重写 | Button+Tooltip+Dropdown |
| `crates/gpui/src/editor/flow_editor.rs` | 修改 | 增加 data_source 字段、set_data_source、insert_node_at_edge |
| `crates/gpui/src/editor/rendering.rs` | 修改 | 增加 render_edge_plus_buttons |
| `crates/gpui/src/editor/hit_test.rs` | 修改 | 增加 EdgePlusButton 变体 + 命中测试 |
| `crates/gpui/src/editor/interaction.rs` | 修改 | 增加 plus button 点击处理 |
| `demo/src/main.rs` | 重写 | 数据驱动 + 3 数据源 |
| `crates/gpui/src/editor/data_source.rs` | 新增（可选） | DataSource enum + to_document |

## 实施顺序

1. **阶段 1**（属性面板）：panel/mod.rs 重写 — 打通数据链路
2. **阶段 2**（工具栏）：toolbar.rs 重构 — 接入 i18n keys
3. **阶段 3**（Demo）：main.rs 数据驱动 — 验证 FlowDocument
4. **阶段 4**（边+按钮）：hit_test + rendering + interaction + flow_editor — 节点添加
5. **阶段 5**（验证）：编译 + 运行

每个阶段完成后立即 `cargo build` 验证编译，避免错误累积。
