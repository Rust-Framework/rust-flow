# 边路由算法

ReactFlow 的纯几何路径算法（`smoothstep_path`/`bezier_path`/`step_path`/`straight_path`）仅基于源/目标端口坐标和方向计算路径，**零障碍感知**——当画布上其他节点落在源到目标的直线/折线轨迹上时，连线会直接穿过节点，造成视觉混乱。

本章介绍 rust-agent-flow 的**混合边路由策略**：以 ReactFlow `smoothstep_path` 为默认（整齐 + 进出段垂直于节点面 + 箭头与入面垂直），仅当几何路径穿过其他节点时，才用 Grid A* 搜索避障正交路径。这样大部分边保持视觉规范，少数穿节点的边走避障路径。

## 架构概述

```
relayout()
  ├── dagre 布局 + 9 步后处理（节点位置）
  └── route_all_edges()              ← 路由编排（混合策略）
        ├── 遍历所有非 LoopBack 边
        ├── 排除 src/dst 节点的障碍
        ├── smoothstep_path()        ← 第一步：ReactFlow 几何路径
        ├── Liang-Barsky 相交检测     ← 检测路径是否穿其他节点
        │   ├── 不穿 → 跳过（不写入缓存）→ 渲染层回退 Normal smoothstep
        │   └── 穿   → 第二步：A* 避障路由
        └── route_edge()             ← Grid A* 寻路（仅穿节点时）
              ├── 构建 OccupancyGrid
              ├── 标记障碍（节点 bounds 外扩 margin）
              ├── A* 4 方向寻路 + 方向约束
              ├── 路径简化（移除共线点）
              └── 返回 waypoints（逻辑坐标）
  → cached_edge_routes: HashMap<EdgeId, Vec<PointF>>（仅含需避障的边）

render_edges()
  ├── 缓存命中 → EdgeRender::Routed（A* 避障 waypoints）
  └── 未命中   → EdgeRender::Normal（smoothstep 整齐 + 垂直进出）
```

**核心思想**：整齐优先，避障按需。`smoothstep_path` 的进出段沿端口 side 方向延伸 20px（垂直于节点面），圆角 12px，箭头自然与入面垂直——这是默认视觉规范。只有当这条规范路径真的穿过其他节点时，才改走 A* 避障。路由在 `relayout` 时一次性计算并缓存，渲染与命中测试复用同一份数据。拖拽/平移不触发 relayout → 复用缓存，避免每帧 A* 搜索。

## Grid A* 算法

### 占用网格（OccupancyGrid）

将画布离散化为规则网格，每个格子标记为「自由」或「障碍」：

| 属性 | 值 | 说明 |
|------|------|------|
| `cell_size` | 10px | 格子边长，越小路径越精细但 A* 搜索空间越大 |
| `OBSTACLE_MARGIN` | 15px | 节点 bounds 外扩量，确保路径与节点保持视觉间距 |
| 网格范围 | 所有节点 bounds 的 union + 外扩 | 覆盖所有可能路径 |

