# 交互状态机

`InteractionState` 是一个枚举，每个变体代表「当前用户正在做的一件事」。它的核心价值是把复杂的鼠标行为收敛成有限个互斥状态，避免「同时又在拖节点又在画边」这种不可能态。

## 五种状态

```rust
pub enum InteractionState {
    Idle,
    Panning {
        start_screen: PointF,   // 屏幕坐标，用于防抖
        origin: PointF,         // 按下时的 viewport.offset
    },
    DraggingNode {
        node_id: NodeId,
        start: PointF,          // 逻辑坐标，按下时的鼠标位置
        node_origin: PointF,    // 按下时的节点 position
    },
    DrawingEdge {
        from_node: NodeId,
        from_port: PortId,
        current: PointF,        // 逻辑坐标，当前鼠标位置
    },
    AddingNodeFromEdge {
        edge_id: EdgeId,
        anchor: PointF,         // 屏幕坐标，浮层定位锚
    },
}
```

## 坐标语义对照

不同状态用不同坐标系，并非随意，而是由「这个状态要和什么比较」决定：

| 状态 | 坐标系 | 原因 |
|------|--------|------|
| `Panning` | 屏幕坐标 | 平移是「屏幕上拖了多少像素」，与逻辑坐标无关 |
| `DraggingNode` | 逻辑坐标 | 节点 position 是逻辑坐标，比较前必须统一 |
| `DrawingEdge` | 逻辑坐标 | 边端点要连到端口，端口在逻辑坐标 |
| `AddingNodeFromEdge` | 屏幕坐标 | 浮层是屏幕空间 UI，不参与视口变换 |
| `Idle` | 无 | 不需要 |

`Panning` 用屏幕坐标是为了「防抖」：直接用 `current_screen - start_screen` 得到屏幕位移，加到 `origin` 上就是新 offset。如果中途 scale 变了（虽然平移时通常不变），这个算法依然正确，因为它只关心屏幕像素。

## 状态转移图

```
                        ┌─────────────────────────────┐
                        │                             │
                        ▼                             │
   ┌────────┐  中键down   ┌─────────┐  mouseup  │
   │  Idle  │────────────►│ Panning │──────────►│
   └────────┘             └─────────┘            │
       │                                           │
       │ 左键down                                   │
       │ hit_test 分发：                            │
       │  ├─ DeleteButton  ──► delete_node ──►Idle │
       │  ├─ ToggleButton  ──► toggle_collapse     │
       │  ├─ EdgePlusButton──► AddingNodeFromEdge  │
       │  ├─ OutPort       ──► DrawingEdge         │
       │  ├─ Node          ──► 选中+DraggingNode   │
       │  └─ Empty         ──► 取消选中            │
       │                                           │
       │                                           │
       ▼                                           │
   ┌──────────────────┐                            │
   │ DraggingNode     │  mouseup ─────────────────►│
   └──────────────────┘                            │
                                                    │
   ┌──────────────────┐                            │
   │ DrawingEdge      │  mouseup:                  │
   │                  │   hit_test InPort?         │
   │                  │   ├─ 是 ──► 创建边 ──► Idle│
   │                  │   └─ 否 ───────────► Idle │
   └──────────────────┘                            │
                                                    │
   ┌──────────────────┐                            │
   │AddingNodeFromEdge│  点击浮层项 ──►             │
   │                  │   insert_node_at_edge      │
   │                  │   ──► Idle                 │
   │                  │  Esc / 空白点击 ──► Idle   │
   └──────────────────┘                            │
                                                    │
                       所有路径最终回到 Idle ◄──────┘
```

## on_mouse_down 的分派

`on_mouse_down` 是状态机的总入口，它的流程非常机械：

