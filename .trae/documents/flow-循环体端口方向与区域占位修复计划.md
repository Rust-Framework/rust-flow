# 循环体端口方向修复 + Loop 区域占位设计计划

## Context

demo 中 Loop 循环体内的 Process 节点（action 类型）端口方向错误。根因是 Phase 2 从 `compute_edge_endpoints` 移除了 `body_nodes` 参数后，body 节点的 `PortSide::Auto` 端口按主布局方向（Horizontal）解析为 Left/Right，但 `align_loop_body_target` 将 body 节点纵向堆叠，期望 Top/Bottom 端口。这导致 `loop_body` 边进入 Process 的 Left 而非 Top，`loop_in` 回环边从 Process 的 Right 而非 Bottom 出发。

同时，loop 节点的区域占位机制存在缺陷：body 组覆写式定位无碰撞检测，回环边预留仅 Y 轴下方固定 100px，不处理 X 轴和回环走廊。需要分析并设计可扩展的区域占位方案。

---

## Part A：修复循环体节点端口方向

### 核心思路

Body 节点存在于"纵向子流"中（由 `align_loop_body_target` 的垂直堆叠决定）。修复方式是向 body 节点传递 `LayoutDirection::Vertical` 作为有效布局方向，让节点自身的 `port_position` 回调决定 Top/Bottom 端口。

**这不是 Phase 2 移除的"强制 Top/Bottom"逻辑**——旧逻辑覆写 `resolve_port` 的返回值（side），新逻辑只改变传入的 `layout` 参数。对 fixed 端口（Loop 的 loop_body/loop_in）无影响，因为回调忽略 layout；对 Auto 端口（Action 的 in/out）回调按 Vertical 返回 Top/Bottom。

### 步骤

#### A1. nodes.rs — body 节点传入 Vertical 布局

文件：`crates/gpui/src/editor/rendering/nodes.rs`

当前 `render_nodes` 在 line 30-33 取主布局方向，line 48 已计算 `is_body`，line 74 `.with_layout(layout)` 传入主布局。

修改：body 节点传入 `Vertical`：

```rust
let effective_layout = if is_body {
    rust_agent_flow::LayoutDirection::Vertical
} else {
    layout
};
// ...
.with_layout(effective_layout)
```

`with_body_mode(is_body)` 可保留（用于 fallback 渲染的 `vertical` 标志），但不再依赖它传播 layout。

#### A2. edge_geometry.rs — compute_edge_endpoints 接收 body_nodes

文件：`crates/gpui/src/editor/rendering/edge_geometry.rs`

重新引入 `body_nodes: &HashSet<NodeId>` 参数（Phase 2 移除的），但用途不同——不覆写 side，而只为 body 节点解析时传入 Vertical：

```rust
pub(crate) fn compute_edge_endpoints(
    edge: &Edge,
    graph: &FlowGraph,
    registry: &NodeRegistry,
    layout: LayoutDirection,
    default_src_side: PortSide,
    default_dst_side: PortSide,
    body_nodes: &HashSet<NodeId>,  // NEW
) -> (PointF, PortSide, PointF, PortSide) {
    let src_layout = if body_nodes.contains(&edge.source) {
        LayoutDirection::Vertical
    } else {
        layout
    };
    let dst_layout = if body_nodes.contains(&edge.target) {
        LayoutDirection::Vertical
    } else {
        layout
    };

    let (src, src_side) = match edge.source_port.as_deref() {
        Some(pid) => resolve_port(src_node, pid, registry, src_layout),
        None => { /* 浮动边逻辑不变 */ }
    };
    let (dst, dst_side) = match edge.target_port.as_deref() {
        Some(pid) => resolve_port(dst_node, pid, registry, dst_layout),
        None => { /* 浮动边逻辑不变 */ }
    };
    // ...
}
```

更新 doc 注释：移除"循环体节点的端口 side 不再被外部强制为 Top/Bottom"的错误描述，改为"body 节点使用 Vertical 布局上下文，由节点 port_position 回调决定 side"。

#### A3. edges.rs — 传递 cached_all_body_nodes

文件：`crates/gpui/src/editor/rendering/edges.rs`

`render_edges` 已有 `body_groups` 参数（line 33），但未传给 `compute_edge_endpoints`。改为传递 `self.cached_all_body_nodes`：

line 60-67 的 `compute_edge_endpoints` 调用增加 `&self.cached_all_body_nodes` 参数。

同时修复 line 106 绘制临时边的 `resolve_port` 调用——如果源节点是 body 节点，传入 Vertical：

