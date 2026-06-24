# 修复开始节点属性面板无法操作 & 参数/变量 Tree 不显示

## 概述

Start 节点属性面板存在两个核心缺陷导致"无法操作"和"Tree 不显示"：
1. **Tree 不显示**：`render_section_tree` 中 Tree 控件未设置高度，gpui-component 的 Tree 需要显式高度才能渲染（渲染为 0px）。
2. **无法操作**：每次 `sync_from_node` 都无条件重建 Tree items（`set_items`），清空选中/展开状态。用户在浮层编辑面板中输入一个字符 → 触发 `SetData` → `sync_from_node` → 重建 Tree → 选中状态丢失 → 浮层关闭，形成"无法操作"的死循环。

修复遵循 UI/UX 设计原则：**系统状态可见性**（Tree 正确显示）、**用户控制与自由**（编辑不被状态重置打断）、**一致性**（与已工作的 per-item Tree 高度处理保持一致）。

## 当前状态分析

### 数据流（导致"无法操作"的回环）
```
用户在浮层编辑 Input
  → InputEvent::Change
  → subscribe_item_inputs 回调
  → sync_list_to_node (mod.rs:669)
  → dispatch_set_data → NodeAction::SetData
  → FlowEditorView::handle_node_action → 更新 node.data
  → 下一帧 render → ensure_panel_view (panel.rs:99-103)
  → sync_from_node (mod.rs:211)
  → rebuild_params_tree / rebuild_variables_tree (mod.rs:237-238)  ← 无条件重建
  → TreeState::set_items → 清空选中/展开状态
  → 浮层详细编辑面板关闭（依赖 self.selected）
```

### Tree 高度对比（导致"不显示"）
- **已工作的 per-item Tree**（`item.rs:758-827`）：显式计算 `tree_height = px(TREE_ENTRY_HEIGHT * entry_count)` 并 `.h(tree_height)`。
- **不工作的 section Tree**（`mod.rs:798-892`）：`tree_el` 无任何高度设置，外层 div 也无高度 → 渲染为 0px。

### 关键文件
- `crates/gpui/src/panel/start/mod.rs`：面板主体、`sync_from_node`、`render_section_tree`、`rebuild_params_tree`/`rebuild_variables_tree`
- `crates/gpui/src/panel/start/item.rs`：`TREE_ENTRY_HEIGHT = 34.0`（第 45 行）、per-item Tree 高度计算参考（第 758-827 行）
- `crates/gpui/src/editor/rendering/panel.rs`：`ensure_panel_view` 调用 `sync_from_node`

## 拟议变更

### 变更 1：为 section Tree 设置动态高度（修复"不显示"）

**文件**：`crates/gpui/src/panel/start/mod.rs`
**位置**：`render_section_tree` 函数（第 777-893 行）

**做什么**：
1. 在 `render_section_tree` 中读取 Tree 的可见 entry 数量（顶层项数 + 展开的子字段数），计算动态高度。
2. 由于 `render_section_tree` 当前不接收 states 切片，需通过 `tree_state.read(cx)` 获取已设置的 items 数量，或新增参数传入 `states: &[ItemState]` 以计算展开后的 entry 总数。
3. 对 `tree_el` 应用 `.h(tree_height)`，并设最小高度（如单行高度）保证空列表时容器仍可见（显示边框 + 可点添加按钮）。

**为什么**：gpui-component 的 Tree 控件需要显式高度才能渲染内容，这是"Tree 不显示"的直接原因。

**怎么做**：
- 方案：为 `render_section_tree` 新增 `states: &[ItemState]` 参数（调用处 `render` 第 700-725 行已有 `self.params_state`/`self.variables_state`）。
- 计算 entry 总数：`顶层项数 + sum(展开项的 fields.len())`。
- 高度：`px(TREE_ENTRY_HEIGHT * entry_count as f32)`，最小 `px(TREE_ENTRY_HEIGHT)`（空列表占位）。
- 将 `item.rs` 的 `TREE_ENTRY_HEIGHT` 常量提升到模块级或 `pub(crate)` 以便 `mod.rs` 复用，避免重复定义。
- 在 `tree_el` 链上追加 `.h(tree_height)`。

### 变更 2：停止无条件重建 Tree，改为按需同步（修复"无法操作"）

**文件**：`crates/gpui/src/panel/start/mod.rs`
**位置**：`sync_from_node`（第 211-240 行）、`rebuild_params_tree`/`rebuild_variables_tree`（第 410-424 行）

**做什么**：
1. 在 `sync_from_node` 中，记录重建前的 items 数量（`old_count`）。
2. 仅当列表项数量发生变化时才调用 `rebuild_params_tree`/`rebuild_variables_tree`（即 `set_items`）；数量不变时跳过重建，保留 Tree 的选中/展开状态。
3. `sync_list` 已经在数量一致时逐项同步 `ItemState` 的值（第 258-262 行），Tree 的 label 文本由 render 闭包实时从 `ItemState` 读取，因此数量不变时无需重建 Tree items 即可反映最新值。

**为什么**：`TreeState::set_items` 会清空选中状态和展开状态。用户编辑浮层 Input 时数量不变，但当前代码无条件重建导致选中丢失、浮层关闭，形成"无法操作"。