```rust
fn on_mouse_down(&mut self, screen: Point<Pixels>, button: MouseButton, cx: &mut Context<Self>) {
    let logical = self.to_logical(screen);

    match button {
        MouseButton::Middle => {
            self.interaction = InteractionState::Panning {
                start_screen: screen.into(),
                origin: self.viewport.offset,
            };
            return;
        }
        MouseButton::Left => {}
        _ => return,
    }

    match self.hit_test(logical) {
        HitResult::DeleteButton(id)    => self.delete_node(id, cx),
        HitResult::ToggleButton(id)    => self.handle_node_action(id, NodeAction::ToggleCollapse, cx),
        HitResult::EdgePlusButton(eid) => {
            self.interaction = InteractionState::AddingNodeFromEdge {
                edge_id: eid,
                anchor: screen.into(),
            };
        }
        HitResult::OutPort(node, port) => {
            self.interaction = InteractionState::DrawingEdge {
                from_node: node, from_port: port, current: logical,
            };
        }
        HitResult::Node(id) => {
            self.selected = Some(id);
            if self.drag_enabled {
                self.interaction = InteractionState::DraggingNode {
                    node_id: id,
                    start: logical,
                    node_origin: self.graph.node(id).position,
                };
            }
        }
        HitResult::InPort(_, _) => { /* 通常不作为起点 */ }
        HitResult::Empty => {
            self.selected = None;
        }
    }
    cx.notify();
}
```

注意：分派完全由 `hit_test` 的结果驱动，状态机本身不做几何判断。这种「命中测试 + 状态分派」的分离让两个模块都可独立测试。

## on_mouse_move 的状态相关行为

```rust
fn on_mouse_move(&mut self, screen: Point<Pixels>, cx: &mut Context<Self>) {
    let logical = self.to_logical(screen);
    match &mut self.interaction {
        InteractionState::Panning { start_screen, origin } => {
            let dx = screen.x.0 - start_screen.x;
            let dy = screen.y.0 - start_screen.y;
            self.viewport.offset = PointF::new(origin.x + dx, origin.y + dy);
        }
        InteractionState::DraggingNode { node_id, start, node_origin } => {
            let new_pos = *node_origin + (logical - *start);
            self.graph.node_mut(*node_id).position = new_pos;
        }
        InteractionState::DrawingEdge { current, .. } => {
            *current = logical;
        }
        _ => {
            // Idle：仅更新 hovered / hovered_plus
            self.update_hover(logical);
        }
    }
    cx.notify();
}
```

`DraggingNode` 的位移公式 `new_pos = node_origin + (logical - start)` 是「按下时的节点位置 + 鼠标逻辑位移」。这种「以按下点为基准」的算法能避免累积误差，即使中途 scale 变化也大致正确。

## on_mouse_up 的收尾

| 当前状态 | 抬起动作 |
|----------|----------|
| `Panning` | 回到 `Idle` |
| `DraggingNode` | 回到 `Idle`，可选触发 `relayout`（若开启了自动布局） |
| `DrawingEdge` | 抬起点 `hit_test`，若 `InPort` 则创建边；否则丢弃。回到 `Idle` |
| `AddingNodeFromEdge` | 通常不因 mouseup 退出，要等浮层点击或 Esc |
| `Idle` | 无 |

`DrawingEdge` 抬起时的端口配对规则：源端口类型与目标端口类型需匹配，且不能连到自己。具体匹配逻辑由 `data_type_provider` 决定（若存在）。

## 为什么不用更多状态？

有人可能想加 `ResizingNode`、`BoxSelecting`、`ConnectingPort` 等状态。框架目前只实现了五态，原因：

- 五态覆盖了核心编辑场景，复杂度可控。
- 新状态需要同步考虑：转移条件、坐标语义、render 时的额外绘制、hit_test 是否需要调整。每加一个状态，状态矩阵翻一倍。
- 框架优先保证「数据驱动 + relayout」的简洁性，手动 resize 等高级交互留给后续扩展。

## 小结

`InteractionState` 用五个变体把鼠标行为切成互斥阶段。坐标语义因状态而异：平移用屏幕坐标，拖节点与画边用逻辑坐标，浮层用屏幕坐标。`on_mouse_down` 完全由 `hit_test` 驱动分派，状态机不做几何判断——这是它能保持简洁的关键。

下一节：[命中测试](hit-test.md)
