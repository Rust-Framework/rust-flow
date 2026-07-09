# Loop 区域占位与连线避障协调计划（Part B）

## 背景

Part A（循环体端口方向修复）已完成：body 节点通过 `body_nodes` 集合获得 `LayoutDirection::Vertical` 有效布局上下文，端口方向与纵向子流排布一致。

Part B 解决用户提出的第二个问题：**Loop 节点的区域占位问题，配合连线避障算法的设计**。

## 现状分析

### Loop 区域的三层空间模型

Loop 节点在画布上占用三层空间：

| 层 | 空间 | 当前负责函数 | 问题 |
|---|------|------------|------|
| L1 | Loop 节点自身 | dagre 布局 | 无（dagre 正确放置） |
| L2 | Body Group 区域（Loop 右侧纵向堆叠的 body 节点） | `align_loop_body_target`（步骤6） | **无碰撞检测**：dagre 放置的其他节点可能与 body 组矩形重叠 |
| L3 | Back-edge 走廊（body 组下方的 U 形回环路由空间） | `reserve_loop_back_edge_space`（步骤3） | **Y 位移过度**：不检查 X 范围，远离走廊的节点被误下移；**位置不准**：在 body 摆位前计算 |

### 管线顺序问题（核心缺陷）

当前 7 步后处理管线（[mod.rs:166-172](file:///d:/GitCode/RF/rust-agent-flow/crates/core/src/layout/dagre/mod.rs#L166-L172)）：

```
1. reorder_branch_targets
2. align_linear_chain
3. reserve_loop_back_edge_space     ← 在 body 摆位前！用 dagre 原始位置算 group_bottom
4. align_loop_in_sources            ← 移动 Loop
5. align_loop_done_target           ← 对齐 done 目标
6. align_loop_body_target           ← 摆位 body 到 Loop 右侧（此时才最终定位）
7. align_post_done_chain
```

**步骤3在步骤6之前**意味着 `reserve_loop_back_edge_space` 用 dagre 原始位置计算 `body_bottom`，而 body 节点的最终位置由步骤6决定。两者可能不同（dagre 可能把 body 放在 Loop 下方而非右侧），导致回环走廊预留位置不准。

### `reserve_loop_back_edge_space` 的 X 范围缺陷

当前过滤条件（[loop_layout.rs:90-99](file:///d:/GitCode/RF/rust-agent-flow/crates/core/src/layout/dagre/loop_layout.rs#L90-L99)）：

```rust
.filter(|(nid, pos)| {
    !body_nodes.contains(nid)
        && **nid != *loop_node
        && Some(**nid) != done_target
        && pos.y > group_bottom   // ← 只检查 Y，不检查 X
})
```

回环边水平段（`loop_back_path` 的 `bottom_y` 水平段）仅在 X 范围 `[approach_x, src.x]` ≈ `[loop.x - 30, body_center_x]` 内路由。X 远离此范围的节点不需要下移，但当前被误下移。

### `RectF` 缺少 `intersects` 方法

[geometry/mod.rs](file:///d:/GitCode/RF/rust-agent-flow/crates/core/src/geometry/mod.rs) 的 `RectF` 有 `union`/`contains`/`expand` 但无 `intersects`。碰撞检测需要矩形重叠判断。

## 设计决策

### 决策1：重排管线——`reserve_loop_back_edge_space` 移到 body 摆位之后

**新管线顺序**（8 步）：

```
1. reorder_branch_targets
2. align_linear_chain
3. align_loop_in_sources            ← 原步骤4（必须在 body 摆位前，因它移动 Loop）
4. align_loop_done_target           ← 原步骤5
5. align_loop_body_target           ← 原步骤6（body 最终定位）
6. avoid_body_group_collision       ← 新增：检测并解决 body 组碰撞
7. reserve_loop_back_edge_space     ← 原步骤3，移到此处（用最终 body 位置算走廊）
8. align_post_done_chain            ← 原步骤7
```

**理由**：
- 步骤5摆位 body 后，步骤6/7 都用最终 body 位置计算，走廊和碰撞检测准确
- `done_target` 已被 `reserve_loop_back_edge_space` 排除（line 95），步骤4的对齐不受影响
- 步骤8在最后运行，可修正 post-done 链的轴向对齐
- post-done 节点通常在 body 组右侧（rank+2 以远），X 范围不在回环走廊内，不会被步骤7误移

**安全性**：
- `align_loop_in_sources` 只移动 Loop 的 cross-axis 位置，不影响 flow-axis（rank）→ body 摆位（步骤5）基于新 Loop 位置计算，正确
- `reserve_loop_back_edge_space` 排除 `done_target` → 步骤4的 done 对齐不被破坏
- `align_post_done_chain` 只调 cross-axis → 即使步骤7下移了某些节点，步骤8会重新对齐链路

### 决策2：`avoid_body_group_collision` 的位移策略

碰撞节点向**右**移动到 `body_group_right + LOOP_BODY_GAP`。

**理由**：
- body 组在 Loop 右侧，向右移保持与 Loop 的距离关系
- 与 `align_loop_body_target` 的 `body_x = loop_pos.x + loop_node.size.w + LOOP_BODY_GAP` 一致
- 不向下移（避免与回环走廊冲突）

### 决策3：回环走廊 X 范围

用 body 组的 X 范围 `[loop_left, body_right]` 作为走廊 X 范围（保守估计，比实际 `[approach_x, src.x]` 略宽）。

## 实施步骤

### B1：为 `RectF` 新增 `intersects` 方法

**文件**：`crates/core/src/geometry/mod.rs`

在 `RectF` impl 中（`union` 方法之后）新增：

```rust
/// 两个轴对齐矩形是否重叠（边相切不算重叠）。
pub fn intersects(self, other: Self) -> bool {
    self.left() < other.right()
        && self.right() > other.left()
        && self.top() < other.bottom()
        && self.bottom() > other.top()
}
```

在 `tests` 模块中新增测试 `rect_intersects`：覆盖重叠、相切（不重叠）、包含、分离四种情况。

### B2：新增 `avoid_body_group_collision` 函数

**文件**：`crates/core/src/layout/dagre/loop_layout.rs`

在 `align_loop_body_target` 之后新增函数：

```rust
/// 检测并解决非 body/非 Loop 节点与 body 组包围盒的重叠。
///
/// 在 `align_loop_body_target` 摆位 body 组之后运行。遍历所有非 body、
/// 非 Loop、非 done-target 节点，检测其 bounds 是否与 body 组包围盒
/// （Loop + 所有 body 节点的 union）重叠。重叠节点右移到
/// `body_group_right + LOOP_BODY_GAP`。
///
/// **位移策略**：仅向右移，避免与回环走廊（下方）冲突。cross-axis（Y）
/// 不变，保持 dagre 的层级排布。
pub(super) fn avoid_body_group_collision(
    graph: &FlowGraph,
    positions: &mut std::collections::HashMap<crate::graph::NodeId, PointF>,
    _direction: LayoutDirection,
    loop_groups: &std::collections::HashMap<crate::graph::NodeId, std::collections::HashSet<crate::graph::NodeId>>,
)
```

**算法**：
1. 对每个 `(loop_node, body_nodes)`：
   a. 计算 body 组包围盒 = `union(Loop.bounds, 所有 body_node.bounds)`
   b. 找到 done target（排除）
   c. 遍历 positions 中非 body、非 Loop、非 done-target 的节点
   d. 若 `node.bounds.intersects(body_group_bounds)`：
      - `pos.x = body_group_right + LOOP_BODY_GAP`
      - `pos.y` 不变

### B3：改进 `reserve_loop_back_edge_space` 的 X 范围过滤

**文件**：`crates/core/src/layout/dagre/loop_layout.rs`

在 `reserve_loop_back_edge_space` 的 `nodes_to_shift` 过滤中增加 X 范围检查：

```rust
// 计算回环走廊 X 范围 = [loop_left, body_group_right]
let loop_pos = positions.get(loop_node)?;
let loop_node_obj = graph.node(*loop_node)?;
let corridor_left = loop_pos.x;
let corridor_right = body_nodes.iter()
    .filter_map(|nid| {
        let pos = positions.get(nid)?;
        let node = graph.node(*nid)?;
        Some(pos.x + node.size.w)
    })
    .fold(loop_pos.x + loop_node_obj.size.w, f32::max);

// 过滤：仅位移 X 在走廊范围内且 Y 在 group_bottom 之下的节点
.filter(|(nid, pos)| {
    let node = graph.node(**nid);
    let node_right = match node { Some(n) => pos.x + n.size.w, None => return false };
    let node_left = pos.x;
    !body_nodes.contains(nid)
        && **nid != *loop_node
        && Some(**nid) != done_target
        && pos.y > group_bottom
        && node_right > corridor_left      // X 范围检查
        && node_left < corridor_right       // X 范围检查
})
```

### B4：重排管线 + 插入新步骤

**文件**：`crates/core/src/layout/dagre/mod.rs`

**导入变更**（line 32-35）：
```rust
use loop_layout::{
    align_loop_body_target, align_loop_done_target, align_loop_in_sources,
    align_post_done_chain, avoid_body_group_collision, reserve_loop_back_edge_space,
};
```

**管线重排**（line 166-172）：
```rust
reorder_branch_targets(graph, &mut positions, direction);
align_linear_chain(graph, &mut positions, direction);
// ↓ 原步骤3移到步骤7（body 摆位后）
align_loop_in_sources(graph, &mut positions, direction);
align_loop_done_target(graph, &mut positions, direction);
align_loop_body_target(graph, &mut positions, direction, &loop_groups);
avoid_body_group_collision(graph, &mut positions, direction, &loop_groups);  // 新增
reserve_loop_back_edge_space(graph, &mut positions, direction, &loop_groups); // 移到此处
align_post_done_chain(graph, &mut positions, direction);
```

**更新管线注释**（line 152-162）：更新为 8 步说明。

### B5：更新文档

**文件**：`docs/rust-agent-flow/07-geometry-layout/dagre-layout.md`

1. 更新 mermaid 图：7 步 → 8 步，新增 `avoid_body_group_collision`，调整 `reserve_loop_back_edge_space` 位置
2. 更新步骤表格：新增步骤6 `avoid_body_group_collision`，步骤7 `reserve_loop_back_edge_space` 标注"移到 body 摆位后"
3. 新增"Loop 区域三层空间模型"章节，说明 L1/L2/L3 三层空间及各自负责函数

**文件**：`docs/rust-agent-flow/07-geometry-layout/port-calc.md`

无需修改（Part A 已更新）。

## 假设与边界

1. **post-done 节点不在回环走廊内**：done target 在 rank+2（body 组右侧以远），post-done 链更远。它们的 X > body_right，不会被 `reserve_loop_back_edge_space` 误移。若极端图结构导致 post-done 节点 X 落入走廊范围，步骤8（`align_post_done_chain`）会重新对齐 cross-axis，但 flow-axis（被下移的 Y）不会被修正——这是已知边界，当前 demo 和典型流程图不触发。
2. **`avoid_body_group_collision` 不级联**：若右移节点后与更右侧节点碰撞，不做二次检测。demo 和典型流程图中 body 组右侧是 done target（已被排除），不触发级联。
3. **`loop-back.md` 文档严重过期**（描述的是旧3点路径+extend，非当前5点U形+node_bounds），但重写是独立任务，不纳入本计划。

## 验证

1. `cargo test -p rust-agent-flow-core` — 验证 `RectF::intersects` 测试 + 现有布局测试通过
2. `cargo test -p rust-agent-flow-gpui` — 验证渲染层测试通过
3. `cargo build` — 编译通过
4. `cargo run -p rust-agent-flow-demo` — 视觉检查：
   - Loop 循环体 Process 节点端口为上(进)/下(出)（Part A 修复）
   - 回环边从 Process 底部出，向下绕过 body 组，左进 Loop 的 loop_in
   - 无节点与 body 组重叠
   - 回环走廊下方无无关节点穿越
