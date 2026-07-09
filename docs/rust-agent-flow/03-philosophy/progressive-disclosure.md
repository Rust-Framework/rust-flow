# 渐进式披露与框架边界

## 渐进式披露的含义

框架按**使用深度**分层暴露能力：初学者用最少代码跑起来，进阶者逐步解锁扩展点，高级用户可下沉到 core 算法层。

## 四层使用深度

### 层级一：开箱即用（5 行代码）

```rust
let view = cx.new(|cx| {
    let mut editor = FlowEditorView::new(graph, cx);
    editor.auto_layout(cx);
    editor
});
```

只用到：`FlowEditorView::new` + `auto_layout`。8 种内置节点、自动布局、交互、属性面板全部就绪。

### 层级二：注入扩展点（加几行）

```rust
// 注入工具栏
editor.add_toolbar_provider(Arc::new(MyToolbar::new()), cx);
// 注入数据类型
editor.set_data_type_provider(Arc::new(MyTypes::new()), cx);
// 注入语法高亮
editor.set_syntax_service(Arc::new(MySyntax::new()), cx);
```

三个 setter，统一 `Arc<dyn Trait>` 模式。不需要修改框架代码。

### 层级三：注册自定义节点

```rust
// 自定义节点实现 IFlowNode
let mut registry = NodeRegistry::new();
registry.register(Arc::new(MyCustomNode::new()));
// 通过 FlowEditorView::new 内部已注册内置节点
// 自定义节点需在构造后注入 registry（或替换）
```

需要实现 `IFlowNode` trait 的 `kind`/`get_view`/`get_panel`/`schema` 四要素。

### 层级四：直接使用 core 算法

```rust
use rust_agent_flow::{
    smoothstep_path, compute_side_from_position, distribute_on_side,
    DagreLayout, LayoutDirection,
};

// 完全脱离 gpui 层，只用算法
let layout = DagreLayout::new().layout(&graph, LayoutDirection::Horizontal);
// core 层仅暴露纯几何工具：方向推导 / 同侧分布 / 边路径
let side = compute_side_from_position(src_center, dst_center);
let pts = distribute_on_side(bounds, side, dir, has_opposite, count);
```

> 注意：旧的批量入口 `resolve_endpoints` **已废弃**（`#[deprecated]`）——它不识别 `PortSpec.fixed` 强约束，也无法调用节点实现的 `port_position` 回调。完整的「边端点解析」已上移到 gpui 渲染层（`resolve_port` + `compute_edge_endpoints`，为 `pub(crate)` 内部 API）。core 层对外只保留 `compute_side_from_position` / `distribute_on_side` / `point_on_side` 等无副作用的纯几何工具，供在非 GPUI 项目中复用。

## 框架边界：能力 vs UI

框架明确划分**能力方法**与**UI 控件**：

| 能力方法（框架提供） | UI 控件（调用侧实现） |
|----------------------|----------------------|
| `toggle_theme()` | 主题切换按钮 |
| `toggle_drag()` | 拖拽开关按钮 |
| `toggle_language()` | 语言切换按钮 |
| `set_graph()` | 数据源选择下拉 |
| `auto_layout()` | 重排按钮 |
| `set_layout_direction()` | 横/纵向切换按钮 |

框架内置工具栏只包含**框架级**操作（布局方向、缩放重置、边类型切换）。产品级控件由调用侧通过 `ToolbarProvider` 注入。

这条边界意味着：

- ✅ 框架不假定你的工具栏布局
- ✅ 你的应用可以自由设计控件样式
- ✅ 框架升级不会破坏你的工具栏 UI

## 框架边界：设计时 vs 运行时

| 维度 | 设计时（框架负责） | 运行时（你负责） |
|------|---------------------|------------------|
| 图结构 | 创建/编辑/序列化 | 解析与调度 |
| 节点 | 渲染卡片/属性面板 | 执行业务逻辑 |
| 连线 | 路径绘制/端点计算 | 数据流传递 |
| 布局 | dagre 排版 | 无 |
| 持久化 | FlowDocument JSON | 加载/存储到你的后端 |

框架产出的是 `FlowDocument`（可序列化的流程定义），如何「执行」这个流程完全由你决定。

## 何时下沉到 core

以下场景建议直接用 core 而非 gpui 层：

| 场景 | 原因 |
|------|------|
| 在服务端验证流程图结构 | 无需 GPUI，core 可独立编译 |
| 自定义布局算法 | 实现 `LayoutEngine` trait |
| 批量处理流程图（CLI 工具） | 无窗口环境 |
| 单元测试图变换 | 纯数据，无渲染依赖 |

## 何时该扩展而非下沉

以下场景应通过扩展点实现，**不要** fork 框架：

| 场景 | 扩展点 |
|------|--------|
| 新增业务节点类型 | `IFlowNode` + `NodeRegistry` |
| 工具栏业务按钮 | `ToolbarProvider` |
| 自定义数据类型 | `IDataTypeProvider` |
| 代码编辑器高亮 | `SyntaxService` |
| 自定义主题颜色 | `Theme` 结构体字段 |

## 小结

渐进式披露让不同深度的用户各取所需：初学者 5 行代码跑起来，进阶者注入扩展点，高级用户下沉 core。框架只提供能力方法，UI 控件与运行时执行留给调用侧——这是清晰而克制的边界。

下一章：[架构全景](../04-architecture/INDEX.md)
