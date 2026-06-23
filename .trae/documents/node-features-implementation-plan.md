# 节点功能完整实现计划

## 概述

本计划是 `node-features-complete-design.md` 设计文档的**执行计划**，记录当前实现进度并规划剩余工作。目标：为 rust-agent-flow 流程编辑器实现节点删除（含连线自动修复）、条件分支节点展开/收起、循环节点展开/收起、属性面板编辑（CodeEditor + rhai 语法）、生产级节点尺寸优化。

**设计文档**：[node-features-complete-design.md](file:///d:/GitCode/RF/rust-agent-flow/.trae/documents/node-features-complete-design.md)（含完整设计细节、ASCII 布局图、代码片段，本计划引用其章节不再重复）

***

## 当前状态分析

### 阶段 1（基础设施）已完成部分

| 文件 | 状态 | 说明 |
| --- | --- | --- |
| [flow_node.rs](file:///d:/GitCode/RF/rust-agent-flow/crates/gpui/src/node/flow_node.rs) | ✅ 完成 | `NodeAction` 枚举、`ActionCallback` 类型、`NodeViewCtx` 扩展 `hovered` + `on_action` |
| [node/mod.rs](file:///d:/GitCode/RF/rust-agent-flow/crates/gpui/src/node/mod.rs) | ✅ 完成 | 导出 `ActionCallback`、`NodeAction` |
| [view.rs](file:///d:/GitCode/RF/rust-agent-flow/crates/gpui/src/node/view.rs) | ✅ 完成 | `NodeView` 扩展 `hovered` + `on_action` 字段、builder 方法、`RenderOnce` 传递到 ctx |
| [panel/mod.rs](file:///d:/GitCode/RF/rust-agent-flow/crates/gpui/src/panel/mod.rs) | ✅ 完成 | `PanelView` 扩展 `on_action` 字段、builder、`RenderOnce` 传递到 ctx |
| [theme.rs](file:///d:/GitCode/RF/rust-agent-flow/crates/gpui/src/theme.rs) | ✅ 完成 | 6 个新颜色字段（delete_btn_*, toggle_btn_*, collapse_pill_*）亮/暗主题值 |
| [flow_editor.rs](file:///d:/GitCode/RF/rust-agent-flow/crates/gpui/src/editor/flow_editor.rs) | ✅ 完成 | `hovered` 字段、`handle_node_action` 方法、`delete_node` 方法（线性桥接） |

### 阶段 1（基础设施）未完成部分

| 文件 | 状态 | 待办 |
| --- | --- | --- |
| [hit_test.rs](file:///d:/GitCode/RF/rust-agent-flow/crates/gpui/src/editor/hit_test.rs) | ❌ 未改 | 新增 `DeleteButton(NodeId)` / `ToggleButton(NodeId)` 变体 + 按钮命中检查 |
| [interaction.rs](file:///d:/GitCode/RF/rust-agent-flow/crates/gpui/src/editor/interaction.rs) | ❌ 未改 | `on_mouse_move` Idle 分支悬停追踪 + `on_mouse_down` 按钮命中处理 |
| [rendering.rs](file:///d:/GitCode/RF/rust-agent-flow/crates/gpui/src/editor/rendering.rs) | ❌ 未改 | `render_nodes`/`render_panel` 接收 `Entity<Self>` + 注入动作回调 + 传递 `hovered` |
| [common.rs](file:///d:/GitCode/RF/rust-agent-flow/crates/gpui/src/builtin/common.rs) | ❌ 未改 | 新增 `render_delete_button` / `render_toggle_button` 辅助函数 |

### 阶段 2-6 全部未开始

- 阶段 2（节点删除 UI）：各节点 `get_view` 添加删除按钮
- 阶段 3（条件展开/收起）：`condition.rs` 收起渲染 + `content_size` + `port_position`
- 阶段 4（循环展开/收起）：`loop_node.rs` 收起渲染 + `content_size` + `port_position` + 循环模式数据
- 阶段 5（属性面板编辑）：rhai 依赖 + 面板状态管理 + 可编辑面板
- 阶段 6（生产级优化）：节点尺寸调整 + 视觉打磨

### 关键 API 调研结果

1. **gpui-component 输入组件**：`Input`（非 TextInput）+ `Entity<InputState>` 状态管理（位于 `crates/ui/src/input/input.rs`）
2. **语法高亮**：`highlighter` 模块支持 Rust/JavaScript/Python 等，**不直接支持 rhai**。rhai 语法接近 Rust，可使用 `Language::Rust` 作为高亮回退
3. **`Context::entity()` API**：需在实现时验证。备选方案：在 `FlowEditorView::new` 中存储 `WeakEntity<Self>` 或在 `render` 中通过 `cx.entity()` 获取

***

## 实现计划

### 阶段 1 补完：基础设施剩余部分

#### 1.1 命中测试扩展

**文件**：[hit_test.rs](file:///d:/GitCode/RF/rust-agent-flow/crates/gpui/src/editor/hit_test.rs)

**改动**：
1. `HitResult` 枚举新增 `DeleteButton(NodeId)` 和 `ToggleButton(NodeId)` 变体
2. 在 `hit_test` 方法中，端口命中检查之后、节点主体命中检查之前，增加按钮命中检查
3. 删除按钮命中检查：仅 `node.kind != "start" && node.kind != "end"` 的节点，按钮区域 = 右上角 20×20（逻辑坐标，距右边缘 4px，距顶 4px）
4. 切换按钮命中检查：仅 `node.kind == "condition" || node.kind == "loop"` 的节点，按钮区域 = 删除按钮左侧 4px，垂直居中于标题栏（标题栏高 36px）

**关键代码**（参考设计文档 2.3 节）：
```rust
pub(crate) enum HitResult {
    Empty,
    Node(NodeId),
    OutPort(NodeId, PortId),
    InPort(NodeId, PortId),
    DeleteButton(NodeId),    // 新增
    ToggleButton(NodeId),    // 新增
}
```

按钮命中检查需在端口命中之后、节点主体命中之前（避免按钮区域被节点主体"吞掉"）。

#### 1.2 交互层扩展

**文件**：[interaction.rs](file:///d:/GitCode/RF/rust-agent-flow/crates/gpui/src/editor/interaction.rs)

**改动**：
1. `on_mouse_move` 的 `InteractionState::Idle` 分支：增加悬停追踪（hit test → 更新 `self.hovered` → `cx.notify()`）
2. `on_mouse_down` 增加按钮命中处理：
   - `HitResult::DeleteButton(node_id)` → `self.delete_node(node_id, cx)`
   - `HitResult::ToggleButton(node_id)` → `self.handle_node_action(node_id, NodeAction::ToggleCollapse, cx)`

**关键代码**（参考设计文档 1.6、2.4 节）：
```rust
// on_mouse_move Idle 分支
InteractionState::Idle => {
    let hit = self.hit_test(logical);
    let new_hovered = match &hit {
        HitResult::Node(id) | HitResult::DeleteButton(id) | HitResult::ToggleButton(id) => Some(*id),
        HitResult::OutPort(id, _) | HitResult::InPort(id, _) => Some(*id),
        HitResult::Empty => None,
    };
    if new_hovered != self.hovered {
        self.hovered = new_hovered;
        cx.notify();
    }
}

// on_mouse_down 新增分支
(MouseButton::Left, HitResult::DeleteButton(node_id)) => {
    self.delete_node(node_id, cx);
}
(MouseButton::Left, HitResult::ToggleButton(node_id)) => {
    self.handle_node_action(node_id, NodeAction::ToggleCollapse, cx);
}
```

#### 1.3 渲染层动作回调注入

**文件**：[rendering.rs](file:///d:/GitCode/RF/rust-agent-flow/crates/gpui/src/editor/rendering.rs)

**改动**：
1. `render_nodes` 签名改为 `fn render_nodes(&self, entity: gpui::Entity<Self>) -> Vec<gpui::AnyElement>`
2. `render_panel` 签名改为 `fn render_panel(&self, entity: gpui::Entity<Self>) -> Option<gpui::AnyElement>`
3. 在两个方法内为每个节点创建动作回调闭包（`Arc<dyn Fn(NodeAction, &mut App) + Send + Sync>`），闭包捕获 `node_id` 和 `entity`，通过 `cx.update_entity` 调用 `handle_node_action`
4. `NodeView::new(...)` 链式调用增加 `.with_hovered(self.hovered == Some(node_id))` 和 `.with_on_action(Some(on_action))`
5. `PanelView::new(...)` 链式调用增加 `.with_on_action(Some(on_action))`
6. [flow_editor.rs](file:///d:/GitCode/RF/rust-agent-flow/crates/gpui/src/editor/flow_editor.rs) 的 `Render::render` 中通过 `cx.entity()` 获取 `Entity<Self>` 并传入

**关键代码**（参考设计文档 1.7 节）：
```rust
pub(crate) fn render_nodes(&self, entity: gpui::Entity<Self>) -> Vec<gpui::AnyElement> {
    // ...
    self.graph.nodes().map(|node| {
        let node_id = node.id;
        let entity = entity.clone();
        let on_action: ActionCallback = Arc::new(move |action, cx| {
            cx.update_entity(entity.clone(), |view, cx| {
                view.handle_node_action(node_id, action, cx);
            });
        });
        let view = NodeView::new(node.clone())
            // ... 现有 builder ...
            .with_hovered(self.hovered == Some(node_id))
            .with_on_action(Some(on_action));
        // ...
    }).collect()
}
```

**`cx.entity()` API 验证**：GPUI 的 `Context<T>` 提供 `entity(&self) -> Entity<T>` 方法。若该 API 不存在，备选方案是在 `FlowEditorView` 中存储 `WeakEntity<Self>`，在 `new` 时无法获取（构造期），改为在首次 `render` 时懒初始化。

#### 1.4 共享按钮渲染函数

**文件**：[common.rs](file:///d:/GitCode/RF/rust-agent-flow/crates/gpui/src/builtin/common.rs)

**改动**：新增两个辅助函数和常量（参考设计文档 2.1 节）：
1. `const DELETE_BTN_SIZE: f32 = 20.0;`
2. `const TOGGLE_BTN_SIZE: f32 = 20.0;`
3. `fn render_delete_button(node_w: f32, scale: f32, theme: &Theme) -> AnyElement` — 绝对定位右上角，×图标
4. `fn render_toggle_button(node_w: f32, scale: f32, collapsed: bool, theme: &Theme) -> AnyElement` — 删除按钮左侧，▽/▷图标

**注意**：按钮位置计算使用**逻辑坐标**（未乘 scale 的节点宽度），函数内部乘 scale 转屏幕坐标。但调用方传入的 `node_w` 应为逻辑宽度（`node.size.w`），函数内 `left = node_w - btn_size * scale - 4.0 * scale`。

***

### 阶段 2：节点删除 UI

#### 2.1 简单节点删除按钮

**文件**：[action.rs](file:///d:/GitCode/RF/rust-agent-flow/crates/gpui/src/builtin/action.rs)、[start.rs](file:///d:/GitCode/RF/rust-agent-flow/crates/gpui/src/builtin/start.rs)、[end.rs](file:///d:/GitCode/RF/rust-agent-flow/crates/gpui/src/builtin/end.rs)

**改动**：
- Action 节点：`get_view` 中，当 `ctx.hovered` 为 true 时，在 `render_node_card` 返回的容器后追加删除按钮子元素
- Start/End 节点：不渲染删除按钮（`deletable = false`）

**实现方式**：由于 `render_node_card` 返回的是 `AnyElement`（已封装容器），无法直接追加子元素。方案：在 `get_view` 中用外层 `div().relative()` 包裹 `render_node_card` 结果，再追加删除按钮：

```rust
fn get_view(&self, node: &Node, ctx: &mut NodeViewCtx) -> AnyElement {
    let card = render_node_card(&visual, w, h, ctx.scale, vertical, ctx.selected);
    if ctx.hovered {
        let mut wrapper = div().relative().w(px(w)).h(px(h));
        wrapper = wrapper.child(card);
        wrapper = wrapper.child(render_delete_button(node.size.w, ctx.scale, &ctx.theme));
        wrapper.into_any_element()
    } else {
        card
    }
}
```

#### 2.2 结构化节点删除按钮

**文件**：[condition.rs](file:///d:/GitCode/RF/rust-agent-flow/crates/gpui/src/builtin/condition.rs)、[loop_node.rs](file:///d:/GitCode/RF/rust-agent-flow/crates/gpui/src/builtin/loop_node.rs)

**改动**：在 `get_view` 的容器构建末尾，当 `ctx.hovered` 为 true 时追加删除按钮子元素（结构化节点的容器已是 `div().relative()`，可直接追加）。

***

### 阶段 3：条件分支节点展开/收起

**文件**：[condition.rs](file:///d:/GitCode/RF/rust-agent-flow/crates/gpui/src/builtin/condition.rs)

**数据存储**：`node.data["collapsed"]: bool`（默认 `false`）

**改动**（参考设计文档三章）：

1. **收起状态渲染**（`get_view`）：
   - 当 `node.data["collapsed"] == true` 时，高度 = `TITLE_H`（36px）
   - 仅渲染标题栏 + toggle 按钮 + delete 按钮（hover）+ "..." 胶囊
   - 不渲染条件项行和 else 行
   - 所有 out 端口合并到标题栏右边缘垂直居中（渲染一个合并端口圆点）

2. **展开状态渲染**（`get_view`）：
   - 保持现有条件项行 + else 行渲染逻辑
   - 额外添加 toggle 按钮（标题栏右侧，显示 "▽"）
   - delete 按钮（hover 时显示）
   - "..." 胶囊不显示

3. **`content_size` 覆写**：
   ```rust
   fn content_size(&self, node: &Node) -> SizeF {
       let collapsed = node.data.get("collapsed").and_then(|v| v.as_bool()).unwrap_or(false);
       let h = if collapsed { TITLE_H } else { content_height(node) };
       SizeF::new(node.size.w, h)
   }
   ```

4. **`port_position` 覆写**：
   - 收起状态：所有 out 端口（if_0, if_1, ..., else）返回同一位置（标题栏右边缘垂直居中）
   - 展开状态：保持现有逻辑

5. **toggle 按钮渲染**：调用 `render_toggle_button`，图标根据 `collapsed` 显示 ▽/▷

6. **"..." 胶囊渲染**：收起时在标题栏 label 右侧渲染小圆角矩形（高 16px，宽 24px），背景 `theme.collapse_pill_bg`，文字 `theme.collapse_pill_text`，内容 "..."

**收起状态横向布局**（参考设计文档 3.6 节）：
```
┌──────────────────────────────────────────┐
│[In]  ◆ Condition  ...  [▽] [×]    [●]→│  h=36
└──────────────────────────────────────────┘
                                         ↑ 合并出口
```

***

### 阶段 4：循环节点展开/收起

**文件**：[loop_node.rs](file:///d:/GitCode/RF/rust-agent-flow/crates/gpui/src/builtin/loop_node.rs)

**数据存储**：
```json
{
    "label": "Loop",
    "loop_mode": "for_each",
    "condition_expr": "i < 10",
    "collapsed": false
}
```

**循环模式**（参考设计文档 4.5 节）：
| 模式 | 值 | 说明 | 条件表达式 |
| --- | --- | --- | --- |
| 数组循环 | `for_each` | 遍历数组每个元素 | 不需要 |
| 条件循环 | `while` | while 条件成立时循环 | 需要（rhai bool 表达式） |
| 计次循环 | `for_loop` | for i in 0..n | 需要（rhai 范围表达式） |
| 批量/并行 | `batch_parallel` | 批量并行处理 | 不需要 |

**改动**（参考设计文档四章）：

1. **收起状态渲染**（`get_view`）：
   - 当 `node.data["collapsed"] == true` 时，高度 = `TITLE_H`（36px）
   - 4 个端口全部放在标题栏边缘，垂直堆叠：
     - `in`: 左边缘，Y = 12（上）
     - `loop_in`: 左边缘，Y = 24（下）
     - `done`: 右边缘，Y = 12（上）
     - `loop_body`: 右边缘，Y = 24（下）
   - "..." 胶囊在标题栏中部
   - toggle 按钮 + delete 按钮

2. **`content_size` 覆写**：
   ```rust
   fn content_size(&self, node: &Node) -> SizeF {
       let collapsed = node.data.get("collapsed").and_then(|v| v.as_bool()).unwrap_or(false);
       let h = if collapsed { TITLE_H } else { TITLE_H + BODY_H };
       SizeF::new(node.size.w, h)
   }
   ```

3. **`port_position` 覆写**：
   - 收起状态：端口垂直堆叠在标题栏两侧（参考设计文档 4.4 节）
   - 展开状态：保持现有逻辑

4. **回环边兼容**：Loop 节点收起后，`loop_body` 和 `loop_in` 端口位置变化，需验证 `loop_back_path` 和 `compute_loop_bounds` 仍能正确计算。`compute_loop_bounds` 基于 `node.bounds()`，收起后节点高度变小，回环边绕过范围会相应缩小，逻辑应自动适应。

***

### 阶段 5：属性面板编辑（CodeEditor + rhai）

#### 5.1 依赖添加

**文件**：[Cargo.toml](file:///d:/GitCode/RF/rust-agent-flow/crates/gpui/Cargo.toml)

```toml
[dependencies]
# ... 现有依赖 ...
rhai = "1"
```

#### 5.2 面板状态管理

**文件**：[flow_editor.rs](file:///d:/GitCode/RF/rust-agent-flow/crates/gpui/src/editor/flow_editor.rs)

**问题**：`PanelView` 是 `RenderOnce`（无状态），每次渲染重建。文本输入和代码编辑器需要持久状态。

**方案**：在 `FlowEditorView` 中存储面板输入状态：
```rust
pub struct FlowEditorView {
    // ... 现有字段 ...
    panel_input_cache: HashMap<NodeId, HashMap<String, String>>,
}
```

当 `NodeAction::SetData` 被触发时，同步更新 `panel_input_cache`。面板渲染时从 cache 读取初始值。

**备选方案**（推荐）：使用 `gpui-component` 的 `Entity<InputState>` 持久状态。在 `FlowEditorView` 中存储 `HashMap<NodeId, HashMap<String, Entity<InputState>>>`，面板渲染时复用已有 entity，避免重建丢失光标状态。

#### 5.3 Condition 节点属性面板

**文件**：[condition.rs](file:///d:/GitCode/RF/rust-agent-flow/crates/gpui/src/builtin/condition.rs) 的 `get_panel` 方法

**替换** `render_simple_panel`，实现可编辑面板（参考设计文档 5.3 节）：
- 节点名称：`Input` 组件（单行），值来自 `node.data["label"]`，编辑完成触发 `NodeAction::SetData("label", value)`
- 条件分支列表：每项一个 `Input`（rhai 语法高亮）+ 删除按钮
- 新增分支按钮：点击后通过 `NodeAction::SetData("conditions", new_conditions_json)` 添加
- Else 兜底：只读显示
- 收起/展开 toggle 按钮

#### 5.4 Loop 节点属性面板

**文件**：[loop_node.rs](file:///d:/GitCode/RF/rust-agent-flow/crates/gpui/src/builtin/loop_node.rs) 的 `get_panel` 方法

**替换** `render_simple_panel`（参考设计文档 5.4 节）：
- 节点名称：`Input` 组件
- 循环模式：RadioGroup 或 Select（4 种模式）
- 条件表达式：`Input`（rhai 语法高亮，多行），仅 while/for_loop 模式显示
- 收起/展开 toggle 按钮

#### 5.5 gpui-component 集成

**API 调研结果**：
- `Input` 组件位于 `gpui_component::input::Input`，需配合 `Entity<InputState>`
- `InputState` 提供 `new(cx)` 创建、`set_value(text, cx)` 设置值、`value()` 获取值
- 语法高亮：`highlighter` 模块支持 Rust/JS/Python，**不直接支持 rhai**。rhai 语法接近 Rust，使用 `Language::Rust` 作为高亮回退

**集成方式**：
1. 在 `FlowEditorView` 中维护 `HashMap<NodeId, HashMap<String, Entity<InputState>>>` 存储面板输入状态
2. `render_panel` 时，为每个字段获取或创建 `Entity<InputState>`
3. `Input::new(&state)` 创建组件，设置 `placeholder`、`mask` 等
4. 通过 `InputState` 的 `on_change` 回调监听输入变化，debounce 后触发 `NodeAction::SetData`

#### 5.6 rhai 语法支持

1. 添加 `rhai = "1"` 依赖
2. 条件表达式编辑器使用 `Language::Rust` 高亮（rhai 语法接近 Rust）
3. 可选：编辑完成时用 `rhai::AST::parse` 校验语法合法性，无效时显示错误提示
4. 本方案不包含 rhai 求值，仅编辑+校验

#### 5.7 简单节点属性面板增强

Action/Start/End 节点的属性面板增加节点名称编辑能力（`Input` 组件），保持与 Condition/Loop 一致的编辑体验。

***

### 阶段 6：生产级节点尺寸与视觉优化

**参考设计文档六章**

#### 6.1 尺寸调整

| 节点 | 现有尺寸 | 调整后 | 说明 |
| --- | --- | --- | --- |
| Start | 120×35 | 120×36 | 高度对齐 TITLE_H |
| End | 120×35 | 120×36 | 同上 |
| Action | 180×35 | 200×44 | 加宽加高，容纳 desc + 按钮 |
| Condition | 220×144(动态) | 240×(动态) | 加宽，容纳按钮 |
| Loop | 220×80 | 240×80 | 加宽，容纳按钮 |

**改动文件**：各节点的 `NodeSchema::new(...).with_size(...)` 调用

#### 6.2 标题栏布局规范

所有节点标题栏统一 36px 高度，右侧布局（从右到左）：
```
[合并端口/出端口] [delete×(hover)] [toggle▽(条件/循环)] ... [label] [in端口]
```

- delete 按钮：20×20，距右边缘 4px，距顶 4px
- toggle 按钮：20×20，在 delete 左侧 4px，垂直居中
- "..." 胶囊（收起时）：在 label 右侧，小圆角矩形（高 16px，宽 24px）

#### 6.3 文本溢出处理

节点 label 过长时截断显示省略号。最大显示宽度 = 节点宽度 - 按钮区域宽度 - padding。

#### 6.4 视觉细节

- 删除按钮 hover 时背景加深
- toggle 按钮 hover 时背景加深
- "..." 胶囊使用半透明背景
- 选中状态边框宽度增加到 2px

***

## 假设与决策

| 决策点 | 选择 | 理由 |
| --- | --- | --- |
| 收起范围 | 仅收起节点内部显示 | 用户确认，不破坏现有 dagre 布局 |
| 删除桥接策略 | 线性桥接 + 多端口直接删除 | 行业标准（n8n/ReactFlow），用户确认 |
| 收起状态存储 | `node.data["collapsed"]` | 随图持久化，简单直接 |
| 动作回调机制 | `Arc<dyn Fn + Send + Sync>` | 满足 GPUI `Send` 要求，低成本克隆 |
| 按钮交互 | 命中测试方案（非 GPUI listener） | 与现有架构一致，避免闭包捕获限制 |
| 面板状态管理 | `Entity<InputState>` 持久状态 | 解决 `RenderOnce` 无状态问题，保留光标位置 |
| rhai 高亮 | `Language::Rust` 回退 | gpui-component 不支持 rhai，rhai 语法接近 Rust |
| rhai 集成深度 | 语法编辑 + 校验，不求值 | 用户要求"rhai嵌入语法"，求值为后续扩展 |

## 风险与注意事项

1. **`Context::entity()` API**：需验证 GPUI 中获取 `Entity<Self>` 的确切方法。若不可用，改用 `WeakEntity` 存储方案。
2. **动态端口**：条件分支新增需要动态端口支持。当前 `NodeSchema` 端口是静态的，需扩展为基于 `node.data` 推导（在 `ConditionNode` 中覆写动态端口方法，命中测试和端口解析时优先使用）。
3. **排版稳定性**：收起/展开触发 `relayout()`，需确保 dagre 布局在节点尺寸变化后保持稳定，不产生跳跃。
4. **回环边兼容**：Loop 节点收起后，`loop_body` 和 `loop_in` 端口位置变化，需确保 `loop_back_path` 和 `compute_loop_bounds` 仍能正确计算。
5. **命中测试优先级**：按钮命中必须在节点主体命中之前检查，否则按钮区域会被节点主体"吞掉"。
6. **`InputState` 生命周期**：节点删除后面板输入状态需清理，避免内存泄漏。

## 验证步骤

1. **编译验证**：`cargo build` 全部通过，无警告
2. **删除功能**：创建 A→B→C 链，删除 B，验证 A→C 自动桥接，排版无断裂
3. **多端口删除**：删除条件/循环节点，验证所有关联边被清除，无残留连线
4. **hover 显示**：鼠标移入节点，删除按钮出现；移出，消失
5. **条件收起**：点击 toggle，条件行隐藏，出口合并到标题栏，整体重排
6. **条件展开**：再次点击 toggle，条件行恢复，出口回到各行，整体重排
7. **循环收起**：点击 toggle，循环体区域隐藏，端口堆叠到标题栏，回环边正确路由
8. **属性编辑**：选中节点，面板中编辑名称，验证节点标题实时更新
9. **分支编辑**：在面板中编辑条件表达式（rhai），验证节点显示更新
10. **新增分支**：点击"添加"，验证新分支行出现，新端口可连线
11. **排版质量**：收起/展开后，dagre 布局保持高质量分层，无重叠/跳跃
12. **主题切换**：亮/暗主题切换，所有新 UI 元素颜色正确

## 实现顺序

1. **阶段 1 补完**：hit_test.rs → interaction.rs → rendering.rs → common.rs（4 文件，基础设施闭环）
2. **阶段 2**：action.rs/start.rs/end.rs/condition.rs/loop_node.rs 删除按钮（5 文件）
3. **阶段 3**：condition.rs 收起/展开（1 文件大改）
4. **阶段 4**：loop_node.rs 收起/展开（1 文件大改）
5. **编译验证**：`cargo build` 确保阶段 1-4 编译通过
6. **阶段 5**：Cargo.toml + flow_editor.rs + condition.rs + loop_node.rs + action.rs/start.rs/end.rs 面板编辑（7 文件）
7. **阶段 6**：各节点尺寸调整 + 视觉打磨
8. **最终验证**：全部验证步骤通过
