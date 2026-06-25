# 流程图数据模型

## 三层抽象

rust-agent-flow 的图模型由三层抽象构成：

```
FlowGraph（图容器）
  ├─ Node（节点）  ← NodeId (slotmap key)
  │    └─ NodeData = serde_json::Value（业务数据）
  └─ Edge（边）    ← EdgeId (slotmap key)
       ├─ source / target: NodeId
       ├─ source_port / target_port: Option<PortId>
       ├─ edge_type: Bezier/Straight/Step/SmoothStep
       └─ kind: Normal/LoopBack
```

## FlowGraph：图容器

`FlowGraph` 用两个 `SlotMap` 存储节点和边，外加版本计数器：

```rust
pub struct FlowGraph {
    nodes: SlotMap<NodeId, Node>,
    edges: SlotMap<EdgeId, Edge>,
    version: u64, // 结构变更时递增
}
```

**slotmap 的意义**：节点/边的 ID 是稳定键，删除一个节点不会使其他节点的 ID 失效（对比 Vec 索引会移位）。这保证了 `NodeId`/`EdgeId` 在整个生命周期可安全传递与缓存。

**版本计数器**：任何结构性变更（增删节点/边、修改位置）都会 `version = wrapping_add(1)`。用于失效缓存的几何数据（如 `PortResolver`），避免使用过期的端口位置。

## Node：节点

```rust
pub struct Node {
    pub id: NodeId,
    pub kind: NodeKind,       // String，匹配 IFlowNode
    pub data: NodeData,       // serde_json::Value
    pub position: PointF,     // 逻辑坐标左上角
    pub size: SizeF,
}
```

- `kind` 是 `String` 而非枚举——支持任意自定义节点类型，无需改 core
- `data` 是自由 JSON——具体字段由 `NodeSchema.fields` 声明约束
- `position`/`size` 在布局后由 dagre 填充

## Edge：边

```rust
pub struct Edge {
    pub id: EdgeId,
    pub source: NodeId,
    pub source_port: Option<PortId>,
    pub target: NodeId,
    pub target_port: Option<PortId>,
    pub edge_type: EdgeType,   // Bezier/Straight/Step/SmoothStep
    pub kind: EdgeKind,        // Normal/LoopBack
}
```

端口是**可选的**——`None` 表示用默认端口（框架自动推导方向）。结构化节点（Condition/Loop）必须指定端口：

```rust
// Condition 的 if_0 出口
edge.source_port = Some("if_0".to_string());

// Loop 的回环边
edge.target_port = Some("loop_in".to_string());
edge.kind = EdgeKind::LoopBack;
```

## Port：端口

端口通过 `PortSpec` 声明（在 NodeSchema 中），运行时用 `PortId`（String）引用：

```rust
pub struct PortSpec {
    pub id: PortId,             // String
    pub direction: PortDirection, // In / Out
    pub side: PortSide,         // Top/Right/Bottom/Left/Auto
    pub label: Option<String>,
}
```

`PortSide::Auto` 让框架根据节点相对位置自动推导方向——大多数节点只需声明 Auto。

## 数据与视图的分离

图模型只存数据，**不含渲染逻辑**。渲染由 gpui 层的 `IFlowNode::get_view` 负责：

```
Node (data) ──→ IFlowNode (by kind) ──→ AnyElement (view)
```

这意味着：

- 同一图可被不同渲染层呈现（如未来的 egui 版本）
- 节点视图逻辑变更不影响图数据
- 图数据可序列化，视图不可

## 增删改查 API

```rust
// 节点
graph.add_node(kind, data) -> NodeId
graph.add_node_with_size(kind, data, size) -> NodeId
graph.remove_node(id) -> Option<Node>     // 同时删除关联边
graph.node(id) -> Option<&Node>
graph.node_mut(id) -> Option<&mut Node>   // 触发 version++
graph.nodes() -> impl Iterator<Item = &Node>

// 边
graph.add_edge(edge) -> EdgeId
graph.remove_edge(id) -> Option<Edge>
graph.out_edges(node) -> impl Iterator<Item = &Edge>
graph.in_edges(node) -> impl Iterator<Item = &Edge>
```

`remove_node` 会**自动清除关联边**（retain 过滤 source/target 匹配的边），保证图一致性。

## 循环体分组

`loop_body_groups()` 是 Loop 节点的核心算法——BFS 展开循环体节点集合：

```rust
pub fn loop_body_groups(&self) -> HashMap<NodeId, HashSet<NodeId>> {
    // 1. 找所有 loop_body 出边，按 Loop 节点分组
    // 2. BFS 沿前向边展开，排除：
    //    - loop_in 回环边（target_port == "loop_in"）
    //    - 回到 Loop 节点本身的边（如 done）
    //    - done 目标节点（防止吸收出口节点）
}
```

这是渲染层隐藏循环体、回环边路由的**唯一数据源**，在 `relayout` 末尾缓存。

## 小结

`FlowGraph` 基于 slotmap 稳定键，`Node`/`Edge`/`Port` 三层抽象清晰分离数据与视图。版本计数器驱动缓存失效，`loop_body_groups` 为循环渲染提供单一真相。图模型不含任何渲染逻辑，确保可复用与可序列化。

下一节：[渲染生命周期](render-lifecycle.md)
