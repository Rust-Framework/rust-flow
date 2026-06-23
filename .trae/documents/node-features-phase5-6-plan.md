# 节点功能完善 — 阶段 5-6 实施计划

## 概述

本计划承接已完成的阶段 1-4（节点删除、条件/循环节点展开收起），聚焦剩余两项工作：
- **阶段 5**：属性面板可编辑化（rhai 嵌入语法 + gpui-component Input 集成）
- **阶段 6**：生产级节点尺寸与视觉优化

---

## 当前状态分析

### 已完成（阶段 1-4，无需改动）

| 模块 | 状态 | 关键文件 |
|------|------|----------|
| 基础设施 | ✅ | `flow_node.rs`（NodeAction/ActionCallback）、`theme.rs`（6 按钮颜色）、`hit_test.rs`（DeleteButton/ToggleButton）、`interaction.rs`（hover+点击）、`rendering.rs`（Entity+ActionCallback 注入）、`flow_editor.rs`（handle_node_action/delete_node） |
| 节点删除 UI | ✅ | `common.rs`（render_delete_button）、`action.rs`（hover 显示删除按钮）、`start.rs`/`end.rs`（无删除按钮） |
| 条件节点展开/收起 | ✅ | `condition.rs`（is_collapsed + 收起/展开渲染 + port_position + content_size） |
| 循环节点展开/收起 | ✅ | `loop_node.rs`（is_collapsed + 收起/展开渲染 + port_position + content_size） |

### 待完成

| 模块 | 状态 | 说明 |
|------|------|------|
| rhai 依赖 | ❌ | Cargo.toml 未添加 rhai crate |
| 属性面板可编辑 | ❌ | condition/loop 的 `get_panel` 仍调用只读 `render_simple_panel` |
| PanelView 有状态化 | ❌ | 当前 `PanelView` 为 `RenderOnce`，无法持有 `Entity<InputState>` |
| 生产级节点尺寸 | ❌ | 节点宽高配比、文本溢出、视觉细节待优化 |

---

## 核心架构决策

### 决策 1：PanelView 重构为有状态实体视图

**问题**：当前 `PanelView` 实现 `RenderOnce`，每次渲染重建实例，无法持有 `Entity<InputState>`，导致 Input 焦点丢失。

**方案**：将 `PanelView` 改为实现 `Render` trait 的 `Entity<PanelView>`，持有可编辑字段的 `Entity<InputState>`。

**改动范围**：
- `crates/gpui/src/panel/mod.rs` — 重构 PanelView
- `crates/gpui/src/editor/rendering.rs` — `render_panel` 改为返回 `AnyView` 而非 `AnyElement`
- `crates/gpui/src/editor/flow_editor.rs` — 持有 `Entity<PanelView>` 字段，选中节点变化时同步

### 决策 2：rhai 语法高亮回退策略

**问题**：gpui-component 的 `highlighter::Language` 枚举不包含 Rhai（Cargo.lock 仅确认 tree-sitter-json）。

**方案**：
- 添加 `rhai` crate 依赖（用于表达式求值，非高亮）
- 语法高亮使用 `Language::Rust` 作为回退（Rhai 语法与 Rust 相似：`let`、`fn`、`if`、运算符）
- 条件表达式编辑器使用 `Input` 多行模式 + `Highlighter::new(Language::Rust)` 手动高亮

### 决策 3：条件分支列表编辑交互

**方案**：
- 属性面板显示条件分支列表（每项一行：label 输入框 + 删除按钮）
- 底部"添加分支"按钮（新增 `if_N` 端口 + 条件项）
- 编辑条件项 label 时，通过 `NodeAction::SetData("conditions", new_array)` 更新整个 conditions 数组
- 端口同步：conditions 数组变化时，`handle_node_action` 的 `SetData` 分支需调用 `sync_node_ports` 重建端口列表

---

## 阶段 5：属性面板可编辑化

### 5.1 添加 rhai 依赖

**文件**：`Cargo.toml`（workspace 根）

