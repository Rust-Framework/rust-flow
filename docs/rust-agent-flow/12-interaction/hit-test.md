# 命中测试

命中测试（hit test）回答一个问题：给定一个逻辑坐标点，它落在了哪个元素上？这是交互分派的基石——`on_mouse_down` 完全依赖 `hit_test` 的结果决定下一步。

## HitResult 枚举

```rust
pub enum HitResult {
    Empty,
    Node(NodeId),
    InPort(NodeId, PortId),
    OutPort(NodeId, PortId),
    EdgePlusButton(EdgeId),
    ToggleButton(NodeId),
    DeleteButton(NodeId),
}
```

## 优先级顺序

命中测试按「从高到低」的优先级顺序探测，第一个命中的就返回：

| 优先级 | 变体 | 几何判定 | 说明 |
|--------|------|----------|------|
| 1 | `DeleteButton` | 点-矩形 | 节点右上角的 × 图标 |
| 2 | `ToggleButton` | 点-矩形 | 节点上的折叠/展开按钮 |
| 3 | `EdgePlusButton` | 点-折线距离 | 边上的「+」按钮 |
| 4 | `OutPort` | 点-圆距离 | 输出端口（源） |
| 5 | `InPort` | 点-圆距离 | 输入端口（目标） |
| 6 | `Node` | 点-矩形 | 节点本体 |
| 7 | `Empty` | 兜底 | 什么都没命中 |

为什么是这个顺序？因为视觉层级与可点击性：删除/折叠按钮在节点最上层，必须最先判；「+」按钮在边上但浮在节点之间，优先于端口；端口比节点本体小且更具体，优先于节点；节点本体最后；空是兜底。

## 几何判定算法

### 点-矩形（point_in_rect）

```rust
fn point_in_rect(p: PointF, pos: PointF, size: SizeF) -> bool {
    p.x >= pos.x && p.x <= pos.x + size.x
        && p.y >= pos.y && p.y <= pos.y + size.y
}
```

用于节点本体、DeleteButton、ToggleButton。注意按钮的矩形是「按钮在节点内的局部矩形 + 节点 position」换算到逻辑坐标后的结果。

### 点-圆（端口）

```rust
fn point_in_port(p: PointF, port_center: PointF, port_outer: f32) -> bool {
    let dx = p.x - port_center.x;
    let dy = p.y - port_center.y;
    dx * dx + dy * dy <= port_outer * port_outer
}
```

`port_outer` 是端口的外圆半径（含描边），通常比视觉半径大一点，方便点击。`port_center` 由节点的 `port_position(node, side, port_id)` 给出，已在逻辑坐标。

### 点-折线（边「+」按钮）

边的「+」按钮不是单点，而是「边折线的中点附近一段区域」。判定转化为「点到折线的最短距离」：

```rust
fn point_to_polyline_dist(p: PointF, pts: &[PointF]) -> f32 {
    pts.windows(2)
        .map(|w| point_to_segment_dist(p, w[0], w[1]))
        .fold(f32::INFINITY, f32::min)
}

fn hit_test_edge_plus(p: PointF, pts: &[PointF]) -> bool {
    point_to_polyline_dist(p, pts) <= PLUS_HIT_RADIUS
}
```

`PLUS_HIT_RADIUS` 是命中半径常量。这种「整条边都可点」的设计让用户不必精确瞄准中点的小按钮，提升可用性。

## 端口位置的计算

端口命中需要先知道端口中心。`port_position` 取决于布局方向：

```
横向布局（Horizontal）              纵向布局（Vertical）
┌──────────┐                       ┌──────────┐
│        ●─┤ out (Right)           │       ●──┤ out (Bottom)
│ ●       │ in (Left)              │          │
└──────────┘                       │ ●        │ in (Top)
                                   └──────────┘
```

```rust
fn port_center(node: &Node, side: PortSide, port_id: PortId) -> PointF {
    match side {
        PortSide::Left   => PointF::new(node.position.x, node.position.y + port_offset(port_id)),
        PortSide::Right  => PointF::new(node.position.x + node.size.x, node.position.y + port_offset(port_id)),
        PortSide::Top    => PointF::new(node.position.x + port_offset(port_id), node.position.y),
        PortSide::Bottom => PointF::new(node.position.x + port_offset(port_id), node.position.y + node.size.y),
    }
}
```

`port_offset(port_id)` 按端口索引乘以端口间距计算纵向/横向偏移。`port_sides()` 给出当前方向的 `(out_side, in_side)`，是端口朝向的单一事实来源。

## 完整 hit_test 流程

```
hit_test(logical_point)
        │
        ▼
┌───────────────────────────────────────┐
│ 1. 遍历可见节点                        │
│    ├─ DeleteButton 命中? ──► 返回      │
│    ├─ ToggleButton 命中? ──► 返回      │
│    ├─ OutPort 命中?     ──► 返回       │
│    ├─ InPort  命中?     ──► 返回       │
│    └─ Node    命中?     ──► 记录候选   │
└───────────────────────────────────────┘
        │ 无节点元素命中
        ▼
┌───────────────────────────────────────┐
│ 2. 遍历可见边的「+」按钮               │
│    └─ EdgePlusButton 命中? ──► 返回    │
└───────────────────────────────────────┘
        │ 无命中
        ▼
   返回候选 Node 或 Empty
```

注意节点元素先于边「+」按钮判定，这与「按钮浮在最上层」的视觉一致：节点自己的删除/折叠按钮永远优先于边上的「+」。

## 可见性过滤

`hit_test` 只考虑「可见节点」：

- `cached_hidden_nodes` 里的节点不参与判定（被折叠隐藏）。
- 折叠节点的 body 不参与判定。
- 不可见图（`EdgeKind::LoopBack` 在某些条件下）的边「+」不参与判定。

这保证了「折叠一个节点后，点它内部区域不会误中子节点」。

## 缩放与命中半径

`hit_test` 在逻辑坐标运行，所以命中半径常量（`port_outer`、`PLUS_HIT_RADIUS`）也是逻辑值。这意味着：

- scale = 1：实际命中像素 = 常量值。
- scale = 2：实际命中像素 = 常量 × 2，放大后更容易点中（合理）。
- scale = 0.5：实际命中像素 = 常量 × 0.5，缩小后更难点中。

如果希望「缩小后也保持可点击性」，可以在 hit_test 时按 `1/scale` 放大命中半径，但框架目前未做此补偿，保持算法简单。

## 小结

`hit_test` 按七级优先级探测：DeleteButton > ToggleButton > EdgePlusButton > OutPort > InPort > Node > Empty。三种几何判定覆盖所有元素：点-矩形、点-圆、点-折线。所有判定都在逻辑坐标完成，命中半径随 scale 自然缩放。理解这套优先级，是调试「我明明点的是 A 却触发了 B」类问题的钥匙。

下一节：[鼠标事件与节点选择浮层](mouse-events-picker.md)
