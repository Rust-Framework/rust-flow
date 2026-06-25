# 第七章 几何与布局引擎

本章进入 `rust-agent-flow` 的「数学核心」：边路径算法、端口端点计算、dagre 自动布局与视口变换。这些算法全部位于 `crates/core`，零 GPUI 依赖，可被任何 Rust 项目复用。边路径算法直接移植自 ReactFlow `@xyflow/xyflow`，保证视觉与业界标准一致。

## 本章小节

| 小节 | 内容 |
|------|------|
| [边路径算法](edge-path-algorithms.md) | Bezier / Straight / Step / SmoothStep / LoopBack 路径生成 |
| [端口端点计算](port-calc.md) | Auto 方向推导与同侧 In/Out 端口分布 |
| [Dagre 布局引擎](dagre-layout.md) | Sugiyama 分层算法与 7 步后处理管线 |
| [Viewport 视口数学](viewport.md) | 平移、缩放与坐标变换 |

## 学习目标

读完本章，你应能：

- 说出五种边路径算法的输出点数与适用场景
- 解释 `bezier_path` 控制点偏移在反向连接时如何防塌缩
- 描述 `resolve_endpoints` 的四步流程与 In/Out 同侧分区策略
- 列出 dagre 后处理管线的 7 个阶段及其执行顺序
- 写出 `Viewport` 的 `to_screen`/`to_logical` 公式与 `zoom_around` 的锚点保持推导

## 前置知识

- 已阅读 [第五章 流程图数据模型](../05-graph-model/INDEX.md) 与 [第六章 Schema 与字段系统](../06-schema-system/INDEX.md)
- 了解三次/二次贝塞尔曲线的基本概念
- 了解有向图的拓扑排序

## 下一步

从 [边路径算法](edge-path-algorithms.md) 开始。
