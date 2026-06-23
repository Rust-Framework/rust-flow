# 修复连线「+」按钮位置、拖动性能、Check 分支排版

## 总结

修复三个问题：
1. 连线「+」按钮位置从连线中点改为靠近源节点出口（距离出口 10px）
2. 节点拖动卡顿严重，优化组件库性能
3. Demo 中 Check 节点条件分支目标节点排版不整齐（X 坐标不齐 + Y 坐标顺序/间距问题）

## 当前状态分析

### 问题1：连线「+」按钮位置

**当前实现**（`crates/gpui/src/editor/rendering.rs` 第 304-347 行 `render_edge_plus_buttons`）：
- 按钮位置 = 节点中心连线的中点
- 计算：`mid = (src_center + dst_center) / 2`，其中 `src_center`/`dst_center` 是节点矩形中心
- 屏幕坐标：`screen = viewport.offset + mid × scale`

**命中测试**（`crates/gpui/src/editor/hit_test.rs` 第 164-204 行 `hit_test_edge_plus`）：
- 使用相同的中点计算逻辑
- 命中半径 12px（逻辑坐标）

**问题**：按钮在连线中点，视觉上突兀，且多条边的中点可能重叠。

### 问题2：节点拖动卡顿

**当前实现**（`crates/gpui/src/editor/interaction.rs` 第 165-180 行）：
- 每次鼠标移动都调用 `cx.notify()` 触发完整重渲染
- `render()` 每帧都调用 `self.graph.loop_body_groups()`（BFS 遍历 O(V+E)）

**性能瓶颈**：
1. **`render_edge_plus_buttons` 每帧为每条边创建 div 元素**（rendering.rs 第 304-347 行）- 这是最昂贵的操作，每条边都创建一个带 Icon 的 div
2. **`loop_body_groups()` 每帧 BFS 遍历**（flow_editor.rs 第 463 行）- 拖动不改变图结构，但每帧都重新计算
3. **Idle 状态 hover 追踪**（interaction.rs 第 188-203 行）- 拖动时不会触发（拖动状态非 Idle），但平移时会
4. **全量重渲染** - 拖动单个节点时，所有节点 + 所有边 + 所有按钮都会重绘

**关键观察**：拖动时不需要显示「+」按钮（用户正在拖动节点，不会点击按钮），跳过 `render_edge_plus_buttons` 可大幅提升性能。

### 问题3：Check 节点分支目标排版不整齐

**Demo 图结构**（`crates/gpui/src/editor/data_source.rs` 第 90-148 行）：
- Condition 节点有 3 条出边：`if_0 → search`, `if_1 → notify`, `else → tool`
- search → adapter → loop_node（search 在 rank 4，adapter 在 rank 5）
- notify → loop_node, tool → loop_node（notify/tool 在 rank 4）
- 三个分支目标理论上应在同一 rank（rank 4），X 坐标应相同

**当前 `reorder_branch_targets` 实现**（`crates/core/src/layout/dagre.rs` 第 212-278 行）：
- 只重排 Y 坐标分配（按端口顺序 if_0 → if_1 → else）
- **不统一 X 坐标** - 如果 dagre 给出的 X 坐标不完全相同，分支目标不在同一列
- **不保证 Y 坐标均匀分布** - 只是重排现有 Y 坐标，不改变 Y 坐标值本身

**后续后处理步骤的影响**：
- `reserve_loop_back_edge_space`：只修改 Y 坐标，可能把部分分支目标下移（如果跨越 body_bottom），破坏 Y 坐标对齐
- 其他后处理步骤不影响分支目标的 X 坐标

## 提议的修改

### 修改1：连线「+」按钮位置改为靠近源节点出口

**文件**：`crates/gpui/src/editor/rendering.rs`

**修改函数**：`render_edge_plus_buttons`（第 278-355 行）

**当前逻辑**：
```rust
let src_center = PointF::new(
    src.position.x + src.size.w * 0.5,
    src.position.y + src.size.h * 0.5,
);
let dst_center = PointF::new(
    dst.position.x + dst.size.w * 0.5,
    dst.position.y + dst.size.h * 0.5,
);
let mid = PointF::new(
    (src_center.x + dst_center.x) * 0.5,
    (src_center.y + dst_center.y) * 0.5,
);
```

**新逻辑**：
- 使用 `compute_edge_endpoints`（已存在，第 72-118 行）获取源端口和目标端口的精确位置
- 计算从源端口到目标端口的方向向量
- 按钮位置 = 源端口位置 + 方向向量 × 10px（逻辑坐标）
- 这样每条边的按钮在该边的源端口附近，同一节点不同端口的按钮不会重叠

**为什么**：用户要求按钮靠近源节点出口，方便点击且视觉清晰。使用端口位置而非节点中心，可以正确处理多端口节点（如 Condition 的 if_0/if_1/else）。