[`OccupancyGrid`](file:///d:/GitCode/RF/rust-agent-flow/crates/core/src/geometry/routing/grid.rs) 的坐标转换：
- `to_grid(p)`：逻辑坐标 → 格子坐标，用 `floor` 使格子 `i` 覆盖半开区间 `[origin + i*cell, origin + (i+1)*cell)`
- `to_logical(x, y)`：格子坐标 → 逻辑坐标（格子中心）
- 两者互为近似逆运算（`to_grid(to_logical(i)) = i`）

### A* 寻路

[`find_path`](file:///d:/GitCode/RF/rust-agent-flow/crates/core/src/geometry/routing/astar.rs) 在占用网格上搜索从 `start` 到 `goal` 的最短正交路径：

- **4 方向移动**（上下左右，不斜行）→ 正交路径
- **启发式**：曼哈顿距离 `|dx| + |dy|`
- **拐弯惩罚**（`TURN_PENALTY = 2.0`）：每次方向变化增加额外代价，鼓励直线段，避免锯齿路径
- **优先队列**：`BinaryHeap` min-heap（按 `f_score = g + h` 排序）

### 方向约束

端口方向决定路径的出入方向：

| 约束 | 来源 | 作用 |
|------|------|------|
| `start_direction` | `Direction::from_side(src_side)` | 第一步必须沿源端口外向方向（如 `Right` → 第一步向右） |
| `goal_direction` | `Direction::inward(dst_side)` | 最后一步必须从目标端口的内向方向进入（如 `Left` → 从右方进入） |

`Direction::from_side` 返回外向方向（边离开端口的方向），`inward` 返回内向方向（边进入端口的方向，即外向的反向）。

**渐进松弛**：若方向约束导致寻路失败（如约束方向第一格被障碍占据），[`route_edge`](file:///d:/GitCode/RF/rust-agent-flow/crates/core/src/geometry/routing/mod.rs) 依次尝试：
1. 双约束（start + goal）
2. 仅 start 约束
3. 仅 goal 约束
4. 无约束

保证在约束过严时仍能找到路径。

### 路径简化

[`simplify_path`](file:///d:/GitCode/RF/rust-agent-flow/crates/core/src/geometry/routing/simplify.rs) 移除共线点，仅保留方向变化的拐点。A* 返回的原始路径包含每个访问的格子，简化后只保留起点、终点和拐点——这是渲染层需要的 minimal waypoints。

## 路由编排

[`route_all_edges`](file:///d:/GitCode/RF/rust-agent-flow/crates/gpui/src/editor/routing.rs) 在 `relayout` 末尾调用，遍历所有边按混合策略计算路由：

**跳过策略**：
- **LoopBack 边**（`target_port == "loop_in"`）：使用专用 `loop_back_path` 的 U 形绕行，A* 无法理解「必须绕过 Loop 组合边界」的语义
- **隐藏节点相关边**：收起的循环体节点已隐藏，连接到这些节点的边不渲染

**混合策略**（每条非 LoopBack 边）：
1. 用 `smoothstep_path(src, dst, src_side, dst_side, 12.0)` 计算 ReactFlow 几何路径（与渲染层 `paint_edge_scaled` 一致的圆角）
2. 用 Liang-Barsky 线段-矩形相交检测路径每一段是否穿过任何其他节点的**原始 bounds**（不外扩 margin，因为用户要的是「不穿节点」即可）
3. **不穿**：跳过该边（不写入缓存）→ 渲染层回退 `EdgeRender::Normal` → `smoothstep_path` 整齐 + 垂直进出 + 规范箭头
4. **穿**：调用 `route_edge` A* 避障路由，obstacles 外扩 `OBSTACLE_MARGIN` 保证路径离节点有间距

**障碍排除**：所有节点原始 bounds 作为碰撞检测候选，但排除当前边的 src/dst 节点自身（路径需要进出这些节点）。`route_edge` 内部还会在 src/dst 周围清除 4×cell_size 区域保证端口可达。

**loop 缝隙障碍**：每个 Loop 节点与其每个 body 节点之间的"连接走廊"作为额外障碍，避免 A* 从 Loop 与 body 节点之间的缝隙穿过（视觉上"穿 loop body"）。缝隙矩形按 Loop 与 body 的相对位置选主轴计算：水平为主时取 `Loop.right` 到 `body.left`（或反向），副轴覆盖两者 y 并集；垂直为主时取 `Loop.bottom` 到 `body.top`（或反向），副轴覆盖两者 x 并集。相比 union bounds（凸包围盒），缝隙矩形只堵缝不扩展成大矩形，不会误包紧邻的非 loop 节点。排除 src/dst 所在的 loop，保证 loop 内部边（loop_body/done）不被自身缝隙阻挡。

**缓存**：结果存入 `FlowEditorView.cached_edge_routes: HashMap<EdgeId, Vec<PointF>>`，**仅包含需避障的边**。不穿节点的边不写入缓存，渲染层走 `Normal` 分支。路由失败的边也不写入，同样回退 `Normal`。

### Liang-Barsky 线段-矩形相交检测

[`segment_intersects_rect`](file:///d:/GitCode/RF/rust-agent-flow/crates/gpui/src/editor/routing.rs) 用 Liang-Barsky 参数化裁剪算法判断线段 (a→b) 是否与轴对齐矩形相交：

- 将线段参数化为 `P(t) = a + t*(b-a), t ∈ [0,1]`
- 对矩形四条边界求 `t` 的进出区间 `[t0, t1]`
- `t0 <= t1` 且区间与 `[0,1]` 有交集 → 相交
- 端点落在矩形内也算相交（`rect.contains` 提前返回）

[`path_intersects_obstacles`](file:///d:/GitCode/RF/rust-agent-flow/crates/gpui/src/editor/routing.rs) 对折线每一段调用 `segment_intersects_rect`，任一段命中即返回 `true`。`smoothstep_path` 返回的折线含圆角采样点（每角 8 个采样），检测所有段确保圆角部分也避障。

## 渲染适配

### EdgeRender 三变体

[`EdgeRender`](file:///d:/GitCode/RF/rust-agent-flow/crates/gpui/src/editor/rendering/edge_geometry.rs) 枚举新增 `Routed` 变体：

| 变体 | 数据 | 绘制函数 | 适用场景 |
|------|------|----------|----------|
| `Normal` | src/dst + sides + edge_type | `paint_edge_scaled` | 路由失败回退 / ReactFlow 几何算法 |
| `LoopBack` | src/dst + node_bounds | `paint_loop_back_edge` | Loop 回环边（U 形虚线） |
| `Routed` | waypoints + edge_type | `paint_edge_routed` | 障碍感知路由（优先） |

### paint_edge_routed

[`paint_edge_routed`](file:///d:/GitCode/RF/rust-agent-flow/crates/gpui/src/edge/edge_view.rs) 对 A* 产生的 waypoints 按 `edge_type` 应用圆角后绘制折线 + 箭头：

| edge_type | 圆角处理 | 视觉效果 |
|-----------|----------|----------|
| `Bezier` | `round_corners(waypoints, 24.0)` | 更大圆角模拟平滑曲线 |
| `SmoothStep` | `round_corners(waypoints, 12.0)` | 与普通 SmoothStep 一致 |
| `Step` / `Straight` | 直接用 waypoints | 直角折线 / 简化后的直线 |

路由边始终用折线绘制（`round_corners` 已采样曲线为多点折线），`is_bezier=false` 确保 `paint_arrow` 用最后两点方向计算箭头。

### 优先级

`render_edges` 构建 `EdgeRender` 时：
1. 非 LoopBack 边优先查 `cached_edge_routes` → 命中则 `Routed`
2. LoopBack 边 → `LoopBack`
3. 未命中/路由失败 → `Normal`（回退）

## 命中测试适配

[`hit_test_edge_plus`](file:///d:/GitCode/RF/rust-agent-flow/crates/gpui/src/editor/hit_test.rs) 计算「+」按钮位置 = 端口位置 + 25px 沿端口 side 方向。路由后：
- `waypoints[0]` = src 端口位置（`route_edge` 保证）
- A* 的 `start_direction` 约束保证第一步沿 `src_side` 外向方向

因此按钮位置与路由路径首段方向一致，**命中测试无需修改**。

## 设计决策

| 决策 | 选择 | 理由 |
|------|------|------|
| **混合策略** | smoothstep 优先 + 碰撞检测 + 选择性 A* | 大部分边保持整齐 + 垂直进出 + 规范箭头，少数穿节点的边走避障 |
| **碰撞检测障碍** | 节点原始 bounds（不外扩 margin） | 用户要「不穿节点」即可；外扩 margin 会让擦边路径误判为穿节点，转走 A* 破坏整齐 |
| **A* 障碍** | 外扩 OBSTACLE_MARGIN | A* 路径需与节点保持视觉间距，避免贴边 |
| **loop 缝隙障碍** | 缝隙矩形（非 union bounds） | 堵 Loop 与 body 之间的缝，避免 A* 穿 loop body；缝隙矩形只堵缝不扩展成大矩形，不误包紧邻的非 loop 节点 |
| **smoothstep 圆角** | 12.0（与渲染层一致） | 检测路径与实际渲染路径吻合，避免「检测不穿但渲染穿」的偏差 |
| 网格分辨率 | 10px/cell | 2000×1500 画布 → 30K 格，A* 搜索空间小；10px 精度足以避免视觉碰撞 |
| 障碍外扩 | 15px | 确保路径与节点保持视觉间距，小于 nodesep(40px) 不过度限制空间 |
| 拐弯惩罚 | 2.0 | 平衡直线偏好与路径长度；太高（10+）绕远少拐弯，太低（0）锯齿 |
| Straight 边 | 路由 | 无障碍时简化为 2 点直线（语义保持），需绕障时变折线 |
| LoopBack 边 | 跳过路由 | 专用 U 形语义，A* 无法理解绕行约束 |
| 路由失败 | 回退几何路径 | A* 可能因网格过大/障碍包围失败，回退保证系统永不中断 |
| 缓存时机 | relayout 末尾 | 图结构变化才重新路由，拖拽/平移复用缓存 |

## 性能特征

- **混合策略开销**：大部分边仅做 smoothstep 几何计算 + Liang-Barsky 相交检测（O(段数 × 障碍数)，远轻于 A*），只有穿节点的少数边才走 A*
- **13 节点 15 边**：relayout 路由耗时 < 3ms（混合策略比全 A* 更快）
- **100+ 节点**：A* 搜索空间增大，可能达 50ms（建议场景节点数 < 50）
- **缓存命中**：拖拽/平移零路由开销，仅渲染复用 waypoints
- **回退开销**：未缓存边走 ReactFlow 几何算法，与改造前一致

## 模块结构

```
crates/core/src/geometry/routing/
├── mod.rs       — route_edge 入口 + 常量 + 渐进松弛编排
├── grid.rs      — OccupancyGrid 占用网格
├── astar.rs     — A* 寻路 + Direction 方向模型
└── simplify.rs  — 路径简化（移除共线点）

crates/gpui/src/editor/
└── routing.rs   — route_all_edges 路由编排（relayout 时调用）
```

`route_edge`、`GRID_CELL_SIZE`、`OBSTACLE_MARGIN`、`TURN_PENALTY` 通过 [`lib.rs`](file:///d:/GitCode/RF/rust-agent-flow/crates/core/src/lib.rs) re-export，gpui 层直接使用。

## 下一步

- [Dagre 布局引擎](dagre-layout.md) — 9 步后处理管线（含 `align_main_flow` 主流对齐）
- [端口端点计算](port-calc.md) — 端口 side 解析（路由的方向约束依赖此结果）
