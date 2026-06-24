# 工具栏扩展机制 + 数据源迁移到调用侧

## 概述

本计划覆盖剩余 3 个步骤：
- **Step 6**：工具栏扩展机制（`ToolbarProvider` trait），让调用侧能注入自定义工具项
- **Step 7**：将数据源选择器从框架移到调用侧（删除框架内 `DataSource` enum + 工具栏下拉，demo 通过 `ToolbarProvider` 自行添加）
- **Step 8**：编译 + 测试验证

## 当前状态分析

### 已完成（Step 1-5）
- dagre rank 提取 → `LayoutResult.ranks` + `FlowEditorView.cached_ranks`
- 通道分配 + 正交路由 → `route_with_channels()` in `edge_path.rs`
- 渲染层集成 → `compute_obstacles_by_rank()` + `paint_edge_scaled` 支持 `obstacles_by_rank`
- Loop 主线 `done` 出口的 + 按钮放在目标节点侧 → `plus_button_at_target(source_port)` 按端口判断

### 待解决问题

**问题 1：工具栏完全硬编码，无扩展机制**
- [toolbar.rs](file:///d:/GitCode/RF/rust-agent-flow/crates/gpui/src/editor/toolbar.rs) 的 `render_toolbar()` 硬编码了 11 个控件
- 调用侧无法添加自定义工具（如数据源选择器、导出按钮、运行按钮等）
- 已有扩展模式可参考：`SyntaxService` trait + `SharedSyntaxService = Arc<dyn Trait>` + `set_syntax_service()` 注入

**问题 2：数据源选择器写在框架里，属于 demo 业务逻辑**
- [data_source.rs](file:///d:/GitCode/RF/rust-agent-flow/crates/gpui/src/editor/data_source.rs) 定义了 `DataSource` enum（AgentFlow / DataPipeline / SimpleFlow）
- 这 3 个数据源是 demo 演示数据，不是框架通用能力
- 工具栏硬编码了数据源下拉菜单（toolbar.rs 第 310-337 行）
- `FlowEditorView` 持有 `data_source: DataSource` 字段 + `set_data_source()` 方法

## 设计决策

### 决策 1：`ToolbarProvider` trait 设计（参考 `SyntaxService` 模式）

```rust
pub trait ToolbarProvider: Send + Sync {
    /// 渲染自定义工具项，追加到内置工具之后。
    fn render_items(&self, ctx: &ToolbarCtx) -> Vec<AnyElement>;
}

pub struct ToolbarCtx {
    pub entity: Entity<FlowEditorView>,  // 用于在回调中 update 编辑器
    pub theme: Theme,
    pub language: Language,
}

pub type SharedToolbarProvider = Arc<dyn ToolbarProvider>;
```

**为什么返回 `Vec<AnyElement>`**：调用侧可能添加多个工具项（如分隔符 + 下拉菜单），`Vec` 比单个 `AnyElement` 更灵活。

**为什么 `ToolbarCtx` 持有 `Entity<FlowEditorView>`**：调用侧在按钮回调中需要 `entity.update(cx, |this, cx| { ... })` 修改编辑器状态（如切换图、触发布局）。这是 GPUI 标准模式，参考 toolbar.rs 中 DropdownMenu 的 `on_click` 回调。

### 决策 2：完全删除框架内 `DataSource`，不保留 trait

**理由**：
- "数据源"是 demo 业务概念，框架只需 `set_graph()` 公开方法即可支持图替换
- 保留 trait 会引入不必要的抽象——调用侧完全可以用自己的 enum/struct
- 用户明确要求"数据源选择就应该在调用侧增加，而不是写在框架里"

**替代方案（已否决）**：将 `DataSource` 转为 trait 保留在框架中。否决原因：框架不需要知道"数据源"概念，只需提供 `set_graph()` 即可。

### 决策 3：内置工具栏保留通用控件，移除数据源下拉

保留的内置控件（框架通用能力）：
- 缩放（放大/百分比/缩小）
- 视图（适应/重置）
- 布局方向（横向/纵向）
- 边类型下拉
- 点阵背景下拉开关
- 拖拽开关
- 主题切换
- 语言切换

移除的控件（demo 业务逻辑）：
- 数据源下拉 → 由 demo 通过 `ToolbarProvider` 添加

### 决策 4：自定义工具项前加视觉分隔符

在内置工具项和自定义工具项之间添加一个 1px 宽的竖线分隔符，视觉上区分框架工具和调用侧工具。

## 改动清单

### Step 6：ToolbarProvider trait

#### 6.1 新建 `crates/gpui/src/editor/toolbar_ext.rs`

```rust
//! 工具栏扩展接口（策略模式）。
//!
//! 调用侧实现 [`ToolbarProvider`] trait，通过
//! [`FlowEditorView::add_toolbar_provider`] 注入自定义工具项。
//! 工具项渲染在内置工具栏末尾，以竖线分隔符区隔。

use std::sync::Arc;

use gpui::{AnyElement, Entity};
use rust_agent_flow::FlowGraph;

use crate::i18n::Language;
use crate::theme::Theme;

use super::flow_editor::FlowEditorView;

/// 工具栏扩展接口（扩展点）。
///
/// 调用侧实现此 trait，通过 `add_toolbar_provider` 注入。
/// `render_items` 返回的元素会追加到内置工具栏末尾。
pub trait ToolbarProvider: Send + Sync {
    /// 渲染自定义工具项。
    ///
    /// 返回的元素追加到内置工具之后。每个元素应是自包含的工具项
    ///（Button、DropdownMenu 等），通过 `ctx.entity` 在回调中更新编辑器。
    fn render_items(&self, ctx: &ToolbarCtx) -> Vec<AnyElement>;
}

/// 共享工具栏扩展类型。
pub type SharedToolbarProvider = Arc<dyn ToolbarProvider>;

/// 工具栏渲染上下文，传给 [`ToolbarProvider`]。
pub struct ToolbarCtx {
    /// 编辑器实体句柄，用于在回调中 `entity.update(cx, |this, cx| { ... })`。
    pub entity: Entity<FlowEditorView>,
    /// 当前主题颜色。
    pub theme: Theme,
    /// 当前 UI 语言。
    pub language: Language,
}
```

#### 6.2 修改 `crates/gpui/src/editor/mod.rs`

- 添加 `mod toolbar_ext;`
- 添加 `pub use toolbar_ext::{SharedToolbarProvider, ToolbarCtx, ToolbarProvider};`
- 移除 `pub use data_source::DataSource;`（Step 7）

#### 6.3 修改 `crates/gpui/src/editor/flow_editor.rs`

**添加字段**：
```rust
pub struct FlowEditorView {
    // ... 现有字段 ...
    /// 自定义工具栏扩展（由调用侧通过 `add_toolbar_provider` 注入）。
    pub custom_toolbar: Vec<SharedToolbarProvider>,
}
```

**`new()` 初始化**：
```rust
custom_toolbar: Vec::new(),
```

**添加方法**：
```rust
/// 注入自定义工具栏扩展。
///
/// 工具项渲染在内置工具栏末尾，以竖线分隔符区隔。
/// 多次调用可注入多个 provider，按注入顺序渲染。
pub fn add_toolbar_provider(&mut self, provider: SharedToolbarProvider, cx: &mut Context<Self>) {
    self.custom_toolbar.push(provider);
    cx.notify();
}
```

#### 6.4 修改 `crates/gpui/src/editor/toolbar.rs`

在 `render_toolbar` 末尾，内置工具项之后：

```rust
// ====== 自定义工具栏扩展 ======
let custom_providers = self.custom_toolbar.clone();
// ... 在 div 链末尾 ...
if !custom_providers.is_empty() {
    // 竖线分隔符
    .child(div().w(px(1.0)).h(px(20.0)).bg(theme.toolbar_border))
    // 各 provider 的工具项
    .children({
        let ctx = ToolbarCtx {
            entity: entity.clone(),
            theme,
            language: lang,
        };
        custom_providers.iter()
            .flat_map(|p| p.render_items(&ctx))
            .collect::<Vec<_>>()
    })
}
```

**注意**：由于 `div().child()` 链式调用需要条件性添加，需要将工具栏构建重构为先收集所有 children 到 `Vec`，再统一 `.children()`。或者用 `if` 分支构建不同的 div。最简方案：将整个 div 构建改为 `let mut toolbar = div()...; ` 然后逐个 `.child()` 添加，最后处理 custom。

#### 6.5 修改 `crates/gpui/src/lib.rs`

```rust
pub use editor::{FlowEditorView, SharedToolbarProvider, ToolbarCtx, ToolbarProvider};
// 移除 DataSource 导出（Step 7）
```

---

### Step 7：数据源迁移到调用侧

#### 7.1 删除 `crates/gpui/src/editor/data_source.rs`

整个文件删除。3 个 `*_doc()` 函数（`agent_flow_doc`、`data_pipeline_doc`、`simple_flow_doc`）移到 demo。

#### 7.2 修改 `crates/gpui/src/editor/mod.rs`

```rust
// 移除 mod data_source;
// 移除 pub use data_source::DataSource;
```

#### 7.3 修改 `crates/gpui/src/editor/flow_editor.rs`

**移除**：
- `use super::data_source::DataSource;`
- `pub data_source: DataSource` 字段
- `set_data_source()` 方法
- `new()` 中的 `data_source: DataSource::default()`

**新增 `set_graph()` 公开方法**（替代 `set_data_source` 的图替换能力）：
```rust
/// 替换当前流程图，重置选中/悬停/视口状态并自动重排。
///
/// 供调用侧在切换数据源等场景使用。
pub fn set_graph(&mut self, graph: FlowGraph, cx: &mut Context<Self>) {
    self.graph = graph;
    self.selected = None;
    self.hovered = None;
    self.hovered_plus = None;
    self.panel_view = None;
    self.viewport = Viewport::default();
    self.relayout();
    cx.notify();
}
```

#### 7.4 修改 `crates/gpui/src/editor/toolbar.rs`

**移除数据源下拉**（第 310-337 行的 `tb-data-source` Button + DropdownMenu）。

**移除相关变量**：
- `let data_source = self.data_source;`
- `use super::data_source::DataSource;`

#### 7.5 修改 `crates/gpui/src/lib.rs`

```rust
// 移除 DataSource 导出
pub use editor::{FlowEditorView, SharedToolbarProvider, ToolbarCtx, ToolbarProvider};
```

#### 7.6 新建 `demo/src/data_sources.rs`

将原 `data_source.rs` 的 3 个 `*_doc()` 函数移到此处，并定义 demo 专用的 `DemoDataSource` enum：

```rust
//! Demo 数据源：预置流程示例。

use rust_agent_flow::{EdgeDef, EdgeType, FlowDocument, FlowGraph, NodeDef, PointF, SizeF};

/// Demo 预置数据源。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum DemoDataSource {
    #[default]
    AgentFlow,
    DataPipeline,
    SimpleFlow,
}

impl DemoDataSource {
    pub fn to_graph(&self) -> FlowGraph {
        FlowGraph::from_document(&self.to_document())
    }

    pub fn to_document(&self) -> FlowDocument {
        match self {
            Self::AgentFlow => agent_flow_doc(),
            Self::DataPipeline => data_pipeline_doc(),
            Self::SimpleFlow => simple_flow_doc(),
        }
    }

    pub fn label(&self) -> &'static str {
        match self {
            Self::AgentFlow => "Agent 编排流程",
            Self::DataPipeline => "数据处理流水线",
            Self::SimpleFlow => "简单顺序流",
        }
    }

    pub fn all() -> &'static [DemoDataSource] {
        &[Self::AgentFlow, Self::DataPipeline, Self::SimpleFlow]
    }
}

// agent_flow_doc() / data_pipeline_doc() / simple_flow_doc()
// 从 crates/gpui/src/editor/data_source.rs 原样搬移
```

#### 7.7 新建 `demo/src/toolbar_provider.rs`

实现 `ToolbarProvider`，提供数据源下拉菜单：

```rust
//! Demo 工具栏扩展：数据源选择器。

use std::sync::{Arc, Mutex};

use gpui::{div, px, AnyElement, InteractiveElement, IntoElement, ParentElement, Styled};
use gpui_component::button::{Button, ButtonVariants};
use gpui_component::menu::{DropdownMenu, PopupMenuItem};
use gpui_component::{IconName, Selectable, Sizable};
use rust_agent_flow_gpui::{FlowEditorView, ToolbarCtx, ToolbarProvider};

use crate::data_sources::DemoDataSource;

/// Demo 数据源选择器（ToolbarProvider 实现）。
pub struct DataSourceToolbar {
    current: Arc<Mutex<DemoDataSource>>,
}

impl DataSourceToolbar {
    pub fn new(initial: DemoDataSource) -> Self {
        Self {
            current: Arc::new(Mutex::new(initial)),
        }
    }
}

impl ToolbarProvider for DataSourceToolbar {
    fn render_items(&self, ctx: &ToolbarCtx) -> Vec<AnyElement> {
        let current = *self.current.lock().unwrap();
        let entity = ctx.entity.clone();
        let current_clone = self.current.clone();

        let btn = Button::new("demo-data-source")
            .icon(IconName::ALargeSmall)
            .small()
            .ghost()
            .tooltip("数据源")
            .dropdown_menu(move |menu, _window, _cx| {
                let mut menu = menu;
                for &ds in DemoDataSource::all() {
                    let label = ds.label();
                    let entity = entity.clone();
                    let current_clone = current_clone.clone();
                    menu = menu.item(
                        PopupMenuItem::new(label)
                            .checked(ds == current)
                            .on_click(move |_, _, cx| {
                                *current_clone.lock().unwrap() = ds;
                                let graph = ds.to_graph();
                                entity.update(cx, |this, cx| {
                                    this.set_graph(graph, cx);
                                });
                            }),
                    );
                }
                menu
            })
            .into_any_element();

        vec![btn]
    }
}
```

#### 7.8 修改 `demo/src/main.rs`

```rust
mod data_sources;
mod toolbar_provider;

use data_sources::DemoDataSource;
use rust_agent_flow_gpui::{CombinedAssets, FlowEditorView, SharedToolbarProvider};
use std::sync::Arc;
use toolbar_provider::DataSourceToolbar;

fn main() {
    gpui_platform::application()
        .with_assets(CombinedAssets)
        .run(move |cx: &mut gpui::App| {
            rust_agent_flow_gpui::init(cx);
            cx.spawn(async move |cx| {
                cx.open_window(gpui::WindowOptions::default(), |window, cx| {
                    let initial_ds = DemoDataSource::AgentFlow;
                    let graph = initial_ds.to_graph();
                    let view = cx.new(|cx| {
                        let mut editor = FlowEditorView::new(graph, cx);
                        editor.auto_layout(cx);
                        // 注入数据源选择器工具栏扩展
                        let provider: SharedToolbarProvider =
                            Arc::new(DataSourceToolbar::new(initial_ds));
                        editor.add_toolbar_provider(provider, cx);
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

---

### Step 8：验证

1. `cargo build -p rust-agent-flow` — core 层编译
2. `cargo build -p rust-agent-flow-gpui` — gpui 层编译
3. `cargo build -p rust-agent-flow-demo` — demo 编译
4. `cargo test` — 全部测试通过
5. `cargo run -p rust-agent-flow-demo` — 手动验证：
   - 工具栏数据源下拉正常工作（切换后图重建 + 自动重排）
   - 内置工具栏功能不受影响（缩放/视图/布局/边类型/网格/拖拽/主题/语言）
   - Loop 节点 `done` 出口的 + 按钮显示在目标节点侧
   - 跨层边避障路由正常（连线不穿过中间层节点）

## 假设与约束

1. **i18n 键保留**：`TbDataSource`、`DataSourceAgentFlow` 等 i18n 键保留在框架中（无害的字符串常量），demo 可选择使用或自定义文案。避免破坏 i18n 模块结构。
2. **`Entity<FlowEditorView>` 可克隆**：GPUI 的 `Entity<T>` 是句柄类型，支持 `clone()`（参考 toolbar.rs 中 `entity.clone()` 用法）。
3. **`AnyElement` 满足 `Send + Sync`**：GPUI 元素类型可跨线程传递（`ToolbarProvider: Send + Sync` 约束要求返回值满足）。
4. **工具栏构建重构**：由于需要条件性追加自定义工具项，`render_toolbar` 内的 div 链式构建需改为可变变量逐个 `.child()` 添加的方式。

## 验证步骤

```bash
# 1. core 层编译
cargo build -p rust-agent-flow

# 2. gpui 层编译
cargo build -p rust-agent-flow-gpui

# 3. demo 编译
cargo build -p rust-agent-flow-demo

# 4. 全部测试
cargo test

# 5. 运行 demo 手动验证
cargo run -p rust-agent-flow-demo
```

手动验证清单：
- [ ] 数据源下拉切换 → 图重建 + 自动重排
- [ ] 内置工具栏所有功能正常
- [ ] Loop `done` 出口 + 按钮在目标节点侧
- [ ] 跨层边避障路由（连线绕过中间层节点）
- [ ] 工具栏自定义项与内置项之间有竖线分隔符
