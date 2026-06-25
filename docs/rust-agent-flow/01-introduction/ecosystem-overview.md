# 生态与 Crate 全景

## Workspace 结构

rust-agent-flow 采用 Cargo Workspace，三 crate 分层：

```
rust-agent-flow/
├── crates/
│   ├── core/          # rust-agent-flow：框架无关层（无 GPUI 依赖）
│   ├── gpui/          # rust-agent-flow-gpui：GPUI 渲染层
│   └── ...
├── demo/              # rust-agent-flow-demo：示例应用
└── Cargo.toml         # workspace 根
```

## 依赖方向规则

**单向依赖，不可逆向**：

```
应用代码 → rust-agent-flow-gpui → rust-agent-flow (core)
                                      ↑
                                 零 GPUI 依赖
```

`core` 是金字塔顶端（最抽象），`gpui` 是渲染实现层，应用代码在最上层。这个分层是刻意的：

- `core` 可被任何 Rust 项目使用（包括非 GPUI 的渲染层）
- 未来可替换渲染层（如 egui）而不影响 `core`
- `core` 的单元测试无需 GPUI 窗口

## core — 框架无关层

`rust-agent-flow` 定义所有数据模型与算法，**不依赖** GPUI。

| 模块 | 职责 |
|------|------|
| `graph` | `FlowGraph`、`Node`、`Edge`、`Port`（slotmap 稳定键） |
| `schema` | `NodeSchema`、`PortSpec`、`FieldSpec`、`FlowDocument` |
| `geometry` | `PointF`/`RectF`/`SizeF`、边路径算法、命中测试、端口计算 |
| `layout` | `LayoutEngine` trait + `DagreLayout`（包装 dagre crate） |
| `viewport` | `Viewport` 平移/缩放变换 |

设计意图：

- 第三方可基于 core 编写可插拔的布局引擎或几何算法
- 单元测试可纯数据验证，无需渲染
- 算法层（边路径、布局）可独立复用

## gpui — 渲染层

`rust-agent-flow-gpui` 实现 GPUI 渲染与交互：

| 模块 | 职责 |
|------|------|
| `editor` | `FlowEditorView` 主视图、交互状态机、命中测试、渲染、图操作 |
| `node` | `IFlowNode` trait、`NodeRegistry`、`NodeView`、语法高亮服务 |
| `edge` | `EdgeView` 边渲染 |
| `panel` | `PanelView` Schema 驱动属性面板、`StartPanelView` |
| `builtin` | 8 种内置节点实现 |
| `data_type` | `IDataType`/`IDataTypeProvider`/`DataTypeRegistry` |
| `theme` | `Theme` 亮暗主题颜色 |
| `i18n` | `Language`/`TKey`/`t()` 中英文 |
| `assets` | `CombinedAssets`/`FlowIcon` 图标资源 |

`editor` 模块按职责进一步拆分（防止单文件膨胀）：

```
editor/
├── flow_editor.rs      # 主视图结构 + Render
├── interaction.rs      # 交互状态机 + 鼠标事件
├── hit_test.rs         # 命中测试
├── graph_ops.rs        # 图变更操作（set_graph/insert_node/delete）
├── rendering/          # 边/节点/面板渲染
├── toolbar.rs          # 内置工具栏
├── toolbar_ext.rs      # ToolbarProvider 扩展接口
├── ports.rs            # 端口位置计算
├── grid.rs             # 点阵背景
└── viewport.rs         # 视口数学
```

## demo — 示例应用

`rust-agent-flow-demo` 演示完整集成：

| 文件 | 职责 |
|------|------|
| `main.rs` | 入口：加载图、注入工具栏扩展 |
| `data_sources.rs` | 三种预置流程（AgentFlow/DataPipeline/SimpleFlow） |
| `toolbar_provider.rs` | `DataSourceToolbar` + `AppControlsToolbar` |
| `data_type_provider.rs` | 自定义数据类型示例（当前为空，作扩展参考） |

## 技术栈

| 依赖 | 用途 |
|------|------|
| `gpui` | Zed 的 GPU UI 框架（git 依赖，锁定特定 rev） |
| `gpui-component` | longbridge 的组件库（Button/Input/Dropdown 等） |
| `dagre` | Sugiyama 分层布局算法（Rust 移植版） |
| `slotmap` | 稳定键的容器（节点/边 ID） |
| `serde` | FlowDocument JSON 序列化 |
| `rhai` | 默认语法高亮的近似映射 |

## 内置节点一览

| kind | 节点 | 端口 | 用途 |
|------|------|------|------|
| `start` | Start | Out | 流程起点，定义输入参数与变量 |
| `end` | End | In | 流程终点，定义返回值 |
| `action` | Action | In + Out | 顺序执行步骤 |
| `condition` | Condition | In + 多 Out | 条件分支（if_0/if_1/.../else） |
| `loop` | Loop | In + Done + LoopBody + LoopIn | 循环迭代 |
| `variable` | Variable | In + Out | 变量定义 |
| `adapter` | Adapter | In + Out | 数据适配 |
| `agent` | Agent | In + Out | 智能体配置 |

## 小结

三 crate 分层体现了**框架无关与渲染实现分离**：core 提供可复用的图模型与算法，gpui 提供 GPUI 渲染与交互，demo 演示集成方式。改渲染层不影响 core 算法，扩展节点不影响框架内核。

下一章：[快速上手](../02-quickstart/INDEX.md)