```rust
let src_layout = if self.cached_all_body_nodes.contains(from_node) {
    LayoutDirection::Vertical
} else {
    layout
};
let (src, src_side) = resolve_port(n, from_port, &registry, src_layout);
```

清理 line 25 过时注释："循环体节点始终强制 Top/Bottom" → 改为"循环体节点使用 Vertical 布局上下文解析端口"。

#### A4. hit_test.rs — 端口命中 + 边命中使用有效布局

文件：`crates/gpui/src/editor/hit_test.rs`

**端口命中**（line 53）：`resolve_port` 调用增加 body 判断：

```rust
let node_layout = if self.cached_all_body_nodes.contains(&node.id) {
    LayoutDirection::Vertical
} else {
    layout
};
let (port_pos, _) = resolve_port(node, &port_spec.id, &self.registry, node_layout);
```

**边命中**（line 180）：`compute_edge_endpoints` 调用增加 `&self.cached_all_body_nodes` 参数。

#### A5. 更新过时注释和文档

- `edge_geometry.rs` line 62-63：移除错误描述
- `edges.rs` line 25：更新描述
- `docs/rust-agent-flow/07-geometry-layout/port-calc.md`：补充 body 节点 Vertical 上下文说明

### 验证

```bash
cargo test -p rust-agent-flow-core
cargo test -p rust-agent-flow-gpui
cargo run -p rust-agent-flow-demo
```

视觉验证：demo 中 Loop → Process 的 `loop_body` 边应从 Loop 右侧出，下拐进入 Process 顶部；`loop_in` 回环边应从 Process 底部出，向下绕回 Loop 左侧。

---

## Part B：Loop 区域占位分析与设计

### 现状分析

Loop 节点在画布上占据三类空间：

```
                ┌──────────────────────────────────────┐
                │           Body Group Area            │
                │  ┌────────┐                          │
                │  │ Body 1 │ (纵向堆叠)               │
                │  ├────────┤                          │
                │  │ Body 2 │                          │
                │  └──┬─────┘                          │
                │     │                                │
                ├─────┼────────────────────────────────┤  ← Back-edge Corridor
                │     ↓                                │     (回环边水平段)
                └─────┴────────────────────────────────┘
  Loop 节点 ←──┘    ↑ approach_x              src.x
```

| 区域 | 当前处理 | 问题 |
|------|----------|------|
| Loop 节点自身 | dagre 定位 | 无问题 |
| Body Group Area | `align_loop_body_target` 覆写定位 | **无碰撞检测**：直接覆写坐标，不检查与非 body 节点重叠 |
| Back-edge Corridor | `reserve_loop_back_edge_space` 下移 100px | **固定偏移**：不随 body 组几何自适应；**仅 Y 轴**：不处理走廊 X 范围外的节点 |
| Body 组右侧 | 无处理 | dagre 可能在 body 组右侧放置节点，导致重叠 |

### 缺陷影响

1. **Body 组与非 body 节点重叠**：`align_loop_body_target` 将 body 节点放到 `loop_pos.x + loop_node.size.w + LOOP_BODY_GAP` 处，但 dagre 可能在该位置放了其他节点（如 done 目标 Summarize），导致视觉重叠
2. **回环边穿过节点**：回环边水平段在 `bottom_y` 处从 `src.x` 到 `approach_x`，如果该 Y 坐标有其他节点，边会穿过节点
3. **固定 100px 不够灵活**：`BACK_EDGE_RESERVE = 100` 由 `bottom_margin(40) + approach_offset(30) + clearance(30)` 派生，但实际需求取决于 body 组宽度和回环边路径

### 设计方向（本期实现范围）

#### B1. Body 组碰撞避让（新增步骤）

在 `align_loop_body_target` **之后**新增 `avoid_body_group_collision`：

```
pipeline 顺序：
6. align_loop_body_target   ← body 组定位（覆写式）
6.5 avoid_body_group_collision  ← NEW：检测 body 组矩形与非 body 节点的重叠，右推冲突节点
7. align_post_done_chain
```

算法：
1. 计算 body 组包围盒（body_nodes 的 union bounds）
2. 遍历非 body、非 Loop 节点，检测是否与 body 组包围盒重叠
3. 重叠节点右移到 body 组右边缘 + gap

