# Hello World 详解

## 最小可运行示例

以下是一个显示空画布的最小 GPUI 应用：

```rust
use rust_agent_flow::FlowGraph;
use rust_agent_flow_gpui::{CombinedAssets, FlowEditorView};

fn main() {
    gpui_platform::application()
        .with_assets(CombinedAssets)
        .run(move |cx: &mut gpui::App| {
            // 初始化框架（必须在打开窗口前调用）
            rust_agent_flow_gpui::init(cx);

            cx.spawn(async move |cx| {
                cx.open_window(gpui::WindowOptions::default(), |window, cx| {
                    let graph = FlowGraph::new(); // 空图
                    let view = cx.new(|cx| FlowEditorView::new(graph, cx));
                    cx.new(|cx| gpui_component::Root::new(view, window, cx))
                })
                .expect("Failed to open window");
            })
            .detach();
        });
}
```

## 逐行解析

### 1. 应用与资源

```rust
gpui_platform::application()
    .with_assets(CombinedAssets)
```

- `gpui_platform::application()` 创建 GPUI 应用实例
- `.with_assets(CombinedAssets)` 注入框架图标资源（节点图标等）
- `CombinedAssets` 合并了 gpui-component 与 rust-agent-flow 的资源

### 2. 框架初始化

```rust
rust_agent_flow_gpui::init(cx);
```

**必须在打开窗口前调用**。它做两件事：

- 调用 `gpui_component::init(cx)` 初始化组件库
- 关闭 `ListItem` 的 `active_highlight`（去除选中态 1px 边框，保留背景高亮）

### 3. 异步开窗

```rust
cx.spawn(async move |cx| {
    cx.open_window(...)
})
.detach();
```

GPUI 的窗口创建在异步上下文中进行，`.detach()` 让任务独立运行。

### 4. 创建视图

```rust
let graph = FlowGraph::new();
let view = cx.new(|cx| FlowEditorView::new(graph, cx));
```

- `FlowGraph::new()` 创建空图
- `FlowEditorView::new(graph, cx)` 构造主视图，内部自动注册 8 种内置节点

### 5. Root 包装

```rust
cx.new(|cx| gpui_component::Root::new(view, window, cx))
```

`gpui_component::Root` 是组件库的根容器，提供主题、悬浮层等基础能力。**必须包裹**你的 `FlowEditorView`。

## 运行后的交互

此时画布是空的，但已具备全部交互能力：

| 操作 | 效果 |
|------|------|
| 中键拖拽 | 平移视口 |
| 滚轮 | 缩放（鼠标锚点） |
| 空白处点击 | 取消选中 |

空图没有节点可拖拽连线，下一节我们用数据驱动构建一个真正的流程图。

## FlowEditorView 的构造时序

```
FlowEditorView::new(graph, cx)
  ├─ NodeRegistry::new()
  ├─ builtin::register_all(&mut registry)  // 注册 8 种内置节点
  ├─ 初始化默认状态：
  │    ├─ viewport = Viewport::default()
  │    ├─ interaction = InteractionState::Idle
  │    ├─ default_edge_type = SmoothStep
  │    ├─ layout_direction = Horizontal
  │    ├─ theme = Theme::light()
  │    └─ language = Language::Zh
  └─ 返回 Self（未布局，需调用 auto_layout）
```

> **关键**：`new` 不会自动布局。创建后需调用 `editor.auto_layout(cx)` 触发 dagre 排版，否则节点堆在原点。

## 完整模板

结合自动布局的最小模板：

```rust
let view = cx.new(|cx| {
    let mut editor = FlowEditorView::new(graph, cx);
    editor.auto_layout(cx); // dagre 自动排版
    editor
});
```

## 小结

`init` → `new` → `auto_layout` 三步即可显示一个可交互的流程画布。框架在构造时自动注册内置节点，你无需手动管理注册表（除非添加自定义节点）。

下一节：[第一个流程图](first-flow.md)
