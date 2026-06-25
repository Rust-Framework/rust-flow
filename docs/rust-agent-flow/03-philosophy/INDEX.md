# 第三章 设计理念与哲学

本章解释 rust-agent-flow「为什么这样设计」，帮助你建立对框架决策的直觉。

## 本章小节

| 小节 | 内容 |
|------|------|
| [核心设计原则](design-principles.md) | 框架无关、策略模式、Schema 驱动 |
| [ReactFlow 的启发](reactflow-inspiration.md) | 边路径算法移植与概念对齐 |
| [GPUI 惯用法与所有权](gpui-idioms.md) | Entity、Context、命中测试与闭包约束 |
| [渐进式披露与框架边界](progressive-disclosure.md) | 框架只提供能力，UI 交给调用侧 |

## 学习目标

读完本章，你应能理解：

1. 为什么 core 层不依赖 GPUI？
2. 为什么用命中测试而非 per-element 事件闭包？
3. 为什么属性面板由 Schema 驱动而非 per-kind 分发？
4. 框架的「能力与 UI 分离」边界在哪里？

## 下一步

从 [核心设计原则](design-principles.md) 开始。
