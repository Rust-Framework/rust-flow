# Node / Edge / Port 三要素

`FlowGraph` 容器里只有两类实体：`Node` 与 `Edge`。`Port` 不是独立实体，而是通过 `NodeSchema` 声明、用 `PortId`（字符串）在边里引用的逻辑端点。本节拆解这三要素的类型设计。

## Node：节点

```rust
pub type NodeKind = String;            // 匹配 IFlowNode::kind
pub type NodeData = serde_json::Value; // 自由 JSON

pub struct Node {
    pub id: NodeId,
    pub kind: NodeKind,
    pub data: NodeData,
    pub position: PointF,   // 逻辑坐标左上角
    pub size: SizeF,
}
```

| 字段 | 设计选择 | 原因 |
|------|----------|------|
| `kind` | `String` 而非枚举 | 支持任意自定义节点，无需改 core |
| `data` | `serde_json::Value` | 字段结构由 `NodeSchema.fields` 声明约束 |
| `position` | `PointF` | 布局后由 dagre 填充，逻辑坐标 |
| `size` | `SizeF` | 渲染层 `sync_node_sizes` 同步真实尺寸 |

`bounds()` 与 `center()` 是两个常用派生方法：

```rust
impl Node {
    pub fn bounds(&self) -> RectF { RectF::new(self.position, self.size) }
    pub fn center(&self) -> PointF { self.bounds().center() }
}
```

几何算法（命中测试、端口分布、回环边边界）都基于 `bounds()`，因此 `size` 必须准确——这是 `sync_node_sizes` 在 `relayout` 中优先执行的原因。

## Edge：边

```rust
pub struct Edge {
    pub id: EdgeId,
    pub source: NodeId,
    pub source_port: Option<PortId>,
    pub target: NodeId,
    pub target_port: Option<PortId>,
    pub edge_type: EdgeType,  // 路径算法
    pub kind: EdgeKind,       // 语义类型
}
```

边用 `NodeId` 引用两端，**端口是可选的**——`None` 表示用默认端口，框架自动推导方向与位置。

### EdgeType：路径算法

`EdgeType` 决定边用什么几何算法绘制：

```rust
pub enum EdgeType {
    Bezier,       // 三次贝塞尔（默认）
    Straight,     // 直线
    Step,         // 正交直角
    SmoothStep,   // 正交圆角
}
```

| 类型 | 点数 | 适用场景 |
|------|------|----------|
| Bezier | 4（P0,ctrl1,ctrl2,P3） | 自由流动连线，默认 |
| Straight | 2 | 简洁直连 |
| Step | 折线 | 严格正交 |
| SmoothStep | 圆角折线 | 正交且美观，结构化流程图 |

具体算法见 [边路径算法](../07-geometry-layout/edge-path-algorithms.md)。

### EdgeKind：语义类型

`EdgeKind` 描述边的语义，与渲染算法正交：

```rust
pub enum EdgeKind {
    Normal,    // 普通连接（默认）
    LoopBack,  // 循环回环边
}
```

`LoopBack` 触发 U 形路由算法（见 `loop_back_path`），并让渲染层用虚线/不同样式区分。两者分离意味着：一条回环边可以同时是 `EdgeKind::LoopBack` + `EdgeType::SmoothStep`，语义与算法各自独立演化。

## Port：端口

端口通过 `PortSpec` 在 `NodeSchema` 中声明，运行时用 `PortId`（字符串）引用：

```rust
pub type PortId = String;

pub enum PortDirection { In, Out }

pub enum PortSide {
    Top, Right, Bottom, Left,
    Auto,  // 默认，框架自动推导
}
```

| 概念 | 类型 | 说明 |
|------|------|------|
| `PortId` | `String` | 节点内唯一，如 `"if_0"`、`"loop_body"`、`"done"` |
| `PortDirection` | 枚举 | 数据流入（In）或流出（Out） |
| `PortSide` | 枚举 | 端口位于节点哪一侧，`Auto` 让框架推导 |

`PortSide` 提供两个工具方法：

```rust
impl PortSide {
    pub fn opposite(self) -> Self { /* Top<->Bottom, Left<->Right, Auto->Auto */ }
    pub fn is_horizontal(self) -> bool { matches!(self, Left | Right) }
}
```

`opposite()` 用于计算对端端口方向，`is_horizontal()` 决定贝塞尔控制点的偏移轴。

### Auto 的浮动行为

`PortSide::Auto`（默认）让框架根据相连节点的相对位置动态选边：

```
节点A 在 B 的右下方
  → B 的出口选 Right（dx 主导）
  → A 的入口选 Left（对端在左）

节点A 在 B 的正上方
  → B 的出口选 Top
  → A 的入口选 Bottom
```

推导逻辑见 [端口端点计算](../07-geometry-layout/port-calc.md) 中的 `compute_side_from_position`：比较 `dx`/`dy` 绝对值，取大者所在轴的方向。大多数普通节点只需声明 `Auto`，结构化节点（Condition/Loop）才需固定 side。

## 端口可选性与结构化节点

普通节点（Start/Action/End）的边通常不指定端口：

```rust
let edge = Edge::new(source_id, target_id); // source_port/target_port 均为 None
```

结构化节点必须指定端口以区分语义出口：

```rust
// Condition 的第一个分支出口
edge.source_port = Some("if_0".to_string());

// Loop 的循环体入口边
edge.target_port = Some("loop_in".to_string());
edge.kind = EdgeKind::LoopBack;

// Loop 的循环体出口
edge.source_port = Some("loop_body".to_string());

// Loop 的完成出口
edge.source_port = Some("done".to_string());
```

这些端口 ID 是框架与渲染层的隐式契约，`loop_body_groups`、`loop_back_path` 等算法都依赖这些字符串字面量。

## 小结

`Node` 用 `String` + JSON 实现开放扩展；`Edge` 把路径算法（`EdgeType`）与语义类型（`EdgeKind`）正交分离；`Port` 不是独立实体而是字符串引用，`Auto` side 让框架自动推导方向。结构化节点通过约定端口 ID（`if_N`/`loop_body`/`loop_in`/`done`）表达控制流语义，这些字符串是贯穿布局与渲染的隐式契约。

下一节：[FlowDocument 互转](document-interop.md)
