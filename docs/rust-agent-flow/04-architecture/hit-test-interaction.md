# 命中测试交互模型

## 为什么需要命中测试

GPUI 的 `listener` 闭包**无法捕获外部变量**（如 `node_id`），因此不能在每个节点 div 上绑定点击闭包。rust-agent-flow 采用**画布统一处理 + 几何命中测试**方案：

```
鼠标事件 → to_logical() → hit_test() → HitResult → 状态分发
```

所有鼠标事件绑定在画布根 div 上，通过几何计算确定点击目标。

## HitResult 枚举

`hit_test` 返回 `HitResult`，按优先级从高到低：

```rust
pub enum HitResult {
    DeleteButton(NodeId),       // 节点删除按钮（最高优先级）
    ToggleButton(NodeId),       // 展开/收起按钮
    EdgePlusButton(EdgeId),     // 边「+」按钮
    OutPort(NodeId, PortId),    // 输出端口（连线起点）
    InPort(NodeId, PortId),     // 输入端口（连线终点）
    Node(NodeId),               // 节点主体
    Empty,                      // 空白区域
}
```

优先级保证：悬停按钮时不会误触节点拖拽；悬停端口时不会误触节点选中。

## 命中测试算法

### 节点命中

```rust
// 点是否在节点矩形内
point_in_rect(point, node.bounds())
```

### 端口命中

端口是圆形，命中测试用**点到圆心距离**判断：

```rust
// 端口外圆半径 port_outer，点击在圆内即命中
let dist = point.distance_to(port_center);
dist <= port_outer
```

端口位置由 `IFlowNode::port_position` 计算（或框架默认按 side 推导）。

### 边「+」按钮命中

「+」按钮位于边路径的中点附近。命中测试用**点到折线距离**：

```rust
let dist = point_to_polyline_distance(point, &edge_points);
dist <= PLUS_HIT_RADIUS
```

## 交互状态机

命中结果驱动 `InteractionState` 状态机：

```rust
pub enum InteractionState {
    Idle,                                              // 空闲
    Panning { start_screen, origin },                  // 平移视口
    DraggingNode { node_id, start, node_origin },      // 拖拽节点
    DrawingEdge { from_node, from_port, current },     // 绘制连线
    AddingNodeFromEdge { edge_id, anchor },            // 选节点类型插入
}
```

### 状态转移

```
Idle
  ├─ 中键按下 → Panning
  ├─ 左键命中 OutPort → DrawingEdge
  ├─ 左键命中 Node → DraggingNode（若 drag_enabled）
  ├─ 左键命中 EdgePlusButton → AddingNodeFromEdge
  ├─ 左键命中 DeleteButton → 删除节点 → Idle
  ├─ 左键命中 ToggleButton → 切换收起 → Idle
  └─ 左键命中 Empty → 取消选中 → Idle

Panning
  └─ 鼠标移动 → 更新 offset → 中键抬起 → Idle

DraggingNode
  └─ 鼠标移动 → 更新 node.position → 左键抬起 → Idle

DrawingEdge
  └─ 鼠标移动 → 更新 current → 左键抬起：
       命中 InPort → 创建边 → Idle
       其他 → 取消 → Idle

AddingNodeFromEdge
  └─ 点击节点类型 → insert_node_at_edge → Idle
  └─ 点击外部 → Idle
```

## 平移用屏幕坐标的原因

`Panning` 状态用**屏幕坐标**（而非逻辑坐标）记录起点：

```rust
Panning { start_screen: PointF, origin: PointF }
```

避免平移过程中 `viewport.offset` 变化导致逻辑坐标反馈抖动——这是参考 ReactFlow 的成熟方案。鼠标移动时：

```rust
let delta = screen_now - start_screen;
viewport.offset = origin + delta; // 纯屏幕空间运算
```

## 节点拖拽用逻辑坐标

`DraggingNode` 用**逻辑坐标**记录起点：

```rust
DraggingNode { node_id, start: PointF, node_origin: PointF }
```

因为节点 position 存储在逻辑空间：

```rust
let delta = logical_now - start;
node.position = node_origin + delta;
```

## 缩放下的坐标转换

所有鼠标事件先转换到逻辑坐标：

```rust
fn to_logical(&self, p: Point<Pixels>) -> PointF {
    self.viewport.to_logical(PointF::new(p.x.as_f32(), p.y.as_f32()))
}
```

`Viewport::to_logical` 的实现：

```rust
fn to_logical(&self, screen: PointF) -> PointF {
    (screen - self.offset) / self.scale
}
```

命中测试在逻辑空间进行，与节点 position 同一坐标系。

## 命中测试的性能

每帧鼠标事件触发一次 `hit_test`，遍历所有节点/边/端口。对于 < 200 节点的场景，线性遍历足够快。若需支持更大规模，可引入空间索引（如四叉树）——当前未内置。

## 小结

命中测试方案是 GPUI 闭包约束下的必然选择：画布统一处理鼠标事件，几何计算确定目标，状态机分发处理。优先级保证按钮/端口不误触节点。平移用屏幕坐标防抖，拖拽用逻辑坐标对齐数据。所有命中在逻辑空间进行，缩放通过 `to_logical` 统一转换。

下一章：[流程图数据模型](../05-graph-model/INDEX.md)
