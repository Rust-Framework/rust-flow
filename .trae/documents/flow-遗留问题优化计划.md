# Flow 排版连线遗留问题优化计划

## 一、任务说明

基于《Flow 排版与连线缺陷分析（修正版）》和已完成的《Flow 端口 side 声明机制重构 - 实施计划（方案 A）》，针对实施计划 6.3 节列出的遗留问题制定优化计划。

**优先级**：Phase 1 优先处理 `port_calc.rs` 与 gpui 层路径统一问题（用户指定），其余按依赖关系排序。

---

## 二、遗留问题清单与优先级

| Phase | 问题 | 来源 | 优先级 | 依赖 |
|-------|------|------|--------|------|
| 1 | port_calc.rs 与 gpui 层路径统一 | 实施计划 6.3.1 | **P0（用户指定）** | 无 |
| 2 | body_nodes 参数清理 + dead code 清理 | 实施计划 6.3.3 | P1 | Phase 1 |
| 3 | PortSide::Auto 防御性断言 | 实施计划 6.3.4 | P1 | Phase 1 |
| 4 | D6 混合 side 路径算法增强 | 分析报告 5.2 | P2 | Phase 1 |
| 5 | D4/D5/D7 纵向布局循环体策略 | 分析报告 5.2 | P2 | Phase 4 |
| 6 | D8 Condition 纵向布局端口对齐 | 分析报告 5.2 | P3 | Phase 5 |
| 7 | 文档同步 | 实施计划 6.3.5 | P3 | Phase 1-6 |

---

## 三、Phase 1：port_calc.rs 与 gpui 层路径统一（P0）

### 3.1 现状：两条并行的 side 计算路径

#### 路径 A：gpui 渲染层（实际用于渲染）

