# 流程编辑器功能完善实施计划

## 概述

本计划针对 rust-agent-flow 流程编辑器的 5 项功能完善任务，基于对现有代码库的完整探索制定。核心目标：工具栏可用性、数据协议统一化、流程编辑能力、属性面板真实编辑与组件化重构。

经用户确认的范围决策：
- **任务2**：完整方案 — FlowDocument JSON 协议 + FieldSpec 字段定义 + 数据驱动 demo + 工具栏下拉切换数据源
- **任务3**：分析 + 最小实现 — 设计文档 + 连线「+」按钮弹出节点面板添加节点
- **任务1+5**：充分重构 — 工具栏改用 gpui-component IconButton/Button + Tooltip；属性面板改用 Button/Switch/Dropdown/Input/CodeEditor

---

## 当前状态分析

### 项目结构

| Crate | 路径 | 职责 |
|---|---|---|
| `rust-agent-flow` (core) | `crates/core` | 框架无关的图模型、几何、布局算法 |
| `rust-agent-flow-gpui` | `crates/gpui` | GPUI 渲染层 |
| `rust-agent-flow-demo` | `demo` | 演示程序 |

### 已完成能力（无需改动）

- 8 个内置节点已实现并注册：Start/End/Action/Condition/Loop/Variable/Adapter/Agent（[builtin/mod.rs](file:///d:/GitCode/RF/rust-agent-flow/crates/gpui/src/builtin/mod.rs#L37-L46)）
- 节点删除（线性桥接 + 级联删边）、Condition/Loop 展开/收起
- Condition/Loop 属性面板可编辑（[panel/mod.rs](file:///d:/GitCode/RF/rust-agent-flow/crates/gpui/src/panel/mod.rs#L416-L653)）
- IFlowNode 策略模式 + 动态端口 + NodeAction 回调同步
- dagre 布局 + i18n 中英文 + 亮/暗主题
- CodeEditor 集成（rhai→rust 近似高亮，[panel/mod.rs#L177-L198](file:///d:/GitCode/RF/rust-agent-flow/crates/gpui/src/panel/mod.rs#L177-L198)）

### 关键差距

| 类别 | 问题 | 位置 |
|---|---|---|
| 工具栏 | 11 个按钮全手写 div，无 Tooltip，部分文字（D1/D2/D3、Bezier）未 i18n | [toolbar.rs](file:///d:/GitCode/RF/rust-agent-flow/crates/gpui/src/editor/toolbar.rs) |
| Schema | 仅 PortSpec/NodeSchema，无字段定义，node.data 为自由 JSON | [core/schema/mod.rs](file:///d:/GitCode/RF/rust-agent-flow/crates/core/src/schema/mod.rs) |
| 数据协议 | 无 FlowDocument 序列化协议，无法保存/加载完整流程 | - |
| Demo | 完全硬编码（[demo/main.rs#L66-L173](file:///d:/GitCode/RF/rust-agent-flow/demo/src/main.rs#L66-L173)），非数据驱动 | demo/main.rs |
| 编辑 | 无添加节点的 UI 入口 | editor/ |
| 面板消失 | 面板容器 div 无事件拦截，点击非输入区冒泡到画布 → hit_test Empty → selected=None → 面板销毁 | [flow_editor.rs#L378-L387](file:///d:/GitCode/RF/rust-agent-flow/crates/gpui/src/editor/flow_editor.rs#L378-L387) + [interaction.rs#L103-L114](file:///d:/GitCode/RF/rust-agent-flow/crates/gpui/src/editor/interaction.rs#L103-L114) |
| 面板只读 | Action/Start/End/Variable/Adapter/Agent 面板为 render_simple_panel 只读 | [panel/mod.rs#L655-L702](file:///d:/GitCode/RF/rust-agent-flow/crates/gpui/src/panel/mod.rs#L655-L702) |
| 面板组件 | 仅用 Input，未用 Switch/Dropdown/Button/Tooltip；宽度固定 320px | panel/mod.rs |
| CodeEditor | 不支持单行模式，Condition 用 rows=2 模拟 | [panel/mod.rs#L192](file:///d:/GitCode/RF/rust-agent-flow/crates/gpui/src/panel/mod.rs#L192) |

---

## 设计决策

### 决策 1：FieldSpec 驱动的属性面板（统一任务2+5）

**问题**：当前属性面板按节点类型 match 分发（render_condition_panel / render_loop_panel / render_simple_panel），新增节点类型需手写面板代码，且 Action/Start/End 只读。

**方案**：在 NodeSchema 中增加 `fields: Vec<FieldSpec>`，描述 node.data 的字段结构。PanelView 根据 schema.fields 自动生成编辑界面，消除 per-kind 分发。复杂列表字段（conditions/params/variables/returns）用 `FieldType::List` 描述。

```rust
// core/schema/mod.rs 新增
pub enum FieldType {
    Text,                                    // 单行文本
    TextArea,                                // 多行文本
    CodeEditor,                              // 单行代码（表达式，无行号）
    CodeBlock,                               // 多行代码（带行号）
    Number,                                  // 数字
    Switch,                                  // 布尔开关
    Dropdown(Vec<DropdownOption>),           // 枚举下拉
    List(ListSpec),                          // 动态列表（条件/参数/变量）
}

pub struct DropdownOption { pub value: String, pub label_key: TKey }

pub struct ListSpec {
    pub item_label_key: TKey,                // 条目标签键（如 "If {n}"）
    pub item_fields: Vec<FieldSpec>,         // 每条记录的字段（如 name/type/value）
    pub add_label_key: TKey,                 // 添加按钮文案键
    pub min_items: usize,                    // 最小条数（Else 兜底等）
}

pub struct FieldSpec {
    pub key: String,                         // node.data 中的键
    pub label_key: TKey,                     // i18n 标签键
    pub field_type: FieldType,
    pub default: serde_json::Value,
    pub placeholder_key: Option<TKey>,
}
```

**收益**：新增节点只需在 schema 声明字段，面板自动生成；统一视觉风格；属性面板真正可编辑。

### 决策 2：FlowDocument 数据协议

```rust
// core/schema/mod.rs 新增
#[derive(Serialize, Deserialize)]
pub struct FlowDocument {
    pub version: String,                     // "1.0"
    pub metadata: FlowMetadata,              // name/description/created
    pub nodes: Vec<NodeDef>,                 // 节点定义
    pub edges: Vec<EdgeDef>,                 // 边定义
}

#[derive(Serialize, Deserialize)]
pub struct FlowMetadata { pub name: String, pub description: Option<String> }

#[derive(Serialize, Deserialize)]
pub struct NodeDef {
    pub kind: String,
    pub data: serde_json::Value,             // 业务数据（label/desc/conditions...）
    pub size: Option<SizeF>,                 // None 时用 schema.default_size
    pub position: Option<PointF>,            // None 时由布局引擎计算
}

#[derive(Serialize, Deserialize)]
pub struct EdgeDef {
    pub source: usize,                       // NodeDef 索引（序列化友好）
    pub target: usize,
    pub source_port: Option<String>,
    pub target_port: Option<String>,
    pub edge_type: Option<EdgeType>,
}
```

FlowGraph 提供 `from_document(doc, registry) -> FlowGraph` 和 `to_document() -> FlowDocument` 转换方法。

### 决策 3：连线「+」按钮添加节点（任务3实现方案）

参考 ReactFlow/n8n 成熟方案：每条边中点显示「+」圆圈按钮，点击弹出节点选择面板，选择节点类型后在边中点插入新节点，自动重连 source→new→target。

**实现要点**：
- 边中点「+」按钮：在内容层（content layer）渲染 absolute div，位置 = 边中点逻辑坐标 × scale
- 命中测试：扩展 HitResult 增加 `EdgePlusButton(edge_id)`
- 节点选择面板：gpui-component PopupMenu，列出可添加的节点类型（Action/Condition/Loop/Variable/Adapter/Agent，排除 Start/End）
- 插入逻辑：`insert_node_on_edge(edge_id, kind)` — 创建节点（default data 来自 schema）→ 删原边 → 加 source→new 和 new→target 两条边 → relayout

### 决策 4：面板消失问题修复

**根因**：[flow_editor.rs#L378-387](file:///d:/GitCode/RF/rust-agent-flow/crates/gpui/src/editor/flow_editor.rs#L378-L387) 的面板容器 div 无 `on_mouse_down` 处理，点击冒泡到画布容器 → `hit_test` 命中 Empty → `selected = None` → 面板销毁。

**修复**：面板容器 div 增加 `.on_mouse_down(MouseButton::Left, |_, _, _, _| {})` 消费点击事件，阻止冒泡。同时为面板添加独立 id 便于命中测试排除。

### 决策 5：单行 CodeEditor

gpui-component 的 `code_editor` 本质是多行编辑器。单行方案：
- 使用 `.code_editor(lang).line_number(false).rows(1)`
- 在 InputState 上拦截 Enter 键（若 API 支持 `on_key_down` 过滤），防止插入换行符
- 若无法拦截按键，回退方案：用普通 `Input` + 等宽字体 + 语法高亮（若 Input 支持高亮），否则纯等宽 Input

实现时需先验证 gpui-component InputState API 是否支持按键拦截。计划默认采用 `code_editor().rows(1).line_number(false)` + 按键过滤。

---

## 实施步骤

### 阶段 1：核心数据协议（任务2基础）

#### 1.1 扩展 Schema 模块

**文件**：[crates/core/src/schema/mod.rs](file:///d:/GitCode/RF/rust-agent-flow/crates/core/src/schema/mod.rs)

新增内容：
- `FieldType` 枚举（Text/TextArea/CodeEditor/CodeBlock/Number/Switch/Dropdown/List）
- `DropdownOption`、`ListSpec`、`FieldSpec` 结构体
- `NodeSchema` 增加 `fields: Vec<FieldSpec>` 字段 + `with_field()` builder
- `FlowDocument` / `FlowMetadata` / `NodeDef` / `EdgeDef` 结构体
- 所有结构体派生 `Serialize/Deserialize/Clone/Debug`

#### 1.2 FlowGraph 序列化转换

**文件**：[crates/core/src/graph/mod.rs](file:///d:/GitCode/RF/rust-agent-flow/crates/core/src/graph/mod.rs)

新增方法：
- `FlowGraph::from_document(doc: &FlowDocument, registry: &NodeRegistry) -> FlowGraph` — 按 NodeDef 创建节点（size/position 可选），按 EdgeDef（索引引用）创建边
- `FlowGraph::to_document(&self, name: String) -> FlowDocument` — 导出为可序列化文档（NodeId→索引映射）

注意：`from_document` 需要 NodeRegistry 来获取 default_size，但 core 层不依赖 gpui 层的 registry。方案：`from_document` 不依赖 registry，size 为 None 时用 `SizeF::new(180.0, 64.0)` 默认值，由 gpui 层调用后用 `sync_node_sizes()` 修正。或让 NodeDef.size 为必填（demo 数据声明尺寸）。

#### 1.3 内置节点声明字段

**文件**：[crates/gpui/src/builtin/*.rs](file:///d:/GitCode/RF/rust-agent-flow/crates/gpui/src/builtin)

为每个内置节点的 `NodeSchema` 添加 `fields`：
- **Start**：`label`(Text), `params`(List[name/type/value])
- **End**：`label`(Text), `returns`(List[name/type])
- **Action**：`label`(Text), `desc`(Text)
- **Condition**：`label`(Text), `conditions`(List[label=CodeEditor])
- **Loop**：`label`(Text), `loop_mode`(Dropdown[for_each/while/for_loop/batch_parallel]), `loop_expr`(CodeBlock)
- **Variable**：`label`(Text), `variables`(List[name/type/value])
- **Adapter**：`label`(Text), `adapter_type`(Text), `config`(TextArea)
- **Agent**：`label`(Text), `model`(Text), `prompt`(TextArea)

---

### 阶段 2：i18n 扩展（任务1基础）

#### 2.1 新增翻译键

**文件**：[crates/gpui/src/i18n.rs](file:///d:/GitCode/RF/rust-agent-flow/crates/gpui/src/i18n.rs)

新增 TKey 变体（工具栏 tooltip + 数据源 + 节点添加）：
```
// 工具栏 tooltip
TbZoomIn, TbZoomOut, TbFitView, TbResetView,
TbLayoutHorizontal, TbLayoutVertical,
TbEdgeType, TbToggleGrid, TbGridDensity, TbToggleDrag, TbToggleTheme, TbToggleLanguage,
TbDataSource,
// 数据源名称
DataSourceAgentFlow, DataSourceDataPipeline, DataSourceSimpleFlow,
// 节点添加
AddNodeTitle, AddNodeAction, AddNodeCondition, AddNodeLoop, AddNodeVariable, AddNodeAdapter, AddNodeAgent,
// 字段通用
FieldRequired, FieldOptional,
```

补充中英文翻译（`t_zh` / `t_en`）。

---

### 阶段 3：工具栏重构 + Tooltip（任务1）

#### 3.1 验证 gpui-component API

实现前先确认 gpui-component 的 Tooltip/Button/IconButton API（查看 `~/.cargo/git/checkouts/gpui-component-*` 源码）：
- `Tooltip::new(text)` 或 `element.tooltip(text)` 扩展方法
- `Button::new(label).on_click(handler)` 
- `IconButton::new(icon).tooltip(text)`

#### 3.2 重构工具栏

**文件**：[crates/gpui/src/editor/toolbar.rs](file:///d:/GitCode/RF/rust-agent-flow/crates/gpui/src/editor/toolbar.rs)

- 将 11 个手写 div 按钮改为 gpui-component `IconButton`/`Button` + `Tooltip`
- 每个按钮的 Tooltip 文案走 i18n（`t(lang, TKey::TbZoomIn)` 等）
- 边类型切换：改用 `Dropdown`（显示当前类型，下拉选择 Straight/Bezier/Step/SmoothStep）
- 点阵密度切换：改用 `Dropdown`（紧凑/标准/稀疏）
- 新增「数据源」Dropdown：切换不同流程数据源（任务2）
- 保留现有功能逻辑（zoom_in/zoom_out/fit_view 等），仅替换 UI 组件
- 保留主题色取自 `self.theme`

#### 3.3 新增数据源切换

**文件**：[crates/gpui/src/editor/flow_editor.rs](file:///d:/GitCode/RF/rust-agent-flow/crates/gpui/src/editor/flow_editor.rs)

- `FlowEditorView` 增加 `data_sources: Vec<FlowDocument>` 和 `current_source_idx: usize`
- 新增 `pub fn set_data_sources(&mut self, sources: Vec<FlowDocument>, cx)` — 设置可切换数据源
- 新增 `pub fn switch_data_source(&mut self, idx: usize, cx)` — 切换：`self.graph = FlowGraph::from_document(...)` + relayout + 清空选中
- 工具栏 Dropdown 选项来自 `data_sources` 的 `metadata.name`

---

### 阶段 4：数据驱动 Demo（任务2）

#### 4.1 重构 Demo

**文件**：[demo/src/main.rs](file:///d:/GitCode/RF/rust-agent-flow/demo/src/main.rs)

- 删除 `build_agent_flow()` 硬编码函数
- 新增 `demo_documents() -> Vec<FlowDocument>` — 返回 3 个数据源：
  1. **Agent 编排流程**（原 demo 迁移）：Start→Planner→Condition→...→End
  2. **数据处理流水线**：Start→Variable→Adapter→Loop(Process)→End
  3. **简单顺序流**：Start→Action→Action→End
- 每个 FlowDocument 用 `serde_json::json!` 或结构体字面量声明（数据驱动，非过程式 add_node）
- `main()` 中：`let sources = demo_documents();` → `editor.set_data_sources(sources, cx);` → `editor.switch_data_source(0, cx);`

#### 4.2 可选：JSON 文件加载

若需演示外部加载，可在 demo 目录放 `examples/*.json` FlowDocument 文件，`FlowGraph::from_document` 反序列化加载。本计划默认用内存结构体声明，JSON 加载作为扩展点（FlowDocument 已支持 serde）。

---

### 阶段 5：属性面板重构（任务4+5）

#### 5.1 修复面板消失问题

**文件**：[crates/gpui/src/editor/flow_editor.rs](file:///d:/GitCode/RF/rust-agent-flow/crates/gpui/src/editor/flow_editor.rs#L378-L387)

面板容器 div 增加事件拦截：
```rust
container = container.child(
    div()
        .absolute()
        .right_0().top_0().bottom_0()
        .id("panel-container")
        .on_mouse_down(MouseButton::Left, |_, _, _, _| {})  // 消费点击，阻止冒泡到画布
        .on_mouse_down(MouseButton::Middle, |_, _, _, _| {})
        .child(panel_view),
);
```

#### 5.2 PanelView 重构为 schema 驱动

**文件**：[crates/gpui/src/panel/mod.rs](file:///d:/GitCode/RF/rust-agent-flow/crates/gpui/src/panel/mod.rs)

**核心改动**：删除 `render_condition_panel` / `render_loop_panel` / `render_simple_panel` 三个 per-kind 方法，替换为统一的 `render_schema_panel`：

- `PanelView` 持有 `fields: Vec<FieldEntity>`，每个 FieldEntity 对应一个 FieldSpec，持有可编辑的 `Entity<InputState>` 或状态
- `build()` 时从 `flow_node.schema().fields` 构建字段实体
- `render_schema_panel()` 遍历 fields，按 FieldType 渲染对应 gpui-component 组件：
  - `Text` → `Input`（单行）
  - `TextArea` → `Input`（多行，rows=4）
  - `CodeEditor` → `Input` code_editor 模式，rows=1，line_number(false)，单行表达式
  - `CodeBlock` → `Input` code_editor 模式，rows=4，line_number(true)
  - `Number` → `Input`（数字过滤）
  - `Switch` → `gpui_component::Switch`
  - `Dropdown` → `gpui_component::Dropdown`
  - `List` → 列表表格（每行渲染 item_fields + 删除按钮 + 底部添加按钮）
- 字段变化通过 `subscribe_in` → `NodeAction::SetData(key, value)` 同步
- `sync_from_node` 遍历 fields 同步值

#### 5.3 面板视觉优化

- 宽度：320px → 300px（更紧凑）
- 内边距：`p_4` → `p_3`
- 间距：`gap(12)` → `gap(8)`
- 分区：用 `divider` 分隔标题区/字段区/列表区
- 标题：节点类型图标 + label + kind 副标题，紧凑排列
- 字段标签：12px font_semibold，左对齐
- 列表表格：紧凑行高（28px），名称/类型/值三列，删除按钮行尾
- 统一使用 theme 颜色

#### 5.4 单行 CodeEditor 实现

**文件**：[crates/gpui/src/panel/mod.rs](file:///d:/GitCode/RF/rust-agent-flow/crates/gpui/src/panel/mod.rs)

`new_code_input` 方法（替代 `new_rhai_input` 用于单行表达式）：
```rust
fn new_code_input(
    syntax_service: &SharedSyntaxService,
    default_value: &str,
    placeholder: &str,
    window: &mut Window,
    cx: &mut Context<Self>,
) -> Entity<InputState> {
    let language = syntax_service.language_for("rhai");
    cx.new(|cx| {
        let mut state = InputState::new(window, cx)
            .default_value(default_value)
            .placeholder(placeholder);
        if let Some(lang) = language {
            // 单行代码：rows=1, 无行号
            state = state.code_editor(lang).line_number(false).rows(1);
            // TODO: 若 InputState 支持 on_key_down，拦截 Enter 防止换行
        } else {
            state = state.multi_line(false);  // 单行模式
        }
        state
    })
}
```

实现时验证：若 `code_editor` 模式下 rows=1 仍接受换行符，需在 `on_change` 回调中过滤 `\n`，或回退到普通 `Input` + 等宽字体。

---

### 阶段 6：流程编辑 — 连线添加节点（任务3）

#### 6.1 分析文档

在计划文件中已包含分析（见下方「流程编辑机制分析」章节），无需单独文档。

#### 6.2 边中点「+」按钮渲染

**文件**：[crates/gpui/src/editor/rendering.rs](file:///d:/GitCode/RF/rust-agent-flow/crates/gpui/src/editor/rendering.rs)

新增 `render_edge_plus_buttons(&self) -> Vec<AnyElement>`：
- 遍历所有边，计算中点（逻辑坐标）
- 在内容层渲染 absolute div「+」按钮，位置 = 中点 × scale
- 按钮样式：圆形，24px（屏幕坐标），居中「+」字符，theme 配色
- 仅在边未被选中/非循环回环边时显示（回环边不插入节点）
- 按钮携带 edge_id（通过命中测试识别，非闭包捕获）

#### 6.3 命中测试扩展

**文件**：[crates/gpui/src/editor/hit_test.rs](file:///d:/GitCode/RF/rust-agent-flow/crates/gpui/src/editor/hit_test.rs)

- `HitResult` 增加 `EdgePlusButton(EdgeId)` 变体
- hit_test 中检查鼠标位置是否命中某边中点「+」按钮（24px 圆形区域）

#### 6.4 交互处理

**文件**：[crates/gpui/src/editor/interaction.rs](file:///d:/GitCode/RF/rust-agent-flow/crates/gpui/src/editor/interaction.rs)

- `on_mouse_down` 增加 `(MouseButton::Left, HitResult::EdgePlusButton(edge_id))` 分支
- 设置 `self.pending_edge_for_insert = Some(edge_id)` + 打开节点选择 PopupMenu
- PopupMenu 选项：Action/Condition/Loop/Variable/Adapter/Agent（排除 Start/End）
- 选择后调用 `insert_node_on_edge(edge_id, kind, cx)`

#### 6.5 插入节点逻辑

**文件**：[crates/gpui/src/editor/flow_editor.rs](file:///d:/GitCode/RF/rust-agent-flow/crates/gpui/src/editor/flow_editor.rs)

新增 `pub(crate) fn insert_node_on_edge(&mut self, edge_id: EdgeId, kind: &str, cx)`：
1. 读取原边 source/target/ports/type
2. 从 registry 获取 schema，用 `schema.default_size` + 默认 data（各字段 default 值）创建新节点
3. 删除原边
4. 添加 source→new（保留 source_port）和 new→target（保留 target_port）两条边
5. `sync_node_sizes()` + `relayout()` + `cx.notify()`

#### 6.6 PopupMenu 集成

使用 gpui-component `PopupMenu` / `Dropdown` 显示节点选择列表，定位到「+」按钮点击位置。需验证 gpui-component PopupMenu API。

---

## 流程编辑机制分析（任务3分析部分）

### 节点添加机制

**当前状态**：无 UI 添加节点入口，仅能通过代码 `graph.add_node_with_size()` 添加。

**方案**：连线「+」按钮 + 节点选择面板（本计划阶段6实现）。

**数据流**：
1. 用户点击边中点「+」→ `HitResult::EdgePlusButton(edge_id)`
2. 弹出 PopupMenu → 用户选择 kind
3. `insert_node_on_edge(edge_id, kind)`：
   - 从 `registry.get(kind).schema()` 获取 default_size + fields 默认值
   - 构造 `node.data`：遍历 schema.fields，填入 `field.default`
   - 创建节点 → 删原边 → 加两条新边 → relayout
4. 新节点自动选中 → 显示属性面板可编辑

### 节点编辑机制

**当前状态**：Condition/Loop 可编辑，其他只读。

**方案**：schema.fields 驱动面板（本计划阶段5实现），所有节点可编辑。

**数据同步机制**（已有，无需改动）：
1. PanelView 字段 Input 变化 → `InputEvent::Change`
2. `subscribe_in` 回调 → `on_action(NodeAction::SetData(key, value))`
3. `FlowEditorView::handle_node_action` 更新 `node.data[key]` + `sync_node_sizes()` + `relayout()`
4. `render` → `ensure_panel_view` → `sync_from_node` 同步回 PanelView（`syncing` 标记防回环）

### 数据同步保障

- **节点→面板**：`sync_from_node` 在每帧 render 时检查节点数据变化并同步
- **面板→节点**：`NodeAction::SetData` 即时更新
- **画布→布局**：`relayout()` 在每次 SetData 后调用，确保尺寸变化后重新排版
- **回环防护**：`syncing: bool` 标记避免 sync 时触发 on_change

---

## 涉及文件清单

| 文件 | 改动类型 | 任务 |
|---|---|---|
| `crates/core/src/schema/mod.rs` | 新增 FieldType/FieldSpec/FlowDocument 等 | 2 |
| `crates/core/src/graph/mod.rs` | 新增 from_document/to_document | 2 |
| `crates/gpui/src/builtin/*.rs` (8个) | 为 schema 添加 fields | 2 |
| `crates/gpui/src/i18n.rs` | 新增翻译键 + 中英文 | 1,2,3 |
| `crates/gpui/src/editor/toolbar.rs` | 重构为 IconButton/Button + Tooltip + Dropdown | 1,2 |
| `crates/gpui/src/editor/flow_editor.rs` | 数据源切换 + insert_node_on_edge + 面板事件拦截 | 2,3,4 |
| `crates/gpui/src/editor/rendering.rs` | 边中点「+」按钮渲染 | 3 |
| `crates/gpui/src/editor/hit_test.rs` | EdgePlusButton 命中 | 3 |
| `crates/gpui/src/editor/interaction.rs` | 「+」按钮交互 + PopupMenu | 3 |
| `crates/gpui/src/panel/mod.rs` | schema 驱动面板重构 + 单行 CodeEditor + 视觉优化 | 4,5 |
| `demo/src/main.rs` | 数据驱动重构 + 3 个数据源 | 2 |

---

## 验证步骤

### 编译验证
```powershell
cargo build --workspace
cargo build -p rust-agent-flow-demo
```

### 运行验证
```powershell
cargo run -p rust-agent-flow-demo
```

### 功能验证清单

**任务1（工具栏 tooltip + i18n）**：
- [ ] 鼠标悬停每个工具栏按钮显示 tooltip
- [ ] 切换中英文，tooltip 文案随之变化
- [ ] 边类型/点阵密度 Dropdown 正常工作

**任务2（数据协议 + 数据驱动 demo）**：
- [ ] 工具栏「数据源」Dropdown 可切换 3 个流程
- [ ] 切换后画布正确显示对应流程节点和边
- [ ] FlowDocument 可序列化（`to_document` 输出 JSON 结构正确）

**任务3（流程编辑）**：
- [ ] 边中点显示「+」按钮
- [ ] 点击「+」弹出节点选择面板
- [ ] 选择节点类型后，新节点插入到边中，原边替换为两条新边
- [ ] 新节点自动选中，属性面板可编辑

**任务4（属性面板编辑 + 同步）**：
- [ ] 点击属性面板任意位置（包括标题、padding 区）面板不消失
- [ ] Action 节点可编辑 label/desc，实时同步到画布
- [ ] Start 节点可编辑 params 列表（增删改），实时同步
- [ ] End 节点可编辑 returns 列表
- [ ] Variable/Adapter/Agent 节点均可编辑
- [ ] 编辑 Condition 条件项，画布节点尺寸自适应

**任务5（面板组件 + 单行 CodeEditor）**：
- [ ] 面板使用 Switch（布尔字段）、Dropdown（枚举字段）、Button（添加/删除）
- [ ] 面板视觉紧凑（300px 宽，p_3 内边距，gap_2 间距）
- [ ] Condition 条件表达式为单行 CodeEditor（无行号、无左边距）
- [ ] Loop 表达式为多行 CodeBlock（带行号）
- [ ] 亮/暗主题下面板样式正确

---

## 假设与风险

### 假设
1. gpui-component (rev e416af7f) 提供 `Tooltip`、`Button`/`IconButton`、`Dropdown`、`Switch`、`PopupMenu` 组件 — 实施前需验证 API
2. `InputState` 的 `code_editor` 模式支持 `rows(1)` 且可拦截 Enter 键 — 若不支持，回退到普通 `Input` + 等宽字体
3. core 层 `FlowGraph::from_document` 不依赖 gpui 层 NodeRegistry — size 默认值由 demo 数据显式声明或用通用默认

### 风险
1. **gpui-component API 差异**：Tooltip/Button 的具体构造方式需对照源码。缓解：阶段3.1 先验证 API
2. **schema 驱动面板的复杂度**：List 字段（conditions/params）的动态增删与 InputState 生命周期管理较复杂。缓解：保留现有 condition_inputs/params_inputs Vec 模式，仅统一渲染逻辑
3. **边中点「+」按钮命中测试**：边在 canvas paint 层，按钮在 div 层，坐标需对齐。缓解：用逻辑坐标计算中点，渲染时 ×scale，命中测试用逻辑坐标
4. **面板重构范围大**：panel/mod.rs 744 行重写风险。缓解：分步进行，先修复消失问题 + Action 可编辑，再逐步迁移其他节点

### 实施顺序建议
1. 阶段1（schema）→ 阶段2（i18n）→ 阶段5.1-5.2（面板修复+schema驱动）— 先打通数据链路
2. 阶段3（工具栏）→ 阶段4（demo）— 数据源切换可视化
3. 阶段6（连线添加节点）— 独立功能
4. 阶段5.3-5.4（视觉优化 + 单行 CodeEditor）— 收尾打磨
