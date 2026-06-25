# rust-agent-flow 应用最佳实践 · 目录

> 面向使用 rust-agent-flow 框架的开发者 · 渐进式披露 · 深入浅出

---

## 开篇

| 文档 | 说明 |
|------|------|
| [前言](FOREWORD.md) | 本书定位、读者画像、阅读路径 |

---

## 第一部分 · 入门与认知

### [第一章 认识 rust-agent-flow](01-introduction/INDEX.md)

- [什么是 rust-agent-flow](01-introduction/what-is-rust-agent-flow.md)
- [适用场景与边界](01-introduction/who-should-use.md)
- [生态与 Crate 全景](01-introduction/ecosystem-overview.md)

### [第二章 快速上手](02-quickstart/INDEX.md)

- [创建项目与依赖](02-quickstart/create-project.md)
- [Hello World 详解](02-quickstart/hello-world.md)
- [第一个流程图](02-quickstart/first-flow.md)
- [运行、调试与验证](02-quickstart/run-and-debug.md)

---

## 第二部分 · 设计思想与架构

### [第三章 设计理念与哲学](03-philosophy/INDEX.md)

- [核心设计原则](03-philosophy/design-principles.md)
- [ReactFlow 的启发](03-philosophy/reactflow-inspiration.md)
- [GPUI 惯用法与所有权](03-philosophy/gpui-idioms.md)
- [渐进式披露与框架边界](03-philosophy/progressive-disclosure.md)

### [第四章 架构全景](04-architecture/INDEX.md)

- [Crate 分层结构](04-architecture/crate-layout.md)
- [流程图数据模型](04-architecture/graph-model.md)
- [渲染生命周期](04-architecture/render-lifecycle.md)
- [命中测试交互模型](04-architecture/hit-test-interaction.md)

---

## 第三部分 · 核心数据模型

### [第五章 流程图数据模型](05-graph-model/INDEX.md)

- [FlowGraph 与 slotmap 键](05-graph-model/flowgraph.md)
- [Node / Edge / Port 三要素](05-graph-model/node-edge-port.md)
- [FlowDocument 互转](05-graph-model/document-interop.md)

### [第六章 Schema 与字段系统](06-schema-system/INDEX.md)

- [NodeSchema 与 PortSpec](06-schema-system/node-schema-port.md)
- [FieldSpec 与字段类型](06-schema-system/fieldspec-types.md)
- [FlowDocument 序列化协议](06-schema-system/flowdocument.md)

### [第七章 几何与布局引擎](07-geometry-layout/INDEX.md)

- [边路径算法](07-geometry-layout/edge-path-algorithms.md)
- [端口端点计算](07-geometry-layout/port-calc.md)
- [Dagre 布局引擎](07-geometry-layout/dagre-layout.md)
- [Viewport 视口数学](07-geometry-layout/viewport.md)

---

## 第四部分 · 节点系统

### [第八章 IFlowNode 扩展接口](08-iflow-node/INDEX.md)

- [策略模式与 IFlowNode](08-iflow-node/strategy-pattern.md)
- [NodeRegistry 注册表](08-iflow-node/noderegistry.md)
- [NodeViewCtx 与 NodeAction](08-iflow-node/nodeviewctx-action.md)

### [第九章 内置节点详解](09-builtin-nodes/INDEX.md)

- [Start / End / Action](09-builtin-nodes/start-end-action.md)
- [Condition 条件分支](09-builtin-nodes/condition-branch.md)
- [Loop 循环迭代](09-builtin-nodes/loop-iteration.md)
- [Variable / Adapter / Agent](09-builtin-nodes/variable-adapter-agent.md)

### [第十章 自定义节点开发](10-custom-node/INDEX.md)

- [实现 IFlowNode](10-custom-node/implement-iflownode.md)
- [动态端口与 ports_for_node](10-custom-node/dynamic-ports.md)
- [port_position 与 content_size](10-custom-node/port-position-size.md)

---

## 第五部分 · 渲染与交互

### [第十一章 FlowEditorView 主视图](11-editor-view/INDEX.md)

- [主视图结构](11-editor-view/structure.md)
- [渲染管线](11-editor-view/render-pipeline.md)
- [视口变换与缩放](11-editor-view/viewport-transform.md)

### [第十二章 交互与命中测试](12-interaction/INDEX.md)

- [交互状态机](12-interaction/interaction-fsm.md)
- [命中测试](12-interaction/hit-test.md)
- [鼠标事件与节点选择浮层](12-interaction/mouse-events-picker.md)

### [第十三章 边渲染与连线](13-edge-rendering/INDEX.md)

- [EdgeView 与边类型](13-edge-rendering/edgeview-types.md)
- [循环回环边](13-edge-rendering/loop-back.md)
- [边「+」按钮与插入节点](13-edge-rendering/plus-button.md)

---

## 第六部分 · 面板与扩展

### [第十四章 Schema 驱动属性面板](14-panel/INDEX.md)

- [PanelView 面板实体](14-panel/panelview.md)
- [FieldState 与同步机制](14-panel/fieldstate-sync.md)
- [Start 节点专属面板](14-panel/start-panel.md)

### [第十五章 扩展点体系](15-extensions/INDEX.md)

- [ToolbarProvider 工具栏扩展](15-extensions/toolbar-provider.md)
- [IDataTypeProvider 数据类型扩展](15-extensions/data-type-provider.md)
- [SyntaxService 语法高亮扩展](15-extensions/syntax-service.md)
- [主题与国际化](15-extensions/theme-i18n.md)

---

## 第七部分 · 工程化与实践

### [第十六章 最佳实践与案例](16-best-practices/INDEX.md)

- [项目组织与集成](16-best-practices/project-structure.md)
- [常见陷阱与排查](16-best-practices/common-pitfalls.md)
- [性能优化技巧](16-best-practices/performance-tips.md)
- [Demo 案例研究](16-best-practices/demo-case-study.md)
