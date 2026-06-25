# 视口变换与缩放

视口（Viewport）是连接「逻辑坐标」与「屏幕坐标」的唯一桥梁。本节梳理它的数学模型、`to_logical` 的语义，以及缩放时为什么节点与边要走两条不同的变换路径。

## Viewport 模型

`Viewport` 用两个字段描述完整的仿射变换：

```rust
pub struct Viewport {
    pub offset: PointF,   // 逻辑原点在屏幕上的偏移（像素）
    pub scale: f32,       // 缩放系数，1.0 = 100%
}
```

完整的「逻辑→屏幕」变换是一个「先缩放、后平移」的仿射映射：

```
screen = logical * scale + offset
```

反过来，「屏幕→逻辑」就是：

```
logical = (screen - offset) / scale
```

## to_logical：屏幕点 → 逻辑点

`FlowEditorView::to_logical` 是命中测试的入口。所有鼠标事件先拿到的是屏幕坐标（`Point<Pixels>`），必须先转成逻辑坐标才能与节点的 `position`（逻辑坐标）比较。

```rust
pub fn to_logical(&self, screen: Point<Pixels>) -> PointF {
    self.viewport.to_logical(screen)
}

// Viewport::to_logical 的等价实现
fn to_logical(&self, screen: Point<Pixels>) -> PointF {
    PointF::new(
        (screen.x.0 - self.offset.x) / self.scale,
        (screen.y.0 - self.offset.y) / self.scale,
    )
}
```

注意 `offset` 存的是「逻辑原点在屏幕上的偏移」，所以减法方向是 `screen - offset`。这一点容易写反，写反会导致平移方向颠倒。

## 缩放锚点

缩放时一个常见需求是「以鼠标为中心放大」。这要求在改变 scale 的同时调整 offset，使鼠标所在逻辑点在缩放前后屏幕坐标不变。

设鼠标屏幕坐标为 `c`，缩放前 `scale = s0`，缩放后 `scale = s1`，对应逻辑点 `L = (c - offset0) / s0`。要求 `c = L * s1 + offset1`，解得：

```
offset1 = c - L * s1
        = c - ((c - offset0) / s0) * s1
```

框架在滚轮缩放回调里就是按这个公式调整 offset，保证「鼠标指向的节点不会因为缩放而跑出视野」。

## 节点缩放：逐元素手动乘

GPUI 的 `div` 不支持整体 `transform: scale`，框架采用「逐元素缩放」策略：

| 元素 | 变换 |
|------|------|
| position | `node.position * scale + offset` |
| size | `node.size * scale` |
| font_size | `base * scale` |
| padding | `base * scale` |
| border_width | 视觉策略，通常不缩放，保持 1px 锐利 |
| port_radius | `base * scale`（命中半径也要同步） |

代价是渲染代码里到处可见 `* scale`，好处是每一项都能单独决定是否缩放。例如想让边框永远 1px，只需不乘 scale 即可。

## 边缩放：PathBuilder 变换

边的几何是一条折线，逐点乘 scale 会破坏圆角关系。框架的做法是「逻辑坐标算路径，整体变换到屏幕」：

```rust
// 1. 逻辑坐标下构建路径
let mut path = PathBuilder::new();
path.move_to(logical_points[0]);
for p in &logical_points[1..] {
    path.line_to(*p);
}
// smoothstep 圆角、step gap 都在逻辑坐标处理

// 2. 整体缩放 + 平移到屏幕空间
path.scale(scale, scale);
path.translate(offset.x, offset.y);

// 3. 线宽手动乘 scale
let stroke = base_stroke_width * scale;
```

这种「先算后变换」的方式让边几何算法（`edge_geometry.rs`）完全运行在逻辑坐标系，不用关心 scale，代码更简洁、可测试。

## 两种策略对比

| 维度 | 节点（逐元素缩放） | 边（路径变换） |
|------|--------------------|----------------|
| 几何复杂度 | 矩形，简单 | 折线 + 圆角 + 箭头 |
| 缩放方式 | 每个字段单独乘 scale | 整条路径一次性 scale |
| 线宽处理 | border 通常不缩放 | stroke_width 乘 scale |
| 圆角处理 | corner_radius 乘 scale | 在逻辑坐标算好，路径变换带过去 |
| 优点 | 每项可控 | 几何算法与 scale 解耦 |
| 缺点 | 代码啰嗦 | 不能逐项微调 |

## 缩放边界与默认值

| 参数 | 默认值 | 说明 |
|------|--------|------|
| `scale` | 1.0 | 100% |
| 最小 scale | 通常 0.2 | 防止缩太小看不见 |
| 最大 scale | 通常 3.0 | 防止放大到模糊 |
| `offset` | (0, 0) | 逻辑原点贴屏幕左上 |
| `grid_spacing` | 24.0 | 逻辑坐标下网格间距 |

`set_grid_spacing` 改的是逻辑间距，渲染时再乘 scale 得到屏幕间距，所以「放大后网格变疏」是自然效果。

## 常见误区

1. **把 offset 当成「相机位置」**。offset 是「逻辑原点在屏幕上的偏移」，方向相反。拖拽时 offset 增加，相当于逻辑原点向右下移动，画面看起来是向右下平移。
2. **缩放时不调 offset**。结果是内容「向原点收缩/扩张」，鼠标指向的节点会跑掉。
3. **命中测试忘了 to_logical**。直接拿屏幕坐标和 `node.position` 比，scale≠1 时全错。
4. **边的 stroke_width 不乘 scale**。放大后线看起来变细，缩小后变粗，视觉不一致。

## 小结

Viewport 用 `offset` + `scale` 描述一个「先缩放后平移」的仿射变换。节点因几何简单走逐元素缩放，边因几何复杂走路径变换。命中测试入口 `to_logical` 把屏幕坐标拉回逻辑空间，是与节点 position 比较的唯一正确姿势。下一章我们会看到，这套坐标变换正是交互状态机工作的基石。

下一章：[第12章 交互与命中测试](../12-interaction/INDEX.md)
