# 边「+」按钮与插入节点

边上的「+」按钮是 rust-agent-flow 的标志性交互：它把「在已有流程中间插入一个新节点」从「删边 → 建节点 → 连两条边」的繁琐操作，简化为一次点击。本节讲清楚按钮如何定位、如何命中，以及点击后 `insert_node_at_edge` 的完整流程。

## 按钮的视觉

在每条可见边的中点附近，渲染一个小圆形「+」按钮：

```
   ●──────┐
          ⊕  ← 「+」按钮
          │
   ●──────┘
```

按钮状态：

| 状态 | 视觉 |
|------|------|
| 默认 | 半透明圆 + 「+」图标 |
| `hovered_plus == Some(eid)` | 不透明 + cursor: pointer + tooltip「插入节点」 |
| `AddingNodeFromEdge` 中 | 该边的「+」按钮高亮，浮层已弹出 |

## plus_button_at_target：源侧还是目标侧

按钮位置由节点的 `IFlowNode::plus_button_at_target` 决定，它回答一个问题：「+」按钮应该出现在边的哪一端？

```rust
pub trait IFlowNode: ... {
    /// 决定「+」按钮锚定在源节点还是目标节点侧。
    /// 返回 None 表示该边不显示「+」。
    fn plus_button_at_target(&self) -> Option<PlusTarget>;
}

pub enum PlusTarget {
    Source, // 锚定在源节点出口附近
    Target, // 锚定在目标节点入口附近
}
```

为什么需要这个选择？因为不同节点的语义不同：

| 节点类型 | 推荐 PlusTarget | 原因 |
|----------|----------------|------|
| `action` | Target | 动作的输出多变，在入口侧插入更直观 |
| `condition` | Source | 条件的分支是关注点，在出口侧插入 |
| `loop` | Source | 循环体的入口是插入点 |
| `agent` | Target | 代理的输入是关注点 |

实际位置计算：

```rust
fn plus_button_position(edge: &Edge, target: PlusTarget,
                        from_pos: PointF, to_pos: PointF) -> PointF {
    match target {
        PlusTarget::Source => from_pos + (to_pos - from_pos) * 0.3,
        PlusTarget::Target => from_pos + (to_pos - from_pos) * 0.7,
    }
}
```

按钮不是放在严格中点，而是放在源侧 30% 或目标侧 70% 处，让按钮「靠近」它语义所属的节点。

## render_edge_plus_buttons 的流程

```
render_edge_plus_buttons
  │
  ├─ 遍历所有可见边
  │    │
  │    ├─ 取源节点 IFlowNode
  │    ├─ 调 plus_button_at_target
  │    │    └─ None ──► 跳过该边
  │    │
  │    ├─ 计算按钮逻辑坐标位置
  │    ├─ 变换到屏幕坐标（*scale + offset）
  │    ├─ 渲染圆形 + 「+」图标
  │    │
  │    └─ hovered_plus 匹配?
  │         ├─ 是 ──► cursor: pointer + tooltip
  │         └─ 否 ──► 默认半透明
  │
  └─ 返回所有按钮的层
```

按钮层在节点层之上、tooltip 之下，保证不被节点遮挡。

## 命中测试

「+」按钮的命中不是「点到圆心」，而是「点到折线距离 ≤ PLUS_HIT_RADIUS」。这把整条边都变成可点击区域，用户不必精确瞄准小圆点：

```rust
fn hit_test_edge_plus(&self, logical: PointF) -> Option<EdgeId> {
    for edge in self.graph.visible_edges() {
        let pts = self.edge_path_points(edge); // 逻辑坐标折线点
        if point_to_polyline_dist(logical, &pts) <= PLUS_HIT_RADIUS {
            return Some(edge.id);
        }
    }
    None
}
```

`PLUS_HIT_RADIUS` 是逻辑坐标常量。命中后 `on_mouse_down` 进入 `AddingNodeFromEdge` 状态，anchor 记录屏幕坐标供浮层定位。

## insert_node_at_edge 全流程

点击浮层项后，调用 `insert_node_at_edge(edge_id, kind, cx)`。这是 `graph_ops.rs` 的核心操作之一，分四步：

```
insert_node_at_edge(edge_id, kind, cx)
  │
  ├─ 1. 读原边信息
  │      src_node = edge.from
  │      dst_node = edge.to
  │      edge_type = edge.edge_type
  │
  ├─ 2. 删原边
  │      graph.remove_edge(edge_id)
  │
  ├─ 3. 创建新节点
  │      schema = registry.schema(kind)
  │      data  = schema.default_data      // schema 提供默认数据
  │      size  = schema.default_size      // schema 提供默认尺寸
  │      new_node = Node::new(kind, data, size)
  │      graph.add_node(new_node)
  │
  ├─ 4. 连两条新边
  │      graph.add_edge(src_node → new_node, edge_type)
  │      graph.add_edge(new_node → dst_node, edge_type)
  │
  ├─ 5. 选中新节点
  │      self.selected = Some(new_node.id)
  │
  └─ 6. relayout + cx.notify
         // Dagre 重新布局，新节点插入到 src 与 dst 之间
```