**如何实现**：
1. 在 `render_edge_plus_buttons` 中，调用 `compute_edge_endpoints` 获取 `(src, _, dst, _)`
2. 计算方向向量 `dir = (dst - src).normalize()`（处理零向量情况）
3. 按钮逻辑位置 = `src + dir × 10.0`
4. 屏幕坐标 = `viewport.offset + button_pos × scale`
5. 需要传入 `registry`、`layout`、`body_groups` 等参数（与 `render_edges` 相同）

**文件**：`crates/gpui/src/editor/hit_test.rs`

**修改函数**：`hit_test_edge_plus`（第 164-204 行）

**同步修改**：命中测试必须使用与渲染相同的位置计算逻辑，否则点击按钮无法命中。

**如何实现**：
1. 在 `hit_test_edge_plus` 中，使用相同的端口位置计算逻辑
2. 需要访问 `self.registry`、`self.layout_direction`、`body_groups`
3. 由于 `hit_test` 是同步方法，需要预先计算 `body_groups` 或在方法内计算

**注意**：`hit_test_edge_plus` 当前不接收 `body_groups` 参数。由于命中测试在鼠标事件中调用，而 `body_groups` 计算成本较高，可以考虑：
- 方案A：在 `hit_test` 中调用 `self.graph.loop_body_groups()`（每次命中测试都计算）
- 方案B：缓存 `body_groups`（与问题2的优化一起做）

选择方案B：缓存 `body_groups`，与问题2的优化合并。

### 修改2：优化节点拖动性能

**文件**：`crates/gpui/src/editor/flow_editor.rs`

**修改1**：添加 `cached_body_groups` 字段

在 `FlowEditorView` 结构体中添加：
```rust
pub cached_body_groups: std::collections::HashMap<NodeId, std::collections::HashSet<NodeId>>,
```

**修改2**：在图结构变化时更新缓存

在以下方法中更新 `cached_body_groups`：
- `relayout`（第 125-140 行）：布局后更新
- `insert_node_at_edge`（第 259-296 行）：插入节点后更新
- `delete_node`（第 343-381 行）：删除节点后更新
- `handle_node_action`（第 299-336 行）：ToggleCollapse/SetData 后更新（通过 relayout）
- `set_data_source`（第 241-253 行）：切换数据源后更新

实际上，所有图结构变化都通过 `relayout()` 或直接修改 `self.graph`。最简单的方案是在 `relayout()` 末尾更新缓存，并在 `set_data_source` 中也更新（因为 `set_data_source` 调用 `relayout`，所以已覆盖）。

但是 `insert_node_at_edge` 和 `delete_node` 都调用 `relayout`，所以在 `relayout` 末尾更新缓存即可。

**修改3**：在 `render()` 中使用缓存

将 `render()` 第 463 行：
```rust
let body_groups = self.graph.loop_body_groups();
```
改为：
```rust
let body_groups = self.cached_body_groups.clone();
```

**为什么 clone**：`render()` 需要 `&self`，而更新缓存需要 `&mut self`。由于 `render` 是 `&mut self`（GPUI 的 Render trait），可以直接使用 `&self.cached_body_groups` 引用，无需 clone。

实际上，`render(&mut self, ...)` 是 `&mut self`，所以可以直接使用 `&self.cached_body_groups`。

**修改4**：拖动时跳过 `render_edge_plus_buttons`

在 `render()` 中（第 513 行），添加条件判断：
```rust
let is_dragging = matches!(self.interaction, InteractionState::DraggingNode { .. });
if !is_dragging {
    container = container.child(self.render_edge_plus_buttons(&body_groups));
}
```

**为什么**：拖动时用户不会点击「+」按钮，跳过渲染可避免每帧创建大量 div 元素，显著提升性能。

**修改5**：平移时也跳过 `render_edge_plus_buttons`（可选）

同样地，平移时也可以跳过「+」按钮渲染：
```rust
let is_interacting = matches!(
    self.interaction,
    InteractionState::DraggingNode { .. } | InteractionState::Panning { .. }
);
if !is_interacting {
    container = container.child(self.render_edge_plus_buttons(&body_groups));
}
```

**修改6**：初始化缓存

在 `FlowEditorView::new`（第 85-106 行）中初始化 `cached_body_groups` 为空 HashMap，并在构造后调用一次 `relayout` 或手动计算。

实际上，`new` 不调用 `relayout`，demo 中的 `auto_layout` 会调用。为了安全，在 `new` 中初始化为空，在 `relayout` 中更新。如果 `render` 在 `relayout` 之前被调用，`cached_body_groups` 为空，`render_edge_plus_buttons` 不会渲染任何按钮（因为 `hidden_nodes` 为空，所有边都渲染），这是可接受的。

### 修改3：修复 Check 节点分支目标排版

**文件**：`crates/core/src/layout/dagre.rs`

**修改函数**：`reorder_branch_targets`（第 212-278 行）

**当前问题**：
1. 只重排 Y 坐标分配，不统一 X 坐标
2. 不保证 Y 坐标均匀分布

