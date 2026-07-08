# Flow 端口 side 声明机制重构 - 实施计划（方案 A）

## 一、任务说明

基于已批准的《Flow 排版与连线缺陷分析（修正版）》第七节推荐方案，实施**方案 A：端口 side 声明机制重构**，解决三个根因缺陷：
- D1：`PortSide` 缺少强弱约束区分
- D2：`compute_edge_endpoints` 外部越权强制 body Top/Bottom
- D3：`derive_side_from_position` 用位置反推 side

对齐 ELK/yFiles 的 weak/strong port constraint 模型：端口 side 归属节点显式声明，外部布局层只读不写。

## 二、改动范围与边界

### 2.1 本次改动范围（gpui 渲染层路径）

聚焦实际用于渲染的 side 计算路径：
- `crates/core/src/graph/port.rs`（PortSide 枚举）
- `crates/core/src/schema/mod.rs`（PortSpec 增加 fixed 标志）
- `crates/gpui/src/node/flow_node.rs`（IFlowNode::port_position 签名变更）
- `crates/gpui/src/editor/ports.rs`（resolve_port 改造，移除 derive_side_from_position）
- `crates/gpui/src/editor/rendering/edge_geometry.rs`（移除强制 Top/Bottom）
- 9 个节点实现（port_position 返回值变更）
- `crates/gpui/src/editor/hit_test.rs`、`crates/gpui/src/editor/rendering/edges.rs`（调用点适配）

### 2.2 本次不改动（标注为范围外）

- **`crates/core/src/geometry/port_calc.rs`**：core 层的 `resolve_endpoints` 是独立的 side 计算路径（基于节点相对位置），逻辑比 gpui 层的 `derive_side_from_position` 更合理，但**未被 gpui 渲染层调用**（仅 lib.rs re-export + 自身测试）。统一两条路径属更大范围重构，不在本次方案 A 范围。本次保留不动，作为遗留问题。
- **算法层缺陷 D4-D8**（纵向布局循环体策略、混合 side 路径算法增强等）：属分析报告第八节方向 B/C/D，不在本次方向 A 范围。
- **文档文件**（docs/、.trae/documents/ 下的 .md）：本次仅改代码，文档同步留待后续。

## 三、详细改动清单

### 改动 1：PortSide 枚举保持不变，增加文档说明

