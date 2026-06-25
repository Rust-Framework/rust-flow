# 第四章 架构全景

本章从全局视角理解 rust-agent-flow 的内部构造与渲染流转。

## 本章小节

| 小节 | 内容 |
|------|------|
| [Crate 分层结构](crate-layout.md) | 各 crate 职责与依赖方向 |
| [流程图数据模型](graph-model.md) | FlowGraph + Node/Edge/Port 三层抽象 |
| [渲染生命周期](render-lifecycle.md) | 从 relayout 到 render 的完整路径 |
| [命中测试交互模型](hit-test-interaction.md) | 画布统一处理事件，几何命中确定目标 |

## 学习目标

读完本章，你应能画出一张从数据变更到画面刷新的完整流程图，并说出每个环节的职责归属。

## 下一步

从 [Crate 分层结构](crate-layout.md) 开始。
