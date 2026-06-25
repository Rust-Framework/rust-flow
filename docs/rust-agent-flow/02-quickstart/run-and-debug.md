# 运行、调试与验证

## 运行 Demo

最快验证框架的方式是运行内置 demo：

```bash
cargo run -p rust-agent-flow-demo
```

首次编译需数分钟（GPUI 从 git 编译）。后续增量编译约 10-30 秒。

## Demo 的三种预置流程

Demo 通过 `DemoDataSource` 枚举提供三种流程，在工具栏切换：

| 数据源 | 演示内容 |
|--------|----------|
| `AgentFlow` | 完整 Agent 编排：Start→Planner→Condition→三路汇合→Loop→Summarize→End |
| `DataPipeline` | 数据管道：顺序处理 + 条件分流 |
| `SimpleFlow` | 最简流程：Start→Action→End |

切换数据源会调用 `FlowEditorView::set_graph`，重置视口并自动重排。

## 交互验证清单

运行后，按以下清单验证交互是否正常：

| 操作 | 预期效果 |
|------|----------|
| 中键拖拽 | 画布平移，光标变 grabbing |
| 滚轮向上 | 以鼠标为锚点放大 |
| 滚轮向下 | 以鼠标为锚点缩小 |
| 左键点击节点 | 选中（边框高亮），右侧显示属性面板 |
| 左键拖拽节点 | 移动节点位置 |
| 左键拖拽出端口 → 入端口 | 创建连线 |
| 悬停边中点「+」按钮 | 显示手型光标 + tooltip |
| 点击「+」按钮 | 弹出节点选择浮层 |
| 点击浮层中的节点类型 | 在边中间插入新节点 |
| 悬停节点 | 显示删除按钮（×） |
| 点击删除按钮 | 删除节点（自动桥接连线 + 重排） |
| 点击 Condition/Loop 的 ▽ 按钮 | 收起/展开 |
| 拖拽面板分隔条 | 调整属性面板宽度 |

## 调试技巧

### 1. 检查图结构

在代码中打印图的状态：

```rust
println!("节点数: {}", graph.nodes().count());
println!("边数: {}", graph.edges().count());
for node in graph.nodes() {
    println!("  {:?} kind={} pos={:?}", node.id, node.kind, node.position);
}
```

### 2. 导出 FlowDocument 验证

```rust
let doc = graph.to_document("debug");
println!("{}", serde_json::to_string_pretty(&doc).unwrap());
```

检查 JSON 中的 `nodes`（kind/data）与 `edges`（source/target 索引、端口）是否符合预期。

### 3. 布局结果检查

`auto_layout` 后检查节点位置：

```rust
editor.auto_layout(cx);
for node in editor.graph.nodes() {
    println!("{} → ({}, {})", node.kind, node.position.x, node.position.y);
}
```

dagre 会按拓扑分层排列，横向布局从左到右。

### 4. 命中测试调试

若点击无响应，检查 `to_logical` 坐标转换是否正确：

```rust
let logical = editor.to_logical(event.position);
println!("屏幕 {:?} → 逻辑 {:?}", event.position, logical);
```

### 5. 端口位置验证

端口位置由 `IFlowNode::port_position` 决定。若连线端点错位，检查节点实现是否覆写了 `port_position` 并返回正确的逻辑坐标。

## 常见运行问题

### 窗口黑屏

- 确认调用了 `rust_agent_flow_gpui::init(cx)`
- 确认用 `gpui_component::Root::new(view, window, cx)` 包裹视图
- 确认 `.with_assets(CombinedAssets)` 注入了资源

### 节点堆在原点

- 未调用 `auto_layout(cx)`，dagre 未运行
- 调用 `editor.auto_layout(cx)` 或 `editor.set_graph(graph, cx)`（内部会 relayout）

### 连线端点错位

- 节点尺寸未同步：检查 `content_size` 是否正确反映渲染高度
- 端口位置错误：检查 `port_position` 返回的逻辑坐标

### 图标不显示

- 未注入 `CombinedAssets`：`.with_assets(CombinedAssets)`
- 节点 kind 未在 `node_icon()` 映射中（自定义节点需提供图标）

## 小结

运行 demo 是验证环境的最快方式。开发中遇到问题时，优先用 `to_document` 导出 JSON 检查图结构，用坐标打印验证命中测试，用位置打印验证布局结果。

下一章：[设计理念与哲学](../03-philosophy/INDEX.md)
