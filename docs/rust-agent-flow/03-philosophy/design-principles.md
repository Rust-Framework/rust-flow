# 核心设计原则

## 原则一：框架无关（Framework-Agnostic Core）

`rust-agent-flow`（core）不依赖任何 UI 框架。它只提供数据模型与算法：

```
core: FlowGraph + Schema + Geometry + Layout + Viewport
      （纯 Rust，零 GPUI 依赖）
```

这意味着：

- 图模型、边路径算法、布局引擎可被**任何** Rust 项目复用
- 未来可基于同一 core 实现不同渲染层（egui、WebAssembly + Canvas 等）
- core 的单元测试无需窗口，纯数据验证

`rust-agent-flow-gpui` 才引入 GPUI，实现 `FlowEditorView`。应用代码只依赖 gpui 层，但可向下穿透到 core 直接使用算法。

## 原则二：策略模式（Strategy Pattern）

节点行为通过 `IFlowNode` trait 多态分发，而非 match 分发：

```rust
pub trait IFlowNode: Send + Sync {
    fn kind(&self) -> &str;
    fn get_view(&self, node: &Node, ctx: &mut NodeViewCtx) -> AnyElement;
    fn get_panel(&self, node: &Node, ctx: &mut NodeViewCtx) -> AnyElement;
    fn schema(&self) -> &NodeSchema;
    // ...可选方法
}
```

`NodeRegistry` 按 `kind` 字符串匹配实现：

```rust
registry.register(Arc::new(StartNode::new()));
registry.register(Arc::new(MyCustomNode::new()));
// 渲染时：registry.get(&node.kind) → Arc<dyn IFlowNode>
```

好处：

- 新增节点类型**不修改框架代码**，只注册新实现
- `kind` 是 `String` 而非枚举——支持任意自定义类型，无需修改 core
- 内置 8 种节点与自定义节点走同一套分发路径

## 原则三：Schema 驱动（Schema-Driven）

节点的 `NodeSchema.fields` 描述 `node.data` 的字段结构，属性面板据此**自动生成**编辑界面：

```rust
NodeSchema::new("action", "Action")
    .with_field(FieldSpec::new("label", "Label", FieldType::Text))
    .with_field(FieldSpec::new("code", "Code", FieldType::CodeBlock))
    .with_field(FieldSpec::new("enabled", "Enabled", FieldType::Switch))
```

`FieldType` 映射到编辑控件：

| FieldType | 控件 |
|-----------|------|
| Text | 单行 Input |
| TextArea | 多行 Input |
| CodeEditor | 单行代码编辑器 |
| CodeBlock | 多行代码编辑器（带行号） |
| Number | 数字 Input |
| Switch | 布尔开关 |
| Dropdown | 下拉选择 |
| List | 动态列表（可增删行） |

这消除了传统的 per-kind 面板分发——所有节点共用一个 `PanelView`，按 schema.fields 渲染。新增节点只需声明字段，面板自动跟上。

## 原则四：能力与 UI 分离

框架**只提供能力方法**，UI 控件由调用侧实现：

| 能力方法 | UI 由谁实现 |
|----------|-------------|
| `toggle_theme()` | 调用侧的工具栏按钮 |
| `toggle_drag()` | 调用侧的工具栏按钮 |
| `set_language()` | 调用侧的工具栏按钮 |
| `set_graph()` | 调用侧的数据源选择器 |

框架内置工具栏只包含**框架级**操作（布局方向切换、缩放重置等）。拖拽开关、主题切换、语言切换、数据源选择——这些跟随目标应用产品的控件，由调用侧通过 `ToolbarProvider` 注入。

这是刻意的边界：框架不应假定你的应用需要什么样的工具栏布局。

## 原则五：单一数据源（Single Source of Truth）

图结构是唯一真相，所有派生数据通过缓存关联：

```
FlowGraph (真相)
  ├─ cached_body_groups     // 循环体分组
  ├─ cached_all_body_nodes  // 扁平化
  ├─ cached_hidden_nodes    // 收起的节点
  └─ version (版本计数)
```

`version` 在任何结构性变更时递增，用于失效缓存的几何数据。缓存在 `relayout` 末尾统一更新，避免每帧重复 O(V+E) 遍历。

## 原则六：声明式数据协议

`FlowDocument` 是流程图的**序列化协议**（JSON）：

- 节点用**数组索引**引用（而非 slotmap 内部 key），保证序列化稳定
- 包含 `version`（协议版本）、`metadata`（名称/描述）、`nodes`、`edges`
- `FlowGraph::from_document` / `to_document` 双向互转

这使流程定义可跨语言消费、可持久化到文件、可通过网络传输。

## 设计原则总览

| 原则 | 体现 | 收益 |
|------|------|------|
| 框架无关 | core 无 GPUI | 算法可复用 |
| 策略模式 | IFlowNode + Registry | 开放扩展 |
| Schema 驱动 | FieldSpec → 面板 | 消除分发 |
| 能力与 UI 分离 | setter + ToolbarProvider | 不侵入产品 |
| 单一数据源 | FlowGraph + 缓存 | 一致性 |
| 声明式协议 | FlowDocument JSON | 可持久化 |

## 小结

这六条原则贯穿整个框架。理解它们，你就能预测框架在新增需求时的设计走向：能用 Schema 声明的不写代码分发，能用 trait 注入的不硬编码，能下沉到 core 的不放在 gpui。

下一节：[ReactFlow 的启发](reactflow-inspiration.md)
