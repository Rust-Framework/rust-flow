# 渲染生命周期

## 从数据变更到画面刷新

完整的渲染链路：

```
数据变更（增删节点/编辑属性）
  ↓
sync_node_sizes()       // 同步节点尺寸到 schema 渲染尺寸
  ↓
relayout()              // dagre 重新排版
  ├─ 更新 cached_body_groups    // 循环体分组
  ├─ 更新 cached_all_body_nodes // 扁平化
  └─ 更新 cached_hidden_nodes   // 收起的节点
  ↓
cx.notify()             // 标记实体为脏
  ↓
render()                // GPUI 调度重绘
  ├─ ensure_panel_view()        // 确保面板实体存在
  ├─ render_edges()             // 边（Canvas 层）
  ├─ render_nodes()             // 节点（Content 层）
  ├─ render_edge_plus_buttons() // 「+」按钮层
  ├─ render_toolbar()           // 工具栏
  └─ panel_view.render()        // 属性面板
```

## relayout 详解

`relayout` 是布局的核心入口，在图结构变化时调用：

```rust
fn relayout(&mut self) {
    // 1. 同步节点尺寸
    self.sync_node_sizes();

    // 2. 运行 dagre 布局
    let result = DagreLayout::new().layout(&self.graph, dir);
    for (node_id, pos) in &result.positions {
        self.graph.node_mut(*node_id).position = *pos;
    }

    // 3. 更新缓存
    self.cached_body_groups = self.graph.loop_body_groups();
    self.cached_all_body_nodes = /* 扁平化 */;
    self.cached_hidden_nodes = /* 收起的循环体 */;
}
```

**为什么需要 sync_node_sizes**：结构化节点（如 Condition）的渲染高度随条件项数量变化，但 `node.size.h` 可能在创建后未更新。布局前同步确保 dagre、命中测试、回环边边界计算使用正确尺寸。

## sync_node_sizes 的优化

每次按键都调用 `sync_node_sizes`（遍历所有节点）+ `relayout`（运行 dagre）代价过高。`update_node_size_if_changed` 是**单节点优化**路径：

```rust
fn update_node_size_if_changed(&mut self, node_id: NodeId) -> bool {
    // 只检查并更新单个节点的渲染尺寸
    // 返回尺寸是否变化 → 仅变化时才触发 relayout
}
```

`SetData` 路径（属性面板编辑）用此方法：只有结构化节点的尺寸真正变化时才重排，普通节点（Action）的文本编辑不会触发 dagre。

## render 的分层组装

`FlowEditorView::render` 将画布分为多个图层：

```
外层 flex 容器（画布 + 分隔条 + 面板）
└─ canvas（flex-1）
   ├─ edges 层（Canvas，直接渲染 SVG path）
   ├─ content 层（绝对定位 div，含所有节点）
   ├─ edge_plus_buttons 层（「+」按钮，交互时隐藏）
   ├─ plus_tooltip 层
   ├─ toolbar 层
   └─ node_picker 层（AddingNodeFromEdge 浮层）
```

**分层原因**：

- 边在节点**下方**（先渲染边再渲染节点，z-order 自然正确）
- 「+」按钮在节点**上方**（可点击）
- 工具栏与浮层在最顶层

## ensure_panel_view 的生命周期

属性面板是独立实体，生命周期跟随选中状态：

```rust
fn ensure_panel_view(&mut self, entity, window, cx) -> Option<...> {
    if self.selected.is_none() {
        self.panel_view = None;  // 取消选中时销毁
        return None;
    }
    if self.panel_view.is_none() {
        // 创建面板（按节点 kind 分发 Generic/Start）
        self.panel_view = Some(create_panel(...));
    }
    // 同步节点数据到面板（防回环）
    panel.sync_from_node(node, window, cx);
}
```

面板创建后每帧调用 `sync_from_node`——但内部有**快速路径**：若 `node.data` 未变化直接返回，避免每帧更新所有 InputState。

## 缓存的更新时机

| 缓存 | 更新时机 | 用途 |
|------|----------|------|
| `cached_body_groups` | relayout 末尾 | 渲染/命中测试循环体 |
| `cached_all_body_nodes` | relayout 末尾 | 判断节点是否在循环体内 |
| `cached_hidden_nodes` | relayout 末尾 | 跳过收起节点的渲染与连线 |

拖动/平移等**不改变图结构**的交互不会触发 relayout，缓存保持有效——避免每帧 O(V+E) 遍历。

## 触发 relayout 的场景

| 场景 | 触发方式 |
|------|----------|
| `auto_layout()` | 直接调用 relayout |
| `set_graph()` | 替换图后 relayout |
| `insert_node_at_edge()` | 拆边插入后 relayout |
| `delete_node()` | 删除+桥接后 relayout |
| `set_layout_direction()` | 切换方向后 relayout |
| 属性面板编辑结构化字段 | `update_node_size_if_changed` 返回 true 时 relayout |

## 小结

渲染生命周期是 `sync_node_sizes → relayout → notify → render` 的循环。relayout 统一更新布局与缓存，render 分层组装画面。`update_node_size_if_changed` 提供单节点优化路径，避免每次按键都全量重排。面板实体随选中状态创建/销毁，sync_from_node 有快速路径防抖。

下一节：[命中测试交互模型](hit-test-interaction.md)
