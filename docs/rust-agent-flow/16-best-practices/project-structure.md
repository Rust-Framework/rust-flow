# 项目组织与集成

把 rust-agent-flow 集成进自己的应用，本质上是在一个 gpui 窗口里放下一个 `FlowEditorView`，再按需注入扩展。本节给出最小骨架、初始化序列、扩展注入时序，以及 Demo 的实际项目结构作为参照。

## crate 依赖关系

rust-agent-flow 分两个核心 crate：

```
rust_agent_flow        ← 纯逻辑：FlowGraph / Node / Edge / FlowDocument / dagre
    ▲
    │ depends
    │
rust_agent_flow_gpui   ← gpui 渲染层：FlowEditorView / PanelView / 扩展点
```

`rust_agent_flow` 不依赖 gpui，可单独用于后端/测试/序列化；`rust_agent_flow_gpui` 在前者基础上提供编辑器视图。你的应用依赖 `rust_agent_flow_gpui` 即可同时获得两者。

## 最小集成骨架

一个能跑起来的集成只需四步：初始化、建图、建视图、注入扩展。

```rust
use std::sync::Arc;
use gpui::AppContext;
use rust_agent_flow_gpui::{CombinedAssets, FlowEditorView, SharedToolbarProvider};

mod toolbar_provider;  // 你的 ToolbarProvider 实现

fn main() {
    gpui_platform::application()
        .with_assets(CombinedAssets)
        .run(move |cx: &mut gpui::App| {
            // 1. 框架初始化（注册内置节点、加载资源）
            rust_agent_flow_gpui::init(cx);

            cx.spawn(async move |cx| {
                cx.open_window(gpui::WindowOptions::default(), |window, cx| {
                    // 2. 准备图数据（来自你的 FlowDocument / JSON / 动态构建）
                    let graph = load_my_graph();

                    let view = cx.new(|cx| {
                        let mut editor = FlowEditorView::new(graph, cx);
                        // 3. 自动布局（不调则节点堆在原点）
                        editor.auto_layout(cx);
                        // 4. 注入扩展
                        editor.add_toolbar_provider(
                            Arc::new(toolbar_provider::MyToolbar::new()),
                            cx,
                        );
                        editor
                    });
                    cx.new(|cx| gpui_component::Root::new(view, window, cx))
                })
                .expect("Failed to open window");
            })
            .detach();
        });
}
```

四个动作缺一不可，下表说明省略各自的后果：

| 动作 | 省略后果 |
|------|----------|
| `init(cx)` | ListItem 选中边框异常等内置状态未注册 |
| `auto_layout(cx)` | 节点全部堆在原点 (0,0) |
| `add_toolbar_provider` | 工具栏只有内置项，无业务控件 |
| `gpui_component::Root::new(view, ...)` | Button/DropdownMenu 等组件无主题上下文 |

## 初始化序列

正确顺序是「框架先就绪、再建视图」：

```
gpui_platform::application().with_assets(CombinedAssets).run(...)
   │
   ├─ rust_agent_flow_gpui::init(cx)      ← 注册内置节点类型、资源
   │
   └─ cx.open_window(...)
        │
        └─ cx.new(|cx| {
               let editor = FlowEditorView::new(graph, cx);  ← 视图构造
               editor.auto_layout(cx);                       ← 布局
               editor.add_toolbar_provider(..., cx);         ← 扩展
               editor.set_syntax_service(..., cx);           ← 扩展（可选）
               editor.set_data_type_provider(..., cx);       ← 扩展（可选）
               editor
           })
```

`init` 必须在 `FlowEditorView::new` 之前，因为它注册内置节点类型（Start/End/Action/Condition/Loop/Variable/Adapter/Agent）；若顺序反了，`new` 时节点注册表为空，图里的节点找不到对应 `IFlowNode`，画布会是空的。

## 扩展注入时序

