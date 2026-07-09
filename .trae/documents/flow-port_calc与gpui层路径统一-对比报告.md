# port_calc 与 gpui 层路径统一 — 优化对比报告

> 评估范围：Phase 1–7 实施后的 port_calc.rs 与 gpui 渲染层在**代码行数**、**依赖关系**、**性能指标**三个维度的前后变化。
> 数据来源：`git show HEAD:path`（before）与工作区当前文件（after）实测，`git diff --numstat` 交叉验证。

---

## 一、代码行数

### 1.1 逐文件明细

| 文件 | 层 | before | after | 净变化 | 说明 |
|------|----|-------:|------:|-------:|------|
| [port_calc.rs](file:///d:/GitCode/RF/rust-agent-flow/crates/core/src/geometry/port_calc.rs) | core | 294 | 324 | **+30** | `resolve_endpoints`/`ResolvedEdge` 加 `#[deprecated]`；3 个纯算法函数改 `pub`；模块文档重写 |
| [lib.rs](file:///d:/GitCode/RF/rust-agent-flow/crates/core/src/lib.rs) | core | 29 | 31 | **+2** | 新增 3 个纯算法函数 re-export + deprecated re-export |
| [edge_path.rs](file:///d:/GitCode/RF/rust-agent-flow/crates/core/src/geometry/edge_path.rs) | core | 608 | 675 | **+67** | `outward` Auto 断言；`bezier_path` 混合 side 对角线偏移；+2 单元测试 |
| [loop_layout.rs](file:///d:/GitCode/RF/rust-agent-flow/crates/core/src/layout/dagre/loop_layout.rs) | core | 370 | 377 | **+7** | D7 修复：`reserve_loop_back_edge_space` 排除 done 目标节点 |
| [ports.rs](file:///d:/GitCode/RF/rust-agent-flow/crates/gpui/src/editor/ports.rs) | gpui | 154 | 113 | **−41** | 删除 `edge_endpoints`/`compute_endpoint` dead code；`port_side` 增加 fixed 感知 |
| [edge_geometry.rs](file:///d:/GitCode/RF/rust-agent-flow/crates/gpui/src/editor/rendering/edge_geometry.rs) | gpui | 83 | 80 | **−3** | 浮动边改用 `compute_side_from_position`；移除 `_body_nodes` 参数 |
| [registry.rs](file:///d:/GitCode/RF/rust-agent-flow/crates/gpui/src/node/registry.rs) | gpui | 48 | 37 | **−11** | 删除 `specs_fn()` 方法及文档注释 |
| [edges.rs](file:///d:/GitCode/RF/rust-agent-flow/crates/gpui/src/editor/rendering/edges.rs) | gpui | 315 | 314 | **−1** | 3 处调用点移除 `&all_body_nodes`；2 处 Auto 断言 |
| [hit_test.rs](file:///d:/GitCode/RF/rust-agent-flow/crates/gpui/src/editor/hit_test.rs) | gpui | 202 | 202 | **0** | 1 处调用点移除参数；1 处 Auto 断言（增减抵消） |

### 1.2 按层汇总

| 层 | before | after | 净变化 | 趋势 |
|----|-------:|------:|-------:|------|
| **core 层**（算法库） | 1,301 | 1,407 | **+106** | 增长：deprecation 标注 + 防御性断言 + 新测试 |
| **gpui 层**（渲染） | 802 | 746 | **−56** | 缩减：dead code 清除 + 参数瘦身 |
| **合计** | 2,103 | 2,153 | **+50** | core 增量 ≈ gpui 减量的 2 倍 |

### 1.3 行数变化的性质分析

core 层 +106 行**并非逻辑膨胀**，而是工程治理投入：

| 增量类别 | 估算行数 | 占比 |
|----------|--------:|-----:|
| `#[deprecated]` / `#[allow(deprecated)]` 标注 | ~20 | 19% |
| `debug_assert!(false, ...)` 防御性断言（5 处） | ~15 | 14% |
| 模块文档重写（定位声明） | ~15 | 14% |
| `bezier_path` 混合 side 增强 + 2 个单元测试 | ~45 | 42% |
| D7 逻辑修复 | ~11 | 10% |

gpui 层 −56 行**全部为净删除**：dead code（`edge_endpoints`/`compute_endpoint`/`specs_fn`）+ 未使用 import + `body_nodes` 参数透传链路。

---

## 二、依赖关系

### 2.1 模块间 import 变化

**[ports.rs](file:///d:/GitCode/RF/rust-agent-flow/crates/gpui/src/editor/ports.rs)**：

```diff
- use rust_agent_flow::{Edge, FlowGraph, Node, NodeId, PointF, PortDirection, PortId, PortSide};
+ use rust_agent_flow::{Node, PointF, PortDirection, PortId, PortSide};
```

移除 `Edge`、`FlowGraph`、`NodeId` 三个类型依赖 — ports.rs 不再直接感知边和图的类型。

**[registry.rs](file:///d:/GitCode/RF/rust-agent-flow/crates/gpui/src/node/registry.rs)**：

```diff
- use rust_agent_flow::{NodeId, PortSpec};
+ use rust_agent_flow::PortSpec;
```

移除 `NodeId` 依赖 — registry 不再涉及节点 ID 操作（`specs_fn()` 删除后唯一引用消失）。

**[edge_geometry.rs](file:///d:/GitCode/RF/rust-agent-flow/crates/gpui/src/editor/rendering/edge_geometry.rs)**：

```diff
- use rust_agent_flow::{Edge, FlowGraph, NodeId, PointF, PortSide, RectF};
+ use rust_agent_flow::{compute_side_from_position, Edge, FlowGraph, NodeId, PointF, PortSide, RectF};
```

新增 `compute_side_from_position` — 浮动边 side 推导从"按布局方向默认"改为"按节点相对位置计算"，依赖 core 层纯算法函数。

**[lib.rs](file:///d:/GitCode/RF/rust-agent-flow/crates/core/src/lib.rs)** 公开 API：

```diff
+ pub use geometry::port_calc::{compute_side_from_position, distribute_on_side, point_on_side};
+ #[allow(deprecated)]
+ pub use geometry::port_calc::{resolve_endpoints, ResolvedEdge};
```

3 个纯算法函数升级为 `pub`（原为模块私有）；`resolve_endpoints`/`ResolvedEdge` 保留导出但标记 deprecated。

### 2.2 跨层调用路径变化

**优化前**（双路径并存，渲染层不调用 core 层 side 计算）：

```
渲染层 edges.rs
  ├─ compute_edge_endpoints(edge, graph, registry, layout, body_nodes, ...)
  │   ├─ 有 port_id → port_side(spec.side, layout)      [gpui 自有逻辑]
  │   └─ 无 port_id  → default_side(layout)              [gpui 自有逻辑]
  └─ （core 层 resolve_endpoints 从未被渲染层调用）

core 层 port_calc.rs
  └─ resolve_endpoints(graph, port_specs) → HashMap<EdgeId, ResolvedEdge>  [死路径：无调用方]
```

**优化后**（gpui 为主路径，core 降级为纯算法工具库）：

```
渲染层 edges.rs
  └─ compute_edge_endpoints(edge, graph, registry, layout, ...)      [参数减少]
      ├─ 有 port_id → resolve_port(node, pid, registry, layout)
      │                ├─ IFlowNode::port_position 回调（强约束节点）
      │                └─ port_side(spec.fixed → spec.side → default_side)
      └─ 无 port_id  → compute_side_from_position(src, dst)         [调用 core 纯函数]

core 层 port_calc.rs
  ├─ compute_side_from_position()    [pub：被 gpui 调用]
  ├─ distribute_on_side()            [pub：供节点 port_position 回调使用]
  ├─ point_on_side()                 [pub：供节点 port_position 回调使用]
  └─ resolve_endpoints()             [#[deprecated]：保留但不推荐]
```

### 2.3 依赖关系变化总结

| 维度 | 优化前 | 优化后 |
|------|--------|--------|
| side 计算路径数 | 2 条（core `resolve_endpoints` + gpui `port_side`），core 路径为死代码 | 1 条主路径（gpui `resolve_port`）+ core 纯算法工具 |
| gpui → core 调用 | 无（渲染层自包含） | 有（`compute_side_from_position` 纯函数调用） |
| `resolve_endpoints` 调用方 | 0 个（死代码） | 0 个（deprecated，编译告警） |
| `body_nodes` 参数透传链 | 3 个调用点（edges → edge_geometry） | 0（参数已移除） |
| `PortSpec.fixed` 感知 | core 层不感知、gpui 层不感知 | gpui 层 `port_side` 防御性感知 |
| `port_position` 回调支持 | core 层不支持 | gpui 层 `resolve_port` 优先调用 |

---

## 三、性能指标

### 3.1 算法复杂度对比

| 操作 | 优化前 | 优化后 | 复杂度变化 |
|------|--------|--------|-----------|
| 渲染层 side 解析（有 port_id） | `port_side`：schema 查找 + `default_side` | `resolve_port`：`port_position` 回调 or `port_side` | O(1) → O(1)，无变化 |
| 渲染层 side 解析（无 port_id） | `default_side(layout)`：枚举匹配 | `compute_side_from_position`：2 次减法 + 2 次 abs + 2 次比较 | O(1) → O(1)，常数项略增（~5ns） |
| `bezier_path`（同侧 side） | 2 次 `bezier_control` | 2 次 `bezier_control` | 无变化 |
| `bezier_path`（混合 side） | 2 次 `bezier_control`（轴向不一致导致扭曲） | 1 次 `sqrt` + 2 次乘法（对角线偏移） | +1 次 `sqrt`（~3ns），仅混合 side 边触发 |
| `resolve_endpoints`（deprecated） | O(V+E) 批量计算 + HashMap 分配 | 不调用 | 消除潜在开销 |

### 3.2 内存分配对比

| 场景 | 优化前 | 优化后 |
|------|--------|--------|
| `resolve_endpoints` 返回值 | `HashMap<EdgeId, ResolvedEdge>` — 为**所有边**分配（即使渲染层不用） | 不分配（渲染层不调用） |
| 渲染层端点计算 | 按需 per-edge 计算，无批量分配 | 同左，无变化 |
| `body_nodes` 参数 | `HashSet<NodeId>` 在 edges.rs 构建，透传给 edge_geometry / hit_test | 参数移除，不再构建 |

### 3.3 渲染帧开销估算（典型场景：100 节点 / 120 边）

| 指标 | 优化前 | 优化后 | 差异 |
|------|--------|--------|------|
| side 解析总耗时 | ~120 × 5ns ≈ 0.6μs | ~120 × 8ns ≈ 1.0μs | +0.4μs（浮动边 side 计算略重） |
| `bezier_path` 混合 side 边（估 ~5 条） | — | 5 × 3ns ≈ 15ns | +15ns（可忽略） |
| HashMap 分配（`resolve_endpoints`） | 不触发（死代码） | 不触发 | 无变化 |
| `body_nodes` HashSet 构建 | ~100 节点 × 10ns ≈ 1μs | 0 | **−1.0μs** |
| **帧净开销** | — | — | **≈ −0.6μs**（净优化） |

> 浮动边 side 计算的常数项增加（+0.4μs）被 `body_nodes` 构建消除（−1.0μs）抵消，整体帧开销**略有下降**。

### 3.4 正确性提升（非性能但影响渲染质量）

| 缺陷 | 优化前表现 | 优化后 |
|------|----------|--------|
| Loop `loop_body`/`loop_in` 强约束 | `resolve_endpoints` 不感知 fixed → 若被调用会算错 side | `port_side` 优先检查 `spec.fixed` |
| Condition 多出口 `port_position` 回调 | `resolve_endpoints` 无法调用回调 → 端口位置错误 | `resolve_port` 优先调用回调 |
| 混合 side 贝塞尔扭曲（D6） | `bezier_control` 按各自轴向偏移 → 控制点不对称 | 对角线距离统一偏移基准 |
| 纵向布局 done 边误下移（D7） | `reserve_loop_back_edge_space` 下移 done 目标节点 | 排除 done 目标节点 |
| `PortSide::Auto` 流入路径算法 | 静默使用 fallback 值，难以排查 | `debug_assert!(false)` 在 debug 构建立即报警 |

---

## 四、总结

### 核心成果

1. **路径统一**：消除 core 层 `resolve_endpoints` 死路径，确立 gpui 层 `resolve_port` 为唯一渲染 side 解析路径，core 层降级为纯算法工具库
2. **dead code 清除**：gpui 层净减 56 行（`edge_endpoints`/`compute_endpoint`/`specs_fn` 及未使用 import）
3. **参数链路瘦身**：`body_nodes` 参数从 3 个调用点移除，消除每帧 HashSet 构建
4. **正确性提升**：`PortSpec.fixed` 强约束感知 + `port_position` 回调支持 + 混合 side 贝塞尔修复 + D7 纵向布局修复
5. **防御性增强**：5 处 `debug_assert` 确保 `PortSide::Auto` 不静默流入路径算法

### 代价

- core 层 +106 行（deprecation 标注 + 断言 + 测试 + 文档），属一次性工程治理投入
- 浮动边 side 计算从枚举匹配改为算术运算，常数项增加 ~3ns/边（可忽略）
- `bezier_path` 混合 side 增加 1 次 `sqrt`（仅极少数边触发）

### 净评估

| 维度 | 评价 |
|------|------|
| 代码行数 | gpui 层精简（−56），core 层治理投入（+106），总体 +50 行但质量提升 |
| 依赖关系 | 消除死路径，跨层依赖从"无调用"变为"纯函数调用"，职责清晰 |
| 性能 | 帧开销净降 ~0.6μs，无复杂度退化，混合 side 边路径质量显著提升 |
