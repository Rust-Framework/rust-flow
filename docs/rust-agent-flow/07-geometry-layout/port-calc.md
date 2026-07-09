# 端口 side 解析与端点计算

边路径算法需要每条边两端的精确坐标与方向（side）。本模块负责把「Node + 可选端口」的抽象引用解析成 `(PointF, PortSide)` 对，是边渲染的前置步骤。

## 两条路径的历史与统一

### 旧路径：`resolve_endpoints`（已废弃）

历史上 core 层提供 `resolve_endpoints` 批量计算所有边的端点。**该函数已废弃**（`#[deprecated]`），原因：

1. **不感知 `PortSpec.fixed`**（强弱约束）：`resolve_side` 只看 `spec.side != Auto`，无法区分强约束端口（fixed=true，如 Loop 的 `loop_body`/`loop_in`）和弱约束端口
2. **不支持节点 `port_position` 回调**：Condition 多出口、Loop 强约束端口等结构化节点依赖 `IFlowNode::port_position` 回调返回精确位置和 side，`resolve_endpoints` 只能基于 schema 的 PortSpec 工作，无法调用此回调

### 新路径：gpui 层 `resolve_port`（当前使用）

渲染层使用 `crates/gpui/src/editor/ports.rs` 的 `resolve_port` + `edge_geometry.rs` 的 `compute_edge_endpoints`，正确处理强弱约束和节点回调：

```rust
// resolve_port 优先级：
// 1. IFlowNode::port_position 回调（节点显式声明位置和 side）
// 2. port_side：schema spec.fixed → spec.side（强约束）
//                spec.side != Auto → spec.side（节点声明）
//                spec.side == Auto → default_side（按布局方向，弱约束）
```

**浮动边**（无 port_id）使用 `compute_side_from_position` 按节点相对位置推导 side，而非按布局方向。

## 统一后的 side 解析策略

| 场景 | side 来源 | 实现位置 |
|------|----------|----------|
| 有 port_id + 节点实现 port_position | 节点回调返回的 side | `resolve_port` → `flow_node.port_position` |
| 有 port_id + spec.fixed=true | spec.side（强约束） | `port_side` 防御性分支 |
| 有 port_id + spec.side != Auto | spec.side（节点声明） | `port_side` |
| 有 port_id + spec.side == Auto | `default_side`（按布局方向，弱约束） | `port_side` → `default_side` |
| 无 port_id（浮动边） | `compute_side_from_position`（按节点相对位置） | `compute_edge_endpoints` |

### 循环体节点的布局上下文

循环体节点（body 节点）由 `align_loop_body_target` 纵向堆叠在 Loop 节点右侧，
构成一个"纵向子流"。虽然主布局方向可能是 Horizontal，但 body 节点的端口
必须按 Vertical 方向解析（In→Top, Out→Bottom），否则端口方向与节点排布
不一致。

**实现方式**：`compute_edge_endpoints` 接收 `body_nodes` 集合，对 body 节点
传入 `LayoutDirection::Vertical` 作为有效布局方向。这不是覆写 side——节点
自身的 `port_position` 回调仍决定最终 side：

- **fixed 端口**（如 Loop 的 loop_body/loop_in）：回调忽略 layout，返回固定 side
- **Auto 端口**（如 Action 的 in/out）：回调按 Vertical 返回 Top/Bottom

同样，节点渲染（`render_nodes`）、端口命中测试（`hit_test`）、边命中测试
均通过 `cached_all_body_nodes` 判断 body 节点，使用 Vertical 布局上下文。


## 保留的纯算法函数

`resolve_endpoints` 虽已废弃，但以下纯算法函数仍作为可复用工具保留（`pub`）：

### compute_side_from_position：相对位置推导

```rust
pub fn compute_side_from_position(self_center: PointF, other_center: PointF) -> PortSide {
    let dx = other_center.x - self_center.x;
    let dy = other_center.y - self_center.y;
    if dx.abs() >= dy.abs() {
        if dx >= 0.0 { PortSide::Right } else { PortSide::Left }
    } else if dy >= 0.0 { PortSide::Bottom } else { PortSide::Top }
}
```

比较 `dx`/`dy` 绝对值，取大者所在轴：水平轴主导则选 Left/Right，垂直轴主导则选 Top/Bottom。用于浮动边（无 port_id）的 side 推导。

### distribute_on_side：同侧多端口均匀分布

```rust
pub fn distribute_on_side(
    bounds: RectF,
    side: PortSide,
    dir: PortDirection,
    has_opposite: bool,
    count: usize,
) -> Vec<PointF>
```

当某个 (node, side) 同时有 In 和 Out 端口时（`has_opposite=true`），两者各占半边避免重叠：

```
同侧有 In+Out：               同侧只有 Out：
┌─────────┐ ← Out 上半        ┌─────────┐
│         │ ● out1            │         │ ● out1
│         │ ● out2            │         │ ● out2
│  node   │ ─── 分界          │  node   │ ● out3
│         │ ● in1             │         │
│         │ ● in2             │         │
└─────────┐ ← In 下半          └─────────┘
```

可被节点 `port_position` 回调调用，实现同侧多端口的均匀分布。

### point_on_side：边上的绝对坐标

```rust
pub fn point_on_side(bounds: RectF, side: PortSide, t: f32, outward: f32) -> PointF
```

`t ∈ [0,1]` 沿 side 参数化位置，`outward` 像素外移避免边压在节点边界上。

## 与边路径算法的衔接

gpui 渲染层通过 `compute_edge_endpoints` 获取端点后，直接喂给边路径算法：

```rust
let (src, src_side, dst, dst_side) = compute_edge_endpoints(edge, &graph, &registry, layout, ...);
let points = match edge.edge_type {
    EdgeType::Bezier => bezier_path(src, dst, src_side, dst_side, 0.5),
    EdgeType::Straight => straight_path(src, dst),
    EdgeType::Step => step_path(src, dst, src_side, dst_side),
    EdgeType::SmoothStep => smoothstep_path(src, dst, src_side, dst_side, 12.0),
};
```

回环边（`target_port == "loop_in"`）走 `loop_back_path`，传入循环体联合包围盒作为 `node_bounds`。

下一节：[Dagre 布局引擎](dagre-layout.md)