**新逻辑**：
1. 按端口顺序排序目标节点（保持现有逻辑）
2. **统一 X 坐标**：计算所有分支目标 X 坐标的中位数，把所有分支目标的 X 坐标设置为该中位数
3. **均匀分布 Y 坐标**：计算 Y 坐标范围 [min, max]，按端口数量均匀分布

**为什么**：
- 统一 X 坐标确保分支目标在同一列对齐
- 均匀分布 Y 坐标确保视觉整齐，避免间距不均

**如何实现**：
```rust
// 现有逻辑：按端口顺序排序
targets.sort_by_key(|(port, _)| branch_port_order(port));

// 收集当前坐标
let current_coords: Vec<(f32, f32)> = targets
    .iter()
    .filter_map(|(_, nid)| positions.get(nid))
    .map(|p| match direction {
        LayoutDirection::Horizontal => (p.x, p.y),
        LayoutDirection::Vertical => (p.y, p.x),  // 注意：纵向布局主轴是 X
    })
    .collect();

if current_coords.len() != targets.len() {
    continue;
}

// 统一主轴坐标（横向布局是 X，纵向布局是 Y）
// 取中位数作为统一值
let mut main_coords: Vec<f32> = current_coords.iter().map(|(m, _)| *m).collect();
main_coords.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
let median_main = main_coords[main_coords.len() / 2];

// 均匀分布副轴坐标（横向布局是 Y，纵向布局是 X）
let mut cross_coords: Vec<f32> = current_coords.iter().map(|(_, c)| *c).collect();
cross_coords.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
let cross_min = cross_coords.first().copied().unwrap_or(0.0);
let cross_max = cross_coords.last().copied().unwrap_or(0.0);
let n = targets.len();
let cross_range = cross_max - cross_min;
let cross_step = if n > 1 { cross_range / (n - 1) as f32 } else { 0.0 };

// 分配：按端口顺序，副轴坐标从小到大
for (i, (_, nid)) in targets.iter().enumerate() {
    if let Some(pos) = positions.get_mut(nid) {
        let new_cross = cross_min + cross_step * i as f32;
        match direction {
            LayoutDirection::Horizontal => {
                pos.x = median_main;  // 统一 X
                pos.y = new_cross;    // 均匀 Y
            }
            LayoutDirection::Vertical => {
                pos.y = median_main;  // 统一 Y
                pos.x = new_cross;    // 均匀 X
            }
        }
    }
}
```

**注意**：
- 横向布局：主轴 = X（左右），副轴 = Y（上下）。分支目标应统一 X（同一列），Y 按端口顺序从上到下均匀分布。
- 纵向布局：主轴 = Y（上下），副轴 = X（左右）。分支目标应统一 Y（同一行），X 按端口顺序从左到右均匀分布。

**验证**：现有测试 `branch_targets_reordered_to_match_port_order`（第 715-752 行）应继续通过。可以添加新测试验证 X 坐标统一和 Y 坐标均匀分布。

## 假设与决策

### 假设
1. **按钮位置**：用户接受同一节点多条出边的按钮在各自源端口附近（不重叠，因为端口位置不同）
2. **性能优化**：拖动/平移时隐藏「+」按钮是可接受的（用户在交互结束后会重新显示）
3. **排版修复**：统一 X 坐标使用中位数，均匀分布 Y 坐标使用现有范围

### 决策
1. **按钮位置计算**：使用 `compute_edge_endpoints` 获取精确端口位置，而非节点中心
2. **性能优化策略**：缓存 `body_groups` + 拖动/平移时跳过按钮渲染（最小改动，最大收益）
3. **排版修复**：在 `reorder_branch_targets` 中统一主轴坐标 + 均匀分布副轴坐标

## 验证步骤

### 1. 编译验证
```bash
cargo build --workspace
```

### 2. 单元测试
```bash
cargo test --workspace
```
重点验证：
- `branch_targets_reordered_to_match_port_order` 测试继续通过
- 添加新测试验证 X 坐标统一和 Y 坐标均匀分布

### 3. 手动验证（运行 demo）
```bash
cargo run --package rust-agent-flow-demo
```

验证问题1：
- 连线「+」按钮出现在源节点出口附近（距离约 10px）
- 同一节点不同端口的按钮不重叠
- 点击按钮仍能弹出节点选择浮层

验证问题2：
- 拖动节点时流畅，无明显卡顿
- 拖动时「+」按钮隐藏，松开后重新显示
- 平移画布时同样流畅

验证问题3：
- Check 节点的三个分支目标（Search/Notify/ToolCall）在同一列（X 坐标对齐）
- Y 坐标按 if_0 → if_1 → else 顺序从上到下排列
- Y 坐标间距均匀

### 4. 回归验证
- 切换数据源（AgentFlow/DataPipeline/SimpleFlow）后排版正常
- 切换布局方向（横向/纵向）后排版正常
- 收起/展开 Condition 节点后排版正常
- 删除节点、插入节点后排版正常
