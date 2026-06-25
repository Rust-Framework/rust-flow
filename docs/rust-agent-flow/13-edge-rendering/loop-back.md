# 循环回环边

普通边连接两个不同节点，方向单一。但流程图里有一类特殊的边：从一个节点出发，绕回到自身或上游节点，形成「循环」。框架用 `EdgeKind::LoopBack` 标记这类边，并给它独立的 U 形路由算法，避免与普通 SmoothStep 边混淆。

## 为什么需要独立的边类型

设想一个 `loop` 节点，它的输出要回到自己的输入端口形成循环。如果用普通 SmoothStep，路径会从源端口出发，经过 Dagre 布局的常规路由，结果往往是：

- 路径穿过节点本体（因为源和目标在节点同侧或距离太近）。
- 与其他边重叠，看不出是「回环」。
- Dagre 布局器本身不支持自环，会给出退化路径。

`EdgeKind::LoopBack` 的解法是：绕到节点外侧画一个 U 形，让回环视觉上「凸出来」，一眼可辨。

## LoopBack 的 U 形路由

U 形路由的核心思想：从源端口出发，先向外延伸一段，画一个半圆/U 形，再回到目标端口。

```
   横向布局（源=右，目标=左）

   ┌──────────────┐
   │   loop node  ●─┐
   │              ● │
   └──────────────┘ │
                    │
        ╔═══════════╝
        ║
        ╚═══════════╗
                    │
   ┌──────────────┐ │
   │              │ │
   │              ●◄┘
   └──────────────┘
```

实际实现是一个「从源侧伸出 → 垂直绕到目标侧 → 回到目标端口」的三段折线 + 圆角：

```rust
fn loop_back_path(from: PointF, from_side: PortSide,
                  to: PointF,   to_side: PortSide,
                  node_box: RectF) -> Vec<PointF> {
    let extend = LOOP_EXTEND; // 向外延伸量
    let mut pts = vec![from];

    // 1. 从源端口向外延伸
    let p1 = from + side_direction(from_side) * extend;
    pts.push(p1);

    // 2. 绕到目标端口同侧的延伸点
    let p2 = to + side_direction(to_side) * extend;
    pts.push(p2);

    // 3. 回到目标端口
    pts.push(to);
    pts
}
```

`side_direction(side)` 给出端口朝向的单位向量（Right→+x，Left→-x，Bottom→+y，Top→-y）。`extend` 控制回环「凸出多少」，通常取节点尺寸的某个比例，保证视觉醒目。

## 与普通 SmoothStep 的对比

| 维度 | SmoothStep | LoopBack |
|------|-----------|----------|
| 路径算法 | 中点转折 + step gap | U 形外绕 |
| 适用 | 不同节点间的边 | 自环或回到上游 |
| Dagre 支持 | 是（标准有向边） | 否，需手动几何 |
| 颜色 | `theme.edge_color` | `theme.edge_loop_back` |
| 圆角 | 转折点圆角 | U 形两端圆角 |
| 「+」按钮 | 边中点 | 通常禁用或特殊定位 |

## 颜色与视觉区分

`theme.edge_loop_back` 通常是一种区别于普通边的颜色（如紫色或橙色），让用户一眼识别「这是回环」。此外，回环边可以加虚线或更粗的线宽，进一步强调：

```rust
let color = match edge.kind {
    EdgeKind::Normal => self.theme.edge_color,
    EdgeKind::LoopBack => self.theme.edge_loop_back,
};
let stroke = match edge.kind {
    EdgeKind::Normal => BASE_STROKE * scale,
    EdgeKind::LoopBack => BASE_STROKE * scale * 1.2, // 稍粗
};
```

## LoopBack 的命中与「+」按钮

回环边的命中测试（`hit_test_edge_plus`）仍用「点到折线距离 ≤ PLUS_HIT_RADIUS」，但因为 U 形折线更长，命中区域更大。是否显示「+」按钮取决于业务：

- 框架默认在回环边上也显示「+」，但按钮位置由 `IFlowNode::plus_button_at_target` 决定。
- 有些节点实现会禁用回环边上的「+」（比如不允许在循环回路上插节点），通过 `plus_button_at_target` 返回 `None` 实现。

## Dagre 布局对 LoopBack 的处理

Dagre 本身不理解 `LoopBack`。框架的做法是：

1. 把 `LoopBack` 边当作普通边喂给 Dagre，让它决定节点位置。
2. 渲染时，识别 `EdgeKind::LoopBack`，丢弃 Dagre 给出的退化路径点，改用 `loop_back_path` 重新计算。

这意味着布局结果（节点位置）由 Dagre 决定，但回环边的视觉路径由框架自己控制。两者解耦。

## 自环 vs 上游回环

`LoopBack` 实际覆盖两种情况：

| 情况 | from_node == to_node? | 路径形态 |
|------|----------------------|----------|
| 自环 | 是 | 单节点外侧画 U |
| 上游回环 | 否 | 从下游节点绕到上游节点 |

自环的 U 形完全在节点外侧；上游回环的 U 形可能跨越多个节点。`loop_back_path` 通过 `extend` 参数自适应——当源与目标距离远时，extend 取较小值；距离近（自环）时取较大值，保证 U 形不被节点遮挡。

```rust
let extend = max(LOOP_MIN_EXTEND, distance(from, to) * 0.3);
```

## 渲染流程

```
EdgeView::render
  │
  ├─ match edge.kind
  │    ├─ Normal   ──► smooth_step / bezier / step
  │    └─ LoopBack ──► loop_back_path
  │
  ├─ PathBuilder 构建路径（逻辑坐标）
  ├─ path.scale + translate（屏幕空间）
  ├─ stroke_width *= scale
  ├─ 选色：edge_color 或 edge_loop_back
  └─ svg + 箭头
```

## 小结

`EdgeKind::LoopBack` 给循环回路一个独立的视觉身份。它的 U 形路由由 `loop_back_path` 在逻辑坐标计算，绕到节点外侧避免与其他边混淆。颜色用 `theme.edge_loop_back` 强调。Dagre 只负责节点位置，回环路径由框架自行决定。下一节讲边「+」按钮——它让任何边都成为「插入新节点」的入口。

下一节：[边「+」按钮与插入节点](plus-button.md)
