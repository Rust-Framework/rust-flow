# EdgeView 与边类型

`EdgeView` 是边的渲染视图对象。它把一条 `Edge`（数据）+ 源/目标节点的几何信息，转化成屏幕上的一条带箭头折线。本节聚焦于它的结构、边类型分派，以及「逻辑坐标算路径、整体变换到屏幕」的核心策略。

## EdgeView 的构成

`EdgeView` 不持有独立状态，而是渲染时根据 `Edge` + 节点几何临时构造：

```rust
pub struct EdgeView<'a> {
    pub edge: &'a Edge,
    pub from_pos: PointF,    // 源端口中心（逻辑坐标）
    pub to_pos: PointF,      // 目标端口中心（逻辑坐标）
    pub from_side: PortSide, // 源端口朝向
    pub to_side: PortSide,   // 目标端口朝向
    pub edge_type: EdgeType, // 路径算法
    pub theme: &'a Theme,
    pub scale: f32,          // 视口缩放
    pub offset: PointF,      // 视口偏移
}
```

`from_side` / `to_side` 由 `FlowEditorView::port_sides()` 给出，与布局方向一致。横向布局时 `(Right, Left)`，纵向时 `(Bottom, Top)`。

## EdgeType 路径算法

```rust
pub enum EdgeType {
    Straight,    // 直线
    SmoothStep,  // 折线 + 圆角（默认）
    Bezier,      // 三次贝塞尔
    Step,        // 直角折线（无圆角）
}
```

各类型的路径生成在 `edge_geometry.rs` 中实现，全部在逻辑坐标运算：

| 类型 | 几何 | 适用场景 |
|------|------|----------|
| `Straight` | 两点直线 | 简单关系图 |
| `Step` | 直角折线，中点转折 | 工程示意图 |
| `SmoothStep` | 折线 + 圆角 + step gap | 默认，平衡美观与清晰 |
| `Bezier` | 三次贝塞尔曲线 | 自由流动视觉 |

`default_edge_type` 默认 `SmoothStep`，可在 `FlowEditorView` 构造后用 `set_edge_type` 修改（影响所有边）。

## SmoothStep 的几何细节

SmoothStep 是最常用的类型，它的路径由「水平/垂直段 + 圆角」组成：

```
源端口 ●──────┐
              │
              └──────● 目标端口
```

关键参数：

| 参数 | 含义 |
|------|------|
| `step_gap` | 水平段的最小长度，避免圆角挤在一起 |
| `corner_radius` | 圆角半径 |
| `from_side` / `to_side` | 决定第一段与最后一段的走向 |

算法骨架（逻辑坐标）：

```rust
fn smooth_step_points(from: PointF, from_side: PortSide,
                      to: PointF,   to_side: PortSide) -> Vec<PointF> {
    let mut pts = vec![from];
    let mid = midpoint_with_gap(from, to, from_side, to_side, STEP_GAP);
    // 根据 from_side / to_side 决定转折点
    pts.push(/* 第一转折点 */);
    pts.push(/* 第二转折点 */);
    pts.push(to);
    // 圆角在 PathBuilder 阶段用 arc/quadratic 加入
    pts
}
```

圆角不是「点」，而是在 `PathBuilder` 画线时对每个转折点用二次贝塞尔或弧线过渡。这保证了缩放后圆角依然光滑。

## 渲染流程：逻辑路径 → 屏幕路径

```rust
impl<'a> EdgeView<'a> {
    pub fn render(&self) -> impl IntoElement {
        // 1. 逻辑坐标下计算路径点
        let logical_pts = match self.edge_type {
            EdgeType::Straight   => vec![self.from_pos, self.to_pos],
            EdgeType::Step       => step_points(/* ... */),
            EdgeType::SmoothStep => smooth_step_points(/* ... */),
            EdgeType::Bezier     => bezier_control_points(/* ... */),
        };

        // 2. PathBuilder 在逻辑坐标构建路径（含圆角）
        let mut path = PathBuilder::new();
        path.move_to(logical_pts[0]);
        // ... line_to / quad_to / arc_to ...

        // 3. 整体变换到屏幕空间
        path.scale(self.scale, self.scale);
        path.translate(self.offset.x, self.offset.y);

        // 4. 线宽手动乘 scale
        let stroke = BASE_STROKE_WIDTH * self.scale;

        // 5. 画线 + 箭头 marker
        svg()
            .path(path.build())
            .stroke(self.theme.edge_color)
            .stroke_width(stroke)
            .marker_end(arrow_marker(self.theme, self.scale))
    }
}
```

为什么是「先算后变换」而不是「逐点乘 scale」？因为圆角、贝塞尔的控制点关系在缩放下会变形——逐点乘 scale 会让圆角变椭圆。整体 `PathBuilder::scale` 是均匀缩放，保持形状。

## 箭头 marker

箭头画在 `to_pos` 处，方向由 `to_side` 决定（指向端口）：

```
to_side = Left:   ──►●
to_side = Right:  ●◄──
to_side = Top:    ▲
to_side = Bottom: ▼
```

箭头大小也乘 scale，保证放大后成比例。箭头颜色取 `theme.edge_color` 或 `theme.edge_arrow`。

## 边的颜色

边的颜色由几个因素决定：

| 条件 | 颜色 |
|------|------|
| 普通边 | `theme.edge_color` |
| `EdgeKind::LoopBack` | `theme.edge_loop_back` |
| `hovered_plus == Some(edge_id)` | `theme.edge_hover` |
| `DrawingEdge` 中的临时边 | `theme.edge_drawing`（虚线） |

着色在 `EdgeView::render` 里根据传入的 `theme` 与边元信息决定。

## step gap 的作用

`step_gap` 解决「源端口与目标端口距离太近时圆角挤变形」的问题。当两点水平距离小于 `2 * step_gap`，算法会让中段折线退化为更简单的形状，避免画出「Z 字形」毛刺。

```
距离足够：                  距离过近（无 gap 处理）：
●──────┐                    ●─┐
       │                      │ ┌──●  ← 毛刺
       └──────●               └─┘
```

## 完整渲染顺序

```
EdgeView::render
  │
  ├─ 1. 逻辑坐标算路径点（smooth_step_points 等）
  ├─ 2. PathBuilder 构建路径（含圆角）
  ├─ 3. path.scale(s, s) + path.translate(offset)
  ├─ 4. stroke_width *= s
  ├─ 5. 选颜色（edge_color / loop_back / hover / drawing）
  └─ 6. svg().path().stroke().marker_end(arrow)
```

## 小结

`EdgeView` 把一条 `Edge` + 节点几何 + 视口参数，转化成屏幕上的折线。路径算法在逻辑坐标完成（SmoothStep 用 step gap + 圆角保证美观），再用 `PathBuilder::scale + translate` 整体变换到屏幕，线宽与箭头单独乘 scale。颜色按边类型与 hover 态选取。下一节讲一种特殊的边——循环回环，它有自己的路由算法。

下一节：[循环回环边](loop-back.md)
