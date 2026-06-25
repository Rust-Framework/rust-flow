# NodeRegistry 注册表

## 一个 HashMap 的故事

`NodeRegistry` 的实现极其简洁——一个 `HashMap<String, Arc<dyn IFlowNode>>`：

```rust
#[derive(Default)]
pub struct NodeRegistry {
    nodes: HashMap<String, Arc<dyn IFlowNode>>,
}

impl NodeRegistry {
    pub fn new() -> Self { Self::default() }

    pub fn register(&mut self, node: Arc<dyn IFlowNode>) {
        let kind = node.kind().to_string();
        self.nodes.insert(kind, node);
    }

    pub fn get(&self, kind: &str) -> Option<Arc<dyn IFlowNode>> {
        self.nodes.get(kind).cloned()
    }

    pub fn port_specs_for(&self, kind: &str) -> Vec<PortSpec> {
        self.nodes.get(kind)
            .map(|n| n.schema().ports.clone())
            .unwrap_or_default()
    }
}
```

没有泛型、没有宏、没有生命周期体操——这就是 trait object 的力量。

## 注册时机

`FlowEditorView::new` 内部调用 `builtin::register_all`，把 8 种内置节点一次性注册：

```rust
pub fn register_all(registry: &mut NodeRegistry) {
    registry.register(Arc::new(StartNode::new()));
    registry.register(Arc::new(EndNode::new()));
    registry.register(Arc::new(ActionNode::new()));
    registry.register(Arc::new(ConditionNode::new()));
    registry.register(Arc::new(LoopNode::new()));
    registry.register(Arc::new(VariableNode::new()));
    registry.register(Arc::new(AdapterNode::new()));
    registry.register(Arc::new(AgentNode::new()));
}
```

注册顺序无关——查找是 O(1) HashMap 查询。`Arc` 让同一实现被多个编辑器实例共享，零拷贝。

## 注册表与 FlowEditorView 的关系

```
FlowEditorView
├── graph: FlowGraph           ← 数据（节点/边）
├── registry: NodeRegistry     ← 渲染策略（kind → IFlowNode）
└── render(): 每帧
      └─ for node in graph.nodes():
            registry.get(&node.kind) → Arc<dyn IFlowNode>
            flow_node.get_view(node, ctx) → AnyElement
```

`graph` 存「有什么」，`registry` 存「怎么画」——两者通过 `kind` 字符串解耦。删除 graph 里的节点不影响 registry；替换 registry 不影响 graph 数据。

## 查找的两种用途

| 调用方 | 方法 | 用途 |
|--------|------|------|
| 渲染层 | `get(kind)` → `get_view`/`get_panel` | 画卡片、画面板 |
| 渲染层 | `get(kind)` → `port_position` | 计算连线端点 |
| 渲染层 | `get(kind)` → `content_size` | 同步节点尺寸 |
| 布局层 | `get(kind)` → `ports_for_node` | 推导 Auto 端口方向 |
| 面板层 | `port_specs_for(kind)` | Schema 校验、字段编辑 |

注意 `port_specs_for` 用的是 `schema().ports`（静态），而布局推导应用 `ports_for_node`（动态）——这是 Condition 节点能动态扩展端口的关键。

## 自定义节点的注册

自定义节点只需在 `FlowEditorView::new` 后追加注册：

```rust
let mut view = FlowEditorView::new(window, cx);
view.registry.register(Arc::new(MyCustomNode::new()));
```

如果 kind 与内置节点冲突，**后注册者覆盖前者**（HashMap::insert 语义）。这让你可以替换内置节点的视觉表现而不改框架代码——例如把 Action 的卡片改成带进度条的版本。

## specs_fn 的设计警示

`NodeRegistry::specs_fn` 返回一个闭包，但实现是占位的：

```rust
pub fn specs_fn(&self) -> impl Fn(NodeId) -> Vec<PortSpec> + '_ {
    |_| Vec::new()  // 占位
}
```

注释说明：registry 不持有 graph，无法单独按 NodeId 查询端口。**实际使用时由 FlowEditorView 构造捕获 graph + registry 的闭包**：

```rust
// 概念示意（实际由 resolve_endpoints 调用方构造）
let specs_fn = |node_id: NodeId| -> Vec<PortSpec> {
    let kind = &self.graph.node(node_id).unwrap().kind;
    self.registry.get(kind)
        .map(|n| n.ports_for_node(self.graph.node(node_id).unwrap()))
        .unwrap_or_default()
};
```

这是一个**有意的接口边界**：registry 是纯查表，graph 是纯数据，把两者结合的闭包由调用方持有——避免 registry 持有 graph 引用导致的生命周期纠缠。

## Arc 克隆的成本

`get` 返回 `Option<Arc<dyn IFlowNode>>`，每次查找都 `Arc::clone`。这是原子引用计数加一，开销约 10-20ns。每帧渲染 N 个节点就有 N 次克隆——对典型流程图（<100 节点）完全可忽略。

如果未来需要极致性能，可以改用 `&Arc<dyn IFlowNode>` 借用（需改 API 返回引用），但当前设计的简单性远比微秒级优化重要。

## 小结

`NodeRegistry` 用最朴素的 HashMap 实现策略分发的核心。`register_all` 集中注册 8 种内置节点，自定义节点通过 `register` 追加。registry 与 graph 通过 `kind` 字符串解耦，specs_fn 占位警示了 registry 不持有 graph 的接口边界。`Arc` 共享让多编辑器实例零成本复用实现。

下一节：[NodeViewCtx 与 NodeAction](nodeviewctx-action.md)