```rust
pub fn insert_node_at_edge(&mut self, edge_id: EdgeId, kind: NodeKind, cx: &mut Context<Self>) {
    // 1. 读原边
    let (src, dst, edge_type) = match self.graph.edge(edge_id) {
        Some(e) => (e.from, e.to, e.edge_type),
        None => return,
    };

    // 2. 删原边
    self.graph.remove_edge(edge_id);

    // 3. 建新节点（用 schema 默认数据 + 默认尺寸）
    let schema = self.registry.schema(kind);
    let new_node = Node::new(kind, schema.default_data.clone(), schema.default_size);
    let new_id = self.graph.add_node(new_node);

    // 4. 连两条新边，沿用原边类型
    self.graph.add_edge(src, new_id, edge_type);
    self.graph.add_edge(new_id, dst, edge_type);

    // 5. 选中 + 重布局
    self.selected = Some(new_id);
    self.relayout();
    cx.notify();
}
```

关键点：新边沿用原边的 `edge_type`，保证视觉一致；新节点用 schema 的 `default_data` 与 `default_size`，无需调用方提供；最后 `relayout` 让 Dagre 把新节点排到 src 与 dst 之间。

## 为什么不手动定位新节点

有人可能想「新节点放在原边中点位置」省一次 relayout。框架没这么做，原因：

- Dagre 布局会重新排所有节点，手动定位会被覆盖。
- 新节点插入后，邻居节点也需要让位，手动算太复杂。
- relayout 是 O(V+E) 的，对常见规模（几十节点）足够快。

只在 `DraggingNode` 这种「用户明确指定位置」的场景才跳过 relayout。

## delete_node 的桥接逻辑

与 `insert_node_at_edge` 对称的是 `delete_node`，它也要处理边：

```
delete_node(node_id)
  │
  ├─ 收集入边（src → node）与出边（node → dst）
  │
  ├─ 删除节点及其所有边
  │
  ├─ 线性桥接：
  │    对每个 (src, dst) 配对，若不存在重复，则
  │    graph.add_edge(src → dst, edge_type)
  │
  └─ relayout
```

「线性桥接」假设节点是线性流程中的一个环节：删掉它后，把前后直接连起来。对于多入多出的节点（如 condition），桥接策略会更保守——可能只桥接「单入单出」的情况，避免产生语义错误的边。

## handle_node_action 的分派

`handle_node_action(node_id, action, cx)` 是节点操作的统一入口：

```rust
pub enum NodeAction {
    Delete,
    ToggleCollapse,
    SetData(Value),
}

pub fn handle_node_action(&mut self, node_id: NodeId, action: NodeAction, cx: &mut Context<Self>) {
    match action {
        NodeAction::Delete => self.delete_node(node_id, cx),
        NodeAction::ToggleCollapse => {
            self.graph.node_mut(node_id).collapsed = !self.graph.node(node_id).collapsed;
            self.relayout();
            cx.notify();
        }
        NodeAction::SetData(data) => {
            self.graph.node_mut(node_id).data = data;
            if self.update_node_size_if_changed(node_id) {
                self.relayout();
            }
            cx.notify();
        }
    }
}
```

`SetData` 后用 `update_node_size_if_changed` 做单节点优化，仅在尺寸真变化时才触发完整 relayout。

## 完整的「+」交互时序

```
用户点边「+」
  │
  ▼
on_mouse_down
  ├─ hit_test → EdgePlusButton(edge_id)
  └─ state = AddingNodeFromEdge { edge_id, anchor: screen }
  │
  ▼
render
  └─ render_node_picker (定位 = anchor + (10,10))
  │
  ▼
用户点浮层项 "action"
  │
  ▼
on_picker_select(action)
  ├─ insert_node_at_edge(edge_id, action, cx)
  │    ├─ 读原边 → 删原边 → 建节点 → 连两新边
  │    └─ selected = new_id, relayout, notify
  └─ state = Idle
  │
  ▼
render
  └─ 新节点已插入，被选中高亮，浮层消失
```

## 小结

边「+」按钮由 `IFlowNode::plus_button_at_target` 决定锚定在源侧或目标侧，位置取边路径的 30%/70% 处。命中用「点到折线距离」而非点到圆心，提升可用性。点击后 `insert_node_at_edge` 走「读原边 → 删原边 → 建新节点（schema 默认数据）→ 连两条新边 → 选中 → relayout」六步。与之对称的 `delete_node` 用「线性桥接」保持流程连通。这套设计让边的「+」成为节点编辑器最顺手的扩展入口。

下一章：[回到目录](../INDEX.md)
