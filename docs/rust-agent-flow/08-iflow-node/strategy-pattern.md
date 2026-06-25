# 策略模式与 IFlowNode

## 从一个问题开始

假设要在画布上渲染一个节点，编辑器需要知道：它长什么样？有哪些端口？端口在哪？选中后右侧面板显示什么？如果把这些逻辑全部塞进 `FlowEditorView`，会得到一个上千行的 `match node.kind { "start" => ..., "end" => ..., ... }`——每加一种节点就要改编辑器核心，违背开闭原则。

策略模式的解法：**把「节点如何渲染」抽象成 trait，编辑器只持有 trait object**。

```
FlowEditorView
   │
   │ 持有
   ▼
NodeRegistry  ──kind──▶  Arc<dyn IFlowNode>
                              │
                              │ 调用
                              ▼
                       get_view / get_panel / schema / port_position / ...
```

## IFlowNode trait 全貌

```rust
pub trait IFlowNode: Send + Sync {
    // 必须实现
    fn kind(&self) -> &str;
    fn get_view(&self, node: &Node, ctx: &mut NodeViewCtx) -> AnyElement;
    fn get_panel(&self, node: &Node, ctx: &mut NodeViewCtx) -> AnyElement;
    fn schema(&self) -> &NodeSchema;

    // 可选覆写（带默认实现）
    fn ports_for_node(&self, node: &Node) -> Vec<PortSpec> { self.schema().ports.clone() }
    fn port_position(&self, _node: &Node, _port_id: &PortId, _layout: LayoutDirection) -> Option<PointF> { None }
    fn content_size(&self, node: &Node) -> SizeF { node.size }
    fn plus_button_at_target(&self, _source_port: Option<&str>) -> bool { false }
}
```

`Send + Sync` 是硬性要求——`Arc<dyn IFlowNode>` 要跨线程传递（GPUI 实体可能在任意线程更新）。`AnyElement` 是 GPUI 的类型擦除元素，让不同节点返回异构视图。

## 方法职责一览

| 方法 | 必须实现 | 默认行为 | 何时覆写 |
|------|----------|----------|----------|
| `kind` | 是 | — | 永不覆写，作为注册表键 |
| `get_view` | 是 | — | 永远实现，画布卡片 |
| `get_panel` | 是 | — | 永远实现，可返回空 div（如 Start 由 PanelView 接管） |
| `schema` | 是 | — | 永远实现，定义端口与默认尺寸 |
| `ports_for_node` | 否 | 返回 `schema().ports` | 端口随数据变化（如 Condition 的 if_i） |
| `port_position` | 否 | `None`，用框架统一算法 | 端口需精确对齐视觉行（如 Condition/Loop） |
| `content_size` | 否 | `node.size` | 节点高度随数据变化（如 Condition 收起/展开） |
| `plus_button_at_target` | 否 | `false`，按钮在源侧 | 出口密集时按钮放目标侧避让（如 Loop 的 done） |

## 默认实现的取舍

四个可选方法都带「安全」的默认值，让简单节点（如 Action）只需实现 4 个必须方法即可工作：

```rust
// ActionNode 只实现了 4 个必须方法，其他全部走默认
impl IFlowNode for ActionNode {
    fn kind(&self) -> &str { "action" }
    fn get_view(&self, node: &Node, ctx: &mut NodeViewCtx) -> AnyElement { /* ... */ }
    fn get_panel(&self, node: &Node, ctx: &mut NodeViewCtx) -> AnyElement { /* ... */ }
    fn schema(&self) -> &NodeSchema { &self.schema }
}
```

但 Action 仍然覆写了 `port_position` 和 `content_size`——这两个方法**虽然默认可用，但覆写后能让端口位置与渲染完全对齐**（默认算法用节点边缘中点，与 Action 的实际渲染中心一致，但显式覆写更稳健、可读性更强）。

## schema 与运行时数据的关系

`schema()` 返回的 `NodeSchema` 是**静态声明**（端口列表、字段定义、默认尺寸），不依赖具体节点实例。`ports_for_node` 则是**运行时查询**，可读 `node.data` 动态生成端口：

```
schema().ports       ← 静态：注册时确定，用于面板/校验
ports_for_node(node) ← 动态：渲染时确定，用于实际连线
```

Condition 的 `ports_for_node` 根据 `node.data["conditions"]` 数组长度生成 `if_0, if_1, ...` 端口——schema 里只声明了默认 2 条，但运行时可以是任意数量。这是策略模式的精髓：**同一份 schema，不同实例不同行为**。

## 为何不用 enum

Rust 习惯用 `enum NodeKind { Start, End, ... }` 表示固定种类。但 rust-agent-flow 选择 `String` + trait object：

| 方案 | 优势 | 劣势 |
|------|------|------|
| enum + match | 编译期穷尽 | 加节点要改 core + 重编译 |
| String + trait object | 开放扩展，core 零改动 | 运行时 dispatch，无法穷尽检查 |

框架定位是「可扩展组件库」，**开放扩展**优先于穷尽性，故选 trait object。代价是 `kind` 字符串拼错时只能运行时发现（注册表返回 `None`）——但 schema 默认值与 `register_all` 集中注册已把风险压到最低。

## Send + Sync 与状态封装

`IFlowNode` 实现通常是**无状态的**（只有 `schema` 字段），状态全部存在 `Node.data` 里。这让 `Arc<dyn IFlowNode>` 可以被多个编辑器实例共享，无需克隆。

如果实现需要持有可变状态（如缓存），必须用 `Mutex`/`RwLock` 内部可变性——但内置 8 种节点都没这么做，渲染是纯函数 `(&Node, &NodeViewCtx) -> AnyElement`。

## 小结

`IFlowNode` 是策略模式的载体：4 个必须方法定义「节点是什么」，4 个可选方法定义「节点如何随数据变形」。默认实现让简单节点零成本接入，覆写方法让结构化节点精确控制端口与尺寸。`String` kind + trait object 牺牲穷尽性换取开放扩展，是框架可扩展性的基石。

下一节：[NodeRegistry 注册表](noderegistry.md)
