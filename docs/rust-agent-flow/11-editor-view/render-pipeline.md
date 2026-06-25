# 渲染管线

`FlowEditorView` 的 `Render` 实现是一条分层的绘制流水线。理解这条流水线的层次，是排查「为什么我的按钮被节点盖住了」「为什么 tooltip 闪一下就没了」这类问题的前提。

## 外层布局：画布 + 分隔条 + 面板

最外层是一个水平 flex 容器，把窗口切成三块：

```
┌──────────────────────────────────────────────────────────┐
│                       FlowEditorView                      │
│  ┌──────────────────────────────────┐ ┌──┐ ┌──────────┐  │
│  │           canvas (flex-1)        │ │  │ │  panel   │  │
│  │                                  │ │分│ │ panel_   │  │
│  │   edges / nodes / plus buttons   │ │隔│ │ width    │  │
│  │   tooltip / toolbar / picker     │ │条│ │ 320px    │  │
│  │                                  │ │  │ │          │  │
│  └──────────────────────────────────┘ └──┘ └──────────┘  │
└──────────────────────────────────────────────────────────┘
```

- 画布 `canvas` 占据 `flex-1`，是主要绘制区域。
- 分隔条是 1px 宽的可拖拽条，`resizing_panel` 状态下拖动可改变 `panel_width`。
- 面板宽度默认 `Pixels(320.0)`，由 `panel_width` 决定。

当 `panel_view` 为 `None` 时，分隔条与面板都不渲染，画布占满整个视图。

## 画布内部分层

画布本身又是一个堆叠容器，按以下顺序自下而上绘制：

| 层 | 内容 | 是否受视口缩放 |
|----|------|----------------|
| 1 | 网格背景（`show_grid` 为真时） | 是（按 scale 缩放间距） |
| 2 | edges 层：所有可见边 | 是（路径变换） |
| 3 | content 层：所有可见节点 | 是（逐元素缩放） |
| 4 | edge_plus_buttons 层：边「+」按钮 | 是（按钮跟随边中点） |
| 5 | tooltip：悬停提示 | 否（屏幕坐标） |
| 6 | toolbar：自定义工具栏 | 否（屏幕坐标） |
| 7 | node_picker：节点选择浮层 | 否（屏幕坐标） |

顺序很关键：边在节点下方，这样节点可以遮住边的端点；「+」按钮在节点上方，避免被节点遮挡；tooltip / toolbar / picker 在最上层，因为它们是「屏幕空间 UI」，不应被画布缩放影响。

## 节点的逐元素缩放

GPUI 的 `div()` 没有原生的「整体 transform: scale」。框架采用的策略是：对节点的每一个几何元素（position、size、字体大小等）手动乘以 `scale`。

```rust
let scale = self.viewport.scale;
let screen_pos = node.position * scale + self.viewport.offset;
let screen_size = node.size * scale;

div()
    .absolute()
    .left(screen_pos.x)
    .top(screen_pos.y)
    .width(screen_size.x)
    .height(screen_size.y)
    // 字体、padding、border 也都要乘 scale
    .text_size(base_font * scale)
    // ...
```

这种「逐元素缩放」的代价是写起来啰嗦，好处是每一层都能精确控制——比如可以让边框线宽不随缩放变化（保持 1px），只让内容缩放。

## 边的路径变换

边不能简单地「逐元素缩放」，因为它的几何是一条折线 + 圆角 + 箭头，逐点乘 scale 会丢失圆角关系。框架的做法是：

1. 在逻辑坐标系下计算完整路径（含 step gap、smoothstep 圆角）。
2. 用 `PathBuilder::scale(scale, scale)` 对整条路径做缩放。
3. 用 `PathBuilder::translate(offset.x, offset.y)` 平移到屏幕位置。
4. 线宽 `stroke_width` 手动乘 scale，保证视觉一致。

```
逻辑坐标路径                屏幕坐标路径
   │                            │
   │  PathBuilder::path(...)    │
   ├──────────────────────────► ├─ scale(s, s)
   │                            ├─ translate(ox, oy)
   │                            │
   │                            ▼
   │                       stroke_width *= s
```

这套变换在 `crates/gpui/src/editor/rendering/edges.rs` 中统一完成，边几何算法（`edge_geometry.rs`）只需关心逻辑坐标。

## tooltip / toolbar / picker 的屏幕坐标

最上面三层故意不参与视口变换。原因：

- tooltip 跟随鼠标，本就是屏幕空间概念。
- toolbar 是固定 UI，缩放它会变模糊。
- picker（节点选择浮层）的 `anchor` 是屏幕坐标，直接用即可。

这也解释了为什么 `InteractionState::AddingNodeFromEdge` 的 `anchor` 字段是屏幕坐标——它直接喂给 picker 定位。

## 渲染与交互的解耦

渲染管线只「读」状态，不「改」状态。所有的状态变更（拖拽位置、绘制中的边终点、浮层显隐）都在交互回调里完成，再通过 `cx.notify()` 触发重绘。这种单向数据流让渲染函数保持纯函数性质，便于推理。

```
交互回调 ──修改──► InteractionState / graph / viewport
                          │
                          ▼
                     cx.notify()
                          │
                          ▼
                      render()   ← 只读状态
                          │
                          ▼
                     新一帧画面
```

## 小结

`FlowEditorView` 的渲染是一条「外层 flex → 画布堆叠 → 各层按缩放策略绘制」的流水线。节点走逐元素缩放，边走路径变换，浮层走屏幕坐标。记住「渲染只读」这条铁律，调试渲染问题时就不会误把状态改动塞进 render。

下一节：[视口变换与缩放](viewport-transform.md)
