# Changelog

All notable changes to **rust-agent-flow** are documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [0.1.0] — 2026-08-16 — 首次 crates.io 发布

### Added

- **首次公开发布**：框架无关的核心 crate `rust-agent-flow` 首次发布到 crates.io。
  - `FlowGraph` 图模型（`NodeId` / `EdgeId` 稳定 slotmap 键）
  - `Viewport` 平移 + 缩放变换数学
  - 几何：边路径、命中测试、`port_calc`、障碍感知 A* 路由
  - 布局：`LayoutEngine` trait + `DagreLayout`
  - 架构：`NodeSchema` / `PortSpec` / `FieldSpec` 属性面板描述模型
- **发布口径**：仅发布框架无关核心（无 GPUI 依赖）。GPUI 渲染层（`rust-agent-flow-gpui`）
  依赖 zed GPUI 的 git 依赖，暂不发布到 crates.io。

> 核心 crate 无 GPUI 依赖，可复用于 CLI 工具或其他渲染后端。

[0.1.0]: https://github.com/Rust-Framework/rust-flow/releases/tag/v0.1.0