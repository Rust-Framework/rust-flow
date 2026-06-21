# 连线算法补充计划：bezier / straight / step / smoothstep

> 补充计划，基于 `gpui-flow-designer-phase2-7-impl.md` Phase 2，聚焦连线算法的渲染层收敛与编译修复。

## 一、现状分析

### Core 层（已完成，无需改动）

| 文件 | 内容 | 状态 |
|---|---|---|
| [edge_path.rs](file:///e:/GitCode/RF/rust-agent-flow/crates/core/src/geometry/edge_path.rs) | 5 个路径函数 `straight_path`/`bezier_path`/`step_path`/`smoothstep_path`/`loop_back_path`，含单元测试 | ✅ 完整 |
| [edge.rs](file:///e:/GitCode/RF/rust-agent-flow/crates/core/src/graph/edge.rs) | `EdgeType` 枚举（4 变体 Bezier/Straight/Step/SmoothStep）+ `EdgeKind`（Normal/LoopBack），serde 序列化 | ✅ 完整 |
| [lib.rs](file:///e:/GitCode/RF/rust-agent-flow/crates/core/src/lib.rs) | re-export 全部 5 个路径函数 | ✅ 完整 |

**结论：4 种连线算法的核心数学实现已 100% 完成，且有测试覆盖。**

### GPUI 渲染层（存在问题）

| 文件 | 问题 |
|---|---|
| [edge_view.rs](file:///e:/GitCode/RF/rust-agent-flow/crates/gpui/src/edge/edge_view.rs) | ✅ API 正确（已修复），但 `paint_polyline`/`paint_arrow` 为私有，无法被 editor 复用 |
| [flow_editor.rs](file:///e:/GitCode/RF/rust-agent-flow/crates/gpui/src/editor/flow_editor.rs) | ❌ 6 类编译错误 + 与 EdgeView 代码重复 |
| [node/view.rs](file:///e:/GitCode/RF/rust-agent-flow/crates/gpui/src/node/view.rs#L61) | ❌ `Styled` 导入错误 |
| [panel/mod.rs](file:///e:/GitCode/RF/rust-agent-flow/crates/gpui/src/panel/mod.rs#L34) | ❌ `Styled` 导入错误 |

### flow_editor.rs 具体编译错误清单

1. **L273, L304**：`use gpui_component::styled::Styled;` → `gpui_component::styled` 模块私有，应改为 `use gpui::Styled;`
2. **L233**：canvas paint 闭包 3 参数 `move |_bounds, window, _cx|` → GPUI 要求 4 参数 `move |_bounds, _size, window, _cx|`
3. **L373**：`PathBuilder::new()` 不存在 → `PathBuilder::stroke(px(1.5))`
4. **L376**：`path.curve_to(p1, p2, p3)` 是二次贝塞尔（2 控制点）→ 三次贝塞尔应用 `path.cubic_bezier_to(to, ctrl_a, ctrl_b)` 即 `cubic_bezier_to(points[3], points[1], points[2])`
5. **L382-383**：`let path = path.build(); window.paint_path(1.5, &path, color);` → `build()` 返回 `Result`，`paint_path` 只接收 2 参数（path by value + color）。应为 `if let Ok(path) = path.build() { window.paint_path(path, gpui::black()); }`
6. **L414, L419-420**：同上，箭头绘制 `PathBuilder::new()` → `PathBuilder::fill()`，build/paint_path 同样修复

### 代码重复问题

`flow_editor.rs` 的 `paint_edge_direct`（L351-386）+ `paint_arrow_direct`（L388-420）与 `edge_view.rs` 的 `paint_polyline`（L70-87）+ `paint_arrow`（L90-123）**逻辑完全相同**，仅因为 `edge_view` 模块内函数为私有而被迫复制。

这违反用户要求"禁止堆叠代码实现目标"和"用最简单的代码实现复杂逻辑"。

## 二、决策

### 决策 1：收敛渲染入口，消除重复

**方案**：将 `edge_view.rs` 中的 `paint_polyline` 和 `paint_arrow` 提升为 `pub(crate)`，再在 `edge/mod.rs` 暴露一个统一入口 `paint_edge(src, dst, src_side, dst_side, edge_type, window)`，内部计算点位后调用 `paint_polyline` + `paint_arrow`。

**收益**：
- 删除 `flow_editor.rs` 中 70 行重复代码（`paint_edge_direct` + `paint_arrow_direct`）
- `EdgeView::into_element` 与 `flow_editor::render_edges` 共用同一渲染路径
- 未来 Phase 7 接入 `LoopBack` 只需改一处

### 决策 2：DrawingEdge 使用可配置默认边类型

`flow_editor.rs` L225 硬编码 `EdgeType::Bezier`。改为从 `FlowEditorView.default_edge_type` 读取，默认值 `EdgeType::Bezier`（与 ReactFlow 一致）。这为后续 UI 切换连线风格留出接口，但**当前不实现 UI 切换**（避免过度设计）。

### 决策 3：Styled 导入统一修复

3 处 `use gpui_component::styled::Styled;` → `use gpui::Styled;`（`flow_editor.rs` L273/L304、`node/view.rs` L61、`panel/mod.rs` L34）。

## 三、实施步骤

### 步骤 1：edge 模块暴露统一渲染入口

**文件**：[crates/gpui/src/edge/edge_view.rs](file:///e:/GitCode/RF/rust-agent-flow/crates/gpui/src/edge/edge_view.rs)

1. 将 `paint_polyline` 和 `paint_arrow` 的可见性从私有改为 `pub(crate)`：
   ```rust
   pub(crate) fn paint_polyline(points: &[PointF], is_bezier: bool, window: &mut Window) { ... }
   pub(crate) fn paint_arrow(points: &[PointF], window: &mut Window) { ... }
   ```

2. 新增统一入口函数 `paint_edge`（放在 `edge_view.rs` 末尾）：
   ```rust
   /// 统一边渲染入口：计算路径点 + 绘制折线 + 绘制箭头。
   /// 供 EdgeView::into_element 和 FlowEditorView::render_edges 共用。
   pub(crate) fn paint_edge(
       src: PointF,
       dst: PointF,
       src_side: PortSide,
       dst_side: PortSide,
       edge_type: EdgeType,
       window: &mut Window,
   ) {
       let points = match edge_type {
           EdgeType::Straight => straight_path(src, dst),
           EdgeType::Bezier => bezier_path(src, dst, src_side, dst_side, 0.5),
           EdgeType::Step => step_path(src, dst, src_side, dst_side),
           EdgeType::SmoothStep => smoothstep_path(src, dst, src_side, dst_side, 8.0),
       };
       let is_bezier = edge_type == EdgeType::Bezier && points.len() == 4;
       paint_polyline(&points, is_bezier, window);
       paint_arrow(&points, window);
   }
   ```

3. `EdgeView::into_element` 改为调用 `paint_edge`（消除 `points()` 方法的内联重复）：
   ```rust
   pub fn into_element(self) -> impl IntoElement {
       let (src, dst, src_side, dst_side, edge_type) =
           (self.src, self.dst, self.src_side, self.dst_side, self.edge_type);
       canvas(
           |bounds, _window, _cx| bounds.size,
           move |_bounds, _size, window, _cx| {
               paint_edge(src, dst, src_side, dst_side, edge_type, window);
           },
       )
   }
   ```
   保留 `points()` 方法（`pub(crate)`），供未来命中测试复用。

**文件**：[crates/gpui/src/edge/mod.rs](file:///e:/GitCode/RF/rust-agent-flow/crates/gpui/src/edge/mod.rs)

更新为：
```rust
//! Edge 模块：边渲染组件，支持 4 种连线算法 + 箭头。

mod edge_view;

pub use edge_view::EdgeView;
pub(crate) use edge_view::paint_edge;
```

### 步骤 2：flow_editor.rs 收敛渲染 + 修复编译

**文件**：[crates/gpui/src/editor/flow_editor.rs](file:///e:/GitCode/RF/rust-agent-flow/crates/gpui/src/editor/flow_editor.rs)

**改动 A**：`render_edges` 使用 `crate::edge::paint_edge`，canvas 闭包改为 4 参数：

```rust
fn render_edges(&self) -> impl IntoElement {
    let edges: Vec<(PointF, PointF, PortSide, PortSide, EdgeType)> = self
        .graph
        .edges()
        .map(|edge| {
            let src = self.graph.node(edge.source).map(|n| n.center()).unwrap_or_default();
            let dst = self.graph.node(edge.target).map(|n| n.center()).unwrap_or_default();
            let src_screen = self.viewport.to_screen(src);
            let dst_screen = self.viewport.to_screen(dst);
            (src_screen, dst_screen, PortSide::Right, PortSide::Left, edge.edge_type)
        })
        .collect();

    let default_edge_type = self.default_edge_type;
    let drawing = match &self.interaction {
        InteractionState::DrawingEdge { from_node, current, .. } => {
            self.graph.node(*from_node).map(|n| {
                let src = self.viewport.to_screen(n.center());
                let dst = self.viewport.to_screen(*current);
                (src, dst, PortSide::Right, PortSide::Left, default_edge_type)
            })
        }
        _ => None,
    };

    canvas(
        |bounds, _window, _cx| bounds.size,
        move |_bounds, _size, window, _cx| {
            for (src, dst, src_side, dst_side, edge_type) in &edges {
                crate::edge::paint_edge(*src, *dst, *src_side, *dst_side, *edge_type, window);
            }
            if let Some((src, dst, src_side, dst_side, edge_type)) = drawing {
                crate::edge::paint_edge(src, dst, src_side, dst_side, edge_type, window);
            }
        },
    )
}
```

**改动 B**：`FlowEditorView` 增加 `default_edge_type` 字段：

```rust
pub struct FlowEditorView {
    pub graph: FlowGraph,
    pub viewport: Viewport,
    pub interaction: InteractionState,
    pub registry: Arc<NodeRegistry>,
    pub selected: Option<NodeId>,
    pub default_edge_type: EdgeType,
}
```

`new()` 中初始化 `default_edge_type: EdgeType::Bezier`。

**改动 C**：删除 `paint_edge_direct`（L351-386）和 `paint_arrow_direct`（L388-420）两个函数。

**改动 D**：修复 `Styled` 导入（L273, L304）：
```rust
use gpui::Styled;  // 替换 use gpui_component::styled::Styled;
```

### 步骤 3：修复 node/view.rs 和 panel/mod.rs 的 Styled 导入

**文件**：[crates/gpui/src/node/view.rs](file:///e:/GitCode/RF/rust-agent-flow/crates/gpui/src/node/view.rs#L61)

L61：`use gpui_component::styled::Styled;` → `use gpui::Styled;`

**文件**：[crates/gpui/src/panel/mod.rs](file:///e:/GitCode/RF/rust-agent-flow/crates/gpui/src/panel/mod.rs#L34)

L34：`use gpui_component::styled::Styled;` → `use gpui::Styled;`

### 步骤 4：验证 minimal_demo 覆盖 4 种边类型

**文件**：[crates/gpui/examples/minimal_demo.rs](file:///e:/GitCode/RF/rust-agent-flow/crates/gpui/examples/minimal_demo.rs)

检查 demo 是否展示 4 种边类型。若当前只用了 Bezier，补充 4 条边分别使用 4 种类型，确保编译和渲染均正确。具体：
- 3 个节点（Start → Action → End）已有
- 2 条边改为：第 1 条 `EdgeType::Bezier`，第 2 条 `EdgeType::SmoothStep`
- 再加 2 个节点 + 2 条边（`Straight`、`Step`）以覆盖全部 4 种

**若 demo 改动过大则保持现状**，仅确保 `EdgeType` 4 变体在代码中可编译即可（核心算法已有单元测试覆盖）。

## 四、验证步骤

1. **编译验证**：
   ```powershell
   cargo check -p rust-agent-flow-gpui --features demo --example minimal_demo
   ```
   预期：0 错误。

2. **Core 层测试**（确认算法未受影响）：
   ```powershell
   cargo test -p rust-agent-flow --lib geometry::edge_path
   ```
   预期：4 个测试全过（straight_two_points / bezier_returns_four_points / step_opposite_horizontal / smoothstep_more_points_than_step）。

3. **Clippy**（可选）：
   ```powershell
   cargo clippy -p rust-agent-flow-gpui --features demo --example minimal_demo -- -D warnings
   ```

## 五、不改动的部分（避免过度设计）

- **不实现 UI 切换连线风格**：`default_edge_type` 字段仅作为内部配置点，不暴露到属性面板或工具栏。等用户明确需要时再加。
- **不接入 `EdgeKind::LoopBack` 渲染**：这是 Phase 7 的任务，本次只做 4 种普通边算法的渲染收敛。
- **不接入 `resolve_endpoints` 端口计算**：这是 Phase 4 的任务，本次 `render_edges` 仍用节点中心 + Right/Left 硬编码。
- **不改 `EdgeType` 枚举**：4 变体已满足需求，不增加 catmull 等其他算法。
- **不改 core 层**：`edge_path.rs` 算法实现完整且有测试，不动。

## 六、风险与缓解

| 风险 | 缓解 |
|---|---|
| `paint_edge` 提升可见性后，模块边界变模糊 | 用 `pub(crate)` 限制在 crate 内，不污染外部 API |
| 删除 `paint_edge_direct` 后，单 canvas 批量绘制性能是否受影响 | 不受影响：`paint_edge` 仍是同步函数调用，在同一 canvas paint 闭包内循环调用 |
| `default_edge_type` 字段增加后，序列化/反序列化是否受影响 | 不受影响：该字段只在 View 上，不在 `FlowGraph`/`Edge` 模型上 |