```rust
pub(super) fn avoid_body_group_collision(
    graph: &FlowGraph,
    positions: &mut HashMap<NodeId, PointF>,
    loop_groups: &HashMap<NodeId, HashSet<NodeId>>,
) {
    for (loop_node, body_nodes) in loop_groups {
        // 计算 body 组包围盒
        let body_bounds = body_bounds(graph, positions, body_nodes);
        // 检测非 body 节点重叠，右推
        for node in graph.nodes() {
            if body_nodes.contains(&node.id) || node.id == *loop_node { continue; }
            let pos = positions.get(&node.id).unwrap();
            let node_rect = RectF::new(pos, node.size);
            if body_bounds.intersects(node_rect) {
                // 右推到 body 组右边缘 + gap
                positions.get_mut(&node.id).unwrap().x =
                    body_bounds.right() + LOOP_BODY_GAP;
            }
        }
    }
}
```

#### B2. 自适应回环边预留

将 `reserve_loop_back_edge_space` 的固定 `BACK_EDGE_RESERVE` 改为基于 body 组几何计算：

```rust
// 回环边走廊高度 = bottom_margin + 走廊水平段高度（箭头+圆角）+ 安全间距
let corridor_height = 40.0 + 30.0 + 30.0; // 仍为 100，但语义明确
// 可选：根据 body 组宽度调整（宽 body 组需要更长的水平段，但不影响高度）
```

实际改动较小——当前 100px 已基本合理。主要改进是**限定预留范围**：

```rust
// 当前：pos.y > group_bottom 的所有节点下移
// 改进：仅下移 X 在 [loop_left, body_right] 范围内的节点
let loop_left = loop_pos.x;
let body_right = body_bounds.right();
// 节点需同时满足 Y > group_bottom 且 X 在走廊范围内
.filter(|(_, pos)| {
    pos.y > group_bottom
    && pos.x >= loop_left - margin
    && pos.x <= body_right + margin
})
```

#### B3. 回环边走廊节点避让（与 B2 协同）

B2 限定了预留范围，但回环边走廊可能仍有未被下移的节点（在走廊 Y 范围但 X 在范围外）。本期不处理——这些节点不在回环边路径上，不影响视觉。未来通用避障可处理。

### 未来扩展（不在本期实现）

- **通用边避障框架**：定义 `Obstacle` trait（节点矩形 + 不可穿过区域），边路径算法（smoothstep/bezier）接收障碍列表，路由时绕行
- **Loop 区域作为 Obstacle**：body 组包围盒 + 回环走廊注册为 Obstacle，主流边自动绕行
- **A* 路由**：复杂场景下用 A* 在网格上搜索避障路径

### 步骤

#### B1. 实现 avoid_body_group_collision

文件：`crates/core/src/layout/dagre/loop_layout.rs`

新增函数 `avoid_body_group_collision`，在 pipeline 中 `align_loop_body_target` 之后调用。

文件：`crates/core/src/layout/dagre/mod.rs`

pipeline 中插入新步骤（line 171 之后）。

#### B2. 改进 reserve_loop_back_edge_space

文件：`crates/core/src/layout/dagre/loop_layout.rs`

修改 `reserve_loop_back_edge_space` 的过滤条件，增加 X 范围限定。计算 body 组右边缘用于范围判断。

#### B3. 更新文档

- `docs/rust-agent-flow/07-geometry-layout/` 相关文档补充区域占位说明

### 验证

```bash
cargo test -p rust-agent-flow-core
cargo run -p rust-agent-flow-demo
```

视觉验证：
- demo 中 Process（body 节点）不与 Summarize/End 重叠
- 回环边不穿过任何节点
- 切换纵向布局时 body 组仍正确定位

---

## 修改文件清单

| 文件 | Part | 改动 |
|------|------|------|
| `crates/gpui/src/editor/rendering/nodes.rs` | A1 | body 节点传入 Vertical 布局 |
| `crates/gpui/src/editor/rendering/edge_geometry.rs` | A2 | `compute_edge_endpoints` 接收 body_nodes，按 body 判断传入 Vertical |
| `crates/gpui/src/editor/rendering/edges.rs` | A3 | 传递 `cached_all_body_nodes`；修复绘制临时边的 layout；清理注释 |
| `crates/gpui/src/editor/hit_test.rs` | A4 | 端口命中 + 边命中使用有效布局 |
| `crates/core/src/layout/dagre/loop_layout.rs` | B1, B2 | 新增 `avoid_body_group_collision`；改进 `reserve_loop_back_edge_space` X 范围限定 |
| `crates/core/src/layout/dagre/mod.rs` | B1 | pipeline 插入 `avoid_body_group_collision` |
| `docs/rust-agent-flow/07-geometry-layout/port-calc.md` | A5 | 补充 body 节点 Vertical 上下文说明 |
