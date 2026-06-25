# 性能优化技巧

流程编辑器是「高频重绘 + 大量实体」的场景：每帧都要遍历节点画边、每次按键都触发数据流。rust-agent-flow 在源码里内置了多处优化，理解它们既能避免重复造轮子，也能指导你在自定义节点/扩展时不再踩到性能反模式。本节按「缓存命中、单点更新、面板生命周期、交互态省略」四类梳理。

## 优化一：relayout 末尾缓存几何分组

**问题**：渲染每帧都要知道「哪些节点属于哪个循环体」「哪些节点被隐藏」「画布分几组」。若每帧从图重新计算，是 O(V+E) 的 HashSet 收集，原本每帧重复 4-5 次。

**框架做法**：在 `relayout` 末尾一次性算好，存进字段：

| 缓存字段 | 内容 | 计算时机 |
|----------|------|----------|
| `cached_body_groups` | `HashMap<NodeId, HashSet<NodeId>>` 循环体分组 | relayout 末尾 |
| `all_body_nodes` | `HashSet<NodeId>` 所有循环体内节点 | relayout 末尾 |
| `hidden_nodes` | `HashSet<NodeId>` 被折叠隐藏的节点 | relayout 末尾 |

渲染时直接 `&cached_body_groups` 引用，零计算。结构性变更（增删节点/边、折叠/展开）触发 `relayout`，缓存随之更新。

**借鉴**：自定义节点若需类似的全局分组信息，不要在 `render` 里现算——在 `relayout` 钩子里算好缓存，render 只读。

## 优化二：render 用引用避免每帧 clone

**问题**：`cached_body_groups` 是 `HashMap`，若 render 每帧 `clone()` 整个 map，大图下开销惊人。

**框架做法**：render 用 `&self.cached_body_groups` 借用，不 clone。因为 render 持有 `&self`，缓存字段不可变，借用安全。

```rust
// 错误：每帧 clone
let groups = self.cached_body_groups.clone();
for (id, body) in &groups { ... }

// 正确：直接借用
let groups = &self.cached_body_groups;
for (id, body) in groups { ... }
```

**借鉴**：自定义渲染里凡是大集合（HashMap/Vec/HashSet），优先考虑能否在 render 期间借用而非 clone。

## 优化三：update_node_size_if_changed 单点检查

**问题**：用户在面板改一个字段，节点尺寸可能变化（如多行文本变长）。若每次按键都全量 `sync_node_sizes` + `relayout`，大图卡顿。

**框架做法**：`update_node_size_if_changed` 只检查**单个节点**的尺寸是否变化，变了才触发该节点相关的局部更新，避免全量 relayout。

**借鉴**：处理「可能引发布局变化的编辑」时，先做单点检查，确认变化再按最小范围触发更新，而非无脑全量重算。

## 优化四：sync_from_node 双路径

**问题**：`ensure_panel_view` 每帧调 `sync_from_node`，若无脑更新所有 InputState，每帧都重置输入框，光标乱跳且浪费。

**框架做法**：两条路径配合：

1. **快速路径**：`self.node.data == node.data` 直接返回，零成本跳过。
2. **慢路径**：逐字段比较，仅在实际变化时 `set_value`，用户正在编辑的字段因值已最新而跳过，光标不动。

```rust
pub fn sync_from_node(&mut self, node, window, cx) {
    if self.node.data == node.data { return; }   // 快速路径
    self.syncing = true;
    // 慢路径：逐字段比较，仅变化时 set_value
    ...
    self.syncing = false;
}
```

**借鉴**：任何「外部数据同步到 UI 状态」的逻辑都应有「全等快速返回 + 逐项按需更新」的双路径，避免无差别 set_value。

## 优化五：panel_view 随选中创建/销毁

**问题**：属性面板持有大量 InputState、Subscription。若常驻，不选中时也占内存、参与事件分发。

**框架做法**：`panel_view: Option<PanelEntity>`，选中节点时 `Some`，取消选中或注入扩展时 `None`。不选中时面板不存在，零开销。

```
选中节点   → ensure_panel_view → panel_view = Some(PanelView::new(...))
取消选中   → panel_view = None  （Subscription 随之释放）
注入扩展   → panel_view = None  → 下帧重建
```

**借鉴**：设计「上下文相关 UI」时，用 `Option<Entity>` 表达存在性，按需创建销毁，而非常驻隐藏。

## 优化六：交互中不渲染边「+」按钮

**问题**：边中点的「+」按钮（用于插入节点）每条边一个，大图下数百个按钮参与命中测试与渲染。拖拽/平移时根本用不到，却白白消耗。

**框架做法**：在 `DraggingNode` / `Panning` 等交互状态下，render 跳过边「+」按钮的渲染：

```rust
// 伪代码
let show_edge_plus = !matches!(self.interaction, InteractionState::DraggingNode(_) | InteractionState::Panning { .. });
if show_edge_plus {
    for edge in edges { render_edge_plus(edge) }
}
```

交互结束恢复渲染。这把交互期间的渲染负载从 O(E) 降到接近 0。

**借鉴**：任何「非交互态才需要」的装饰元素（按钮、提示、悬浮控件），在交互态显式省略渲染。

## 优化七：render 期间不持有 flow_node 借用

**问题**：`render_schema_panel` 需要遍历 `schema.fields`，但 `flow_node: Option<Arc<dyn IFlowNode>>` 若在循环里持有借用，后续 `&mut self` 调用 `render_field` 会编译失败（借用冲突）。

**框架做法**：先 clone 出 `Vec<FieldSpec>` 再遍历，释放 `self.flow_node` 借用：

```rust
let fields: Vec<FieldSpec> = match &self.flow_node {
    Some(fn_) => fn_.schema().fields.clone(),   // clone 后借用立即结束
    None => Vec::new(),
};
for (i, field) in fields.iter().enumerate() {
    col = col.child(self.render_field(i, field, ...));  // &mut self 可用
}
```

`Vec<FieldSpec>` 的 clone 是浅拷贝几个字段描述，开销远小于「持有借用导致无法 &mut self」的设计代价。

**借鉴**：render 里既要读 schema 又要调 `&mut self` 方法时，先 clone 出所需数据再进入循环。

## 性能反模式自查

自定义节点/扩展时避开这些：

| 反模式 | 后果 | 正解 |
|--------|------|------|
| render 里现算全局分组 | 每帧 O(V+E) | relayout 缓存，render 借用 |
| render 里 clone 大 HashMap | 每帧大拷贝 | 用 `&` 借用 |
| 每次按键全量 relayout | 大图卡顿 | 单点检查 `update_node_size_if_changed` |
| sync 时无脑 set_value | 光标乱跳 | 双路径：全等返回 + 按需更新 |
| 面板常驻不销毁 | 内存与事件开销 | `Option<Entity>` 按需创建销毁 |
| 交互态仍渲染装饰按钮 | 命中测试开销 | 交互态省略非必要元素 |

## 小结

框架的性能优化围绕「避免每帧重复计算」「避免无差别更新」「按需存在」「交互态减负」四条思路。relayout 末尾缓存几何分组、render 借用而非 clone、单点尺寸检查、sync 双路径、panel_view 随选中生灭、交互态省略「+」按钮——这七处已内建，自定义时应顺着同一思路，避免在错误的地方自己造轮子。

下一节：[Demo 案例研究](demo-case-study.md)