扩展注入的通用原则：**在首帧渲染前注入**。一旦 panel_view 已被 `ensure_panel_view` 创建，替换型注入（syntax/data-type/language）会销毁它重建——虽然结果正确，但多一次重建开销。在 `cx.new` 闭包内、`auto_layout` 前后注入都是安全的，因为闭包返回前没有渲染发生。

```rust
cx.new(|cx| {
    let mut editor = FlowEditorView::new(graph, cx);
    // 注入顺序无要求，但建议：替换型在前、累积型在后
    editor.set_syntax_service(Arc::new(MySyntax), cx);
    editor.set_data_type_provider(Arc::new(MyTypes), cx);
    editor.add_toolbar_provider(Arc::new(MyToolbar), cx);
    editor.auto_layout(cx);   // 布局最后做，避免与重建冲突
    editor
})
```

## Demo 项目结构

Demo 是「最小完整集成」的范本，结构如下：

```
demo/
├── Cargo.toml
├── data/
│   ├── agent_flow.json       ← 预置流程（FlowDocument JSON）
│   ├── data_pipeline.json
│   └── simple_flow.json
└── src/
    ├── main.rs               ← 入口：加载图 + 建视图 + 注入扩展
    ├── data_sources.rs       ← DemoDataSource 枚举 + to_graph
    ├── toolbar_provider.rs   ← DataSourceToolbar / AppControlsToolbar
    └── data_type_provider.rs ← DemoDataTypeProvider（空，作扩展参考）
```

职责清晰分离：

| 文件 | 角色 | 是否必需 |
|------|------|----------|
| `main.rs` | 集成入口，组装框架 | 必需 |
| `data_sources.rs` | 数据驱动：流程定义 → FlowGraph | 可替换为你的数据源 |
| `toolbar_provider.rs` | 业务工具栏扩展 | 按需 |
| `data_type_provider.rs` | 业务数据类型扩展 | 按需 |
| `data/*.json` | 流程定义数据 | 可替换 |

这种「数据在 JSON、逻辑在代码、扩展在 provider」的分层，让新增流程只需加 JSON 文件、新增工具栏控件只需加 provider——核心集成代码（main.rs）几乎不动。

## 数据驱动加载

Demo 用 `include_str!` 把 JSON 编译期嵌入，运行时反序列化为 `FlowDocument` 再转 `FlowGraph`：

```rust
const AGENT_FLOW_JSON: &str = include_str!("../data/agent_flow.json");

impl DemoDataSource {
    pub fn to_graph(&self) -> FlowGraph {
        FlowGraph::from_document(&self.to_document())
    }
    pub fn to_document(&self) -> FlowDocument {
        let json = match self { Self::AgentFlow => AGENT_FLOW_JSON, ... };
        serde_json::from_str(json).expect("内置 JSON 解析失败")
    }
}
```

`include_str!` 适合内置示例；真实项目可改为运行时 `std::fs::read_to_string` 或网络加载，`FlowDocument` 协议不变。节点/边定义与渲染逻辑完全解耦——这是 rust-agent-flow 数据驱动设计的核心收益。

## 何时需要自定义节点

框架内置 8 种节点类型覆盖了控制流编排的绝大多数场景。需要自定义 `IFlowNode` 的典型信号：

- 节点数据 schema 内置 8 种 `FieldType` 表达不了（如需要文件上传控件）
- 节点视图需要动态交互（如内联预览、实时计算结果）
- 节点端口结构高度特殊（如多输入汇聚节点）

只是字段不同？优先用 `NodeSchema` 声明，享受 schema 驱动面板。需要特殊视图？实现 `IFlowNode::get_view` 自定义渲染，但仍可复用 schema 驱动面板（除非像 Start 那样走特例）。

## 小结

集成四步：`init` → `new` → `auto_layout` → 注入扩展，顺序不可乱。扩展在首帧前注入最省。Demo 的「JSON 数据 + provider 扩展 + main 装配」分层是可复用的项目骨架：新增流程加 JSON、新增控件加 provider，核心代码稳定。

下一节：[常见陷阱与排查](common-pitfalls.md)