**改动**：
```toml
[workspace.dependencies]
# ... 现有依赖 ...
rhai = "1.19"
```

**文件**：`crates/gpui/Cargo.toml`

**改动**：
```toml
[dependencies]
# ... 现有依赖 ...
rhai = { workspace = true }
```

**说明**：rhai 用于后续条件表达式求值（本阶段仅引入依赖，不实现求值逻辑）。

### 5.2 重构 PanelView 为有状态实体视图

**文件**：`crates/gpui/src/panel/mod.rs`

**当前结构**（RenderOnce，无状态）：
```rust
pub struct PanelView {
    pub node: Node,
    pub flow_node: Option<Arc<dyn IFlowNode>>,
    pub theme: Theme,
    pub on_action: Option<ActionCallback>,
}
impl RenderOnce for PanelView { ... }
```

**目标结构**（Render，有状态）：
```rust
pub struct PanelView {
    node: Node,
    flow_node: Option<Arc<dyn IFlowNode>>,
    theme: Theme,
    on_action: Option<ActionCallback>,
    // 可编辑字段对应的 InputState
    label_input: Entity<InputState>,
    // Condition 节点专用
    condition_inputs: Vec<Entity<InputState>>,  // 每个条件项一个
    // Loop 节点专用
    loop_mode_input: Entity<InputState>,        // 循环模式（下拉或输入）
    loop_expr_input: Entity<InputState>,        // 条件表达式（多行）
    // 同步标记：避免节点更新时回环触发 on_change
    syncing: bool,
}

impl PanelView {
    pub fn new(node: Node, flow_node: Option<Arc<dyn IFlowNode>>, theme: Theme, on_action: Option<ActionCallback>, cx: &mut App) -> Entity<Self> {
        cx.new(|cx| {
            let label_input = cx.new(|cx| InputState::new(cx).set_value(label_of(&node).as_ref(), cx));
            // ... 初始化其他 InputState ...
            Self { node, flow_node, theme, on_action, label_input, ... syncing: false }
        })
    }

    /// 节点数据变化时，从 node 同步到 InputState（避免回环）
    pub fn sync_from_node(&mut self, node: Node, cx: &mut Context<Self>) {
        self.syncing = true;
        self.node = node;
        // 同步 label
        let label = label_of(&self.node);
        self.label_input.update(cx, |s, cx| { s.set_value(label.as_ref(), cx); });
        // 同步 conditions / loop expr ...
        self.syncing = false;
        cx.notify();
    }
}

impl Render for PanelView {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        // 根据 node.kind 渲染不同面板
        // 使用 cx.listener() 绑定 on_change
    }
}
```

**关键实现细节**：

1. **on_change 回调**（使用 `cx.listener`）：
```rust
Input::new(self.label_input.clone())
    .placeholder("节点名称")
    .on_change(cx.listener(|this, state: &mut Entity<InputState>, cx| {
        if this.syncing { return; }  // 避免回环
        let value = state.read(cx).value().to_string();
        if let Some(on_action) = &this.on_action {
            on_action(NodeAction::SetData("label".into(), serde_json::json!(value)), cx);
        }
    }))
```

2. **条件分支列表渲染**：
```rust
// 每个条件项一行
for (i, cond_input) in self.condition_inputs.iter().enumerate() {
    col = col.child(
        div().flex().items_center().gap(px(8.0))
            .child(div().text_size(px(12.0)).child(format!("If {}", i + 1)))
            .child(Input::new(cond_input.clone()).placeholder("条件表达式").w(px(200.0)))
            .child(delete_branch_button(i, cx))  // 删除该分支
    );
}
// 添加分支按钮
col = col.child(add_branch_button(cx));
```

