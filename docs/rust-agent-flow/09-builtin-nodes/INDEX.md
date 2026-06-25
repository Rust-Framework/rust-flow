# 第九章 内置节点详解

第八章我们看了 `IFlowNode` trait 的抽象设计。本章逐个拆解 `builtin::register_all` 注册的 8 种内置节点——它们构成了图灵完备控制流的最小完备集：顺序、分支、循环、变量、适配、智能体。

8 种节点按复杂度递增分四组：基础顺序流（Start/End/Action）、结构化分支（Condition）、结构化循环（Loop）、配置型节点（Variable/Adapter/Agent）。每一组都遵循「标题栏 + 主体」统一布局，但端口拓扑和尺寸推导各有差异。

## 本章小节

| 小节 | 内容 |
|------|------|
| [Start / End / Action](start-end-action.md) | 顺序流三件套：起点、终点、步骤 |
| [Condition 条件分支](condition-branch.md) | 动态端口、多分支出口、收起态 |
| [Loop 循环迭代](loop-iteration.md) | 4 端口拓扑、循环体支线、回环边 |
| [Variable / Adapter / Agent](variable-adapter-agent.md) | 变量、数据适配、智能体配置 |

## 学习目标

读完本章，你应能说出每种节点的端口拓扑、`port_position` 算法、`content_size` 推导逻辑，以及哪些节点覆写了 `ports_for_node`/`plus_button_at_target`。更重要的是，理解**为什么** Condition 覆写 `ports_for_node` 而 Loop 不覆写、**为什么** Loop 覆写 `plus_button_at_target` 而 Condition 不覆写——这些差异背后是对视觉冲突与布局正确性的精确权衡。

## 下一步

从 [Start / End / Action](start-end-action.md) 开始。
