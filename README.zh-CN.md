[English](README.md) | **简体中文**

# rust-agent-flow

[![License](https://img.shields.io/badge/license-MIT-blue.svg)](/LICENSE)

RustAgentFlow 是一个基于 **GPUI**（Zed 编辑器背后的 UI 框架）与 **gpui-component** 构建的**框架无关的工作流设计组件库**。它为开发者提供了一个 ReactFlow 风格的视觉流程设计器，并将纯粹图核心与 GPUI 渲染层清晰分离。

图核心的图模型、几何计算与布局是渲染器无关的，因此同一个 `FlowGraph` 可以驱动任何未来的渲染后端。GPUI 层则把该模型转化为一个可交互的画布编辑器，支持平移 / 缩放、端口拖拽连线、障碍感知边路由、由 Schema 驱动的属性面板、国际化与主题。

```
rust-agent-flow        → 图模型、视口数学、几何、布局（不依赖 GPUI）
rust-agent-flow-gpui   → 画布渲染器、FlowEditorView、交互状态机
```

## 为什么

大多数工作流 UI 被构建为一个大而紧耦合的控件。rust-agent-flow 将问题拆分开来：

- **`core`** 保持框架无关——纯数据结构（`FlowGraph`）、几何（`PointF` / `RectF`）、边路径算法与可插拔的布局引擎。它易于单元测试，也可被命令行工具或其它后端复用。
- **`gpui`** 负责所有可视化内容——点阵网格画布、交互状态机、命中测试、节点 / 边渲染以及属性面板。

边渲染借用了 gpui-component 图表（`Catmull-Rom → 立方 Bézier`）与 ReactFlow 风格端口感知 Bézier 曲线的算法，并在网格上加入障碍感知的 A* 路由。

## 架构

```
┌──────────────────────────────────────────────────────────┐
│ rust-agent-flow-gpui   (渲染器，控件中不含业务逻辑)       │
│  FlowEditorView ─ FlowGraph + Viewport + 交互状态机      │
│  NodeRegistry / IFlowNode       (策略模式)               │
│  Schema 驱动的属性面板           (FieldSpec 渲染)         │
│  工具栏扩展、主题、i18n (en / zh)                         │
├──────────────────────────────────────────────────────────┤
│ rust-agent-flow  (core — 框架无关，不依赖 GPUI)          │
│  FlowGraph      稳定 slotmap key (NodeId / EdgeId)       │
│  Viewport       平移 + 缩放变换数学                      │
│  geometry       边路径、命中测试、port_calc、路由         │
│  layout         LayoutEngine trait + DagreLayout         │
│  schema         NodeSchema / PortSpec / FieldSpec        │
└──────────────────────────────────────────────────────────┘
```

### rust-agent-flow（核心层）

- **`FlowGraph`** —— 使用稳定 `slotmap` key 的节点与有向边，带单调版本计数器用于缓存失效，并完整支持 `FlowDocument`（反）序列化互转。
- **`schema`** —— 声明式节点 Schema（`NodeSchema`、`PortSpec`、`FieldSpec`、`FlowDocument`）。`NodeSchema.fields` 驱动自动生成的属性面板编辑器（Text / TextArea / CodeEditor / CodeBlock / Number / Switch / Dropdown / List）。
- **`geometry`** —— 基于 f32 的 `PointF` / `SizeF` / `RectF`、边路径算法（`bezier`、`straight`、`step`、`smoothstep`、`loop_back`、`round_corners`）、命中测试，以及用于多输出端口侧分布计算的 `port_calc`。
- **`routing`** —— 在网格上做障碍感知的 A* 边路由，带网格单元尺寸、障碍边距与转弯惩罚参数。
- **`layout`** —— 带 `DagreLayout` 实现的 `LayoutEngine` trait（封装 `dagre` crate，即 ReactFlow 使用的同一套 Sugiyama 算法），支持水平 / 垂直方向与循环体分组。
- **`viewport`** —— 平移偏移 + 缩放比例，支持屏幕 ↔ 逻辑坐标变换与保持锚点固定的 `zoom_around`。

### rust-agent-flow-gpui（渲染层）

- **`FlowEditorView`** —— 单一的 GPUI `Render` 视图，持有图、视口与交互状态。
- **点阵网格画布**，支持中键平移与鼠标锚点缩放（滚轮）。
- 可在网格上**拖拽节点**；通过拖拽**将输出端口连接到输入端口**。
- **交互式边绘制** —— 多种边类型、箭头标记、边中点 `+` 按钮（弹出节点选择器并在边的中间插入节点）。
- **基于命中测试的交互** —— 画布统一处理鼠标事件，通过几何命中测试确定目标节点 / 端口（而非为每个节点绑定事件闭包）。
- **内置节点** —— `start`、`end`、`action`、`condition`、`loop`、`variable`、`adapter`、`agent`，覆盖图灵完备的控制流（顺序、分支、带回边的循环体）。
- **`NodeRegistry` + `IFlowNode`** —— 通过策略模式在运行时注册自定义节点类型。
- **Schema 驱动的属性面板** —— 右侧面板由 `NodeSchema.fields` 自动生成。
- **工具栏扩展** —— 调用方通过 `ToolbarProvider` 注入自定义工具项。
- **主题 + 国际化** —— 英文 / 简体中文，带数据类型与节点类型标签映射。

## 快速开始

由于 GPUI 从 git 源码编译，首次构建可能需要数分钟。

```bash
cargo run -p rust-agent-flow-demo
```

如果默认的 Agent 流程不满足需求，可以通过 `DemoDataSource` 换用任意 JSON 流程文档（`demo/data/*.json`）：

```rust
use rust_agent_flow_gpui::{CombinedAssets, FlowEditorView, SharedToolbarProvider};

// 加载流程，用 dagre 自动排版（与 ReactFlow 相同的 Sugiyama 算法），
// 然后注册调用方自定义的工具栏扩展。
let mut editor = FlowEditorView::new(graph, cx);
editor.auto_layout(cx);
editor.add_toolbar_provider(data_source_toolbar, cx);
editor.add_toolbar_provider(app_controls_toolbar, cx);
```

如需完整的架构叙述，可查阅随仓库发布的文档：

```bash
# 仓库内文档位于 docs/rust-agent-flow
```

## 文档

仓库内置一份完整的文档书（位于 [`docs/rust-agent-flow`](docs/rust-agent-flow)），涵盖入门介绍、快速开始、设计哲学、架构、图模型、Schema 系统、几何与布局、自定义节点、编辑器视图、交互、边渲染、面板、扩展与最佳实践。

## Crates

| Crate | 说明 |
|-------|------|
| [`rust-agent-flow`](crates/core) | 核心层：`FlowGraph`、`Viewport`、几何（`bezier`、`catmull`、`smoothstep`、`loop_back`、命中测试）、A* 边路由、`DagreLayout`、`NodeSchema` / `PortSpec` / `FieldSpec` / `FlowDocument`。不依赖 GPUI。 |
| [`rust-agent-flow-gpui`](crates/gpui) | GPUI 渲染层：`FlowEditorView`、节点注册表、交互状态机、属性面板、工具栏扩展、主题与国际化。 |
| [`rust-agent-flow-demo`](demo) | 可运行的 GPUI 设计器演示应用。 |

> 尚未发布到 [crates.io](https://crates.io)。在 workspace `[workspace.dependencies]` 中，`gpui`、`gpui-component` 等依赖从 git 拉取。

## License 许可证

基于 [MIT License](LICENSE) 授权。