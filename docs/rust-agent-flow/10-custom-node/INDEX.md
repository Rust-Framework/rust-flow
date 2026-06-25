# 第十章 自定义节点开发

第八、九章分别讲了 `IFlowNode` 抽象与 8 种内置节点的实现。本章把这些知识落地——教你从零实现一个自定义节点，并精确控制动态端口与端口定位。

自定义节点是 rust-agent-flow 扩展性的终极体现：只需实现一个 trait、注册一次，就能让画布支持任意新节点类型，无需改框架一行代码。本章按「最小实现 → 动态端口 → 精确定位」的难度递增分三节，每节都基于前节的代码增量扩展。

## 本章小节

| 小节 | 内容 |
|------|------|
| [实现 IFlowNode](implement-iflownode.md) | kind/get_view/get_panel/schema 四要素与注册 |
| [动态端口与 ports_for_node](dynamic-ports.md) | 随数据变化的端口列表 |
| [port_position 与 content_size](port-position-size.md) | 精确端口定位与内容驱动尺寸 |

## 学习目标

读完本章，你应能独立实现一个自定义节点：定义 schema、渲染卡片、处理面板交互、按需覆写动态端口与位置方法。更重要的是，理解**何时该覆写哪个方法**——避免过度设计（简单节点无需覆写全部方法），也避免欠设计（结构化节点必须覆写 `content_size`）。

## 下一步

从 [实现 IFlowNode](implement-iflownode.md) 开始。