- **文件**：[ports.rs](file:///d:/GitCode/RF/rust-agent-flow/crates/gpui/src/editor/ports.rs) + [edge_geometry.rs](file:///d:/GitCode/RF/rust-agent-flow/crates/gpui/src/editor/rendering/edge_geometry.rs)
- **入口**：`compute_edge_endpoints` → `resolve_port`
- **side 解析**：
  1. 优先 `IFlowNode::port_position` 回调（节点显式声明 side）
  2. 回退 `port_side`：schema spec.side != Auto → spec.side；Auto → `default_side`（按布局方向）
- **位置计算**：`port_position_by_side`（节点边缘中点）
- **特点**：单边计算，支持节点自治，感知 fixed（通过 port_position 回调）
- **缺陷**：
  - 多端口同侧会重叠在中点（无分布算法）
  - 浮动边（无 port_id）用 `default_side`（按布局方向），不如相对位置合理
  - `edge_endpoints` / `compute_endpoint` 是 dead code（`#[allow(dead_code)]`）

#### 路径 B：core 层（未被渲染层调用）

- **文件**：[port_calc.rs](file:///d:/GitCode/RF/rust-agent-flow/crates/core/src/geometry/port_calc.rs)
- **入口**：`resolve_endpoints`（批量计算所有边）
- **side 解析**：`resolve_side` → spec.side != Auto → spec.side；Auto → `compute_side_from_position`（按两节点中心相对位置 dx/dy）
- **位置计算**：`distribute_on_side`（同侧均匀分布 + In/Out 半边分区）
- **特点**：批量计算，基于节点相对位置决定 side，有分布算法
- **缺陷**：
  - **不感知 `fixed` 字段**（[L146](file:///d:/GitCode/RF/rust-agent-flow/crates/core/src/geometry/port_calc.rs#L146) 只看 `spec.side != Auto`）
  - **不支持节点 `port_position` 回调**（Condition 多出口、Loop 强约束端口必需）
  - `registry.specs_fn()` 是占位符（[registry.rs L51-L56](file:///d:/GitCode/RF/rust-agent-flow/crates/gpui/src/node/registry.rs#L51-L56) 返回空 Vec）
  - side 解析策略（相对位置）与渲染层（布局方向）冲突

#### 两条路径差异对照

| 维度 | gpui 层 | core 层 |
|------|---------|---------|
| 实际使用 | ✅ 渲染/命中测试/+按钮 | ❌ 仅 lib.rs re-export + 测试 |
| side（有 port_id, Auto） | `default_side`（布局方向） | `compute_side_from_position`（相对位置） |
| side（无 port_id, 浮动边） | `default_side`（布局方向） | `compute_side_from_position`（相对位置） |
| 端口位置 | 边缘中点 | 同侧均匀分布 + In/Out 半边分区 |
| 节点自定义位置 | ✅ 支持 | ❌ 不支持 |
| fixed 感知 | ✅ 隐式感知 | ❌ 不感知 |
| 接口形态 | 单边计算 | 批量计算 |
| In/Out 同侧防重叠 | ❌ 无 | ✅ 半边分区 |

### 3.2 统一方向决策：以 gpui 层为主路径，core 层降级为纯算法库

**理由**：
1. gpui 层已实际用于渲染，支持节点自治（port_position 回调），符合"端口属于节点"原则
2. core 层 `resolve_endpoints` 是未完工的并行实现，不感知 fixed + 不支持 port_position 回调，无法满足结构化节点需求
3. core 层的 `distribute_on_side` / `compute_side_from_position` 是有价值的纯算法，应保留为可复用工具
4. 保持节点自治架构（节点 port_position 回调负责多端口分布），不引入批量预计算层

### 3.3 改动清单

#### 改动 1.1：core 层 port_calc.rs 重构——废弃批量入口，保留纯算法

**文件**：[crates/core/src/geometry/port_calc.rs](file:///d:/GitCode/RF/rust-agent-flow/crates/core/src/geometry/port_calc.rs)

**内容**：
1. `resolve_endpoints` + `ResolvedEdge` 标记 `#[deprecated]` 并增加文档说明：不感知 fixed + 不支持 port_position，推荐使用 gpui 层路径
2. `distribute_on_side` / `point_on_side` / `compute_side_from_position` 改为 `pub`，作为可复用纯函数
3. 更新模块文档说明定位变更：从"端点计算入口"降级为"纯算法工具库"

```rust
//! Port-side calculation utilities.
//!
//! Historically this module provided `resolve_endpoints`, a batch endpoint
//! resolver. That function is **deprecated** — it does not respect
//! `PortSpec.fixed` (strong constraints) and cannot invoke node-provided
//! `port_position` callbacks. The gpui rendering layer uses its own
//! `resolve_port` path (in `crates/gpui/src/editor/ports.rs`) which correctly
//! handles both.
//!
//! The remaining functions here are pure geometric utilities reused by the
//! gpui layer:
//! - `compute_side_from_position`: derive side from relative node positions
//!   (for floating edges without a port_id)
//! - `distribute_on_side`: evenly distribute multiple ports on one side
//! - `point_on_side`: absolute position at parameter t along a side
```

**保留函数**：
- `compute_side_from_position(self_center, other_center) -> PortSide` —— 浮动边 side 推导
- `distribute_on_side(bounds, side, dir, has_opposite, count) -> Vec<PointF>` —— 多端口分布
- `point_on_side(bounds, side, t, outward) -> PointF` —— 边上参数化位置

**废弃函数**：
- `resolve_endpoints` —— `#[deprecated(note = "Use gpui layer resolve_port instead")]`
- `resolve_side` —— 内部函数，随 `resolve_endpoints` 一起标记
- `ResolvedEdge` —— `#[deprecated]`

#### 改动 1.2：lib.rs 调整 re-export

**文件**：[crates/core/src/lib.rs](file:///d:/GitCode/RF/rust-agent-flow/crates/core/src/lib.rs) L32

**内容**：
- 保留 `resolve_endpoints` / `ResolvedEdge` 的 re-export（`#[deprecated]` 会传递），但增加 `#[allow(deprecated)]`
- 新增 re-export：`compute_side_from_position` / `distribute_on_side` / `point_on_side`

```rust
pub use geometry::port_calc::{
    compute_side_from_position, distribute_on_side, point_on_side,
};
#[allow(deprecated)]
pub use geometry::port_calc::{resolve_endpoints, ResolvedEdge};
```

#### 改动 1.3：gpui 层 resolve_port 增强——fixed 显式感知 + 浮动边相对位置

**文件**：[crates/gpui/src/editor/ports.rs](file:///d:/GitCode/RF/rust-agent-flow/crates/gpui/src/editor/ports.rs)

**内容**：

**1.3a：`port_side` 增加 fixed 防御性感知**

当前 `port_side`（L59-L75）只看 `spec.side != Auto`。增加 fixed 感知：fixed=true 时即使 side 声明异常也返回 spec.side（防御性，正常情况节点应已实现 port_position 回调）。

```rust
pub(crate) fn port_side(
    registry: &NodeRegistry,
    kind: &str,
    port_id: &str,
    layout: LayoutDirection,
) -> PortSide {
    if let Some(flow_node) = registry.get(kind) {
        if let Some(spec) = flow_node.schema().ports.iter().find(|p| p.id == port_id) {
            // Strong constraint (fixed=true): always use declared side.
            // Weak constraint (fixed=false): Auto → default_side by layout.
            if spec.fixed || spec.side != PortSide::Auto {
                return spec.side;
            }
            return default_side(spec.direction, layout);
        }
    }
    layout_default_for_unknown(port_id, layout)
}
```

**1.3b：`compute_edge_endpoints` 浮动边改用相对位置**

当前浮动边（无 port_id）用 `default_side`（按布局方向）。改为 `compute_side_from_position`（按节点相对位置），更符合"浮动边无明确方向"的语义。

**文件**：[crates/gpui/src/editor/rendering/edge_geometry.rs](file:///d:/GitCode/RF/rust-agent-flow/crates/gpui/src/editor/rendering/edge_geometry.rs)

```rust
pub(crate) fn compute_edge_endpoints(
    edge: &Edge,
    graph: &FlowGraph,
    registry: &NodeRegistry,
    layout: LayoutDirection,
    _body_nodes: &HashSet<NodeId>,
    default_src_side: PortSide,  // 保留参数（fallback 兜底）
    default_dst_side: PortSide,
) -> (PointF, PortSide, PointF, PortSide) {
    let src_node = match graph.node(edge.source) {
        Some(n) => n,
        None => return (PointF::default(), default_src_side, PointF::default(), default_dst_side),
    };
    let dst_node = match graph.node(edge.target) {
        Some(n) => n,
        None => return (PointF::default(), default_src_side, PointF::default(), default_dst_side),
    };

    let (src, src_side) = match edge.source_port.as_deref() {
        Some(pid) => resolve_port(src_node, pid, registry, layout),
        None => {
            // Floating edge: derive side from relative position.
            let side = compute_side_from_position(src_node.center(), dst_node.center());
            (port_position_by_side(src_node, side), side)
        }
    };

    let (dst, dst_side) = match edge.target_port.as_deref() {
        Some(pid) => resolve_port(dst_node, pid, registry, layout),
        None => {
            let side = compute_side_from_position(dst_node.center(), src_node.center());
            (port_position_by_side(dst_node, side), side)
        }
    };

    (src, src_side, dst, dst_side)
}
```

**注意**：当前所有边都有 port_id（节点 schema 声明了端口），浮动边路径属防御性兜底。`default_src_side` / `default_dst_side` 参数保留作为最终 fallback（节点不存在时）。

#### 改动 1.4：删除 gpui 层 dead code

**文件**：[crates/gpui/src/editor/ports.rs](file:///d:/GitCode/RF/rust-agent-flow/crates/gpui/src/editor/ports.rs) L117-L168

**内容**：删除 `edge_endpoints` + `compute_endpoint` 函数（`#[allow(dead_code)]` 标记的残留代码，功能已被 `compute_edge_endpoints` 取代）。

```rust
// 删除以下两个函数：
// - edge_endpoints (L122-L147)
// - compute_endpoint (L151-L168)
```

#### 改动 1.5：registry 清理 specs_fn 占位符

**文件**：[crates/gpui/src/node/registry.rs](file:///d:/GitCode/RF/rust-agent-flow/crates/gpui/src/node/registry.rs) L46-L56

**内容**：删除 `specs_fn()` 方法（占位符，从未实际使用，`resolve_endpoints` 已废弃）。

```rust
// 删除：
// pub fn specs_fn(&self) -> impl Fn(NodeId) -> Vec<PortSpec> + '_ { |_| Vec::new() }
```

**保留**：`port_specs_for(kind)` 方法（实际可用，返回节点的 PortSpec 列表）。

#### 改动 1.6：统一后的 side 解析策略（单一来源）

| 场景 | side 来源 | 实现位置 |
|------|----------|----------|
| 有 port_id + 节点实现 port_position | 节点回调返回的 side | `resolve_port` → `flow_node.port_position` |
| 有 port_id + spec.fixed=true | spec.side（强约束） | `port_side` 防御性分支 |
| 有 port_id + spec.side != Auto | spec.side（节点声明） | `port_side` |
| 有 port_id + spec.side == Auto | `default_side`（按布局方向，弱约束） | `port_side` → `default_side` |
| 无 port_id（浮动边） | `compute_side_from_position`（按节点相对位置） | `compute_edge_endpoints` |
| 节点不存在（fallback） | `default_src/dst_side`（按布局方向） | `compute_edge_endpoints` 兜底 |

### 3.4 Phase 1 验证

- `cargo check --all` 编译通过
- `cargo test --all` 单元测试通过（port_calc.rs 测试需适配 `#[deprecated]` 警告）
- 手动验证：
  - 横向/纵向布局下，有 port_id 的边 side 正确（按布局方向或节点声明）
  - 浮动边（如有）side 按相对位置推导
  - Loop 的 loop_body/loop_in 仍为强约束 Right/Left
  - Condition 多出口仍正常均布

---

## 四、Phase 2：body_nodes 参数清理 + dead code 清理（P1）

### 4.1 body_nodes 参数清理

**文件**：
- [edge_geometry.rs](file:///d:/GitCode/RF/rust-agent-flow/crates/gpui/src/editor/rendering/edge_geometry.rs) L64-L72
- [edges.rs](file:///d:/GitCode/RF/rust-agent-flow/crates/gpui/src/editor/rendering/edges.rs) L61-L69, L197-L205, L286-L294

**内容**：`compute_edge_endpoints` 的 `_body_nodes: &HashSet<NodeId>` 参数在 Phase 1 方案 A 实施后已不再使用（加下划线前缀）。移除该参数 + 所有调用点适配。

**影响调用点**：
- `render_edges`（edges.rs L61）
- `render_edge_plus_buttons`（edges.rs L197）
- `render_plus_tooltip`（edges.rs L286）
- `hit_test.rs` 的调用点（如有）

### 4.2 其他 dead code 清理

- 检查 `default_src_side` / `default_dst_side` 参数是否可简化（浮动边改用相对位置后，仅在节点不存在时兜底）
- 检查 `port_sides()` 方法是否仍需要（[flow_editor.rs L160](file:///d:/GitCode/RF/rust-agent-flow/crates/gpui/src/editor/flow_editor.rs#L160)）

---

## 五、Phase 3：PortSide::Auto 防御性断言（P1）

### 5.1 增加断言

**文件**：
- [edge_path.rs](file:///d:/GitCode/RF/rust-agent-flow/crates/core/src/geometry/edge_path.rs) L20-L28（`outward` 的 Auto 分支）
- [ports.rs](file:///d:/GitCode/RF/rust-agent-flow/crates/gpui/src/editor/ports.rs) L49-L55（`port_position_by_side` 的 Auto 分支）
- [edges.rs](file:///d:/GitCode/RF/rust-agent-flow/crates/gpui/src/editor/rendering/edges.rs) L228, L315（+ 按钮偏移的 Auto 分支）
- [hit_test.rs](file:///d:/GitCode/RF/rust-agent-flow/crates/gpui/src/editor/hit_test.rs) L210

**内容**：在路径算法层的 Auto 分支增加 `debug_assert!(false, "PortSide::Auto should be resolved before path calculation")`，确保 Auto 不流传到路径算法层。

**保留**：Auto 分支的回退逻辑（返回 Right）不删除，作为 release 构建的防御性回退。

```rust
PortSide::Auto => {
    debug_assert!(false, "Auto side should be resolved before path calculation");
    PointF::new(1.0, 0.0)  // 防御性回退
}
```

---

## 六、Phase 4：D6 混合 side 路径算法增强（P2）

### 6.1 问题

**文件**：[edge_path.rs L340-L355](file:///d:/GitCode/RF/rust-agent-flow/crates/core/src/geometry/edge_path.rs#L340-L355)

`bezier_control` 在混合 side 场景（如 Loop `loop_body`: Right → 循环体 Top）下，源控制点沿 X 轴偏移，目标控制点沿 Y 轴偏移，贝塞尔曲线形态扭曲，可能穿过节点。

### 6.2 改动方向

1. **`bezier_control` 混合 side 处理**：当 src_side 和 dst_side 的轴向不一致（一个水平一个垂直）时，增加专门的处理分支。可选策略：
   - 策略 A：控制点偏移量取两侧距离的较小值，避免单侧过度外突
   - 策略 B：混合 side 时切换到 smoothstep_path（正交路由），避免贝塞尔扭曲
2. **`rf_get_points` 混合 side 验证**：当前已有 mixed-position 处理（L133-L146, L170-L183），需验证 Right→Top 等场景的路径形态

### 6.3 验证

- 新增单元测试：bezier_path / smoothstep_path 在 Right→Top、Right→Bottom、Bottom→Left 等混合 side 下的路径不穿节点
- 手动验证：Loop 节点的 loop_body → 循环体节点（Right→Top/Bottom）连线形态正常

---

## 七、Phase 5：D4/D5/D7 纵向布局循环体策略（P2）

### 7.1 D4：align_loop_body_target 无视布局方向

**文件**：[loop_layout.rs L249-L315](file:///d:/GitCode/RF/rust-agent-flow/crates/core/src/layout/dagre/loop_layout.rs#L249-L315)

**问题**：`_direction: LayoutDirection` 参数未使用，body 组始终定位在 Loop 右侧（`body_x = loop_pos.x + loop_node.size.w + LOOP_BODY_GAP`）。

**改动方向**：
- 纵向布局下，body 组定位策略调整：改为 Loop 下方横向排列，或保持右侧但调整 done 目标位置避免冲突
- 需决策：纵向布局下 body 组的理想位置（右侧纵向堆叠 vs 下方横向排列）

### 7.2 D5：loop_back_path 两种布局都向下绕

**文件**：[edge_path.rs L423-L426](file:///d:/GitCode/RF/rust-agent-flow/crates/core/src/geometry/edge_path.rs#L423-L426)

**问题**：`_horizontal: bool` 参数不影响路径，两种布局都向下绕（5 点 U 形）。

**改动方向**：
- 纵向布局下（body 组在 Loop 右侧），回环边改为向右绕（RIGHT → UP → LEFT），避免跨越整个 Loop 宽度
- 需与 D4 的 body 组定位策略协同

### 7.3 D7：reserve_loop_back_edge_space 纵向布局误伤

**文件**：[loop_layout.rs L53-L96](file:///d:/GitCode/RF/rust-agent-flow/crates/core/src/layout/dagre/loop_layout.rs#L53-L96)

**问题**：纵向布局下，done 目标节点 Y 可能 > body 组 group_bottom，被误下移 100px。

**改动方向**：
- 增加布局方向判断：纵向布局下调整 reserve 逻辑或 reserve 方向
- 与 D4 body 组定位策略协同

### 7.4 Phase 5 决策点

**需用户决策**：纵向布局下 Loop 循环体 body 组的理想定位
- 选项 A：保持右侧纵向堆叠（现状），优化回环边路径向右绕
- 选项 B：改为下方横向排列，回环边向下绕（与横向布局对称）

---

## 八、Phase 6：D8 Condition 纵向布局端口对齐（P3）

### 8.1 问题

- [condition.rs L498-L525](file:///d:/GitCode/RF/rust-agent-flow/crates/gpui/src/builtin/condition.rs#L498-L525)：纵向布局下所有出口在底部沿宽度均匀分布
- [branch.rs L53-L186](file:///d:/GitCode/RF/rust-agent-flow/crates/core/src/layout/dagre/branch.rs#L53-L186)：`reorder_branch_targets` 按 if_N 顺序在 cross-axis（X）重排目标

端口位置（节点内均布 X）与目标位置（dagre + 重排后 X）独立计算，可能产生连线交叉。

### 8.2 改动方向

1. 端口均布策略与 `reorder_branch_targets` 重排策略协同：让端口 X 顺序与目标 X 顺序一致
2. 或：Condition 纵向布局端口位置由目标位置反推（读取 dagre 布局后的目标 X，按比例映射到节点底部）

### 8.3 依赖

- 需 Phase 5 完成（纵向布局策略明确后）再处理

---

## 九、Phase 7：文档同步（P3）

### 9.1 需同步的文档

| 文档 | 同步内容 |
|------|----------|
| [docs/rust-agent-flow/07-geometry-layout/port-calc.md](file:///d:/GitCode/RF/rust-agent-flow/docs/rust-agent-flow/07-geometry-layout/port-calc.md) | 说明 `resolve_endpoints` 已废弃，纯算法函数保留；新增 gpui 层 `resolve_port` 路径说明 |
| [docs/rust-agent-flow/08-iflow-node/noderegistry.md](file:///d:/GitCode/RF/rust-agent-flow/docs/rust-agent-flow/08-iflow-node/noderegistry.md) | 移除 `specs_fn` 占位警示，说明已删除 |
| [docs/rust-agent-flow/03-philosophy/reactflow-inspiration.md](file:///d:/GitCode/RF/rust-agent-flow/docs/rust-agent-flow/03-philosophy/reactflow-inspiration.md) | 更新 `resolve_endpoints` 引用为 `resolve_port` |
| [docs/rust-agent-flow/03-philosophy/progressive-disclosure.md](file:///d:/GitCode/RF/rust-agent-flow/docs/rust-agent-flow/03-philosophy/progressive-disclosure.md) | 更新 `resolve_endpoints` 示例代码 |
| [docs/rust-agent-flow/01-introduction/what-is-rust-agent-flow.md](file:///d:/GitCode/RF/rust-agent-flow/docs/rust-agent-flow/01-introduction/what-is-rust-agent-flow.md) | 更新 `resolve_endpoints` 引用 |
| [docs/rust-agent-flow/06-schema-system/node-schema-port.md](file:///d:/GitCode/RF/rust-agent-flow/docs/rust-agent-flow/06-schema-system/node-schema-port.md) | 增加 `PortSpec.fixed` 强约束说明 |

### 9.2 新增文档

- 建议新增 `docs/rust-agent-flow/07-geometry-layout/port-resolution.md`：统一描述 port side 解析策略（Phase 1 改动 1.6 的表格）+ 强弱约束模型

---

## 十、实施顺序与依赖

```
Phase 1 (P0, port_calc 统一)
  ├── 改动 1.1: port_calc.rs 重构
  ├── 改动 1.2: lib.rs re-export
  ├── 改动 1.3: resolve_port 增强
  ├── 改动 1.4: 删除 dead code
  └── 改动 1.5: registry 清理
        │
        ├──→ Phase 2 (P1, 参数清理)
        ├──→ Phase 3 (P1, Auto 断言)
        └──→ Phase 4 (P2, D6 混合 side)
                │
                └──→ Phase 5 (P2, D4/D5/D7 纵向循环体)
                        │
                        └──→ Phase 6 (P3, D8 Condition 对齐)

Phase 7 (P3, 文档同步) ← 跟随 Phase 1-6 完成
```

---

## 十一、验证方式

### 11.1 编译验证
```bash
cargo check --all
cargo check --package rust-agent-flow-core
cargo check --package rust-agent-flow-gpui
```

### 11.2 单元测试
```bash
cargo test --package rust-agent-flow-core
cargo test --package rust-agent-flow-gpui
```

**重点关注**：
- port_calc.rs 测试适配 `#[deprecated]`（`#[allow(deprecated)]`）
- 新增测试：浮动边 side 推导、fixed 防御性感知、混合 side 路径形态

### 11.3 手动验证（demo）
1. 横向/纵向布局切换，所有节点连线 side 正确
2. Loop 节点 loop_body/loop_in 强约束 Right/Left 不变
3. Condition 多出口均布正常
4. 浮动边（如有）side 按相对位置推导
5. 混合 side 边路径无穿节点、无扭曲

### 11.4 回归验证
- 边的 + 按钮位置正确
- 命中测试端口点击区域正确
- Loop 循环体回环边路径正确

---

## 十二、假设与决策

### 12.1 已决策

1. **统一方向**：以 gpui 层为主路径，core 层 port_calc.rs 降级为纯算法库（改动 1.1-1.5）
2. **保持节点自治架构**：多端口分布由节点 port_position 回调负责，不引入批量预计算层
3. **浮动边 side 策略**：改用 `compute_side_from_position`（相对位置），而非 `default_side`（布局方向）
4. **`resolve_endpoints` 废弃而非删除**：`#[deprecated]` 标记，保留纯算法函数

### 12.2 假设

1. 当前所有边都有 port_id（节点 schema 声明了端口），浮动边路径属防御性兜底，改动 1.3b 影响范围小
2. `default_src_side` / `default_dst_side` 参数在浮动边改用相对位置后，仅在节点不存在时兜底，仍需保留
3. Phase 5 的纵向布局 body 组定位策略需用户决策（选项 A vs B）

### 12.3 待决策（Phase 5）

- 纵向布局下 Loop 循环体 body 组的理想定位：
  - 选项 A：保持右侧纵向堆叠，优化回环边路径向右绕
  - 选项 B：改为下方横向排列，回环边向下绕（与横向布局对称）

---

## 十三、风险与缓解

### 13.1 浮动边 side 策略变化

**风险**：浮动边从 `default_side`（布局方向）改为 `compute_side_from_position`（相对位置），可能改变现有边的 side。

**缓解**：当前所有边都有 port_id，浮动边属防御性兜底，实际影响小。需验证 demo 中无浮动边。

### 13.2 `resolve_endpoints` 废弃的兼容性

**风险**：`resolve_endpoints` 被 lib.rs re-export，外部消费者（如有）可能依赖。

**缓解**：`#[deprecated]` 标记提供迁移提示，纯算法函数仍可用。当前仅文档引用，无实际外部消费者。

### 13.3 Phase 5 纵向布局策略不确定性

**风险**：D4/D5/D7 的改动方向依赖 body 组定位决策（选项 A vs B），未决策前无法细化。

**缓解**：Phase 5 标记为 P2，可在 Phase 1-4 完成后再决策。
