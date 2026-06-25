# 第八章 IFlowNode 扩展接口

第三部分我们梳理了流程图的数据模型与布局引擎——那是「图的骨架」。本章进入「图的皮肤」：每个节点 kind 对应一个 `IFlowNode` 实现，负责把 `Node`（数据）翻译成画布上的卡片与右侧属性面板。

rust-agent-flow 没有把渲染逻辑写死在编辑器里，而是用 **策略模式 + trait object** 抽象出来：编辑器只持有 `Arc<dyn IFlowNode>`，按 `kind` 字符串分发。这种设计带来三个直接收益：

- 内置 8 种节点与未来任意自定义节点走同一条渲染路径
- 同一个数据图可以被不同渲染层复用（core 完全不依赖 gpui）
- 替换某一种节点的视觉表现，无需改动编辑器主视图

## 本章小节

| 小节 | 内容 |
|------|------|
| [策略模式与 IFlowNode](strategy-pattern.md) | trait 设计、方法职责与默认实现取舍 |
| [NodeRegistry 注册表](noderegistry.md) | kind → `Arc<dyn IFlowNode>` 的注册/查找/注入 |
| [NodeViewCtx 与 NodeAction](nodeviewctx-action.md) | 渲染上下文、动作回调与编辑器处理路径 |

## 学习目标

读完本章，你应能画出从 `Node` 到屏幕像素的完整调用链，说清楚 `ports_for_node`/`port_position`/`content_size`/`plus_button_at_target` 四个可覆写方法各自的默认行为与何时该覆写，并理解 `NodeAction` 如何把视图层动作安全地回传到编辑器。

## 下一步

从 [策略模式与 IFlowNode](strategy-pattern.md) 开始。
