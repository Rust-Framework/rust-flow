# 主视图结构

`FlowEditorView` 是一个持有大量状态的 GPUI 视图。理解它的字段构成，是理解后续渲染与交互的前提。

## 字段总览

按职责分组，`FlowEditorView` 的字段大致可以划分为五类：

| 类别 | 字段 | 职责 |
|------|------|------|
| 数据模型 | `graph: FlowGraph` | 被编辑的有向图 |
| 视图变换 | `viewport: Viewport` | offset / scale / 逻辑↔屏幕变换 |
| 交互状态 | `interaction: InteractionState` | 当前所处的交互阶段（Idle/Panning/Dragging/...） |
| 选择状态 | `selected`, `hovered`, `hovered_plus` | 当前选中节点、悬停节点、悬停的「+」边 |
| 节点元信息 | `registry: Arc<NodeRegistry>` | 节点 schema、内容尺寸、默认数据 |
| 渲染配置 | `default_edge_type`, `layout_direction`, `show_grid`, `grid_spacing`, `drag_enabled` | 渲染风格开关 |
| 主题与语言 | `theme`, `language`, `syntax_service` | 视觉风格与语法高亮 |
| 面板 | `panel_view`, `panel_width`, `resizing_panel` | 右侧属性面板 |
| 布局缓存 | `cached_body_groups`, `cached_all_body_nodes`, `cached_hidden_nodes` | 折叠后的子图映射 |
| 扩展点 | `custom_toolbar`, `data_type_provider` | 外部插件注入 |

## 构造流程

`FlowEditorView::new(graph, cx)` 做的事情远不止「把 graph 存起来」：

```rust
pub fn new(graph: FlowGraph, cx: &mut Context<Self>) -> Self {
    let mut registry = NodeRegistry::new();
    register_all(&mut registry); // 注册所有内置节点类型

    let mut view = Self {
        graph,
        viewport: Viewport::default(),
        interaction: InteractionState::Idle,
        registry: Arc::new(registry),
        selected: None,
        hovered: None,
        hovered_plus: None,
        default_edge_type: EdgeType::SmoothStep,   // 默认 SmoothStep
        layout_direction: LayoutDirection::Horizontal,
        show_grid: true,
        grid_spacing: 24.0,
        drag_enabled: true,
        theme: Theme::default(),
        panel_view: None,
        syntax_service: cx.default_syntax_service(),
        language: Language::Rust,
        cached_body_groups: HashMap::new(),
        cached_all_body_nodes: HashSet::new(),
        cached_hidden_nodes: HashSet::new(),
        custom_toolbar: Vec::new(),
        data_type_provider: None,
        panel_width: Pixels(320.0),
        resizing_panel: false,
    };
    view.relayout();           // 首次布局
    view
}
```

关键点：构造时立即调用 `relayout()`，确保第一帧渲染时所有节点都已有合法 position。

## 布局：relayout 的三段式

`relayout()` 是主视图与 Dagre 布局器的握手点，分三步：

```
┌──────────────────────────────────────────────────────┐
│ 1. sync_node_sizes()                                 │
│    遍历所有节点，用 IFlowNode::content_size 同步      │
│    node.size。保证布局器拿到准确尺寸。                 │
├──────────────────────────────────────────────────────┤
│ 2. DagreLayout.layout(&mut graph, direction)         │
│    调用 Dagre 算法，回填每个节点的 position。          │
├──────────────────────────────────────────────────────┤
│ 3. 更新三份缓存                                       │
│    cached_body_groups:  NodeId -> 折叠子节点集合       │
│    cached_all_body_nodes: 所有 body 节点扁平集合       │
│    cached_hidden_nodes:  当前被折叠隐藏的节点集合      │
└──────────────────────────────────────────────────────┘
```

为什么需要这三份缓存？因为折叠节点是「把子图收进父节点」的操作，渲染时需要快速判断：

- 这个节点是否是某折叠节点的 body（属于 `cached_all_body_nodes`）？
- 这个节点的 body 集合是什么（查 `cached_body_groups`）？
- 这个节点当前应不应该被画出来（不在 `cached_hidden_nodes` 里）？

### 单节点尺寸优化

`sync_node_sizes()` 会遍历所有节点，开销不可忽视。当用户只是修改了单个节点的内容（比如编辑了一段代码），调用 `relayout()` 太重。于是有了 `update_node_size_if_changed(node_id)`：

```rust
pub fn update_node_size_if_changed(&mut self, node_id: NodeId) -> bool {
    // 仅当该节点 content_size 真正变化时才返回 true
    // 调用方据此决定是否需要触发完整 relayout
}
```

返回值约定：`true` 表示尺寸变了，调用方应再触发一次 `relayout()`；`false` 表示可跳过。

## 公开 API 一览

| 方法 | 作用 |
|------|------|
| `auto_layout(cx)` | `relayout()` + `cx.notify()`，对外暴露的「重新布局」入口 |
| `set_layout_direction(dir)` | 切换横向 / 纵向布局 |
| `set_drag_enabled(b)` | 开关节点拖拽 |
| `set_grid_spacing(f)` | 调整网格密度 |
| `set_show_grid(b)` | 显示 / 隐藏网格 |
| `toggle_theme()` / `set_theme(t)` | 切换主题 |
| `set_syntax_service(s)` | 注入语法高亮服务 |
| `add_toolbar_provider(p)` | 注册自定义工具栏项 |
| `set_data_type_provider(p)` | 注入数据类型推断器 |
| `set_language(l)` / `toggle_language()` | 切换代码语言 |
| `set_graph(graph, cx)` | 整体替换图，并重置 selected/hovered/panel/viewport |
| `insert_node_at_edge(edge, kind, cx)` | 在边上插入节点 |
| `handle_node_action(node, action, cx)` | 分发 Delete / ToggleCollapse / SetData |

## port_sides：方向决定端口朝向

```rust
pub fn port_sides(&self) -> (PortSide, PortSide) {
    match self.layout_direction {
        LayoutDirection::Horizontal => (PortSide::Right, PortSide::Left),
        LayoutDirection::Vertical   => (PortSide::Bottom, PortSide::Top),
    }
}
```

返回 `(out_side, in_side)`。横向布局时输出端口在右、输入端口在左；纵向布局时输出在下、输入在上。这个函数贯穿边几何与命中测试，是「方向感」的单一事实来源。

## set_graph：彻底重置

`set_graph(graph, cx)` 不是简单赋值，而是「软重启」：

```
set_graph
 ├─ graph = new_graph
 ├─ selected   = None
 ├─ hovered    = None
 ├─ hovered_plus = None
 ├─ panel_view = None
 ├─ viewport   = Viewport::default()   // 回到原点，scale=1
 └─ relayout()
```

这保证切换图时不会残留旧图的选择状态或视口偏移，避免「我看到选中了一个不存在的节点」这类 bug。

## 小结

`FlowEditorView` 是一个「胖视图」：它直接持有图、视口、交互状态、缓存和扩展点，没有把它们拆到多个 Entity。这种设计牺牲了一些解耦，换来的是渲染与交互之间零成本的数据共享——这正是节点编辑器这种「高频命中测试 + 高频重绘」场景所需要的。

下一节：[渲染管线](render-pipeline.md)
