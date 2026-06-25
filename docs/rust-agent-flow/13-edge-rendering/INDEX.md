# 第13章 边渲染与连线

边是节点编辑器的「血管」。一条边不仅要画出从源端口到目标端口的连线，还要处理折线路由、圆角、箭头、循环回环、以及在边上插入新节点所需的「+」按钮。本章把这三件事拆开讲清楚：

1. `EdgeView` 与边类型（SmoothStep / Straight / Bezier 等）如何决定路径形状。
2. 循环回环边（`EdgeKind::LoopBack`）为什么需要独立的 U 形路由算法。
3. 边「+」按钮如何定位、命中、并触发 `insert_node_at_edge` 流程。

## 本章小节

| 小节 | 内容 |
|------|------|
| [EdgeView 与边类型](edgeview-types.md) | EdgeView 结构、EdgeType 路径算法、逻辑坐标计算 |
| [循环回环边](loop-back.md) | LoopBack 的 U 形路由、theme.edge_loop_back 着色 |
| [边「+」按钮与插入节点](plus-button.md) | plus_button_at_target 定位、命中半径、insert_node_at_edge 全流程 |

## 学习目标

读完本章后，你应当能够：

- 说出 `EdgeType::SmoothStep` 的圆角与 step gap 是如何在逻辑坐标计算的。
- 解释为什么边路径要「先在逻辑坐标算好，再用 PathBuilder::scale + translate 整体变换」。
- 描述 `EdgeKind::LoopBack` 的 U 形路由几何，并说明它为什么不复用普通边算法。
- 写出 `insert_node_at_edge` 的四步：读原边 → 删原边 → 建新节点（schema 默认数据）→ 连两条新边 → relayout。
- 在自己的节点实现里正确提供 `plus_button_at_target`，决定「+」按钮出现在源侧还是目标侧。

## 关键源码定位

| 模块 | 路径 |
|------|------|
| 边渲染 | `crates/gpui/src/editor/rendering/edges.rs` |
| 边几何 | `crates/gpui/src/editor/rendering/edge_geometry.rs` |
| 图操作 | `crates/gpui/src/editor/graph_ops.rs` |
| 节点 trait | `crates/gpui/src/editor/node_traits.rs`（`IFlowNode::plus_button_at_target`） |

## 下一步

从 [EdgeView 与边类型](edgeview-types.md) 开始。
