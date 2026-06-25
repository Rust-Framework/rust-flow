# Crate 分层结构

## 依赖方向规则

**单向依赖，不可逆向**：

```
应用代码 → rust-agent-flow-gpui → rust-agent-flow (core)
                                      ↑
                                 零 GPUI 依赖
```

`core` 是金字塔顶端（最抽象），`gpui` 是渲染实现层，应用代码在最上层。

## core — 框架无关层

`rust-agent-flow` 定义所有数据模型与算法，**不依赖** GPUI。

设计意图：

- 第三方可基于 core 编写可插拔的布局引擎或几何算法
- 单元测试可纯数据验证，无需渲染
- 算法层可被非 GPUI 项目复用

核心模块：

| 模块 | 职责 | 关键类型 |
|------|------|----------|
| `graph` | 图数据结构 | `FlowGraph`、`Node`、`Edge`、`Port*` |
| `schema` | 声明式定义 | `NodeSchema`、`PortSpec`、`FieldSpec`、`FlowDocument` |
| `geometry` | 几何与算法 | `PointF`/`RectF`/`SizeF`、`edge_path`、`hit_test`、`port_calc` |
| `layout` | 布局引擎 | `LayoutEngine` trait、`DagreLayout` |
| `viewport` | 视口变换 | `Viewport` |

## gpui — 渲染层

`rust-agent-flow-gpui` 实现 GPUI 渲染与交互：

| 模块 | 职责 | 关键类型 |
|------|------|----------|
| `editor` | 主视图与交互 | `FlowEditorView`、`InteractionState`、`HitResult` |
| `node` | 节点扩展 | `IFlowNode`、`NodeRegistry`、`NodeView`、`SyntaxService` |
| `edge` | 边渲染 | `EdgeView` |
| `panel` | 属性面板 | `PanelView`、`PanelEntity`、`StartPanelView` |
| `builtin` | 内置节点 | `StartNode`/`EndNode`/`ActionNode`/`ConditionNode`/`LoopNode`/... |
| `data_type` | 数据类型 | `IDataType`、`IDataTypeProvider`、`DataTypeRegistry` |
| `theme` | 主题 | `Theme` |
| `i18n` | 国际化 | `Language`、`TKey`、`t()` |
| `assets` | 资源 | `CombinedAssets`、`FlowIcon` |

`editor` 模块按职责拆分（避免单文件膨胀）：

| 子文件 | 职责 |
|--------|------|
| `flow_editor.rs` | 主视图结构 + Render 实现 |
| `interaction.rs` | 交互状态机 + 鼠标事件 |
| `hit_test.rs` | 命中测试 |
| `graph_ops.rs` | 图变更操作（set_graph/insert/delete） |
| `rendering/` | 边/节点/面板渲染 |
| `toolbar.rs` | 内置工具栏 |
| `toolbar_ext.rs` | ToolbarProvider 扩展接口 |
| `ports.rs` | 端口位置计算 |
| `grid.rs` | 点阵背景 |
| `viewport.rs` | 视口数学 |

## demo — 示例应用

| 文件 | 职责 |
|------|------|
| `main.rs` | 入口：加载图、注入工具栏扩展 |
| `data_sources.rs` | 三种预置流程定义 |
| `toolbar_provider.rs` | `DataSourceToolbar` + `AppControlsToolbar` |
| `data_type_provider.rs` | 自定义数据类型示例 |

## 跨层数据流

```
应用层: DemoDataSource.to_graph()
   ↓ FlowDocument → FlowGraph::from_document
core层: FlowGraph (nodes + edges)
   ↓ DagreLayout::layout()
   ↓ resolve_endpoints()
gpui层: FlowEditorView.render()
   ↓ sync_node_sizes() → relayout() → render_edges/nodes/panel
   ↓ 命中测试 → InteractionState → graph_ops
```

## 模块稳定性分层

框架按**变更频率**组织文件，将稳定逻辑与易变逻辑分离：

| 稳定性 | 模块 | 说明 |
|--------|------|------|
| 极稳定 | core/graph、core/schema、core/geometry | 数据模型与算法，很少改 |
| 稳定 | editor/flow_editor、editor/graph_ops | 核心结构，偶发调整 |
| 易变 | editor/rendering、panel/render、builtin/* | 渲染细节与节点实现，常迭代 |

这解释了为什么 `flow_editor.rs` 只有结构定义和 Render，渲染细节拆到 `rendering/` 目录——内核稳定，外壳可变。

## 小结

Crate 分层体现了**框架无关与渲染实现分离**：core 提供可复用的图模型与算法，gpui 提供 GPUI 渲染与交互。改渲染层不影响 core 算法，扩展节点不影响框架内核。editor 模块按职责与稳定性进一步拆分，便于独立演进。

下一节：[流程图数据模型](graph-model.md)
