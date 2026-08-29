# rust-agent-flow 文档

本目录为 **rust-agent-flow：Rust 可视化流程/节点图框架（无 GPUI 依赖核心 + GPUI 渲染层）** 的 canonical 源，覆盖节点图数据模型、渲染与交互。

## 结构

```
docs/
└── rust-agent-flow/     # 文档根，对应 /api/docs/rust-agent-flow/*
    ├── FOREWORD.md       # 前言
    ├── INDEX.md          # 全书目录
    ├── INDEX.json        # 文档网站左侧菜单
    └── 01-introduction/  # 章节（共 16 章）
        ├── 02-quickstart/         # 快速上手
        ├── 03-philosophy/         # 设计理念
        ├── 04-architecture/       # 架构
        ├── 05-graph-model/        # 图数据模型
        ├── 06-schema-system/      # Schema 系统
        ├── 07-geometry-layout/    # 几何与布局
        ├── 08-iflow-node/         # IFlowNode 接口
        ├── 09-builtin-nodes/      # 内置节点
        ├── 10-custom-node/        # 自定义节点
        ├── 11-editor-view/        # 编辑器视图
        ├── 12-interaction/        # 交互
        ├── 13-edge-rendering/     # 边渲染
        ├── 14-panel/              # 面板
        ├── 15-extensions/         # 扩展能力
        └── 16-best-practices/     # 最佳实践
```

## 阅读

- [前言](rust-agent-flow/FOREWORD.md) — 了解本书定位、读者画像与阅读路径
- [全书目录](rust-agent-flow/INDEX.md)

## 阅读建议

- **快速上手**：从[第 2 章 快速上手](rust-agent-flow/02-quickstart/)开始，创建一个可运行的最小流程
- **数据模型**：重点阅读[第 5 章 图数据模型](rust-agent-flow/05-graph-model/)，理解节点 / 边 / 端口的核心抽象
- **最佳实践**：参考[第 16 章 最佳实践](rust-agent-flow/16-best-practices/)，含性能优化与常见陷阱排查

## 维护

编辑 `docs/rust-agent-flow/` 下的 Markdown 即可；Docbit 启动时会自动确保 `INDEX.json` 存在。