**怎么做**：
- 在 `sync_from_node` 中，`sync_list` 调用前后比较 `params_state.len()` / `variables_state.len()`：
  ```rust
  let old_params_count = self.params_state.len();
  let old_vars_count = self.variables_state.len();
  // ... sync_list 调用 ...
  if self.params_state.len() != old_params_count {
      self.rebuild_params_tree(cx);
  }
  if self.variables_state.len() != old_vars_count {
      self.rebuild_variables_tree(cx);
  }
  ```
- 注意：`sync_list` 在数量变化时会 clear + 重建 states（第 263-274 行），此时 `states.len()` 反映新数量，与 `old_count` 不同 → 触发 Tree 重建，正确。
- 数量不变时：`sync_list` 逐项同步值（第 258-262 行），Tree items 的 id 结构不变，选中/展开状态保留，render 闭包读取最新 `ItemState` 渲染最新文本，正确。

### 变更 3：移除 render 闭包内的 `entity.update`（修复潜在重入/卡顿）

**文件**：`crates/gpui/src/panel/start/mod.rs`
**位置**：`render_section_tree` 的 tree render_item 闭包（第 800-855 行）

**做什么**：
1. 在进入 tree render_item 闭包前，将所需的 `params_state`/`variables_state` 数据克隆到一个 `Vec`（包含渲染行所需的不可变快照：item_idx、field_idx、name、type、value、is_optional、is_array 等）。
2. 闭包内直接从快照读取数据渲染，不再调用 `entity.update`。
3. 由于 `ItemState` 含 `Entity<InputState>`（不可简单 Clone），需提取渲染所需的纯数据到一个小结构体 `RowSnapshot`，在 render 前构建。

**为什么**：在 GPUI 渲染闭包内调用 `entity.update` 是反模式，可能触发 observer 回调（`cx.observe(&params_tree, ...)`）导致重入，引发 panic 或无限渲染循环。这也是"无法操作"的潜在加重因素。

**怎么做**：
- 定义轻量快照结构（可放在 `mod.rs` 内）：
  ```rust
  struct RowSnapshot {
      item_idx: usize,
      field_idx: Option<usize>,
      name: String,
      type_str: String,
      value: String,
      is_optional: bool,
      is_array: bool,
      is_variable: bool,
  }
  ```
- 在 `render_section_tree` 调用前（或函数内、tree 构造前），遍历 `states` 构建 `Vec<RowSnapshot>`（包含顶层项 + 展开项的子字段），按 Tree 的扁平顺序排列。
- tree render_item 闭包改为 `move |ix, entry, selected, _window, _cx| { ... }`，从 `Vec<RowSnapshot>` 按 `parse_tree_item_id(&entry.item().id)` 索引读取，调用纯渲染函数（不依赖 `cx`/`entity`）。
- 渲染行函数（`render_tree_item_row`/`render_tree_field_row`）若当前依赖 `cx` 读取 `InputState`，需改为接收 `RowSnapshot` 参数。注意：这些行目前是"紧凑摘要"只读展示（实际编辑在浮层面板），因此用快照渲染是安全的。

> **注意**：此项改动较大。若闭包内 `entity.update` 当前未引发实际崩溃，可作为 P1 在变更 1、2 之后再做。但鉴于用户要求遵循 UI/UX 原则（稳定性），建议一并修复。实施时优先保证变更 1、2 生效，再验证是否需要变更 3。

## 假设与决策

1. **假设**：gpui-component 的 `Tree` 控件确实需要显式高度（依据：`item.rs` 中已工作的 per-item Tree 显式设置 `.h(tree_height)`，而 section Tree 未设置且不显示）。
2. **假设**：`TreeState::set_items` 会重置选中/展开状态（依据：gpui-component 通用行为 + 当前"编辑即关闭浮层"的现象）。
3. **决策**：Tree 行的紧凑摘要为只读展示（实际编辑在浮层详细面板），因此用 `RowSnapshot` 快照渲染不会丢失交互能力。交互（选中、展开、添加、删除）仍通过 Tree 控件自身事件 + `entity` 方法处理，不在 render 闭包内。
4. **决策**：不修改 `StartNode` schema（`builtin/start.rs`）——Start 节点使用独立的 `StartPanelView` 而非通用 `PanelView`，schema 不匹配是潜在问题但非本次 bug 原因，避免扩大范围。
5. **决策**：不清理 `section.rs` 死代码和 `item.rs::render_item` 死代码——属于独立重构任务，不在本次修复范围，避免引入风险。

## 验证步骤

1. **编译验证**：`cargo build` 通过，无新增 warning。
2. **Tree 显示验证**：运行 demo，选中 Start 节点，确认参数区和变量区 Tree 可见，显示 3 个参数和 4 个变量（含 context 的展开箭头）。
3. **操作验证（核心）**：
   - 点击 Tree 中某参数项 → 浮层详细编辑面板弹出。
   - 在浮层中修改 name/value → 输入流畅，浮层不关闭，Tree 摘要实时更新。
   - 切换 type/is_optional/is_array → 浮层保持，Tree 摘要更新。
4. **展开验证**：点击 context 项的展开箭头 → 子字段（topic、priority）显示；再次点击收起。展开状态在编辑其他项时保持。
5. **增删验证**：点击"添加参数"→ Tree 新增一行；选中某项删除 → Tree 移除该行；数量变化时 Tree 正确重建。
6. **切换节点验证**：选中其他节点再切回 Start 节点 → 面板正确重建，Tree 显示最新数据。
7. **空列表验证**：删除所有参数 → 参数区 Tree 容器仍显示边框和"添加参数"按钮（最小高度占位）。