**文件**：[crates/core/src/graph/port.rs](file:///d:/GitCode/RF/rust-agent-flow/crates/core/src/graph/port.rs) L15-L28

**内容**：PortSide 枚举本身不变（`Top/Right/Bottom/Left/Auto`），但更新文档注释说明强弱约束由 PortSpec.fixed 决定，而非 PortSide 自身。

**理由**：采用分析报告推荐的"选项 b"（PortSpec 增加 fixed 标志），比"选项 a"（PortSide 增加 Fixed 变体）更简洁，且不破坏现有 PortSide 的 match 分支。

```rust
/// Which side of a node a port sits on.
///
/// `Auto` means the framework computes the side based on layout direction
/// (weak constraint). Concrete sides (Top/Right/Bottom/Left) may be either
/// weak or strong, determined by `PortSpec.fixed`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default, Serialize, Deserialize)]
pub enum PortSide {
    Top,
    Right,
    Bottom,
    Left,
    #[default]
    Auto,
}
```

---

### 改动 2：PortSpec 增加 `fixed: bool` 字段

**文件**：[crates/core/src/schema/mod.rs](file:///d:/GitCode/RF/rust-agent-flow/crates/core/src/schema/mod.rs) L22-L52

**内容**：
1. PortSpec 增加 `fixed: bool` 字段，`#[serde(default)]` 默认 false
2. `PortSpec::new` 签名不变（fixed 默认 false，保持向后兼容）
3. 增加 `with_fixed(self, bool) -> Self` 构建器方法

```rust
pub struct PortSpec {
    pub id: PortId,
    pub direction: PortDirection,
    #[serde(default)]
    pub side: PortSide,
    /// Strong constraint flag: when true, `side` is fixed by the node
    /// implementation and must not be overridden by the layout layer.
    /// When false (default), `side` is a weak constraint (Auto lets the
    /// framework compute by layout direction).
    #[serde(default)]
    pub fixed: bool,
    pub label: Option<String>,
}

impl PortSpec {
    pub fn new(id: impl Into<PortId>, direction: PortDirection, side: PortSide) -> Self {
        Self { id: id.into(), direction, side, fixed: false, label: None }
    }

    pub fn with_label(mut self, label: impl Into<String>) -> Self {
        self.label = Some(label.into());
        self
    }

    /// Mark this port's side as a strong constraint (fixed by node impl,
    /// not overridable by layout layer).
    pub fn with_fixed(mut self, fixed: bool) -> Self {
        self.fixed = fixed;
        self
    }
}
```

**影响**：所有现有 `PortSpec::new(...)` 调用无需改动（fixed 默认 false）。只有需要强约束的端口（Loop 的 loop_body/loop_in）需追加 `.with_fixed(true)`。

---

### 改动 3：IFlowNode::port_position 签名变更，返回 (PointF, PortSide)

**文件**：[crates/gpui/src/node/flow_node.rs](file:///d:/GitCode/RF/rust-agent-flow/crates/gpui/src/node/flow_node.rs) L86-L106

**内容**：`port_position` 返回类型从 `Option<PointF>` 改为 `Option<(PointF, PortSide)>`，节点实现显式声明 side。

```rust
/// 自定义端口位置计算（可选，不依赖渲染上下文）。
///
/// 返回 `(位置, side)`。位置为逻辑坐标（节点 position 为左上角原点的绝对坐标），
/// side 为端口所在边（用于边路径算法的方向控制）。
///
/// **强弱约束语义**：
/// - 弱约束端口（PortSpec.fixed = false）：节点实现可返回 `None` 让框架按
///   布局方向计算，或返回 `(pos, side)` 显式指定（如 Condition 的多出口均布）
/// - 强约束端口（PortSpec.fixed = true）：节点实现必须返回 `(pos, side)`，
///   外部布局层只读不写，不覆盖此 side
///
/// 默认返回 `None`（用框架统一算法按 side 计算节点边缘中点）。
fn port_position(
    &self,
    _node: &Node,
    _port_id: &PortId,
    _layout: LayoutDirection,
) -> Option<(PointF, PortSide)> {
    None
}
```

---

### 改动 4：9 个节点实现的 port_position 适配新签名

**文件列表**（所有 `fn port_position` 实现）：
- [action.rs](file:///d:/GitCode/RF/rust-agent-flow/crates/gpui/src/builtin/action.rs) L204
- [adapter.rs](file:///d:/GitCode/RF/rust-agent-flow/crates/gpui/src/builtin/adapter.rs) L195
- [agent.rs](file:///d:/GitCode/RF/rust-agent-flow/crates/gpui/src/builtin/agent.rs) L207
- [condition.rs](file:///d:/GitCode/RF/rust-agent-flow/crates/gpui/src/builtin/condition.rs) L566
- [end.rs](file:///d:/GitCode/RF/rust-agent-flow/crates/gpui/src/builtin/end.rs) L201
- [loop_node.rs](file:///d:/GitCode/RF/rust-agent-flow/crates/gpui/src/builtin/loop_node.rs) L349
- [start.rs](file:///d:/GitCode/RF/rust-agent-flow/crates/gpui/src/builtin/start.rs) L220
- [variable.rs](file:///d:/GitCode/RF/rust-agent-flow/crates/gpui/src/builtin/variable.rs) L222
- [flow_node.rs](file:///d:/GitCode/RF/rust-agent-flow/crates/gpui/src/node/flow_node.rs) L99（默认实现，已在改动 3 处理）

**适配规则**：
- **简单节点**（action/adapter/agent/end/start/variable）：返回 `Some((pos, side))`，side 按 layout 决定（横向 In→Left/Out→Right，纵向 In→Top/Out→Bottom）。或保持返回 `None`（让框架回退到 side-based）。**推荐**：保持返回 `None`，减少改动量——这些节点的端口位置由 `port_position_by_side` + `default_side` 计算即可，无需显式声明。
- **Condition 节点**：返回 `Some((pos, side))`，side 按 layout 决定（横向 Out→Right，纵向 Out→Bottom）。Condition 的多出口均布需要显式位置，必须返回 Some。
- **Loop 节点**：返回 `Some((pos, side))`，关键改动如下：

**Loop 节点 port_position 改造**（[loop_node.rs L349-L382](file:///d:/GitCode/RF/rust-agent-flow/crates/gpui/src/builtin/loop_node.rs#L349-L382)）：

```rust
fn port_position(
    &self,
    node: &Node,
    port_id: &PortId,
    layout: LayoutDirection,
) -> Option<(PointF, PortSide)> {
    let left = node.position.x;
    let right = node.position.x + node.size.w;
    let top = node.position.y;
    let mid_x = node.position.x + node.size.w * 0.5;
    let node_mid_y = node.position.y + node.size.h * 0.5;
    let bottom = node.position.y + TITLE_H + BODY_H;
    let body_mid_y = node.position.y + TITLE_H + BODY_H * 0.5;

    match port_id.as_str() {
        // 主线 In→Done：随布局方向切换（弱约束语义，但显式返回 side）
        "in" => match layout {
            LayoutDirection::Horizontal => Some((PointF::new(left, node_mid_y), PortSide::Left)),
            LayoutDirection::Vertical => Some((PointF::new(mid_x, top), PortSide::Top)),
        },
        "done" => match layout {
            LayoutDirection::Horizontal => Some((PointF::new(right, node_mid_y), PortSide::Right)),
            LayoutDirection::Vertical => Some((PointF::new(mid_x, bottom), PortSide::Bottom)),
        },
        // 循环体支线：固定右/左（强约束，对应 PortSpec.fixed = true）
        "loop_body" => Some((PointF::new(right, body_mid_y), PortSide::Right)),
        "loop_in" => Some((PointF::new(left, body_mid_y), PortSide::Left)),
        _ => None,
    }
}
```

**Loop 节点 schema 改造**（[loop_node.rs L103-L106](file:///d:/GitCode/RF/rust-agent-flow/crates/gpui/src/builtin/loop_node.rs#L103-L106)）：

```rust
.with_port(PortSpec::new("in", PortDirection::In, PortSide::Auto))
.with_port(PortSpec::new("done", PortDirection::Out, PortSide::Auto))
// loop_body / loop_in 为强约束端口：固定右/左，外部布局层不可覆盖
.with_port(PortSpec::new("loop_body", PortDirection::Out, PortSide::Right).with_fixed(true))
.with_port(PortSpec::new("loop_in", PortDirection::In, PortSide::Left).with_fixed(true))
```

**Condition 节点 port_position 改造**：返回 `(pos, side)`，side 按 layout 决定。需读取 condition.rs L566-L620 现有实现后追加 side。

---

### 改动 5：resolve_port 改造，移除 derive_side_from_position

**文件**：[crates/gpui/src/editor/ports.rs](file:///d:/GitCode/RF/rust-agent-flow/crates/gpui/src/editor/ports.rs) L86-L143

**内容**：
1. `resolve_port` 使用 `port_position` 返回的 side，不再反推
2. 删除 `derive_side_from_position` 函数（L122-L143）
3. 更新模块文档（L1-L8）

```rust
/// 计算指定端口的精确位置（统一入口）。
///
/// 1. 先尝试 `IFlowNode::port_position`（自定义位置 + 显式 side）
///    - 返回 Some((pos, side)) 时直接使用，side 为节点显式声明
/// 2. 回退到 side-based 计算（schema side，Auto 按布局方向回退）
///
/// 返回 `(位置, side)`。side 用于边的路径算法（bezier/smoothstep 方向控制）。
pub(crate) fn resolve_port(
    node: &Node,
    port_id: &str,
    registry: &NodeRegistry,
    layout: LayoutDirection,
) -> (PointF, PortSide) {
    if let Some(flow_node) = registry.get(&node.kind) {
        let pid: PortId = port_id.to_string();
        let core_layout = match layout {
            LayoutDirection::Horizontal => rust_agent_flow::LayoutDirection::Horizontal,
            LayoutDirection::Vertical => rust_agent_flow::LayoutDirection::Vertical,
        };
        if let Some((pos, side)) = flow_node.port_position(node, &pid, core_layout) {
            return (pos, side);
        }
    }

    // 回退到 side-based
    let side = port_side(registry, &node.kind, port_id, layout);
    (port_position_by_side(node, side), side)
}
```

**删除**：`derive_side_from_position` 函数（L118-L143）及其文档注释。

---

### 改动 6：compute_edge_endpoints 移除强制 Top/Bottom

**文件**：[crates/gpui/src/editor/rendering/edge_geometry.rs](file:///d:/GitCode/RF/rust-agent-flow/crates/gpui/src/editor/rendering/edge_geometry.rs) L55-L108

**内容**：
1. 删除 `force_src_bottom` / `force_dst_top` 逻辑（L80-L105）
2. 统一使用 `resolve_port` 或 default_side
3. 更新文档注释（L55-L61）

```rust
/// 计算边的端点。
///
/// **端口策略**（强弱约束模型）：
/// - 端口 side 由节点声明（PortSpec + IFlowNode::port_position），外部只读不写
/// - 强约束端口（fixed=true）：side 由节点实现决定，本函数不覆盖
/// - 弱约束端口（fixed=false, side=Auto）：按布局方向回退到默认 side
///
/// 循环体节点的端口 side 不再被外部强制为 Top/Bottom。循环体的垂直子流语义
/// 由 Loop 节点的 loop_body/loop_in 强约束 side + 边路径算法协同保证。
pub(crate) fn compute_edge_endpoints(
    edge: &Edge,
    graph: &FlowGraph,
    registry: &NodeRegistry,
    layout: LayoutDirection,
    _body_nodes: &HashSet<NodeId>,  // 保留参数（调用方未变），但不再用于强制 side
    default_src_side: PortSide,
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
        None => (port_position_by_side(src_node, default_src_side), default_src_side),
    };

    let (dst, dst_side) = match edge.target_port.as_deref() {
        Some(pid) => resolve_port(dst_node, pid, registry, layout),
        None => (port_position_by_side(dst_node, default_dst_side), default_dst_side),
    };

    (src, src_side, dst, dst_side)
}
```

**注意**：`body_nodes` 参数保留（避免调用方签名变更），但加下划线前缀表示未使用。后续可清理调用方移除该参数（属范围外优化）。

---

### 改动 7：port_side 增加 fixed 感知（可选增强）

**文件**：[crates/gpui/src/editor/ports.rs](file:///d:/GitCode/RF/rust-agent-flow/crates/gpui/src/editor/ports.rs) L57-L73

**内容**：`port_side` 函数当前只看 `spec.side != Auto`，增加 fixed 感知：fixed=true 时即使 side=Auto 也应被节点 port_position 处理（不会走到这里），fixed=false + Auto 时按布局方向回退。

**实际影响**：当前逻辑 `if spec.side != PortSide::Auto { return spec.side; }` 已隐含正确——fixed=true 的端口 side 非 Auto（Loop 的 loop_body=Right, loop_in=Left），会返回 spec.side。无需改动 `port_side`，但建议增加断言或注释说明。

**决策**：不改 `port_side` 逻辑，仅增加注释说明 fixed 语义。

---

### 改动 8：调用点适配（hit_test.rs / edges.rs）

**文件**：
- [hit_test.rs](file:///d:/GitCode/RF/rust-agent-flow/crates/gpui/src/editor/hit_test.rs) L53, L182
- [edges.rs](file:///d:/GitCode/RF/rust-agent-flow/crates/gpui/src/editor/rendering/edges.rs) L61, L108, L197, L286

**内容**：这些调用点使用 `resolve_port` 和 `compute_edge_endpoints` 的返回值 `(PointF, PortSide)`。由于返回类型未变（仍是元组），**调用点无需改动**。需验证编译通过。

---

### 改动 9：PortSide::Auto 残留分支处理

**文件**：
- [ports.rs L52](file:///d:/GitCode/RF/rust-agent-flow/crates/gpui/src/editor/ports.rs#L52)（port_position_by_side 的 Auto 分支）
- [edge_path.rs L26](file:///d:/GitCode/RF/rust-agent-flow/crates/core/src/geometry/edge_path.rs#L26)（outward 的 Auto 分支）
- [hit_test.rs L210](file:///d:/GitCode/RF/rust-agent-flow/crates/gpui/src/editor/hit_test.rs#L210)
- [edges.rs L228, L315](file:///d:/GitCode/RF/rust-agent-flow/crates/gpui/src/editor/rendering/edges.rs#L228)

**内容**：这些是 `PortSide::Auto` 的 match 分支。改造后，Auto 应在 `resolve_port` / `port_side` 阶段已被解析为具体 side，不应流传到路径算法层。但作为防御性回退，保留 Auto 分支（回退到 Right）。

**决策**：不改这些分支，保留作为防御性回退。后续可增加 `debug_assert!(side != Auto)` 断言（属范围外优化）。

---

## 四、实施顺序

1. **改动 2**：PortSpec 增加 fixed 字段（core 层，无破坏性）
2. **改动 1**：PortSide 文档更新（core 层）
3. **改动 3**：IFlowNode::port_position 签名变更（gpui 层接口）
4. **改动 4**：9 个节点实现适配（含 Loop schema 的 with_fixed）
5. **改动 5**：resolve_port 改造 + 删除 derive_side_from_position
6. **改动 6**：compute_edge_endpoints 移除强制逻辑
7. **改动 7-9**：注释增强 + 验证编译
8. **验证**：cargo check + cargo test + 手动验证 Loop/Condition 节点渲染

## 五、验证方式

### 5.1 编译验证
```bash
cargo check --all
cargo check --package rust-agent-flow-gpui
```

### 5.2 单元测试
```bash
cargo test --package rust-agent-flow-core
cargo test --package rust-agent-flow-gpui
```

**重点关注**：
- `crates/core/src/geometry/port_calc.rs` 测试（未改动，应通过）
- `crates/core/src/geometry/edge_path.rs` 测试（loop_back_path 5 点结构）
- 新增测试：PortSpec.fixed 序列化/反序列化、Loop 端口 side 显式声明

### 5.3 手动验证（demo）

1. 横向布局下，Loop 节点 `loop_body` 右出、`loop_in` 左进，循环体节点 Top/Bottom 连线正常
2. 纵向布局下，Loop 节点 `in` 上进、`done` 下出，`loop_body`/`loop_in` 仍右/左（强约束不变）
3. Condition 节点多出口均布正常，横向右出、纵向下出
4. 切换布局方向，主线端口随方向切换，支线固定端口不变
5. 边路径无穿节点、无扭曲（D6 混合 side 路径问题属范围外，但需确认未恶化）

### 5.4 回归验证

- Loop 循环体回环边（EdgeRender::LoopBack）路径正确
- 边的 + 按钮位置正确（edges.rs 的 compute_edge_endpoints 调用）
- 命中测试（hit_test.rs）端口点击区域正确

## 六、假设与决策

### 6.1 已决策

1. **强弱约束实现**：采用 PortSpec.fixed: bool（选项 b），而非 PortSide 增加 Fixed 变体（选项 a）。理由：更简洁，不破坏现有 match 分支。
2. **PortSpec::new 签名不变**：fixed 默认 false，通过 with_fixed 构建器设置。所有现有调用无需改动。
3. **简单节点 port_position 保持返回 None**：action/adapter/agent/end/start/variable 节点的端口位置由 port_position_by_side + default_side 计算，无需显式声明（减少改动量）。
4. **body_nodes 参数保留**：compute_edge_endpoints 的 body_nodes 参数保留（加下划线前缀），避免调用方签名变更。后续清理属范围外。
5. **port_calc.rs 不改动**：core 层 resolve_endpoints 未被 gpui 渲染层使用，统一两条路径属更大范围重构，不在本次方案 A 范围。
6. **循环体节点 side 行为【用户确认】**：移除外部强制后，循环体节点保持弱约束 Auto，随主流方向（横向→Left/Right 水平编排，纵向→Top/Bottom 垂直编排）。接受横向布局下循环体从垂直编排变为水平编排的视觉变化。循环体的垂直编排需求留给 D4（Loop 布局策略）解决，而非修改循环体端口 side。

### 6.2 假设

1. 循环体节点（Action 等）保持弱约束 Auto，其端口 side 由 default_side 按布局方向计算。纵向布局下循环体节点 In→Top/Out→Bottom（由 default_side 计算，非外部强制）。
2. Loop 的 loop_body/loop_in 强约束 side（Right/Left）+ 循环体节点弱约束 side（纵向 Top/Bottom）形成混合 side 边（Right→Top），由现有边路径算法处理。D6（混合 side 路径算法增强）属范围外，本次不增强，但需验证未恶化。
3. PortSpec.fixed 字段的 serde default = false 保证旧图文件（无 fixed 字段）反序列化兼容。

### 6.3 遗留问题（范围外，供后续）

1. **port_calc.rs 与 gpui 层路径统一**：core 层 resolve_endpoints（基于节点相对位置）逻辑更合理，但未接入渲染层。统一需决策保留哪条路径。
2. **D6 混合 side 路径算法增强**：bezier_control / rf_get_points 对 Right→Top 等混合 side 的处理。
3. **body_nodes 参数清理**：compute_edge_endpoints 移除 body_nodes 参数 + 调用方适配。
4. **PortSide::Auto 防御性断言**：在路径算法层增加 debug_assert!(side != Auto)。
5. **文档同步**：docs/ 下的 PortSpec/PortSide 文档需同步 fixed 字段说明。

## 七、风险与缓解

### 7.1 循环体节点 side 行为变化

**风险**：移除 compute_edge_endpoints 的强制 Top/Bottom 后，循环体节点 side 改由 default_side 计算。纵向布局下仍为 Top/Bottom（一致），但横向布局下变为 Left/Right（原本强制 Top/Bottom）。

**影响分析**：横向布局下，循环体节点原本被强制 Top/Bottom（垂直子流），改造后变为 Left/Right（随主流方向）。这**改变了横向布局下循环体的视觉形态**——循环体节点不再垂直编排，而是水平编排。

**缓解**：此变化符合"端口 side 归属节点，外部不强制"的原则。若需保持横向布局下循环体垂直编排，应由循环体节点自身声明（或 Loop 节点通过边语义要求），但这属于 D4（align_loop_body_target 无视布局方向）的范围，不在本次方案 A 内。

**决策**：接受横向布局下循环体节点 side 行为变化（从强制 Top/Bottom 变为随主流 Left/Right）。若用户需保持原行为，需在循环体节点实现中显式声明（属后续 D4 范围）。

### 7.2 IFlowNode::port_position 签名破坏性变更

**风险**：返回类型从 `Option<PointF>` 改为 `Option<(PointF, PortSide)>`，所有实现必须适配。

**缓解**：共 9 个实现，已在改动 4 列出。简单节点可保持返回 None（无需适配返回值）。只有显式返回 Some 的节点（Loop/Condition）需适配。

### 7.3 序列化兼容性

**风险**：PortSpec 增加 fixed 字段，旧图文件反序列化可能失败。

**缓解**：`#[serde(default)]` 保证旧文件（无 fixed 字段）反序列化为 false。需验证 cargo test 通过。
