# 鼠标事件与节点选择浮层

本节把鼠标事件的流转串起来，并重点讲「从边插入节点」时弹出的节点选择浮层（node picker）——它是 `AddingNodeFromEdge` 状态的可视化产物。

## 事件三段式

鼠标交互本质上是 down / move / up 三段。框架把它们映射到 `InteractionState` 的进入、更新、退出：

| 事件 | 调用 | 作用 |
|------|------|------|
| `on_mouse_down` | `to_logical` + `hit_test` + 分派 | 进入新状态 |
| `on_mouse_move` | 按状态更新位置 / offset / current | 状态内更新 |
| `on_mouse_up` | 按状态收尾（创建边 / 结束拖拽 / 回 Idle） | 退出状态 |

每次更新后都调 `cx.notify()` 触发重绘，保证视觉与状态同步。

## hovered 的更新

`Idle` 状态下，`on_mouse_move` 还要维护 `hovered` 与 `hovered_plus`，用于：

- `hovered: Option<NodeId>`：鼠标悬停的节点，渲染时高亮边框。
- `hovered_plus: Option<EdgeId>`：鼠标悬停的边「+」按钮，渲染时改变 cursor 为手型并显示 tooltip。

```rust
fn update_hover(&mut self, logical: PointF) {
    self.hovered = match self.hit_test(logical) {
        HitResult::Node(id) | HitResult::DeleteButton(id) | HitResult::ToggleButton(id) => Some(id),
        _ => None,
    };
    self.hovered_plus = match self.hit_test_edge_plus(logical) {
        Some(eid) => Some(eid),
        None => None,
    };
}
```

hovered 与 hovered_plus 可以同时为 Some（鼠标在节点上方的边上），渲染时各自处理。

## AddingNodeFromEdge 的入口

当 `hit_test` 返回 `EdgePlusButton(edge_id)`，状态机进入 `AddingNodeFromEdge`：

```rust
HitResult::EdgePlusButton(eid) => {
    self.interaction = InteractionState::AddingNodeFromEdge {
        edge_id: eid,
        anchor: screen.into(),   // 屏幕坐标！
    };
}
```

`anchor` 用屏幕坐标，因为浮层是屏幕空间 UI，不参与视口变换。这避免了「打开浮层后用户一缩放，浮层就飘走」的问题——浮层会稳定地贴在打开时的屏幕位置。

## render_node_picker 的定位

浮层定位公式很简单：

```
浮层左上角 = anchor + (10, 10)
```

`(10, 10)` 是固定偏移，让浮层稍微离开鼠标，避免遮挡点击点。浮层本身是一个固定宽度的 v_flex 列表：

```
┌─────────────────────┐
│  action             │
│  condition          │
│  loop               │
│  variable           │
│  adapter            │
│  agent              │
└─────────────────────┘
```

## 6 种可插入节点

浮层列出 6 种可在边上插入的节点类型：

| 类型 | 说明 |
|------|------|
| `action` | 执行动作节点 |
| `condition` | 条件分支节点 |
| `loop` | 循环节点 |
| `variable` | 变量读写节点 |
| `adapter` | 适配器节点 |
| `agent` | 代理节点 |

点击某项后调用：

```rust
fn on_picker_select(&mut self, kind: NodeKind, cx: &mut Context<Self>) {
    if let InteractionState::AddingNodeFromEdge { edge_id, .. } = self.interaction {
        self.insert_node_at_edge(edge_id, kind, cx);
        self.interaction = InteractionState::Idle;
        cx.notify();
    }
}
```

`insert_node_at_edge` 的细节见第 13 章 plus-button 一节，这里只需知道它会：拆原边 → 用 schema 默认数据创建新节点 → 连 src→new→dst 两条边 → 选中 + relayout。

## 防冒泡：浮层如何不触发画布事件

浮层是画布的子元素，鼠标事件默认会冒泡到画布，导致「点浮层项时画布也以为是点空白」。框架的解法是浮层根 div 拦截 mouse_down：

```rust
div()
    .absolute()
    .left(anchor.x + 10.0)
    .top(anchor.y + 10.0)
    .on_mouse_down(MouseButton::Left, |_, _, cx| {
        cx.stop_propagation();   // 关键：阻止冒泡到画布
    })
    .children(items.map(|kind| {
        div()
            .on_mouse_down(MouseButton::Left, move |_, view, cx| {
                view.update(cx, |v, cx| v.on_picker_select(kind, cx));
                cx.stop_propagation();
            })
            // ...
    }))
```

`stop_propagation` 保证画布的 `on_mouse_down` 不会收到这次事件，避免误判为「点空白 → 取消选中」。

## 退出 AddingNodeFromEdge 的方式

| 触发 | 行为 |
|------|------|
| 点击浮层项 | `insert_node_at_edge` → Idle |
| 按 Esc | 直接 Idle（取消） |
| 点击画布空白 | hit_test 返回 Empty → Idle（取消） |
| 点击浮层外其他元素 | stop_propagation 保证浮层项先处理；空白则回到上一条 |

注意：单纯移动鼠标不会退出 `AddingNodeFromEdge`，这与 `DrawingEdge`（移动只更新 current）一致——状态只能由「明确动作」退出。

## 节点选择的视觉反馈

`selected` 与 `hovered` 在渲染时的体现：

| 状态 | 视觉变化 |
|------|----------|
| `selected = Some(id)` | 节点边框加粗 / 高亮色 |
| `hovered = Some(id)` | 节点边框轻微高亮 |
| `hovered_plus = Some(eid)` | 边「+」按钮 cursor: pointer，显示 tooltip |
| `AddingNodeFromEdge` | 显示 node picker 浮层 |
| `DrawingEdge` | 显示从源端口到 current 的临时虚线边 |

这些视觉态都在 render 里根据 InteractionState 与 selected/hovered 计算得出，不另存字段。

## 完整事件分派速查

```
mouse_down(screen)
  │
  ├─ Middle ──────────────────────► Panning
  │
  └─ Left
       │
       ├─ DeleteButton(id)    ──► delete_node ─► Idle
       ├─ ToggleButton(id)    ──► toggle_collapse ─► Idle
       ├─ EdgePlusButton(eid) ──► AddingNodeFromEdge
       ├─ OutPort(node,port)  ──► DrawingEdge
       ├─ InPort(node,port)   ──► (忽略或选中节点)
       ├─ Node(id)            ──► selected=id, [DraggingNode]
       └─ Empty               ──► selected=None

mouse_move(screen)
  │
  ├─ Panning            ──► offset += screen - start
  ├─ DraggingNode       ──► node.position = origin + (logical - start)
  ├─ DrawingEdge        ──► current = logical
  └─ Idle               ──► 更新 hovered / hovered_plus

mouse_up(screen)
  │
  ├─ Panning            ──► Idle
  ├─ DraggingNode       ──► Idle [可选 relayout]
  ├─ DrawingEdge        ──► hit_test InPort? 创建边 : 丢弃; Idle
  └─ Idle               ──► 无
```

## 小结

鼠标事件通过 down/move/up 三段映射到状态机的进入/更新/退出。`AddingNodeFromEdge` 状态用屏幕坐标 anchor 定位浮层，固定偏移 (10,10)。浮层通过 `stop_propagation` 阻止冒泡，保证点击浮层项不会被画布误判。`selected` 与 `hovered` 不存独立字段，而是由 render 根据状态实时计算视觉反馈。

下一章：[第13章 边渲染与连线](../13-edge-rendering/INDEX.md)
