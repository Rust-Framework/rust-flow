# 障碍感知边路由 — Phase 3-6 续作计划

## 摘要

本计划是 [障碍感知边路由架构改造计划.md](file:///d:/GitCode/RF/rust-agent-flow/.trae/documents/障碍感知边路由架构改造计划.md) 的续作，聚焦于**剩余未完成**的 Phase 3-6。

**已完成**（Phase 1-2，代码已落地）：
- Phase 1：`align_main_flow`（[main_flow.rs](file:///d:/GitCode/RF/rust-agent-flow/crates/core/src/layout/dagre/main_flow.rs)）— 3 单元测试通过，已集成到管线第 3 步
- Phase 2：`routing/` 模块 4 个文件（[grid.rs](file:///d:/GitCode/RF/rust-agent-flow/crates/core/src/geometry/routing/grid.rs)、[astar.rs](file:///d:/GitCode/RF/rust-agent-flow/crates/core/src/geometry/routing/astar.rs)、[simplify.rs](file:///d:/GitCode/RF/rust-agent-flow/crates/core/src/geometry/routing/simplify.rs)、[mod.rs](file:///d:/GitCode/RF/rust-agent-flow/crates/core/src/geometry/routing/mod.rs)）— `route_edge` 入口 + 常量已 re-export（[lib.rs:33](file:///d:/GitCode/RF/rust-agent-flow/crates/core/src/lib.rs#L33)）
- `simplify.rs` 类型错误已修复，待 `cargo test` 验证

**待实施**（Phase 3-6）：
- Phase 3：路由编排 — gpui 层 `route_all_edges` + `FlowEditorView.cached_edge_routes` 缓存
- Phase 4：渲染层适配 — `EdgeRender::Routed` 变体 + `paint_edge_routed`
- Phase 5：命中测试验证 — 按钮位置无需改动，仅需验证
- Phase 6：文档 + 测试 + 构建

---

## 当前状态分析

### Phase 2 验证状态

`simplify.rs` 中的 `isize/usize` 减法错误已修复（4 行），但 `cargo test -p rust-agent-flow --lib routing` 尚未重新运行。**Phase 3 开始前必须先验证 Phase 2 构建/测试通过**。

### 关键改造点（已通过代码阅读确认）

| 文件 | 改造点 | 现状 |
|------|--------|------|
| [flow_editor.rs](file:///d:/GitCode/RF/rust-agent-flow/crates/gpui/src/editor/flow_editor.rs) | 新增 `cached_edge_routes` 字段 + `relayout` 末尾调用 | 字段不存在，`relayout` 在 L188-210 更新 body 缓存后结束 |
| [editor/mod.rs](file:///d:/GitCode/RF/rust-agent-flow/crates/gpui/src/editor/mod.rs) | 添加 `mod routing;` | 当前 10 个子模块 |
| [edge_geometry.rs](file:///d:/GitCode/RF/rust-agent-flow/crates/gpui/src/editor/rendering/edge_geometry.rs) | `EdgeRender` 枚举新增 `Routed` 变体 | 当前 2 个变体（Normal/LoopBack），L16-31 |
| [edges.rs](file:///d:/GitCode/RF/rust-agent-flow/crates/gpui/src/editor/rendering/edges.rs) | `render_edges` 优先用缓存路由 + canvas paint 新增 `Routed` 分支 | L50-97 构建 `edge_renders`，L133-149 paint match |
| [edge_view.rs](file:///d:/GitCode/RF/rust-agent-flow/crates/gpui/src/edge/edge_view.rs) | 新增 `paint_edge_routed` 函数 | 当前有 `paint_edge_scaled`/`paint_loop_back_edge`/`paint_polyline`/`paint_arrow` |
| [hit_test.rs](file:///d:/GitCode/RF/rust-agent-flow/crates/gpui/src/editor/hit_test.rs) | 仅验证，不改代码 | `hit_test_edge_plus` 用端口+25px 偏移，路由后首段方向一致 |

### RectF API（已确认存在）

[geometry/mod.rs](file:///d:/GitCode/RF/rust-agent-flow/crates/core/src/geometry/mod.rs) L102/140/148/156：
- `RectF::from_center(center, size)` — 构造
- `expand(amount)` — 外扩
- `intersects(other)` — 相交判断
- `union(other)` — 并集

---

## 实施方案

### Phase 3：路由编排（gpui 层）

#### 3.1 新建 `crates/gpui/src/editor/routing.rs`

**职责**：遍历所有非 LoopBack 边，调用 `route_edge` 计算避障路径，返回 `HashMap<EdgeId, Vec<PointF>>`。

**用户决策**："全部路由" — 所有非 LoopBack 边类型（含 Straight）都走路由器。Straight 边若路径无障碍则简化为 2 点直线（语义保持），需绕障时变折线（用户接受的语义）。

```rust
//! 路由编排：在 relayout 时一次性为所有非 LoopBack 边计算避障路径并缓存。
//!
//! 路由失败的边不包含在返回值中，渲染层回退到 ReactFlow 几何算法。

use std::collections::{HashMap, HashSet};

use rust_agent_flow::{
    EdgeId, FlowGraph, NodeId, PointF, PortSide, RectF,
    route_edge, GRID_CELL_SIZE, OBSTACLE_MARGIN,
};

use crate::node::NodeRegistry;
use super::flow_editor::LayoutDirection;
use super::rendering::compute_edge_endpoints;

/// 为所有非 LoopBack 边计算路由路径。
///
/// - LoopBack 边（target_port == "loop_in"）跳过，使用专用 loop_back_path
/// - 隐藏节点（收起的循环体）相关的边跳过
/// - 路由失败的边不包含在返回值中，调用方回退到几何路径
///
/// 障碍处理：所有节点 bounds 外扩 OBSTACLE_MARGIN 作为障碍，
/// 但排除当前边的 src/dst 节点自身（路径需要进出这些节点）。
pub(crate) fn route_all_edges(
    graph: &FlowGraph,
    registry: &NodeRegistry,
    body_nodes: &HashSet<NodeId>,
    hidden_nodes: &HashSet<NodeId>,
    layout: LayoutDirection,
    src_side_default: PortSide,
    dst_side_default: PortSide,
) -> HashMap<EdgeId, Vec<PointF>> {
    // 1. 收集所有节点 bounds（外扩 margin）作为障碍候选
    let all_obstacles: Vec<(NodeId, RectF)> = graph
        .nodes()
        .map(|n| (n.id, n.bounds().expand(OBSTACLE_MARGIN)))
        .collect();

    let mut routes = HashMap::new();

    for edge in graph.edges() {
        // 跳过 LoopBack 边
        if edge.target_port.as_deref() == Some("loop_in") {
            continue;
        }
        // 跳过隐藏节点相关边
        if hidden_nodes.contains(&edge.source) || hidden_nodes.contains(&edge.target) {
            continue;
        }

        let (src, src_side, dst, dst_side) = compute_edge_endpoints(
            edge, graph, registry, layout,
            src_side_default, dst_side_default, body_nodes,
        );

        // 排除 src/dst 节点自身的障碍（路径需进出这些节点）
        let obstacles: Vec<RectF> = all_obstacles
            .iter()
            .filter(|(nid, _)| *nid != edge.source && *nid != edge.target)
            .map(|(_, r)| *r)
            .collect();

        if let Some(waypoints) = route_edge(
            src, dst, src_side, dst_side,
            &obstacles, GRID_CELL_SIZE,
        ) {
            routes.insert(edge.id, waypoints);
        }
        // 路由失败：不插入，渲染层回退到 EdgeRender::Normal
    }

    routes
}
```

#### 3.2 `editor/mod.rs` 添加模块声明

在 [mod.rs](file:///d:/GitCode/RF/rust-agent-flow/crates/gpui/src/editor/mod.rs) 现有模块列表中（L15-24 之间）插入：
```rust
mod routing;
```
按字母序放在 `mod ports;` 之后、`mod rendering;` 之前。

#### 3.3 `flow_editor.rs` 新增缓存字段 + relayout 调用

**新增字段**（在 [flow_editor.rs](file:///d:/GitCode/RF/rust-agent-flow/crates/gpui/src/editor/flow_editor.rs) `cached_hidden_nodes` 之后，约 L100）：
```rust
/// 缓存的边路由路径（EdgeId → waypoints）。
///
/// 在 `relayout` 末尾更新，`render_edges` 优先使用，`hit_test` 复用。
/// 路由失败的边不包含在此 map 中，渲染层回退到几何路径。
/// 拖拽/平移不触发 relayout → 复用缓存，避免每帧 A* 搜索。
pub cached_edge_routes: HashMap<EdgeId, Vec<PointF>>,
```

**构造初始化**（`new` 方法，L142 附近）：
```rust
cached_edge_routes: HashMap::new(),
```

**relayout 末尾调用**（在 [flow_editor.rs](file:///d:/GitCode/RF/rust-agent-flow/crates/gpui/src/editor/flow_editor.rs) L210 `cached_hidden_nodes` 更新之后，方法结束前）：
```rust
// 计算边路由缓存：为所有非 LoopBack 边计算避障路径。
// 与 cached_body_groups/cached_all_body_nodes/cached_hidden_nodes 一同更新，
// 保证渲染与命中测试使用同一份路由数据。
let (src_side_default, dst_side_default) = self.port_sides();
self.cached_edge_routes = super::routing::route_all_edges(
    &self.graph,
    &self.registry,
    &self.cached_all_body_nodes,
    &self.cached_hidden_nodes,
    self.layout_direction,
    src_side_default,
    dst_side_default,
);
```

**import 调整**：`flow_editor.rs` L33 已有 `EdgeId` import，无需额外添加。

---

### Phase 4：渲染层适配

#### 4.1 `edge_geometry.rs` — EdgeRender 新增 Routed 变体

在 [edge_geometry.rs](file:///d:/GitCode/RF/rust-agent-flow/crates/gpui/src/editor/rendering/edge_geometry.rs) L16-31 的 `EdgeRender` 枚举中，新增第三个变体：
```rust
pub(super) enum EdgeRender {
    Normal {
        src: PointF,
        dst: PointF,
        src_side: PortSide,
        dst_side: PortSide,
        edge_type: rust_agent_flow::EdgeType,
    },
    LoopBack {
        src: PointF,
        dst: PointF,
        horizontal: bool,
        node_bounds: RectF,
        edge_type: rust_agent_flow::EdgeType,
    },
    Routed {
        waypoints: Vec<PointF>,
        edge_type: rust_agent_flow::EdgeType,
    },
}
```

#### 4.2 `edges.rs` — 优先使用路由路径

**4.2a 构建 edge_renders 优先检查缓存**（[edges.rs](file:///d:/GitCode/RF/rust-agent-flow/crates/gpui/src/editor/rendering/edges.rs) L57-96）：

在 `.map(|edge| { ... })` 闭包开头，先检查 `cached_edge_routes`：
```rust
.map(|edge| {
    let is_loop_back = edge.target_port.as_deref() == Some("loop_in");

    // 优先使用路由路径（LoopBack 边除外，它有专用 loop_back_path）
    if !is_loop_back {
        if let Some(waypoints) = self.cached_edge_routes.get(&edge.id) {
            return EdgeRender::Routed {
                waypoints: waypoints.clone(),
                edge_type: edge.edge_type,
            };
        }
    }

    let (src, src_side, dst, dst_side) = compute_edge_endpoints(
        edge, &self.graph, &registry, layout,
        src_side_default, dst_side_default, &self.cached_all_body_nodes,
    );

    if is_loop_back {
        // LoopBack 逻辑不变
        let node_bounds = body_groups
            .get(&edge.target)
            .map(|body| compute_loop_bounds(&self.graph, edge.target, body))
            .unwrap_or_else(|| {
                self.graph.node(edge.target).map(|n| n.bounds()).unwrap_or_default()
            });
        EdgeRender::LoopBack {
            src, dst, horizontal: horizontal_layout, node_bounds,
            edge_type: edge.edge_type,
        }
    } else {
        // 回退到几何路径（路由失败或未缓存）
        EdgeRender::Normal {
            src, dst, src_side, dst_side,
            edge_type: edge.edge_type,
        }
    }
})
```

**4.2b canvas paint 新增 Routed 分支**（[edges.rs](file:///d:/GitCode/RF/rust-agent-flow/crates/gpui/src/editor/rendering/edges.rs) L133-149）：

在 `match er` 中新增 `Routed` 分支，并 import `paint_edge_routed`：
```rust
use crate::edge::{paint_edge_routed, paint_edge_scaled, paint_loop_back_edge};

// ... in paint closure:
match er {
    EdgeRender::Normal { src, dst, src_side, dst_side, edge_type } => {
        paint_edge_scaled(*src, *dst, *src_side, *dst_side, *edge_type,
            s, total_offset, edge_default_color, window);
    }
    EdgeRender::LoopBack { src, dst, horizontal, node_bounds, edge_type } => {
        paint_loop_back_edge(*src, *dst, *horizontal, *node_bounds, *edge_type,
            s, total_offset, edge_loop_back_color, window);
    }
    EdgeRender::Routed { waypoints, edge_type } => {
        paint_edge_routed(waypoints, *edge_type,
            s, total_offset, edge_default_color, window);
    }
}
```

#### 4.3 `edge_view.rs` — 新增 paint_edge_routed

在 [edge_view.rs](file:///d:/GitCode/RF/rust-agent-flow/crates/gpui/src/edge/edge_view.rs) `paint_loop_back_edge` 之后新增：

```rust
/// 绘制路由边：对 A* 产生的 waypoints 应用圆角后绘制折线 + 箭头。
///
/// - Bezier: `round_corners(waypoints, 24.0)` — 更大圆角模拟平滑曲线
/// - SmoothStep: `round_corners(waypoints, 12.0)` — 与普通 SmoothStep 一致
/// - Step / Straight: 直接使用 waypoints（直角折线 / 路由简化后的直线）
///
/// 路由边始终用折线绘制（`round_corners` 已采样曲线为多点折线），
/// `is_bezier=false` 确保 `paint_arrow` 用最后两点方向算箭头。
#[allow(clippy::too_many_arguments)]
pub(crate) fn paint_edge_routed(
    waypoints: &[PointF],
    edge_type: EdgeType,
    scale: f32,
    offset: Point<Pixels>,
    color: Rgba,
    window: &mut Window,
) {
    if waypoints.len() < 2 {
        return;
    }
    let points = match edge_type {
        EdgeType::Bezier => round_corners(waypoints, 24.0),
        EdgeType::SmoothStep => round_corners(waypoints, 12.0),
        _ => waypoints.to_vec(),
    };
    paint_polyline(&points, false, false, scale, offset, color, window);
    paint_arrow(&points, false, scale, offset, color, window);
}
```

---

### Phase 5：命中测试验证

**结论：无需代码修改，仅验证**。

[hit_test.rs](file:///d:/GitCode/RF/rust-agent-flow/crates/gpui/src/editor/hit_test.rs) 的 `hit_test_edge_plus`（L171-227）使用 `compute_edge_endpoints` 获取端口位置 + side，然后 `base + 25px 沿 side 方向` 计算按钮位置。

路由后：
- `waypoints[0]` = src 端口位置（`route_edge` 保证，见 [routing/mod.rs:125](file:///d:/GitCode/RF/rust-agent-flow/crates/core/src/geometry/routing/mod.rs#L125)）
- A* 的 `start_direction` 约束保证第一步沿 `src_side` 外向方向（见 [astar.rs:181-185](file:///d:/GitCode/RF/rust-agent-flow/crates/core/src/geometry/routing/astar.rs#L181-L185)）

因此按钮位置（src + 25px 沿 src_side）与路由路径首段方向一致，命中测试正确。

**验证项**：运行 demo，点击边「+」按钮应能正确触发拆边插入面板。

---

### Phase 6：文档与验证

#### 6.1 新建文档 `docs/rust-agent-flow/07-geometry-layout/edge-routing.md`

内容大纲：
1. **架构概述**：障碍感知边路由的设计动机（ReactFlow 纯几何算法零障碍感知）与整体流程（relayout → route_all_edges → 缓存 → render 复用）
2. **Grid A* 算法**：占用网格离散化、4 方向寻路、曼哈顿启发式、拐弯惩罚、方向约束（start/goal）、渐进松弛
3. **路由编排**：`route_all_edges` 调用时机（relayout 末尾）、缓存策略（`cached_edge_routes`）、障碍排除（src/dst 节点）
4. **渲染适配**：`EdgeRender::Routed` 变体、`paint_edge_routed`、回退策略（路由失败 → Normal 几何路径）
5. **设计决策**：网格 10px、margin 15px、拐弯惩罚 2.0、LoopBack 跳过路由、全部边类型路由
6. **性能特征**：13 节点 15 边 < 5ms；relayout 时一次性计算，拖拽/平移复用缓存

#### 6.2 更新 `INDEX.md`

[INDEX.md](file:///d:/GitCode/RF/rust-agent-flow/docs/rust-agent-flow/07-geometry-layout/INDEX.md) 新增「边路由」章节条目（在「端口端点计算」与「Dagre 布局引擎」之间）：

```markdown
| [边路由算法](edge-routing.md) | 障碍感知 Grid A* 路由 + 渲染适配 |
```

学习目标新增一条：
```markdown
- 描述障碍感知边路由的占用网格、A* 方向约束与回退策略
```

#### 6.3 更新 `dagre-layout.md`

管线步骤从 8 步更新为 9 步，新增第 3 步 `align_main_flow`（说明：跨合并点拉齐主流节点到中位数中心线）。

#### 6.4 测试与构建验证

1. **Phase 2 验证**（Phase 3 前必须通过）：
   ```
   cargo test -p rust-agent-flow --lib routing
   cargo test -p rust-agent-flow --lib main_flow
   ```

2. **整体单元测试**：
   ```
   cargo test --lib
   ```

3. **构建验证**：
   ```
   cargo build -p rust-agent-flow
   cargo build
   ```

4. **视觉验证**（demo）：
   - 主线 Start→Vars→Agent→Planner→Check→Adapter→Loop→Summarize→End 在同一水平线
   - Check→Notify 边不穿过其他节点
   - Search→Adapter 边不穿过其他节点
   - Loop 回环边保持 U 形绕行（LoopBack 不走路由）
   - 循环体 Process 节点连线方向正确
   - 边「+」按钮位置正确，点击可拆边插入节点

5. **性能验证**：relayout 耗时 < 50ms（13 节点场景）

---

## 实施顺序与任务清单

| 步骤 | 文件 | 操作 | 依赖 |
|------|------|------|------|
| 1 | — | `cargo test -p rust-agent-flow --lib routing` 验证 Phase 2 | 无 |
| 2 | `crates/gpui/src/editor/routing.rs` | 新建 `route_all_edges` | 步骤 1 通过 |
| 3 | `crates/gpui/src/editor/mod.rs` | 添加 `mod routing;` | 步骤 2 |
| 4 | `crates/gpui/src/editor/flow_editor.rs` | 新增字段 + relayout 调用 + new 初始化 | 步骤 2-3 |
| 5 | `crates/gpui/src/editor/rendering/edge_geometry.rs` | EdgeRender 新增 Routed 变体 | 无 |
| 6 | `crates/gpui/src/edge/edge_view.rs` | 新增 `paint_edge_routed` | 无 |
| 7 | `crates/gpui/src/editor/rendering/edges.rs` | 优先用缓存 + paint 新增 Routed 分支 + import | 步骤 4-6 |
| 8 | — | `cargo build` 验证 Phase 3-4 编译 | 步骤 2-7 |
| 9 | — | 运行 demo 视觉验证 + 命中测试验证（Phase 5） | 步骤 8 |
| 10 | `docs/.../edge-routing.md` | 新建文档 | 步骤 9 |
| 11 | `docs/.../INDEX.md` | 新增章节条目 | 步骤 10 |
| 12 | `docs/.../dagre-layout.md` | 管线 8→9 步 | 步骤 10 |
| 13 | — | `cargo test --lib` + `cargo build` 最终验证 | 步骤 12 |

---

## 设计决策与假设

### 决策 1：全部边类型路由（含 Straight）
用户明确指示"全部路由"。Straight 边无障碍时 A* 简化为 2 点直线（语义保持），需绕障时变折线（用户接受的语义扩展）。

### 决策 2：LoopBack 边跳过路由
LoopBack 边有专用 `loop_back_path`（5 点 U 形绕行 Loop 下方/左侧），语义明确。A* 无法理解"必须绕过 Loop 组合边界"的语义约束，路由会破坏 U 形语义。

### 决策 3：隐藏节点相关边跳过路由
收起的循环体节点（`cached_hidden_nodes`）已隐藏，连接到这些节点的边不渲染，路由它们无意义且浪费计算。

### 决策 4：路由失败回退到几何路径
A* 可能因网格过大、障碍包围等原因失败。回退到 `EdgeRender::Normal`（ReactFlow 几何算法），系统永不中断。

### 决策 5：relayout 时一次性计算并缓存
避免每帧重复 A* 搜索。图结构变化（增删节点/边）触发 relayout → 重新路由；拖拽/平移不触发 relayout → 复用缓存。

### 决策 6：障碍排除 src/dst 节点自身
所有节点 bounds 外扩 margin 作为障碍，但当前边的 src/dst 节点排除（路径需进出这些节点）。`route_edge` 内部还会在 src/dst 周围清除 4×grid_size 区域保证可达。

### 假设
- 节点尺寸 ≥ 35px（demo 最小高度），10px 网格可正确表达
- 画布 < 5000×5000（50K 格），A* < 50ms
- demo 场景节点数 < 50，性能无忧
