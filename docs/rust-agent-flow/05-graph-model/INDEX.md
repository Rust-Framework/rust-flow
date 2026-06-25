# 第五章 流程图数据模型

本章深入 `rust-agent-flow` 的图数据底层：基于 slotmap 稳定键的 `FlowGraph`、`Node`/`Edge`/`Port` 三要素，以及与 `FlowDocument` 的双向互转。第四章已给出全局概览，本章把每一层拆开讲透。

## 本章小节

| 小节 | 内容 |
|------|------|
| [FlowGraph 与 slotmap 键](flowgraph.md) | 稳定键、版本计数器与增删改查 API |
| [Node / Edge / Port 三要素](node-edge-port.md) | 节点、边、端口的方向、侧与可选性 |
| [FlowDocument 互转](document-interop.md) | JSON 序列化、索引引用与循环体分组 |

## 学习目标

读完本章，你应能：

- 说出 slotmap 相对 `Vec` 在删除场景下的稳定性收益
- 解释 `version` 计数器驱动缓存失效的工作机制
- 区分 `EdgeType`（路径算法）与 `EdgeKind`（语义类型）的职责
- 写出 `FlowGraph::from_document` 中索引到 `NodeId` 的映射过程
- 说明 `loop_body_groups` 为什么是循环体渲染的唯一数据源

## 前置知识

- 已阅读 [第四章 架构全景](../04-architecture/INDEX.md)
- 了解 Rust 的所有权与 `SlotMap` 基本用法

## 下一步

从 [FlowGraph 与 slotmap 键](flowgraph.md) 开始。