3. **添加/删除分支**：
```rust
fn add_branch(&mut self, cx: &mut Context<Self>) {
    let new_input = cx.new(|cx| InputState::new(cx));
    self.condition_inputs.push(new_input);
    // 同步到 node.data["conditions"]
    self.sync_conditions_to_node(cx);
}

fn delete_branch(&mut self, idx: usize, cx: &mut Context<Self>) {
    self.condition_inputs.remove(idx);
    self.sync_conditions_to_node(cx);
}

fn sync_conditions_to_node(&self, cx: &mut Context<Self>) {
    let conditions: Vec<serde_json::Value> = self.condition_inputs.iter().enumerate()
        .map(|(i, input)| {
            let label = input.read(cx).value().to_string();
            serde_json::json!({ "id": format!("if_{}", i), "label": label })
        })
        .collect();
    if let Some(on_action) = &self.on_action {
        on_action(NodeAction::SetData("conditions".into(), serde_json::json!(conditions)), cx);
    }
}
```

### 5.3 修改 rendering.rs 的 render_panel

**文件**：`crates/gpui/src/editor/rendering.rs`

**当前**：
```rust
pub(crate) fn render_panel(&self, entity: Entity<Self>) -> Option<gpui::AnyElement> {
    // ... 创建 PanelView 并返回 AnyElement
}
```

**改为**：返回 `Option<AnyView>`（GPUI 视图实体），由 `FlowEditorView` 持有。

**文件**：`crates/gpui/src/editor/flow_editor.rs`

**改动**：
```rust
pub struct FlowEditorView {
    // ... 现有字段 ...
    panel_view: Option<Entity<PanelView>>,  // 新增：持有面板视图实体
}

// render 方法中
fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
    // ... 现有渲染 ...
    // 选中节点变化时，创建或更新 panel_view
    if let Some(node_id) = self.selected {
        let node = self.graph.nodes.get(node_id).cloned();
        if let Some(node) = node {
            let flow_node = self.registry.get(&node.kind).cloned();
            let on_action = /* 创建 ActionCallback */;
            if let Some(pv) = &self.panel_view {
                pv.update(cx, |view, cx| view.sync_from_node(node, cx));
            } else {
                self.panel_view = Some(PanelView::new(node, flow_node, theme, on_action, cx));
            }
        }
    } else {
        self.panel_view = None;
    }
    // 渲染 panel_view
}
```

### 5.4 Condition 节点可编辑面板

**文件**：`crates/gpui/src/builtin/condition.rs`

**改动 `get_panel`**：由于 PanelView 现在是有状态视图，`get_panel` 的角色变化——面板渲染逻辑移至 `PanelView::render` 中根据 `node.kind` 分发。

**Condition 面板内容**：
1. **节点名称**（label）— Input 单行
2. **条件分支列表** — 每项一个 Input（rhai 表达式）+ 删除按钮
3. **添加分支按钮**
4. **Else 兜底说明**（只读文本，提示 else 为自动兜底）

**条件表达式 rhai 高亮**：
```rust
use gpui_component::highlighter::{Highlighter, Language};

// 在渲染条件表达式 Input 时，应用 Rust 语法高亮作为 rhai 近似
let highlighter = Highlighter::new(Language::Rust, cx);
// 注：具体高亮集成方式需验证 gpui-component API
```

### 5.5 Loop 节点可编辑面板

**文件**：`crates/gpui/src/builtin/loop_node.rs`

**Loop 面板内容**：
1. **节点名称**（label）— Input 单行
2. **循环模式** — 选择器（4 种模式）：
   - 数组循环 (For Each)
   - 条件循环 (While / Do-Until)
   - 计次循环 (For Loop)
   - 批量/并行循环
3. **条件表达式** — Input 多行（rhai 嵌入语法，Rust 高亮回退）

**循环模式存储**：
```json
{
  "label": "Loop",
  "loop_mode": "for_each",  // for_each | while | for_loop | batch_parallel
  "loop_expr": "item.price > 100"  // rhai 表达式
}
```

**循环模式选择器**：由于 gpui-component 的 Dropdown/Select 组件 API 未验证，初版使用 4 个按钮切换（点击高亮当前模式），后续可升级为 Dropdown。

### 5.6 handle_node_action 增强：条件分支端口同步

**文件**：`crates/gpui/src/editor/flow_editor.rs`

