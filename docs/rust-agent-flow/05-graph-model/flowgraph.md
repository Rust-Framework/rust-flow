# FlowGraph 与 slotmap 键

`FlowGraph` 是整个框架的图容器，承载所有节点与边。它用 `slotmap` 存储元素，用单调递增的 `version` 计数器驱动缓存失效。理解这两个机制是阅读后续所有章节的基础。

## 数据结构

```rust
pub struct FlowGraph {
    nodes: SlotMap<NodeId, Node>,
    edges: SlotMap<EdgeId, Edge>,
    /// Monotonic version counter, bumped on any structural change.
    /// Used to invalidate cached geometry (e.g. `PortResolver`).
    version: u64,
}
```

| 字段 | 类型 | 职责 |
|------|------|------|
| `nodes` | `SlotMap<NodeId, Node>` | 节点存储，键为 `NodeId` |
| `edges` | `SlotMap<EdgeId, Edge>` | 边存储，键为 `EdgeId` |
| `version` | `u64` | 结构性变更计数器，`wrapping_add(1)` 递增 |

`NodeId`/`EdgeId` 由 `new_key_type!` 宏生成，是具备类型签名的句柄，编译期即与普通整数区分开：

```rust
new_key_type! { pub struct NodeId; }
new_key_type! { pub struct EdgeId; }
```

## slotmap 稳定键的意义

对比两种存储方案在删除场景下的行为：

```
方案A：Vec<Node>
  删除索引 1 后，其后元素前移 → 所有 >=2 的索引失效
  外部缓存的 NodeId(3) 现在指向了原来的 Node(4)

方案B：SlotMap<NodeId, Node>
  删除 NodeId(1) 后，仅该槽位标记空闲
  NodeId(2)、NodeId(3) 仍然指向原节点 → 外部缓存继续有效
```

这保证了 `NodeId`/`EdgeId` 在整个生命周期内可安全传递与缓存。选中状态、属性面板、命中测试缓存都持有 `NodeId`，删除相邻节点不会让它们失效。

> 注意：slotmap 内部用「版本号 + 槽位」双重编码键，复用已删除槽位时会递增版本号，因此持有**已删除**节点的旧 `NodeId` 不会意外命中新节点，而是返回 `None`。

## version 计数器

任何结构性变更都会 `version = wrapping_add(1)`，触发点包括：

| 操作 | 是否递增 version |
|------|----------------|
| `add_node` / `add_node_with_size` | 是 |
| `remove_node` | 是（含级联删边） |
| `node_mut` / `edges_mut` | 是（位置/数据可能变） |
| `add_edge` / `remove_edge` | 是 |
| `node` / `edges`（只读） | 否 |

`wrapping_add` 保证溢出时安全回绕，避免在极端长生命周期进程里 panic。

**用途**：渲染层在 `relayout` 末尾会基于当前 `version` 缓存几何数据（端口位置、循环体分组等）。下一次结构性变更使 `version` 变化，缓存即被识别为过期并重算。这比「每次访问都重算」高效得多——拖拽、平移等不改图结构的交互不触发 relayout，缓存持续有效。

## 增删改查 API

```rust
// 节点
graph.add_node(kind, data) -> NodeId              // 默认尺寸 180×64
graph.add_node_with_size(kind, data, size) -> NodeId
graph.remove_node(id) -> Option<Node>             // 自动清除关联边
graph.node(id) -> Option<&Node>                   // 只读，不递增 version
graph.node_mut(id) -> Option<&mut Node>           // 可变，递增 version
graph.nodes() / node_ids()                        // 迭代器

// 边
graph.add_edge(edge) -> EdgeId
graph.remove_edge(id) -> Option<Edge>
graph.edge(id) -> Option<&Edge>
graph.edges() / edges_mut() / edge_ids()
graph.out_edges(node) -> impl Iterator<Item = &Edge>  // 出边
graph.in_edges(node) -> impl Iterator<Item = &Edge>   // 入边
```

### 删除节点的级联清理

`remove_node` 内部用 `retain` 过滤所有引用该节点的边，保证图一致性：

```rust
pub fn remove_node(&mut self, id: NodeId) -> Option<Node> {
    let node = self.nodes.remove(id)?;
    // Remove all edges referencing this node.
    self.edges.retain(|_, e| e.source != id && e.target != id);
    self.version = self.version.wrapping_add(1);
    Some(node)
}
```

这意味着删除节点不需要调用方手动删边——但调用方若持有这些边的 `EdgeId` 缓存，需要在下一次 `relayout` 时通过 `version` 失效后重建。

### node_mut 的副作用

`node_mut` 是只读 `node` 的可变对应版本，但它**会递增 version**：

```rust
pub fn node_mut(&mut self, id: NodeId) -> Option<&mut Node> {
    self.version = self.version.wrapping_add(1);
    self.nodes.get_mut(id)
}
```

这是一个刻意的保守设计：调用方拿到 `&mut Node` 后可能修改 `position`/`size`，这些都会影响几何缓存。把 version 递增放在 `node_mut` 而非「真正修改时」，避免漏判。代价是即便只读式地获取可变引用也会失效缓存——但相比漏判导致脏缓存，这点代价可接受。

## 默认尺寸与位置

`add_node` 创建的节点默认：

- `position = PointF::zero()`——布局引擎会在 `relayout` 时填入实际坐标
- `size = SizeF::new(180.0, 64.0)`——通用默认值，结构化节点（Condition/Loop）应用 `add_node_with_size` 指定更准确的尺寸

```rust
self.nodes.insert_with_key(|key| Node {
    id: key,
    kind: kind.into(),
    data,
    position: crate::geometry::PointF::zero(),
    size: crate::geometry::SizeF::new(180.0, 64.0),
})
```

gpui 层的 `sync_node_sizes()` 会在 `relayout` 前根据 schema 修正实际渲染尺寸，因此 180×64 只是一个占位默认值。

## 小结

`FlowGraph` = slotmap 稳定键 + version 失效计数器。slotmap 保证删除不破坏既有 ID 的有效性，version 让几何缓存能精准失效而不必每帧重算。增删改查 API 中，所有写操作（含 `node_mut`）都会递增 version，这是保守但安全的缓存策略。

下一节：[Node / Edge / Port 三要素](node-edge-port.md)
