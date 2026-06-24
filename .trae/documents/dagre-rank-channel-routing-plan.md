# 基于 dagre rank 的通道分配 + 正交路由智能避障方案

## 概述

本方案解决两个问题：

1. **智能避障**：当前边路由算法（bezier/smoothstep）只考虑 src→dst 两点，不考虑中间节点，导致跨层边穿过中间 rank 的节点。方案利用 dagre 的 rank 信息，在中间 rank 的节点间隙中分配通道，生成避障正交路径。
2. **Loop 节点 + 按钮位置分析**：确认 Loop 节点未覆写 `plus_button_at_target`，导致 + 按钮聚集在 Loop 源端口侧，与 trait 文档注释的预期不符。

***

## 当前状态分析

### 1. dagre rank 信息：可用但未提取

* dagre 0.1.1 的 `NodeLabel` 结构体确认有 `pub rank: Option<i32>` 字段（通过 `cargo doc` 验证）

* [dagre.rs:116-127](file:///d:/GitCode/RF/rust-agent-flow/crates/core/src/layout/dagre.rs#L116-L127) 布局后只读取了 `label.x`、`label.y`、`label.width`、`label.height`，**未读取** **`label.rank`**

* [LayoutResult](file:///d:/GitCode/RF/rust-agent-flow/crates/core/src/layout/mod.rs#L22-L25) 只有 `positions: HashMap<NodeId, PointF>`，无 rank 字段

### 2. 避障函数：已实现但是死代码

* [edge\_path.rs:425](file:///d:/GitCode/RF/rust-agent-flow/crates/core/src/geometry/edge_path.rs#L425) `segment_intersects_rect` — 线段-矩形相交检测，从未被调用

* [edge\_path.rs:474](file:///d:/GitCode/RF/rust-agent-flow/crates/core/src/geometry/edge_path.rs#L474) `detour_around_rect` — 单障碍物 6 点正交绕行，从未被调用

* 两者在 [lib.rs:28](file:///d:/GitCode/RF/rust-agent-flow/crates/core/src/lib.rs#L28) 导出，但 gpui 渲染层无任何调用

### 3. 边渲染管线：无避障

* [edge\_view.rs:204-224](file:///d:/GitCode/RF/rust-agent-flow/crates/gpui/src/edge/edge_view.rs#L204-L224) `paint_edge_scaled` 直接根据 src/dst 端点计算路径，不考虑中间节点

* 路径算法（bezier/smoothstep/step/straight）均为两点路由，无障碍物感知

* 唯一的"避障"是 Loop 回环边的 `loop_back_path`（硬编码 U 型绕行）

### 4. Loop 节点 + 按钮位置：未应用目标侧设置

* [flow\_node.rs:125-134](file:///d:/GitCode/RF/rust-agent-flow/crates/gpui/src/node/flow_node.rs#L125-L134) trait 默认 `plus_button_at_target() -> false`

* **没有任何节点覆写此方法**，包括 Loop

* trait 文档注释明确说"Loop 应覆写返回 true"，但 [loop\_node.rs](file:///d:/GitCode/RF/rust-agent-flow/crates/gpui/src/builtin/loop_node.rs) 的 `impl IFlowNode` 未覆写

* Loop 端口位置（[loop\_node.rs:352-385](file:///d:/GitCode/RF/rust-agent-flow/crates/gpui/src/builtin/loop_node.rs#L352-L385)）：

  * `done`：右出（横向）/ 下出（纵向），位于 `node_mid_y`（y + 40）

  * `loop_body`：始终右出，位于 `body_mid_y`（y + 58）

  * 两个出口端口都在右侧，垂直距离仅 18px → + 按钮聚集

***

## 方案设计

### 第一部分：dagre rank 提取与存储

#### 改动 1：扩展 LayoutResult 增加 rank 信息

**文件**：[crates/core/src/layout/mod.rs](file:///d:/GitCode/RF/rust-agent-flow/crates/core/src/layout/mod.rs)

```rust
pub struct LayoutResult {
    pub positions: HashMap<NodeId, PointF>,
    /// dagre 分配的 rank（层号），用于避障路由。
    /// rank 从 0 开始，沿布局方向递增。
    pub ranks: HashMap<NodeId, i32>,
}
```

**原因**：通道分配算法需要知道每个节点在第几层，才能判断边是否跨越中间层。

#### 改动 2：在 dagre.rs 中提取 rank

**文件**：[crates/core/src/layout/dagre.rs](file:///d:/GitCode/RF/rust-agent-flow/crates/core/src/layout/dagre.rs)

在 [第 114-128 行](file:///d:/GitCode/RF/rust-agent-flow/crates/core/src/layout/dagre.rs#L114-L128) 的位置提取循环中，读取 `label.rank`：

```rust
let mut positions = std::collections::HashMap::new();
let mut ranks = std::collections::HashMap::new();
for (node_id, key) in &id_map {
    if let Some(label) = g.node(key) {
        if let (Some(x), Some(y)) = (label.x, label.y) {
            positions.insert(*node_id, PointF::new(...));
        }
        if let Some(rank) = label.rank {
            ranks.insert(*node_id, rank);
        }
    }
}
// ... 后处理保持不变 ...
LayoutResult { positions, ranks }
```

#### 改动 3：在 FlowEditorView 中缓存 rank 信息

**文件**：[crates/gpui/src/editor/flow\_editor.rs](file:///d:/GitCode/RF/rust-agent-flow/crates/gpui/src/editor/flow_editor.rs)

在 [relayout() 方法](file:///d:/GitCode/RF/rust-agent-flow/crates/gpui/src/editor/flow_editor.rs#L136-L155) 中，将 rank 存入新字段：

```rust
pub struct FlowEditorView {
    // ... 现有字段 ...
    /// 缓存的节点 rank（层号），由 relayout() 更新。
    /// 用于避障路由：判断边是否跨越中间层。
    cached_ranks: HashMap<NodeId, i32>,
}
```

```rust
pub(crate) fn relayout(&mut self) {
    // ... 现有逻辑 ...
    let result: LayoutResult = DagreLayout::new().layout(&self.graph, dir);
    for (node_id, pos) in &result.positions {
        if let Some(node) = self.graph.node_mut(*node_id) {
            node.position = *pos;
        }
    }
    self.cached_ranks = result.ranks;
    self.cached_body_groups = self.graph.loop_body_groups();
}
```

***

### 第二部分：通道分配 + 正交避障路由算法

#### 核心算法设计

**目标**：对于跨越 2+ 层的边（`dst.rank > src.rank + 1`），在中间层的节点间隙中找到通道，生成避障正交路径。

**算法流程**（以横向布局为例，纵向布局对称）：

```
对于每条边 (src → dst)：
  1. 获取 src.rank 和 dst.rank
  2. 如果 dst.rank <= src.rank + 1 → 使用当前路由（无中间层障碍）
  3. 如果 dst.rank > src.rank + 1（跨层边）：
     a. 对每个中间层 R (src.rank+1 到 dst.rank-1)：
        - 收集层 R 的所有节点，按 Y 坐标排序
        - 计算边自然路径（src→dst 直线）在层 R 的 Y 坐标
        - 在层 R 的节点间隙中找到离自然路径最近的通道 Y
        - 如果没有间隙（节点覆盖整个范围），在层 R 上方/下方绕行
     b. 生成正交路径：src → 通道1 → 通道2 → ... → dst
     c. 应用 round_corners 圆角化
```

**通道查找算法**（横向布局，找 Y 坐标）：

```
find_channel(rank_nodes, desired_y, margin):
  1. 将 rank_nodes 按 Y 排序
  2. 收集所有间隙：[node[i].bottom + margin, node[i+1].top - margin]
  3. 也考虑首尾间隙：(-∞, first.top - margin] 和 [last.bottom + margin, +∞)
  4. 在所有间隙中找到离 desired_y 最近的 Y 坐标
  5. 返回该 Y 坐标作为通道
```

#### 改动 4：实现通道分配路由函数

**文件**：[crates/core/src/geometry/edge\_path.rs](file:///d:/GitCode/RF/rust-agent-flow/crates/core/src/geometry/edge_path.rs)

新增函数：

```rust
/// 基于 dagre rank 的通道分配正交避障路由。
///
/// 对于跨层边（跨越 2+ 个 rank），在中间 rank 的节点间隙中分配通道，
/// 生成避障正交路径。对于相邻层边，回退到当前路由算法。
///
/// 参数：
/// - `src`, `dst`: 源/目标端口坐标
/// - `src_side`, `dst_side`: 源/目标端口侧
/// - `obstacles`: 中间层节点矩形列表（已排除 src 和 dst 节点），按 rank 分组
/// - `direction`: 布局方向
/// - `border_radius`: 圆角半径
///
/// 返回：正交路径点列表（已圆角化）
pub fn route_with_channels(
    src: PointF,
    dst: PointF,
    src_side: PortSide,
    dst_side: PortSide,
    obstacles_by_rank: &[Vec<RectF>],
    horizontal: bool,
    border_radius: f32,
) -> Vec<PointF> {
    // 1. 如果无中间层障碍，回退到 smoothstep
    if obstacles_by_rank.is_empty() || obstacles_by_rank.iter().all(|v| v.is_empty()) {
        return smoothstep_path(src, dst, src_side, dst_side, border_radius);
    }

    // 2. 对每个中间层，找到通道坐标
    let channels = find_channels(src, dst, obstacles_by_rank, horizontal);

    // 3. 生成正交路径：src → 各通道 → dst
    let raw_path = build_orthogonal_path(src, dst, src_side, dst_side, &channels, horizontal);

    // 4. 圆角化
    round_corners(&raw_path, border_radius)
}
```

**通道查找函数**：

```rust
/// 在中间层节点间隙中查找通道坐标。
///
/// 横向布局：返回 Y 坐标列表（每层一个通道 Y）
/// 纵向布局：返回 X 坐标列表
fn find_channels(
    src: PointF,
    dst: PointF,
    obstacles_by_rank: &[Vec<RectF>],
    horizontal: bool,
) -> Vec<f32> {
    const MARGIN: f32 = 30.0; // 通道与节点的安全间距

    let mut channels = Vec::with_capacity(obstacles_by_rank.len());

    for (i, rank_obstacles) in obstacles_by_rank.iter().enumerate() {
        if rank_obstacles.is_empty() {
            // 该层无节点，用自然路径坐标
            let t = (i + 1) as f32 / (obstacles_by_rank.len() + 1) as f32;
            let natural = if horizontal {
                src.y + (dst.y - src.y) * t
            } else {
                src.x + (dst.x - src.x) * t
            };
            channels.push(natural);
            continue;
        }

        // 计算自然路径在该层的坐标
        let t = (i + 1) as f32 / (obstacles_by_rank.len() + 1) as f32;
        let desired = if horizontal {
            src.y + (dst.y - src.y) * t
        } else {
            src.x + (dst.x - src.x) * t
        };

        // 按交叉轴坐标排序节点
        let mut sorted: Vec<(f32, f32)> = rank_obstacles
            .iter()
            .map(|r| {
                if horizontal {
                    (r.top(), r.bottom())
                } else {
                    (r.left(), r.right())
                }
            })
            .collect();
        sorted.sort_by(|a, b| a.0.partial_cmp(&b.0).unwrap());

        // 收集所有间隙
        let mut gaps: Vec<(f32, f32)> = Vec::new();
        // 首部间隙
        gaps.push((f32::MIN, sorted[0].0 - MARGIN));
        // 中间间隙
        for w in sorted.windows(2) {
            gaps.push((w[0].1 + MARGIN, w[1].0 - MARGIN));
        }
        // 尾部间隙
        gaps.push((sorted.last().unwrap().1 + MARGIN, f32::MAX));

        // 找到离 desired 最近的间隙，取间隙中心（或 desired，取在间隙内的值）
        let channel = gaps
            .iter()
            .map(|(lo, hi)| {
                let clamped = desired.max(*lo).min(*hi);
                let center = (*lo + *hi) * 0.5;
                // 优先用 desired（如果在间隙内），否则用间隙中心
                if clamped == desired {
                    desired
                } else {
                    center
                }
            })
            .min_by(|a, b| {
                (a - desired).abs()
                    .partial_cmp(&(b - desired).abs())
                    .unwrap()
            })
            .unwrap_or(desired);

        channels.push(channel);
    }

    channels
}
```

**正交路径构建函数**：

```rust
/// 根据通道坐标构建正交路径。
///
/// 横向布局（通道为 Y 坐标）：
///   src → (mid_x[i], src.y) → (mid_x[i], channel[i]) → (mid_x[i+1], channel[i]) → ...
///   其中 mid_x[i] 是相邻通道之间的 X 坐标
///
/// 纵向布局（通道为 X 坐标）：
///   src → (src.x, mid_y[i]) → (channel[i], mid_y[i]) → (channel[i], mid_y[i+1]) → ...
fn build_orthogonal_path(
    src: PointF,
    dst: PointF,
    src_side: PortSide,
    dst_side: PortSide,
    channels: &[f32],
    horizontal: bool,
) -> Vec<PointF> {
    if horizontal {
        build_orthogonal_horizontal(src, dst, src_side, dst_side, channels)
    } else {
        build_orthogonal_vertical(src, dst, src_side, dst_side, channels)
    }
}
```

#### 改动 5：在渲染层集成避障路由

**文件**：[crates/gpui/src/editor/rendering.rs](file:///d:/GitCode/RF/rust-agent-flow/crates/gpui/src/editor/rendering.rs)

修改 `EdgeRender` 枚举和 `render_edges` 方法，增加障碍物信息：

```rust
enum EdgeRender {
    Normal {
        src: PointF,
        dst: PointF,
        src_side: PortSide,
        dst_side: PortSide,
        edge_type: EdgeType,
        /// 中间层障碍物（按 rank 分组），用于通道分配路由
        obstacles_by_rank: Vec<Vec<RectF>>,
    },
    LoopBack { /* 不变 */ },
}
```

在 `render_edges` 中，为每条 Normal 边计算障碍物：

```rust
// 计算中间层障碍物
let obstacles_by_rank = compute_obstacles_by_rank(
    edge,
    &self.graph,
    &self.cached_ranks,
    &all_body_nodes,
    direction,
);
```

**新增函数** **`compute_obstacles_by_rank`**：

```rust
/// 计算边的中间层障碍物（按 rank 分组）。
///
/// 对于边 src→dst，收集所有 rank 在 (src.rank, dst.rank) 之间的节点
/// （排除 src、dst 自身和隐藏节点），按 rank 分组返回矩形列表。
fn compute_obstacles_by_rank(
    edge: &Edge,
    graph: &FlowGraph,
    ranks: &HashMap<NodeId, i32>,
    hidden_nodes: &HashSet<NodeId>,
    _direction: LayoutDirection,
) -> Vec<Vec<RectF>> {
    let src_rank = ranks.get(&edge.source).copied().unwrap_or(0);
    let dst_rank = ranks.get(&edge.target).copied().unwrap_or(0);

    // 不跨层或反向边：无中间层障碍
    if dst_rank <= src_rank + 1 {
        return Vec::new();
    }

    // 收集中间层节点
    let mut by_rank: HashMap<i32, Vec<RectF>> = HashMap::new();
    for node in graph.nodes() {
        if node.id == edge.source || node.id == edge.target {
            continue;
        }
        if hidden_nodes.contains(&node.id) {
            continue;
        }
        if let Some(&rank) = ranks.get(&node.id) {
            if rank > src_rank && rank < dst_rank {
                by_rank.entry(rank).or_default().push(node.bounds());
            }
        }
    }

    // 按 rank 排序输出
    let mut sorted_ranks: Vec<i32> = by_rank.keys().copied().collect();
    sorted_ranks.sort();
    sorted_ranks.into_iter().map(|r| by_rank.remove(&r).unwrap()).collect()
}
```

#### 改动 6：修改 paint\_edge\_scaled 支持避障路由

**文件**：[crates/gpui/src/edge/edge\_view.rs](file:///d:/GitCode/RF/rust-agent-flow/crates/gpui/src/edge/edge_view.rs)

修改 `paint_edge_scaled` 和 `EdgeRender::Normal` 的处理逻辑：

```rust
pub(crate) fn paint_edge_scaled(
    src: PointF,
    dst: PointF,
    src_side: PortSide,
    dst_side: PortSide,
    edge_type: EdgeType,
    obstacles_by_rank: &[Vec<RectF>],  // 新增参数
    horizontal: bool,                   // 新增参数
    scale: f32,
    offset: Point<Pixels>,
    color: Rgba,
    window: &mut Window,
) {
    let points = match edge_type {
        EdgeType::Straight => straight_path(src, dst),
        EdgeType::Bezier => {
            // Bezier 不支持避障，但如果有障碍物则降级为 smoothstep
            if obstacles_by_rank.iter().all(|v| v.is_empty()) {
                bezier_path(src, dst, src_side, dst_side, 0.5)
            } else {
                route_with_channels(src, dst, src_side, dst_side, obstacles_by_rank, horizontal, 12.0)
            }
        }
        EdgeType::Step => {
            if obstacles_by_rank.iter().all(|v| v.is_empty()) {
                step_path(src, dst, src_side, dst_side)
            } else {
                let raw = route_with_channels(src, dst, src_side, dst_side, obstacles_by_rank, horizontal, 0.0);
                raw // step 不圆角
            }
        }
        EdgeType::SmoothStep => {
            if obstacles_by_rank.iter().all(|v| v.is_empty()) {
                smoothstep_path(src, dst, src_side, dst_side, 12.0)
            } else {
                route_with_channels(src, dst, src_side, dst_side, obstacles_by_rank, horizontal, 12.0)
            }
        }
    };
    let is_bezier = edge_type == EdgeType::Bezier && points.len() == 4 && obstacles_by_rank.iter().all(|v| v.is_empty());
    paint_polyline(&points, is_bezier, false, scale, offset, color, window);
    paint_arrow(&points, is_bezier, scale, offset, color, window);
}
```

***

### 第三部分：Loop 节点主线出口 + 按钮位置修复

#### 分析结论

| 项                                | 现状                                           | 预期                    |
| -------------------------------- | -------------------------------------------- | --------------------- |
| `plus_button_at_target` trait 默认 | `false`（无参数）                                 | —                     |
| Loop 节点是否覆写                      | **否**，继承默认 `false`                           | 仅 `done` 主线出口为 `true` |
| `done` 边 + 按钮位置                  | Loop 右侧 (right+25, node\_mid\_y)             | done 目标左侧             |
| `loop_body` 边 + 按钮位置             | Loop 右侧 (right+25, body\_mid\_y)             | **保持源侧**（Loop 右侧）     |
| 两个按钮垂直距离                         | 仅 18px（node\_mid\_y=y+40, body\_mid\_y=y+58） | 主线按钮移走，仅剩 loop\_body  |

**问题**：Loop 的 `done` 和 `loop_body` 两个出口端口都在右侧，垂直距离仅 18px。两个 + 按钮聚集在 Loop 右侧。

**需求**：只将主线（`done`）出口的 + 按钮移到目标侧，`loop_body` 出口保持源侧不变。

**挑战**：当前 `plus_button_at_target(&self) -> bool` 是**无参数**方法，无法区分是哪个出口端口。需要改为**按端口判断**。

#### 改动 7：将 plus\_button\_at\_target 改为按端口判断

**文件**：[crates/gpui/src/node/flow\_node.rs](file:///d:/GitCode/RF/rust-agent-flow/crates/gpui/src/node/flow_node.rs)

修改 trait 方法签名，增加 `source_port` 参数：

```rust
/// 边「+」按钮是否应放置在目标节点一侧（而非默认的源节点一侧）。
///
/// **默认**：`false`（按钮在源节点出口附近）。
///
/// 某些结构化节点的特定出口端口位置特殊，按钮放在源端会与节点其他端口
/// 或回环边视觉冲突。此类节点可覆写此方法，按 `source_port` 判断是否
/// 将按钮放到目标端。
///
/// `source_port` 为边的源端口 ID（如 `"done"`、`"loop_body"`），`None` 表示
/// 无显式端口（使用默认端口）。
fn plus_button_at_target(&self, _source_port: Option<&str>) -> bool {
    false
}
```

#### 改动 8：Loop 节点按端口覆写

**文件**：[crates/gpui/src/builtin/loop\_node.rs](file:///d:/GitCode/RF/rust-agent-flow/crates/gpui/src/builtin/loop_node.rs)

在 `impl IFlowNode for LoopNode` 中添加：

```rust
/// Loop 节点的主线（`done`）出口 + 按钮放在目标节点侧。
///
/// Loop 的 `done` 和 `loop_body` 两个出口端口都在右侧，垂直距离仅 18px。
/// 将 `done`（主线出口）的 + 按钮移到目标节点侧，避免与 `loop_body`
/// 的 + 按钮聚集。`loop_body` 出口保持源侧（Loop 右侧），因为循环体
/// 节点就在 Loop 右侧，源侧按钮位置自然。
fn plus_button_at_target(&self, source_port: Option<&str>) -> bool {
    match source_port {
        Some("done") => true,
        _ => false,
    }
}
```

#### 改动 9：更新三处调用点传入 source\_port

**文件 1**：[crates/gpui/src/editor/rendering.rs](file:///d:/GitCode/RF/rust-agent-flow/crates/gpui/src/editor/rendering.rs)

`render_edge_plus_buttons`（第 339-345 行）：

```rust
let at_target = self
    .graph
    .node(edge.source)
    .and_then(|n| registry.get(&n.kind))
    .map(|fn_| fn_.plus_button_at_target(edge.source_port.as_deref()))
    .unwrap_or(false);
```

`render_plus_tooltip`（第 428-434 行）：同样修改。

**文件 2**：[crates/gpui/src/editor/hit\_test.rs](file:///d:/GitCode/RF/rust-agent-flow/crates/gpui/src/editor/hit_test.rs)

`hit_test_edge_plus`（第 195-201 行）：

```rust
let at_target = self
    .graph
    .node(edge.source)
    .and_then(|n| self.registry.get(&n.kind))
    .map(|fn_| fn_.plus_button_at_target(edge.source_port.as_deref()))
    .unwrap_or(false);
```

**效果**：

* `done` 边：+ 按钮移到 done 目标节点的入口端口侧（横向布局→左侧，纵向布局→上侧）

* `loop_body` 边：+ 按钮保持在 Loop 右侧（body\_mid\_y + 25px 偏移），因为循环体节点就在右侧

* 只移走主线按钮，不再聚集

***

### 第四部分：工具栏调用侧定制机制

#### 当前状态分析

工具栏完全硬编码在 [toolbar.rs](file:///d:/GitCode/RF/rust-agent-flow/crates/gpui/src/editor/toolbar.rs) 的 `render_toolbar` 方法中，11 个控件以链式 `.child()` 内联写死。调用侧（[demo/src/main.rs](file:///d:/GitCode/RF/rust-agent-flow/demo/src/main.rs)）仅调用 `FlowEditorView::new(graph, cx)`，无法注入自定义工具栏项。

数据源选择器同样是封闭枚举 `DataSource`（[data\_source.rs:14-22](file:///d:/GitCode/RF/rust-agent-flow/crates/gpui/src/editor/data_source.rs#L14-L22)），3 个预置流程写死在框架内，调用方无法添加业务自己的流程模板。

项目已有两个成熟的 trait + setter 注入扩展模式：

* `IFlowNode` trait → `NodeRegistry::register()` 注册

* `SyntaxService` trait → `FlowEditorView::set_syntax_service()` 注入

#### 改动 10：ToolbarProvider trait + 注入机制

**新增文件**：`crates/gpui/src/editor/toolbar_ext.rs`

```rust
use gpui::{AnyElement, App, Window};
use crate::theme::Theme;
use crate::i18n::Language;

/// 工具栏渲染上下文，供 ToolbarProvider 实现方构建 UI。
pub struct ToolbarCtx<'a> {
    pub window: &'a mut Window,
    pub cx: &'a mut App,
    pub theme: Theme,
    pub language: Language,
    pub scale_pct: i32,
}

/// 工具栏扩展点：调用方可实现此 trait 注入自定义工具栏项。
///
/// 自定义项在框架内置项**之后**渲染。实现方通过 `ToolbarCtx` 获取
/// 主题、语言等上下文，返回任意 GPUI 元素。
pub trait ToolbarProvider: Send + Sync {
    /// 返回自定义工具栏项（在框架内置项之后渲染）。
    fn items(&self, ctx: &mut ToolbarCtx) -> Vec<AnyElement>;
}
```

**文件**：[crates/gpui/src/editor/flow\_editor.rs](file:///d:/GitCode/RF/rust-agent-flow/crates/gpui/src/editor/flow_editor.rs)

在 `FlowEditorView` 增加字段和方法：

```rust
pub struct FlowEditorView {
    // ... 现有字段 ...
    /// 自定义工具栏提供者列表（在内置项之后渲染）。
    pub custom_toolbar: Vec<Arc<dyn ToolbarProvider>>,
}

impl FlowEditorView {
    pub fn new(graph: FlowGraph, _cx: &mut Context<Self>) -> Self {
        // ... 现有初始化 ...
        custom_toolbar: Vec::new(),
    }

    /// 添加自定义工具栏提供者。
    pub fn add_toolbar_provider(&mut self, provider: Arc<dyn ToolbarProvider>, cx: &mut Context<Self>) {
        self.custom_toolbar.push(provider);
        cx.notify();
    }
}
```

**文件**：[crates/gpui/src/editor/toolbar.rs](file:///d:/GitCode/RF/rust-agent-flow/crates/gpui/src/editor/toolbar.rs)

在 `render_toolbar` 末尾追加自定义项渲染：

```rust
// ====== 自定义工具栏项（调用侧注入）======
for provider in &self.custom_toolbar {
    let mut ctx = ToolbarCtx {
        window, cx, theme: self.theme, language: self.language,
        scale_pct: (self.viewport.scale * 100.0) as i32,
    };
    for item in provider.items(&mut ctx) {
        container = container.child(item);
    }
}
```

**文件**：[crates/gpui/src/editor/mod.rs](file:///d:/GitCode/RF/rust-agent-flow/crates/gpui/src/editor/mod.rs)

导出新模块：

```rust
pub(crate) mod toolbar_ext;
pub use toolbar_ext::{ToolbarProvider, ToolbarCtx};
```

**文件**：[crates/gpui/src/lib.rs](file:///d:/GitCode/RF/rust-agent-flow/crates/gpui/src/lib.rs)

公开导出：

```rust
pub use editor::{DataSource, FlowEditorView, ToolbarProvider, ToolbarCtx};
```

#### 改动 11：数据源选择器从框架移到调用侧

**目标**：将 `DataSource` 枚举从框架内移除，数据源选择器由调用侧通过 `ToolbarProvider` 注入。

**文件**：[crates/gpui/src/editor/data\_source.rs](file:///d:/GitCode/RF/rust-agent-flow/crates/gpui/src/editor/data_source.rs)

将 `DataSource` 枚举改为 trait：

```rust
/// 数据源 trait：调用方实现此 trait 提供流程模板。
pub trait DataSource: Send + Sync {
    /// 数据源唯一标识。
    fn id(&self) -> &str;
    /// 显示标签（可国际化）。
    fn label(&self, lang: Language) -> String;
    /// 转换为可编辑的流程图。
    fn to_graph(&self) -> FlowGraph;
}
```

**文件**：[crates/gpui/src/editor/flow\_editor.rs](file:///d:/GitCode/RF/rust-agent-flow/crates/gpui/src/editor/flow_editor.rs)

将 `data_source: DataSource` 字段改为：

```rust
/// 当前数据源（由调用侧注入）。
pub data_source: Option<Arc<dyn DataSource>>,
```

移除 `set_data_source` 方法中的硬编码逻辑，改为通用：

```rust
pub fn set_data_source(&mut self, ds: Arc<dyn DataSource>, cx: &mut Context<Self>) {
    self.graph = ds.to_graph();
    self.data_source = Some(ds);
    self.relayout();
    self.fit_view(cx);
}
```

**文件**：[crates/gpui/src/editor/toolbar.rs](file:///d:/GitCode/RF/rust-agent-flow/crates/gpui/src/editor/toolbar.rs)

移除工具栏中硬编码的数据源 Dropdown（第 310-337 行），改为由调用侧通过 `ToolbarProvider` 注入。

**文件**：[demo/src/main.rs](file:///d:/GitCode/RF/rust-agent-flow/demo/src/main.rs)

调用侧实现数据源选择器：

```rust
// 1. 定义业务数据源
struct AgentFlowDataSource;
impl DataSource for AgentFlowDataSource { ... }

struct DataPipelineDataSource;
impl DataSource for DataPipelineDataSource { ... }

// 2. 实现工具栏提供者，渲染数据源下拉菜单
struct DemoToolbarProvider {
    data_sources: Vec<Arc<dyn DataSource>>,
    current: usize,
}

impl ToolbarProvider for DemoToolbarProvider {
    fn items(&self, ctx: &mut ToolbarCtx) -> Vec<AnyElement> {
        vec![
            // 渲染数据源 DropdownMenu
            Button::new("demo-data-source")
                .icon(IconName::ALargeSmall)
                .dropdown_menu(move |menu, _, _| { ... })
                .into_any_element(),
        ]
    }
}

// 3. 注入到编辑器
let view = cx.new(|cx| {
    let mut editor = FlowEditorView::new(graph, cx);
    editor.add_toolbar_provider(Arc::new(DemoToolbarProvider { ... }), cx);
    editor.auto_layout(cx);
    editor
});
```

**效果**：

* 框架不再包含任何业务数据源定义

* 调用侧通过 `ToolbarProvider` 注入任意自定义工具栏项

* 数据源选择器由 demo 自己实现，可包含业务特定的流程模板

***

## 实施步骤

### Step 1: 扩展 LayoutResult 增加 rank（core 层）

* 修改 [layout/mod.rs](file:///d:/GitCode/RF/rust-agent-flow/crates/core/src/layout/mod.rs)：LayoutResult 增加 `ranks` 字段

* 修改 [dagre.rs](file:///d:/GitCode/RF/rust-agent-flow/crates/core/src/layout/dagre.rs)：提取 `label.rank`

* 更新 Default 实现

### Step 2: 缓存 rank 到 FlowEditorView（gpui 层）

* 修改 [flow\_editor.rs](file:///d:/GitCode/RF/rust-agent-flow/crates/gpui/src/editor/flow_editor.rs)：增加 `cached_ranks` 字段

* 在 `relayout()` 中填充 `cached_ranks`

* 在构造函数中初始化为空 HashMap

### Step 3: 实现通道分配路由算法（core 层）

* 在 [edge\_path.rs](file:///d:/GitCode/RF/rust-agent-flow/crates/core/src/geometry/edge_path.rs) 新增：

  * `route_with_channels()` — 主入口

  * `find_channels()` — 通道查找

  * `build_orthogonal_path()` — 正交路径构建

  * `build_orthogonal_horizontal()` / `build_orthogonal_vertical()` — 方向特定实现

* 在 [lib.rs](file:///d:/GitCode/RF/rust-agent-flow/crates/core/src/lib.rs) 导出新函数

* 添加单元测试

### Step 4: 集成避障路由到渲染层（gpui 层）

* 修改 [rendering.rs](file:///d:/GitCode/RF/rust-agent-flow/crates/gpui/src/editor/rendering.rs)：

  * `EdgeRender::Normal` 增加 `obstacles_by_rank` 字段

  * 新增 `compute_obstacles_by_rank()` 函数

  * `render_edges` 中为每条边计算障碍物

* 修改 [edge\_view.rs](file:///d:/GitCode/RF/rust-agent-flow/crates/gpui/src/edge/edge_view.rs)：

  * `paint_edge_scaled` 增加 `obstacles_by_rank` 和 `horizontal` 参数

  * 有障碍物时调用 `route_with_channels`，无障碍物时保持原逻辑

  * 临时连线（DrawingEdge）传空障碍物列表

### Step 5: Loop 主线 + 按钮位置修复（按端口判断）

* 修改 [flow\_node.rs](file:///d:/GitCode/RF/rust-agent-flow/crates/gpui/src/node/flow_node.rs)：`plus_button_at_target` 增加 `source_port: Option<&str>` 参数

* 修改 [loop\_node.rs](file:///d:/GitCode/RF/rust-agent-flow/crates/gpui/src/builtin/loop_node.rs)：覆写 `plus_button_at_target`，仅 `done` 端口返回 `true`

* 修改 [rendering.rs](file:///d:/GitCode/RF/rust-agent-flow/crates/gpui/src/editor/rendering.rs)：两处调用传入 `edge.source_port.as_deref()`

* 修改 [hit\_test.rs](file:///d:/GitCode/RF/rust-agent-flow/crates/gpui/src/editor/hit_test.rs)：调用传入 `edge.source_port.as_deref()`

### Step 6: 工具栏扩展机制（ToolbarProvider trait）

* 新增 [toolbar\_ext.rs](file:///d:/GitCode/RF/rust-agent-flow/crates/gpui/src/editor/toolbar_ext.rs)：定义 `ToolbarProvider` trait 和 `ToolbarCtx`

* 修改 [flow\_editor.rs](file:///d:/GitCode/RF/rust-agent-flow/crates/gpui/src/editor/flow_editor.rs)：增加 `custom_toolbar` 字段和 `add_toolbar_provider` 方法

* 修改 [toolbar.rs](file:///d:/GitCode/RF/rust-agent-flow/crates/gpui/src/editor/toolbar.rs)：`render_toolbar` 末尾追加自定义项渲染

* 修改 [mod.rs](file:///d:/GitCode/RF/rust-agent-flow/crates/gpui/src/editor/mod.rs) 和 [lib.rs](file:///d:/GitCode/RF/rust-agent-flow/crates/gpui/src/lib.rs)：导出新类型

### Step 7: 数据源选择器从框架移到调用侧

* 修改 [data\_source.rs](file:///d:/GitCode/RF/rust-agent-flow/crates/gpui/src/editor/data_source.rs)：`DataSource` 从枚举改为 trait

* 修改 [flow\_editor.rs](file:///d:/GitCode/RF/rust-agent-flow/crates/gpui/src/editor/flow_editor.rs)：`data_source` 字段改为 `Option<Arc<dyn DataSource>>`

* 修改 [toolbar.rs](file:///d:/GitCode/RF/rust-agent-flow/crates/gpui/src/editor/toolbar.rs)：移除硬编码数据源 Dropdown

* 修改 [demo/src/main.rs](file:///d:/GitCode/RF/rust-agent-flow/demo/src/main.rs)：实现 `DataSource` trait 和 `ToolbarProvider`，注入到编辑器

### Step 8: 验证

* `cargo build` 确保编译通过

* `cargo test -p rust-agent-flow-core` 运行 core 层测试

* `cargo test -p rust-agent-flow-gpui` 运行 gpui 层测试

* 手动验证：在 demo 中创建跨层边，确认不穿过中间节点

* 手动验证：Loop 节点 `done` 边的 + 按钮出现在目标侧，`loop_body` 边保持在源侧

* 手动验证：demo 中数据源选择器正常工作

***

## 假设与决策

### 假设

1. dagre 0.1.1 的 `NodeLabel.rank` 在 `layout()` 执行后会被填充为 `Some(rank_value)`（已通过 cargo doc 确认字段存在，需在运行时验证值非 None）
2. dagre 的 rank 从 0 开始沿布局方向递增（TB: 上→下，LR: 左→右）
3. 循环体节点（body\_nodes）的 rank 在 Loop 的 rank 和 done 目标的 rank 之间，会被正确识别为中间层障碍物

### 设计决策

1. **仅对跨层边（dst.rank > src.rank + 1）应用通道路由**：相邻层边无中间障碍，保持原路由算法（bezier/smoothstep）的视觉效果
2. **Bezier 有障碍物时降级为 smoothstep**：Bezier 是曲线，无法做正交避障；降级后使用通道分配 + 圆角正交
3. **通道查找优先用自然路径坐标**：在间隙内优先用直线插值坐标，减少不必要的绕行
4. **MARGIN = 30px**：通道与节点的安全间距，与现有 `detour_around_rect` 的 MARGIN 一致
5. **不修改 loop\_back\_path**：回环边已有专用路由算法，不在本方案范围内
6. **`plus_button_at_target`** **改为按端口判断**：增加 `source_port: Option<&str>` 参数，Loop 仅对 `done`（主线出口）返回 `true`，`loop_body` 保持 `false`。这样主线 + 按钮移到目标侧，循环体 + 按钮保持在 Loop 源侧
7. **ToolbarProvider 追加式扩展**：自定义项在内置项之后渲染，不破坏内置功能。与 `IFlowNode`/`SyntaxService` 的 trait + setter 模式一致
8. **DataSource 从枚举改为 trait**：框架不再包含业务数据源定义，调用方通过实现 trait 提供流程模板，通过 `ToolbarProvider` 注入选择器 UI

### 风险

1. **dagre rank 可能为 None**：如果 dagre 在某些边缘情况（如单节点图）不分配 rank，需回退到无障碍路由。代码中用 `unwrap_or(0)` 处理。
2. **通道查找可能找不到间隙**：如果某层节点密集覆盖整个交叉轴范围，通道会落在层外（上方/下方），路径会绕行到层外。这是可接受的行为。
3. **性能**：`compute_obstacles_by_rank` 对每条边遍历所有节点（O(V)），总复杂度 O(E×V)。对于典型流程图（<100 节点）可接受。如需优化，可在 relayout 时预计算 rank→nodes 索引。