**当前 `handle_node_action` 的 `SetData` 分支**：
```rust
NodeAction::SetData(key, value) => {
    if let Some(node) = graph.nodes.get_mut(node_id) {
        node.data[key] = value;
        self.sync_node_sizes(cx);
        self.relayout(cx);
    }
}
```

**增强**：当 key == "conditions" 时，需同步端口列表（新增/删除 if_N 端口）：
```rust
NodeAction::SetData(key, value) => {
    if let Some(node) = graph.nodes.get_mut(node_id) {
        node.data[key.clone()] = value.clone();
        // 条件分支变化时同步端口
        if key == "conditions" && node.kind == "condition" {
            self.sync_condition_ports(node_id, cx);
        }
        self.sync_node_sizes(cx);
        self.relayout(cx);
    }
}

fn sync_condition_ports(&mut self, node_id: NodeId, cx: &mut Context<Self>) {
    // 根据 node.data["conditions"] 重建端口列表
    // 保留 "in" 和 "else" 端口，重建 if_0, if_1, ...
    // 同步 schema.ports
}
```

---

## 阶段 6：生产级节点尺寸与视觉优化

### 6.1 节点尺寸调整

**目标**：所有节点宽高配比合理，内容显示生产级水平。

**当前尺寸**：
- Start/End: 120×48
- Action: 180×72
- Condition: 220×144（TITLE_H=36 + 3×ITEM_H=36）
- Loop: 220×80（TITLE_H=36 + BODY_H=44）

**优化方案**：

| 节点类型 | 当前 | 优化后 | 理由 |
|----------|------|--------|------|
| Start | 120×48 | 120×48 | 保持，椭圆胶囊形足够 |
| End | 120×48 | 120×48 | 保持 |
| Action | 180×72 | 200×64 | 宽度+20容纳长文本，高度-8更紧凑 |
| Condition | 220×144 | 240×144 | 宽度+20容纳条件表达式 |
| Loop | 220×80 | 240×80 | 宽度+20与 Condition 统一 |
| Condition 收起 | 220×36 | 240×36 | 宽度统一 |
| Loop 收起 | 220×36 | 240×36 | 宽度统一 |

**文件改动**：
- `crates/gpui/src/builtin/action.rs` — schema size
- `crates/gpui/src/builtin/condition.rs` — schema size
- `crates/gpui/src/builtin/loop_node.rs` — schema size

### 6.2 文本溢出处理

**问题**：条件表达式、Action 描述可能超出节点宽度。

**方案**：使用 `text_ellipsis()` 或 `truncate()` 处理溢出。

**文件改动**：
- `crates/gpui/src/builtin/condition.rs` — 条件项 label 溢出处理
- `crates/gpui/src/builtin/action.rs` — desc 溢出处理
- `crates/gpui/src/builtin/loop_node.rs` — desc 溢出处理

```rust
// 条件项 label 渲染
div()
    .text_size(px(12.0 * s))
    .text_color(t.cond_item_text)
    .overflow_hidden()       // 溢出隐藏
    .text_ellipsis()         // 省略号
    .child(format!("If {}", cond_label))
```

**注**：需验证 `text_ellipsis()` 是否为 gpui-component 的 StyledExt 方法，或使用 gpui 原生的 `truncate()`。

### 6.3 视觉细节打磨

**优化项**：

1. **端口悬停高亮**：鼠标悬停端口时，端口圆圈放大或高亮边框
   - 文件：`hit_test.rs`（检测端口悬停）、`rendering.rs`（渲染高亮）
   - 复杂度：中，需在 InteractionState 增加 `hovered_port: Option<(NodeId, PortId)>`

2. **节点选中阴影**：选中节点添加外阴影或更明显的边框
   - 文件：各 builtin 节点的 `get_view`
   - 当前已有 `border_selected` 颜色，可增加 `box_shadow`

3. **连线优化**：收起节点的连线使用更平滑的贝塞尔曲线
   - 文件：`rendering.rs` 的 `render_edges`
   - 当前已有贝塞尔曲线，可调整控制点

4. **按钮交互反馈**：删除/切换按钮 hover 时背景色变化
   - 文件：`common.rs` 的 `render_delete_button` / `render_toggle_button`
   - 需增加 hover 状态检测（hit_test 已支持）

**优先级**：6.1 和 6.2 为必做，6.3 视时间情况选择性实现。

---

## 实施顺序

1. **5.1** 添加 rhai 依赖 → `cargo build` 验证
2. **5.2** 重构 PanelView 为有状态实体视图 → `cargo build` 验证
3. **5.3** 修改 rendering.rs + flow_editor.rs 集成 PanelView → `cargo build` 验证
4. **5.4** Condition 可编辑面板 → `cargo build` 验证
5. **5.5** Loop 可编辑面板 → `cargo build` 验证
6. **5.6** 条件分支端口同步 → `cargo build` 验证
7. **6.1** 节点尺寸调整 → `cargo build` 验证
8. **6.2** 文本溢出处理 → `cargo build` 验证
9. **6.3** 视觉细节打磨（可选）→ `cargo build` 验证
10. **最终验证**：`cargo build` + 手动运行 demo 验证交互

---

## 风险与缓解

### 风险 1：gpui-component Input API 不确定

**问题**：gpui-component 源码未 check out，Input/InputState 的确切 API 签名未验证。

**缓解**：
- 实施前先执行 `cargo fetch` check out 源码
- 阅读 `gpui-component/src/input/` 目录确认 API
- 若 API 与预期不符，调整 PanelView 实现

### 风险 2：PanelView 重构导致回归

**问题**：PanelView 从 RenderOnce 改为 Render 是架构级改动，可能影响现有面板渲染。

**缓解**：
- 保持 `render_simple_panel` 用于 Start/End/Action 节点（无需编辑的节点）
- 仅 Condition/Loop 节点使用新的可编辑面板
- 增量改动，每步 `cargo build` 验证

### 风险 3：条件分支端口同步可能破坏连线

**问题**：动态增删 if_N 端口时，已存在的连线可能引用失效端口。

**缓解**：
- `sync_condition_ports` 时，清理引用已删除端口的边
- 保留 `if_0`、`if_1` 等端口的稳定性（按索引复用）
- 新增端口不自动连线，需用户手动连接

### 风险 4：rhai 高亮回退效果不佳

**问题**：Rust 高亮可能无法正确识别 rhai 特有语法（如 `#{...}` 对象字面量）。

**缓解**：
- 初版接受 Rust 高亮作为近似
- 后续可扩展 highlighter 支持 Rhai（需集成 tree-sitter-rhai）

---

## 验证步骤

1. **编译验证**：每个子阶段完成后执行 `cargo build`
2. **功能验证**（手动运行 demo）：
   - 选中 Condition 节点 → 面板显示可编辑的节点名称 + 条件列表
   - 编辑节点名称 → 节点标题栏实时更新
   - 添加条件分支 → 节点新增 if_N 端口 + 条件项行
   - 删除条件分支 → 节点移除对应端口 + 条件项行 + 连线清理
   - 选中 Loop 节点 → 面板显示节点名称 + 循环模式 + 条件表达式
   - 切换循环模式 → 节点描述更新
   - 编辑条件表达式 → 面板状态保存
   - 收起/展开 Condition/Loop → 排版正确重排
   - 节点尺寸视觉检查 → 宽高配比合理，无文本溢出

---

## 假设与决策

1. **假设**：gpui-component 的 `Input` 组件通过 `Entity<InputState>` 驱动，支持 `on_change` 回调（基于 longbridge/gpui-component v0.5.2 的公开 API 知识）
2. **决策**：rhai 语法高亮使用 `Language::Rust` 回退（避免集成 tree-sitter-rhai 的额外复杂度）
3. **决策**：循环模式选择器初版使用按钮组（避免 Dropdown API 不确定性）
4. **决策**：条件分支端口同步采用索引复用策略（if_0, if_1, ... 按顺序）
5. **假设**：`text_ellipsis()` 或等效方法可用（需验证 gpui-component StyledExt）
