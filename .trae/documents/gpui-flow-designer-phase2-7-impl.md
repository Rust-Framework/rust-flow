# GPUI 流程设计器 Phase 2-7 实施计划

## 一、Summary

基于已完成的 Phase 1（core 层：图模型、几何算法、端点计算、布局引擎、Viewport），本计划详细规划 Phase 2-7 的 GPUI 渲染层实施，最终交付一个类 ReactFlow 的智能体编排可视化设计器。

**核心原则**（用户强调）：
- 高内聚低耦合，core 层零 GPUI 依赖，gpui 层依赖 core
- 用最简单的代码实现复杂逻辑，禁止堆叠代码
- 尽可能抄袭 ReactFlow 成熟实现，避免走弯路
- 确保完整设计实现，不遗漏导致运行异常

**用户已确认的决策**：
1. Phase 2 实现完整 FSM（含 DrawEdge 连线交互）
2. 修复 CI 系统依赖问题
3. 补充 core/src/lib.rs 的 re-export

---

## 二、Current State Analysis

### 已完成（Phase 1）

| 模块 | 文件 | 状态 |
|---|---|---|
| 几何类型 | `crates/core/src/geometry/mod.rs` | ✅ PointF/SizeF/RectF |
| 连线算法 | `crates/core/src/geometry/edge_path.rs` | ✅ straight/bezier/step/smoothstep/loop_back |
| 端点计算 | `crates/core/src/geometry/port_calc.rs` | ✅ resolve_endpoints + 防重叠分布 |
| 命中测试 | `crates/core/src/geometry/hit_test.rs` | ✅ point_in_rect/polyline_distance |
| 图模型 | `crates/core/src/graph/mod.rs` | ✅ FlowGraph CRUD + version |
| 节点 | `crates/core/src/graph/node.rs` | ✅ Node/NodeId/NodeKind/NodeData |
| 边 | `crates/core/src/graph/edge.rs` | ✅ Edge/EdgeId/EdgeType(4)/EdgeKind(2) |
| 端口 | `crates/core/src/graph/port.rs` | ✅ PortId/PortDirection/PortSide(含 Auto) |
| Schema | `crates/core/src/schema/mod.rs` | ✅ NodeSchema/PortSpec |
| 布局 | `crates/core/src/layout/{mod,dagre}.rs` | ✅ LayoutEngine trait + DagreLayout(feature) |
| 视口 | `crates/core/src/viewport.rs` | ✅ Viewport + zoom_around |

### 待实施（Phase 2-7）

`crates/gpui/` 仅为 stub（`src/lib.rs` 只有一行注释，`Cargo.toml` 无任何依赖）。CI 中 `cargo test -p rust-agent-flow-gpui --lib` 和 `cargo check -p rust-agent-flow-gpui --example minimal_demo --features demo` 当前会失败。

### 已知偏差需修正

1. `core/src/lib.rs` 未 re-export `edge_path`/`port_calc`/`hit_test` 函数 → Phase 2 补充
2. `.github/workflows/ci.yml` 未安装 GPUI 系统依赖 → Phase 2 修复
3. README 提到 `catmull` 但未实现 → 不实现 catmull，README 后续更新

---

## 三、Architecture（分层架构）

```
┌─────────────────────────────────────────────────────┐
│  crates/gpui  (rust-agent-flow-gpui)                │
│  ┌───────────────────────────────────────────────┐  │
│  │ editor/  FlowEditorView + Viewport + FSM      │  │
│  │ node/    IFlowNode + NodeRegistry + NodeView  │  │
│  │ edge/    EdgeView (canvas + PathBuilder)      │  │
│  │ panel/   属性面板容器                          │  │
│  │ builtin/ Start/End/Action/Condition/Loop      │  │
│  └───────────────────────────────────────────────┘  │
│         依赖 ↓ (workspace)                          │
├─────────────────────────────────────────────────────┤
│  crates/core (rust-agent-flow)                      │
│  geometry / graph / layout / schema / viewport      │
│  (零 GPUI 依赖，纯 f32 数学)                        │
└─────────────────────────────────────────────────────┘
```

**依赖方向**：gpui → core（单向）。core 层不知道 GPUI 存在，所有渲染相关类型（AnyElement、Window、Context）仅出现在 gpui 层。

---

## 四、Proposed Changes（按 Phase 详细文件清单）

### Phase 2：GPUI 编辑器骨架（完整 FSM）

**目标**：让 CI 的 3 个步骤全部通过；实现可平移、缩放、拖拽节点、连线交互的最小可用编辑器。

#### 2.1 修复 CI 系统依赖

**文件**：`.github/workflows/ci.yml`

**改动**：在 `dtolnay/rust-toolchain@stable` 步骤后、测试步骤前，添加系统依赖安装步骤：

```yaml
- name: Install system dependencies (Linux)
  if: runner.os == 'Linux'
  run: |
    sudo apt-get update
    sudo apt-get install -y libx11-dev libxi-dev libgl1-mesa-dev libasound2-dev libfontconfig1-dev libfreetype6-dev libssl-dev pkg-config
```

**原因**：GPUI 在 Linux 编译需要 X11、ALSA、FontConfig、FreeType、OpenGL 等系统库。

#### 2.2 补充 core 层 re-export

**文件**：`crates/core/src/lib.rs`

**改动**：在现有 re-export 后追加：

```rust
pub use geometry::edge_path::{
    bezier_path, loop_back_path, smoothstep_path, step_path, straight_path,
};
pub use geometry::hit_test::{point_in_rect, point_to_polyline_distance};
pub use geometry::port_calc::{resolve_endpoints, ResolvedEdge};
```

**原因**：gpui 层频繁调用这些函数，完整路径冗长。补充后可直接 `use rust_agent_flow::bezier_path`。

#### 2.3 补齐 gpui crate 依赖

**文件**：`crates/gpui/Cargo.toml`

**改动**：完整重写为：

```toml
[package]
name = "rust-agent-flow-gpui"
version = "0.1.0"
edition = "2021"

[lib]
name = "rust_agent_flow_gpui"

[dependencies]
rust-agent-flow = { workspace = true }
gpui = { workspace = true }
gpui_platform = { workspace = true }
gpui-component = { workspace = true }
gpui-component-assets = { workspace = true }
anyhow = "1"
serde_json = "1"
log = "0.4"

[features]
default = []
demo = []
dagre = ["rust-agent-flow/dagre"]
```

**要点**：
- `dagre` feature 透传到 core 层（`rust-agent-flow/dagre`），Phase 5 使用
- 不在 default 启用 dagre，保持 core 默认轻量

#### 2.4 lib.rs 入口

**文件**：`crates/gpui/src/lib.rs`

**内容**：

```rust
//! `rust-agent-flow-gpui` — GPUI 渲染层。
//!
//! 提供 FlowEditorView 及 IFlowNode 扩展接口，基于 rust-agent-flow core 层
//! 实现类 ReactFlow 的可视化流程设计器。

pub mod editor;
pub mod edge;
pub mod node;
pub mod panel;
pub mod builtin;

pub use editor::FlowEditorView;
pub use node::{IFlowNode, NodeRegistry, NodeView};
pub use edge::EdgeView;

/// 初始化 GPUI 组件库（必须在打开窗口前调用）。
pub fn init(cx: &mut gpui::App) {
    gpui_component::init(cx);
    gpui_component_assets::init(cx);
}
```

**要点**：`init` 封装 `gpui_component::init` + assets 初始化，demo 调用方只需一次调用。

#### 2.5 editor 模块

**文件**：`crates/gpui/src/editor/mod.rs`

```rust
mod flow_editor;
mod viewport;
mod interaction;

pub use flow_editor::FlowEditorView;
pub use interaction::InteractionState;
```

#### 2.6 交互状态机

**文件**：`crates/gpui/src/editor/interaction.rs`

**设计**：4 状态 FSM，覆盖用户所有交互场景。

```rust
use rust_agent_flow::{NodeId, PortId, PointF};

#[derive(Debug, Clone)]
pub enum InteractionState {
    /// 空闲：无交互进行中
    Idle,
    /// 平移视口：记录鼠标起点
    Panning { start: PointF, origin: PointF },
    /// 拖拽节点：记录节点 id 和鼠标起点（逻辑坐标）
    DraggingNode {
        node_id: NodeId,
        start: PointF,
        node_origin: PointF,
    },
    /// 绘制连线：记录起点节点/端口，current 为当前鼠标位置（逻辑坐标）
    DrawingEdge {
        from_node: NodeId,
        from_port: PortId,
        current: PointF,
    },
}

impl Default for InteractionState {
    fn default() -> Self {
        Self::Idle
    }
}
```

**要点**：
- `Panning.origin` 保存视口 offset 起点，移动时 `offset = origin + (current - start)`
- `DraggingNode.node_origin` 保存节点 position 起点，避免累计误差
- `DrawingEdge.current` 是逻辑坐标（已用 viewport.to_logical 转换）

#### 2.7 视口交互

**文件**：`crates/gpui/src/editor/viewport.rs`

**职责**：封装视口相关的鼠标事件处理逻辑（不直接持有 GPUI 类型，纯数学）。

```rust
use rust_agent_flow::{PointF, Viewport};

/// 处理滚轮缩放，以鼠标位置为锚点。
pub fn handle_zoom(viewport: &mut Viewport, mouse_logical: PointF, delta: f32) {
    let factor = if delta < 0.0 { 1.1 } else { 1.0 / 1.1 };
    viewport.zoom_around(mouse_logical, factor);
}

/// 处理平移拖拽，返回新的 offset。
pub fn handle_pan(origin: PointF, start: PointF, current: PointF) -> PointF {
    // offset 是逻辑坐标的负偏移
    PointF::new(
        origin.x + (current.x - start.x),
        origin.y + (current.y - start.y),
    )
}
```

**要点**：视口数学已在 core 层 `Viewport` 实现，此处仅封装事件→数学的映射。注意 GPUI 的 `scroll_wheel` delta 方向：`delta.y < 0` 向上滚（放大），`> 0` 向下滚（缩小）。

#### 2.8 FlowEditorView 主视图

**文件**：`crates/gpui/src/editor/flow_editor.rs`

**核心结构**：

```rust
use std::sync::Arc;
use gpui::*;
use rust_agent_flow::*;
use crate::node::{NodeRegistry, NodeView};
use crate::edge::EdgeView;

pub struct FlowEditorView {
    pub graph: FlowGraph,           // 持有图模型（后续可改为 Entity<FlowGraph> 共享）
    pub viewport: Viewport,
    pub interaction: InteractionState,
    pub registry: Arc<NodeRegistry>,
    pub selected: Option<NodeId>,
    // 缓存：基于 graph.version() 失效
    resolved_edges: std::cell::RefCell<Option<(u64, HashMap<EdgeId, ResolvedEdge>)>>,
}
```

**Render 实现**（伪代码，实际需匹配 GPUI API）：

```rust
impl Render for FlowEditorView {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        div()
            .size_full()
            .relative()
            .on_mouse_down(MouseButton::Middle, cx.listener(Self::on_pan_start))
            .on_mouse_move(cx.listener(Self::on_mouse_move))
            .on_mouse_up(MouseButton::Middle, cx.listener(Self::on_pan_end))
            .on_scroll_wheel(cx.listener(Self::on_scroll))
            .child(self.render_canvas(window, cx))
            .when_some(self.selected, |this, _| {
                this.child(self.render_panel(window, cx))
            })
    }
}
```

**render_canvas**：
1. 用 `canvas(prepaint, paint)` 绘制网格背景（可选，MVP 可省略）
2. 边层：遍历 `graph.edges()`，每个 Edge 调用 `EdgeView::new(edge, resolved).into_any_element()`
3. 节点层：遍历 `graph.nodes()`，每个 Node 用 `div().absolute().left(screen_x).top(screen_y)` 定位，child 为 `NodeView::new(node, registry).into_any_element()`

**事件处理方法**：
- `on_pan_start`：记录 `Panning { start, origin: viewport.offset }`
- `on_mouse_move`：根据 interaction 状态分发
  - `Panning` → `viewport.offset = handle_pan(...)` + `cx.notify()`
  - `DraggingNode` → `graph.node_mut(id).position = node_origin + (current - start)` + `cx.notify()`
  - `DrawingEdge` → `interaction.current = mouse_logical` + `cx.notify()`
- `on_pan_end`：`interaction = Idle`
- `on_scroll`：`handle_zoom(&mut viewport, mouse_logical, delta.y)` + `cx.notify()`
- `on_node_down`（节点上鼠标按下）：`DraggingNode { node_id, start, node_origin }`
- `on_node_up`：`interaction = Idle`
- `on_port_down`（端口上鼠标按下）：`DrawingEdge { from_node, from_port, current }`
- `on_port_up`（端口上鼠标抬起）：若在目标端口上，`graph.add_edge(...)`；否则取消

**要点**：
- 所有鼠标坐标先用 `viewport.to_logical(screen_point)` 转换为逻辑坐标
- 节点位置存储逻辑坐标，渲染时用 `viewport.to_screen(node.position)` 转换
- `cx.notify()` 触发重绘

#### 2.9 NodeView 节点视图

**文件**：`crates/gpui/src/node/mod.rs`

```rust
mod view;
mod flow_node;  // Phase 3 实现，Phase 2 先声明空 trait
mod registry;   // Phase 3 实现

pub use view::NodeView;
pub use flow_node::{IFlowNode, NodeViewCtx};
pub use registry::NodeRegistry;
```

**文件**：`crates/gpui/src/node/view.rs`（Phase 2 简化版）

```rust
use gpui::*;
use rust_agent_flow::{Node, NodeId, PortDirection, PortId, PortSide, RectF};

pub struct NodeView {
    pub node: Node,
    pub on_down: Option<Box<dyn Fn(NodeId, PointF, &mut Window, &mut App) + Send>>,
    pub on_port_down: Option<Box<dyn Fn(NodeId, PortId, &mut Window, &mut App) + Send>>,
}

impl NodeView {
    pub fn new(node: Node) -> Self { /* ... */ }
}

impl RenderOnce for NodeView {
    fn render(self, _window: &mut Window, _cx: &mut App) -> impl IntoElement {
        let bounds = self.node.bounds();
        div()
            .absolute()
            .w(bounds.size.w as f32)
            .h(bounds.size.h as f32)
            .bg(gpui::white())
            .border_1()
            .border_color(gpui::black())
            .rounded_md()
            .shadow_sm()
            .child(div().p_2().child(self.node.kind.0.clone()))
            // 端点 Handle（简化版：四边居中各一个圆点）
            .child(self.render_handle(PortSide::Left, PortDirection::In))
            .child(self.render_handle(PortSide::Right, PortDirection::Out))
    }
}
```

**Phase 2 简化策略**：
- 节点卡片：白底黑边圆角矩形 + 显示 kind 文字
- 端点：左右各一个 8px 圆点（左进右出），用 `div().absolute()` 定位
- 不接入 IFlowNode（Phase 3 接入），直接用 Node.kind 显示

#### 2.10 EdgeView 边视图

**文件**：`crates/gpui/src/edge/mod.rs`

```rust
mod edge_view;
pub use edge_view::EdgeView;
```

**文件**：`crates/gpui/src/edge/edge_view.rs`

```rust
use gpui::*;
use rust_agent_flow::{Edge, EdgeType, PointF, PortSide};
use rust_agent_flow::{bezier_path, straight_path, step_path, smoothstep_path};

pub struct EdgeView {
    pub src: PointF,       // 屏幕坐标
    pub dst: PointF,       // 屏幕坐标
    pub src_side: PortSide,
    pub dst_side: PortSide,
    pub edge_type: EdgeType,
}

impl CanvasRender for EdgeView {  // 或自定义 paint 逻辑
    fn paint(&self, window: &mut Window, _cx: &mut App) {
        let points: Vec<PointF> = match self.edge_type {
            EdgeType::Straight => straight_path(self.src, self.dst),
            EdgeType::Bezier => bezier_path(self.src, self.dst, self.src_side, self.dst_side, 0.5),
            EdgeType::Step => step_path(self.src, self.dst, self.src_side, self.dst_side),
            EdgeType::SmoothStep => smoothstep_path(self.src, self.dst, self.src_side, self.dst_side, 8.0),
        };
        // 用 PathBuilder 构建路径
        let mut path = PathBuilder::new();
        path.move_to(points[0]);
        match self.edge_type {
            EdgeType::Bezier => {
                path.curve_to(points[1], points[2], points[3]);
            }
            _ => {
                for p in points.iter().skip(1) {
                    path.line_to(*p);
                }
            }
        }
        let path = path.build();
        window.paint_path(1.0, &path, gpui::black());  // stroke
        // 箭头：在 dst 点画三角形，方向由最后一段决定
        self.paint_arrow(window, &points);
    }
}
```

**要点**：
- bezier 返回 4 点（P0, ctrl1, ctrl2, P3），用 `curve_to`
- 其他返回折线点序列，用 `line_to`
- 箭头：取最后两点方向，画 8px 等边三角形
- Phase 2 先用节点中心作为 src/dst（简化），Phase 4 接入 `resolve_endpoints`

**渲染方式**：在 FlowEditorView 的 `render_canvas` 中，用 `canvas(prepaint, paint)` 包装，paint 回调里遍历所有边调用 `EdgeView::paint`。

#### 2.11 minimal_demo 示例

**文件**：`crates/gpui/examples/minimal_demo.rs`

```rust
use rust_agent_flow::*;
use rust_agent_flow_gpui::FlowEditorView;

fn main() {
    let app = Application::new();
    app.run(move |cx: &mut App| {
        rust_agent_flow_gpui::init(cx);
        cx.open_window(WindowOptions::default(), |window, cx| {
            let mut graph = FlowGraph::new();
            let n1 = graph.add_node("start", serde_json::json!({}));
            let n2 = graph.add_node("action", serde_json::json!({}));
            graph.add_edge(Edge::new(n1, n2));
            // 设置初始位置避免重叠
            graph.node_mut(n1).unwrap().position = PointF::new(100.0, 100.0);
            graph.node_mut(n2).unwrap().position = PointF::new(400.0, 200.0);

            let view = cx.new(|cx| FlowEditorView::new(graph, cx));
            cx.new(|cx| Root::new(view, window, cx))
        });
    });
}
```

**要点**：
- 必须调用 `rust_agent_flow_gpui::init(cx)` 初始化组件库
- 用 `Root::new` 包裹（gpui-component 要求）
- demo feature 下编译，CI 验证

#### 2.12 panel 模块（Phase 2 空壳）

**文件**：`crates/gpui/src/panel/mod.rs`

```rust
//! 属性面板容器（Phase 3 接入 IFlowNode::get_panel）。
//! Phase 2 仅提供空壳，避免 lib.rs 编译失败。
```

#### 2.13 builtin 模块（Phase 2 空壳）

**文件**：`crates/gpui/src/builtin/mod.rs`

```rust
//! 内置节点实现（Phase 3 起逐步添加）。
```

---

### Phase 3：IFlowNode 扩展接口 + 注册表 + 内置节点

**目标**：实现策略模式，按 NodeKind 匹配 IFlowNode 实现；提供 Start/End/Action 三个内置节点。

#### 3.1 IFlowNode trait

**文件**：`crates/gpui/src/node/flow_node.rs`

```rust
use std::sync::Arc;
use gpui::*;
use rust_agent_flow::{Node, NodeId, NodeSchema, PortId, PointF, RectF};

/// 节点渲染上下文，提供给 IFlowNode 方法使用。
pub struct NodeViewCtx<'a> {
    pub window: &'a mut Window,
    pub cx: &'a mut App,
    pub selected: bool,
}

/// 节点扩展接口（策略模式）。
/// 每个 NodeKind 对应一个 IFlowNode 实现，提供卡片视图和属性面板。
pub trait IFlowNode: Send + Sync {
    /// 节点 kind 标识，用于注册表匹配。
    fn kind(&self) -> &str;

    /// 节点卡片布局界面（画布上显示的节点主体）。
    fn get_view(&self, node: &Node, ctx: &mut NodeViewCtx) -> AnyElement;

    /// 选中时右侧属性面板布局界面。
    fn get_panel(&self, node: &Node, ctx: &mut NodeViewCtx) -> AnyElement;

    /// 节点 Schema（端口定义、默认尺寸等）。
    fn schema(&self) -> &NodeSchema;

    /// 自定义端口位置计算（可选，默认返回 None 表示用框架统一算法）。
    /// 特殊节点（如条件分支）覆写此方法以精确控制端口位置。
    fn resolve_port(&self, _port: &PortId, _bounds: RectF, _ctx: &mut NodeViewCtx) -> Option<PointF> {
        None
    }
}
```

**要点**：
- `get_view`/`get_panel` 返回 `AnyElement`，GPUI 的类型擦除元素
- `resolve_port` 默认返回 None，特殊节点（条件分支）覆写
- `NodeViewCtx` 持有 window 和 cx，供节点实现调用 GPUI API

#### 3.2 NodeRegistry 注册表

**文件**：`crates/gpui/src/node/registry.rs`

```rust
use std::collections::HashMap;
use std::sync::Arc;
use super::IFlowNode;

#[derive(Default)]
pub struct NodeRegistry {
    nodes: HashMap<String, Arc<dyn IFlowNode>>,
}

impl NodeRegistry {
    pub fn new() -> Self { Self::default() }

    pub fn register(&mut self, node: Arc<dyn IFlowNode>) {
        let kind = node.kind().to_string();
        self.nodes.insert(kind, node);
    }

    pub fn get(&self, kind: &str) -> Option<&Arc<dyn IFlowNode>> {
        self.nodes.get(kind)
    }

    /// 提供给 port_calc 的回调：按 NodeKind 返回 PortSpec 列表。
    pub fn port_specs_for(&self, kind: &str) -> Vec<rust_agent_flow::PortSpec> {
        self.get(kind).map(|n| n.schema().ports.clone()).unwrap_or_default()
    }
}
```

#### 3.3 更新 NodeView 接入 IFlowNode

**文件**：`crates/gpui/src/node/view.rs`（Phase 3 重构）

**改动**：NodeView 持有 `Arc<dyn IFlowNode>`，render 时调用 `flow_node.get_view(node, ctx)`。

```rust
pub struct NodeView {
    pub node: Node,
    pub flow_node: Arc<dyn IFlowNode>,
    // 事件回调保持不变
}

impl RenderOnce for NodeView {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let mut ctx = NodeViewCtx { window, cx, selected: false };
        let content = self.flow_node.get_view(&self.node, &mut ctx);
        div().absolute().size_full().child(content)
    }
}
```

#### 3.4 内置节点：Start/End

**文件**：`crates/gpui/src/builtin/start_end.rs`

```rust
use std::sync::Arc;
use gpui::*;
use rust_agent_flow::*;
use crate::node::{IFlowNode, NodeViewCtx};

pub struct StartNode;
pub struct EndNode;

impl IFlowNode for StartNode {
    fn kind(&self) -> &str { "start" }
    fn schema(&self) -> &NodeSchema {
        // 静态 schema：1 个 Out 端口，Right 侧
        // 用 OnceLock 缓存避免重复构造
        static SCHEMA: std::sync::OnceLock<NodeSchema> = std::sync::OnceLock::new();
        SCHEMA.get_or_init(|| NodeSchema::new("start", "Start")
            .add_port(PortSpec::new("out", PortDirection::Out, PortSide::Right, "Out"))
            .default_size(SizeF::new(120.0, 60.0)))
    }
    fn get_view(&self, _node: &Node, _ctx: &mut NodeViewCtx) -> AnyElement {
        div().px_4().py_2().bg(gpui::rgb(0x22c55e)).text_color(gpui::white())
            .rounded_full().child("Start").into_any_element()
    }
    fn get_panel(&self, _node: &Node, _ctx: &mut NodeViewCtx) -> AnyElement {
        div().p_4().child("Start 节点（无配置）").into_any_element()
    }
}

// EndNode 类似，颜色红色，端口为 In/Left
```

#### 3.5 内置节点：Action

**文件**：`crates/gpui/src/builtin/action.rs`

```rust
pub struct ActionNode;

impl IFlowNode for ActionNode {
    fn kind(&self) -> &str { "action" }
    fn schema(&self) -> &NodeSchema {
        // In/Left + Out/Right
    }
    fn get_view(&self, node: &Node, _ctx: &mut NodeViewCtx) -> AnyElement {
        // 显示 node.data 中的 action 名称
        let label = node.data.get("name").and_then(|v| v.as_str()).unwrap_or("Action");
        div().p_3().bg(gpui::white()).border_1().rounded_md()
            .child(label).into_any_element()
    }
    fn get_panel(&self, node: &Node, _ctx: &mut NodeViewCtx) -> AnyElement {
        // 显示 action 配置表单（Phase 3 简化：只读展示 data）
        div().p_4().child(format!("Action 配置: {}", node.data)).into_any_element()
    }
}
```

#### 3.6 builtin 模块注册

**文件**：`crates/gpui/src/builtin/mod.rs`

```rust
mod start_end;
mod action;

use std::sync::Arc;
use crate::node::NodeRegistry;

pub fn register_all(registry: &mut NodeRegistry) {
    registry.register(Arc::new(start_end::StartNode));
    registry.register(Arc::new(start_end::EndNode));
    registry.register(Arc::new(action::ActionNode));
}
```

#### 3.7 属性面板容器

**文件**：`crates/gpui/src/panel/mod.rs`

```rust
use gpui::*;
use rust_agent_flow::{Node, NodeId};
use crate::node::{IFlowNode, NodeViewCtx};
use std::sync::Arc;

pub struct PanelView {
    pub node: Node,
    pub flow_node: Arc<dyn IFlowNode>,
}

impl RenderOnce for PanelView {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let mut ctx = NodeViewCtx { window, cx, selected: true };
        let content = self.flow_node.get_panel(&self.node, &mut ctx);
        div().w_80().h_full().bg(gpui::rgb(0xf8fafc)).border_l_1()
            .child(content)
    }
}
```

#### 3.8 FlowEditorView 接入 Registry

**文件**：`crates/gpui/src/editor/flow_editor.rs`（更新）

**改动**：
- 构造函数中调用 `builtin::register_all(&mut registry)`
- `render_node` 时从 registry 查 IFlowNode，构造 NodeView
- `selected` 时渲染 PanelView

---

### Phase 4：智能端点计算接入 + 边渲染完善

**目标**：将 core 层 `resolve_endpoints` 接入 gpui 层，实现端口感知的精确连线；添加缓存避免重复计算。

#### 4.1 EdgeView 接入 ResolvedEdge

**文件**：`crates/gpui/src/edge/edge_view.rs`（更新）

**改动**：EdgeView 接收 `ResolvedEdge`（含 src/dst 屏幕坐标 + side），不再用节点中心。

```rust
pub struct EdgeView {
    pub resolved: ResolvedEdge,  // 屏幕坐标
    pub edge_type: EdgeType,
    pub edge_kind: EdgeKind,  // Normal / LoopBack
}
```

**paint 逻辑更新**：
- `EdgeKind::LoopBack` → 调用 `loop_back_path`（Phase 7 完整实现，Phase 4 先用普通路径）
- `EdgeKind::Normal` → 按 edge_type 分发 4 种算法

#### 4.2 FlowEditorView 缓存 ResolvedEdge

**文件**：`crates/gpui/src/editor/flow_editor.rs`（更新）

**改动**：实现基于 `graph.version()` 的缓存：

```rust
fn resolved_edges(&self) -> &HashMap<EdgeId, ResolvedEdge> {
    let mut cache = self.resolved_edges.borrow_mut();
    if let Some((ver, map)) = cache.as_ref() {
        if *ver == self.graph.version() {
            return unsafe { &*(map as *const _) }; // 简化：实际用 RefCell::map
        }
    }
    // 重新计算
    let registry = self.registry.clone();
    let specs_fn = |id: NodeId| -> Vec<PortSpec> {
        self.graph.node(id)
            .map(|n| registry.port_specs_for(&n.kind.0))
            .unwrap_or_default()
    };
    let map = resolve_endpoints(&self.graph, specs_fn);
    // 转换为屏幕坐标
    let screen_map: HashMap<_, _> = map.into_iter()
        .map(|(eid, re)| (eid, ResolvedEdge {
            src: self.viewport.to_screen(re.src),
            src_side: re.src_side,
            dst: self.viewport.to_screen(re.dst),
            dst_side: re.dst_side,
        }))
        .collect();
    *cache = Some((self.graph.version(), screen_map));
    // 返回引用（实际实现需处理 RefCell 借用）
}
```

**要点**：实际实现需注意 RefCell 借用规则，可能需要 clone 或重构为 `Entity` 状态字段。

#### 4.3 端点 Handle 渲染

**文件**：`crates/gpui/src/node/view.rs`（更新）

**改动**：NodeView 根据 schema.ports 渲染 Handle，位置由 `resolve_port` 或框架算法决定。

```rust
// 在 get_view 返回的元素上叠加 Handle
fn render_handles(&self, schema: &NodeSchema) -> Vec<AnyElement> {
    schema.ports.iter().map(|port| {
        let pos = self.flow_node.resolve_port(&port.id, self.node.bounds(), ctx)
            .unwrap_or_else(|| default_port_position(self.node.bounds(), port.side));
        div().absolute()
            .left(pos.x).top(pos.y)
            .w_2().h_2().rounded_full()
            .bg(gpui::rgb(0x3b82f6))
            .on_mouse_down(MouseButton::Left, /* 触发 DrawingEdge */)
            .into_any_element()
    }).collect()
}
```

---

### Phase 5：布局引擎 + 方向切换

**目标**：接入 dagre 自动排版，支持纵向/横向布局切换。

#### 5.1 启用 dagre feature

**文件**：`crates/gpui/Cargo.toml`（更新）

**改动**：在 `[features]` 中：
```toml
dagre = ["rust-agent-flow/dagre"]
```
demo 默认启用：`default = ["dagre"]`（仅 demo 方便展示，lib 默认不启用）。

实际策略：`demo = ["dagre"]`，让 demo 自动启用 dagre。

#### 5.2 FlowEditorView 添加布局字段

**文件**：`crates/gpui/src/editor/flow_editor.rs`（更新）

```rust
pub struct FlowEditorView {
    // ... 现有字段
    pub layout_direction: LayoutDirection,
    #[cfg(feature = "dagre")]
    pub layout_engine: Option<Box<dyn LayoutEngine>>,
}
```

**方法**：
```rust
pub fn auto_layout(&mut self, cx: &mut Context<Self>) {
    #[cfg(feature = "dagre")]
    {
        if let Some(engine) = &self.layout_engine {
            if let Ok(result) = engine.layout(&self.graph, self.layout_direction) {
                for (id, pos) in &result.positions {
                    if let Some(node) = self.graph.node_mut(*id) {
                        node.position = *pos;
                    }
                }
                cx.notify();
            }
        }
    }
}

pub fn toggle_direction(&mut self, cx: &mut Context<Self>) {
    self.layout_direction = match self.layout_direction {
        LayoutDirection::Vertical => LayoutDirection::Horizontal,
        LayoutDirection::Horizontal => LayoutDirection::Vertical,
    };
    self.auto_layout(cx);
}
```

#### 5.3 工具栏按钮

**文件**：`crates/gpui/src/editor/flow_editor.rs`（更新 render）

在画布上方添加工具栏：
- "自动排版" 按钮 → `auto_layout`
- "切换方向" 按钮 → `toggle_direction`

用 gpui-component 的 `Button::new("layout").label("自动排版").on_click(...)`。

#### 5.4 更新 demo

**文件**：`crates/gpui/examples/minimal_demo.rs`（更新）

添加多个节点和边，演示自动排版效果。

---

### Phase 6：条件分支节点

**目标**：实现条件分支节点，节点出端口从分支列表项连出，精确计算端口位置。

#### 6.1 ConditionNode 实现

**文件**：`crates/gpui/src/builtin/condition.rs`

```rust
use std::sync::Arc;
use gpui::*;
use rust_agent_flow::*;
use crate::node::{IFlowNode, NodeViewCtx};

pub struct ConditionNode;

/// 条件分支数据结构（存储在 node.data 中）
/// { "branches": [{"id": "b1", "label": "条件1"}, {"id": "b2", "label": "否则"}] }
impl IFlowNode for ConditionNode {
    fn kind(&self) -> &str { "condition" }
    fn schema(&self) -> &NodeSchema {
        // 静态 schema：1 个 In/Left，Out 端口动态（由 resolve_port 计算）
        // schema 中只声明 In，Out 端口在 get_view 中根据 branches 动态渲染
        static SCHEMA: std::sync::OnceLock<NodeSchema> = std::sync::OnceLock::new();
        SCHEMA.get_or_init(|| NodeSchema::new("condition", "条件分支")
            .add_port(PortSpec::new("in", PortDirection::In, PortSide::Left, "In"))
            .default_size(SizeF::new(240.0, 160.0)))
    }

    fn get_view(&self, node: &Node, ctx: &mut NodeViewCtx) -> AnyElement {
        let branches: Vec<BranchItem> = node.data.get("branches")
            .and_then(|v| serde_json::from_value(v.clone()).ok())
            .unwrap_or_default();

        let mut col = div().v_flex().w_full();
        // 标题栏
        col = col.child(div().px_3().py_2().bg(gpui::rgb(0xf59e0b)).child("条件分支"));
        // 分支列表
        for (i, branch) in branches.iter().enumerate() {
            col = col.child(
                div().px_3().py_2().border_t_1().flex().items_center()
                    .child(div().flex_1().child(branch.label.clone()))
                    .child(self.render_branch_handle(i))  // 右侧出端口
            );
        }
        div().w_full().h_full().bg(gpui::white()).border_1().rounded_md()
            .overflow_hidden().child(col).into_any_element()
    }

    fn get_panel(&self, node: &Node, ctx: &mut NodeViewCtx) -> AnyElement {
        // 编辑分支列表：添加/删除/重命名分支
        // Phase 6 简化：只读展示
        div().p_4().child(format!("分支配置: {}", node.data)).into_any_element()
    }

    fn resolve_port(&self, port: &PortId, bounds: RectF, _ctx: &mut NodeViewCtx) -> Option<PointF> {
        // 端口 id 格式："branch_<index>"
        if let Some(idx_str) = port.0.strip_prefix("branch_") {
            if let Ok(idx) = idx_str.parse::<usize>() {
                // 计算分支项的 y 坐标
                let title_h = 40.0;  // 标题栏高度
                let branch_h = 36.0; // 每个分支项高度
                let y = bounds.origin.y + title_h + branch_h * idx as f32 + branch_h * 0.5;
                let x = bounds.origin.x + bounds.size.w;  // 右边
                return Some(PointF::new(x, y));
            }
        }
        None
    }
}

#[derive(serde::Deserialize)]
struct BranchItem { id: String, label: String }
```

**要点**：
- `resolve_port` 覆写：根据端口 id（`branch_0`, `branch_1`...）精确计算 y 坐标
- 端口位置 = 标题栏高度 + 分支索引 * 分支高度 + 半个分支高度
- get_view 中渲染分支项时，Handle 位置必须与 resolve_port 计算一致

#### 6.2 注册

**文件**：`crates/gpui/src/builtin/mod.rs`（更新）

```rust
mod condition;
pub fn register_all(registry: &mut NodeRegistry) {
    // ... 现有
    registry.register(Arc::new(condition::ConditionNode));
}
```

#### 6.3 port_calc 兼容性

**文件**：`crates/gpui/src/edge/edge_view.rs`（更新）

**改动**：渲染边时，优先调用 `flow_node.resolve_port`，若返回 None 再用 `resolve_endpoints` 的结果。

```rust
fn resolve_edge_endpoints(&self, edge: &Edge) -> (PointF, PointF) {
    let src_node = self.graph.node(edge.source).unwrap();
    let dst_node = self.graph.node(edge.target).unwrap();
    let src_flow = self.registry.get(&src_node.kind.0);
    let dst_flow = self.registry.get(&dst_node.kind.0);

    let src = src_flow.and_then(|f| f.resolve_port(&edge.source_port, src_node.bounds(), ctx))
        .or_else(|| self.resolved_edges().get(&edge.id).map(|r| r.src))
        .unwrap_or(src_node.center());
    // dst 同理
    (src, dst)
}
```

---

### Phase 7：循环节点 + 回环连线

**目标**：实现循环节点，符合循环条件时由循环条件出端口右出-向下回环-左侧绕回。

#### 7.1 LoopNode 实现

**文件**：`crates/gpui/src/builtin/loop_node.rs`

```rust
pub struct LoopNode;

/// 循环节点数据结构：
/// { "condition": "i < 10", "loop_body_target": "node_id" }
impl IFlowNode for LoopNode {
    fn kind(&self) -> &str { "loop" }
    fn schema(&self) -> &NodeSchema {
        static SCHEMA: std::sync::OnceLock<NodeSchema> = std::sync::OnceLock::new();
        SCHEMA.get_or_init(|| NodeSchema::new("loop", "循环")
            .add_port(PortSpec::new("in", PortDirection::In, PortSide::Left, "In"))
            .add_port(PortSpec::new("out_done", PortDirection::Out, PortSide::Right, "完成"))
            .add_port(PortSpec::new("out_loop", PortDirection::Out, PortSide::Right, "循环体"))
            .default_size(SizeF::new(200.0, 120.0)))
    }

    fn get_view(&self, node: &Node, ctx: &mut NodeViewCtx) -> AnyElement {
        let condition = node.data.get("condition")
            .and_then(|v| v.as_str()).unwrap_or("true");
        div().v_flex().w_full().h_full().bg(gpui::white()).border_1().rounded_md()
            .child(div().px_3().py_2().bg(gpui::rgb(0x8b5cf6)).text_color(gpui::white()).child("循环"))
            .child(div().px_3().py_2().child(format!("条件: {}", condition)))
            // 两个出端口：out_done（标题栏右）、out_loop（循环条件右）
            .into_any_element()
    }

    fn resolve_port(&self, port: &PortId, bounds: RectF, _ctx: &mut NodeViewCtx) -> Option<PointF> {
        match port.0.as_str() {
            "in" => Some(PointF::new(bounds.origin.x, bounds.origin.y + 20.0)),  // 标题栏左
            "out_done" => Some(PointF::new(bounds.origin.x + bounds.size.w, bounds.origin.y + 20.0)),  // 标题栏右
            "out_loop" => Some(PointF::new(bounds.origin.x + bounds.size.w, bounds.origin.y + 60.0)),  // 循环条件右
            _ => None,
        }
    }

    fn get_panel(&self, node: &Node, ctx: &mut NodeViewCtx) -> AnyElement {
        div().p_4().child(format!("循环节点配置: {}", node.data)).into_any_element()
    }
}
```

#### 7.2 回环边渲染

**文件**：`crates/gpui/src/edge/edge_view.rs`（更新）

**改动**：`EdgeKind::LoopBack` 时调用 `loop_back_path`：

```rust
fn paint(&self, window: &mut Window, _cx: &mut App) {
    let points = match self.edge_kind {
        EdgeKind::LoopBack => {
            // 需要循环节点的 bounds 来计算回环路径
            // bounds 由 FlowEditorView 传入
            loop_back_path(self.resolved.src, self.resolved.dst, self.horizontal, self.node_bounds)
        }
        EdgeKind::Normal => {
            match self.edge_type {
                EdgeType::Bezier => bezier_path(...),
                // ...
            }
        }
    };
    // 绘制路径
}
```

**EdgeView 结构更新**：
```rust
pub struct EdgeView {
    pub resolved: ResolvedEdge,
    pub edge_type: EdgeType,
    pub edge_kind: EdgeKind,
    pub horizontal: bool,        // 布局方向（影响 loop_back）
    pub node_bounds: RectF,      // 循环节点 bounds（仅 LoopBack 用）
}
```

#### 7.3 FlowEditorView 识别回环边

**文件**：`crates/gpui/src/editor/flow_editor.rs`（更新）

**改动**：渲染边时，检查 edge.edge_kind：
- `EdgeKind::LoopBack` → 构造 EdgeView 时传入 node_bounds 和 horizontal
- 判定逻辑：边的 source 节点 kind 为 "loop" 且 source_port 为 "out_loop" → LoopBack

```rust
fn edge_kind_for(&self, edge: &Edge) -> EdgeKind {
    if let Some(node) = self.graph.node(edge.source) {
        if node.kind.0 == "loop" && edge.source_port.0 == "out_loop" {
            return EdgeKind::LoopBack;
        }
    }
    EdgeKind::Normal
}
```

#### 7.4 注册

**文件**：`crates/gpui/src/builtin/mod.rs`（更新）

```rust
mod loop_node;
pub fn register_all(registry: &mut NodeRegistry) {
    // ... 现有
    registry.register(Arc::new(loop_node::LoopNode));
}
```

#### 7.5 更新 demo

**文件**：`crates/gpui/examples/minimal_demo.rs`（更新）

添加循环节点演示，展示回环连线效果。

---

## 五、Assumptions & Decisions

| # | 决策 | 理由 |
|---|---|---|
| 1 | Phase 2 实现完整 FSM（含 DrawEdge） | 用户确认；避免 Phase 4 重构 |
| 2 | 修复 CI 系统依赖 | 用户确认；确保 CI 通过 |
| 3 | 补充 core/src/lib.rs re-export | 用户确认；简化 gpui 层调用 |
| 4 | FlowEditorView 直接持有 FlowGraph（非 Entity） | MVP 简化；后续可改为 Entity 共享 |
| 5 | NodeView 用 RenderOnce（无状态） | 节点无独立状态，由父视图管理 |
| 6 | EdgeView 用 canvas paint（非 RenderOnce） | 边需要 PathBuilder，不适合 div 布局 |
| 7 | IFlowNode 返回 AnyElement | 类型擦除，支持异构节点 |
| 8 | resolve_port 默认返回 None | 特殊节点覆写，普通节点用框架算法 |
| 9 | Schema 用 OnceLock 缓存 | 避免每次调用重复构造 |
| 10 | dagre feature 仅 demo 启用 | 保持 lib 默认轻量 |
| 11 | 条件分支端口 id 格式 `branch_<idx>` | resolve_port 解析约定 |
| 12 | 循环节点 out_loop 端口触发 LoopBack | 约定优于配置 |
| 13 | 不实现 catmull 算法 | README 偏差，4 种算法已满足需求 |
| 14 | port_calc 缓存基于 graph.version() | 已有版本号机制，零成本失效 |

---

## 六、Verification

### Phase 2 验证

```bash
# 1. core 层测试仍通过
cargo test -p rust-agent-flow

# 2. gpui lib 编译通过
cargo test -p rust-agent-flow-gpui --lib

# 3. demo 编译通过
cargo check -p rust-agent-flow-gpui --example minimal_demo --features demo

# 4. demo 运行（手动验证）
cargo run -p rust-agent-flow-gpui --example minimal_demo --features demo
# 验证：窗口打开、显示 2 个节点 + 1 条边、中键平移、滚轮缩放、左键拖拽节点
```

### Phase 3 验证

```bash
cargo run -p rust-agent-flow-gpui --example minimal_demo --features demo
# 验证：Start/End/Action 节点显示不同样式、点击节点显示右侧属性面板
```

### Phase 4 验证

```bash
cargo run -p rust-agent-flow-gpui --example minimal_demo --features demo
# 验证：边从节点端口（非中心）连出、同侧进出端点不重叠、4 种 EdgeType 切换正常
```

### Phase 5 验证

```bash
cargo run -p rust-agent-flow-gpui --example minimal_demo --features demo,dagre
# 验证：点击"自动排版"按钮节点自动排列、点击"切换方向"纵向↔横向切换
```

### Phase 6 验证

```bash
cargo run -p rust-agent-flow-gpui --example minimal_demo --features demo
# 验证：条件分支节点显示标题栏+分支列表、边从分支列表项右侧端口连出
```

### Phase 7 验证

```bash
cargo run -p rust-agent-flow-gpui --example minimal_demo --features demo
# 验证：循环节点显示标题栏+循环条件、out_loop 端口连线 U 形回环
```

### CI 验证

```bash
# 模拟 CI 完整流程
cargo test -p rust-agent-flow
cargo test -p rust-agent-flow-gpui --lib
cargo check -p rust-agent-flow-gpui --example minimal_demo --features demo
```

---

## 七、实施顺序与依赖

```
Phase 2 (GPUI 骨架 + FSM)
    ↓
Phase 3 (IFlowNode + Registry + 内置节点)
    ↓
Phase 4 (端点计算接入) ←── 可与 Phase 5 并行
    ↓
Phase 5 (布局引擎 + 方向切换)
    ↓
Phase 6 (条件分支) ←── 可与 Phase 7 并行
    ↓
Phase 7 (循环节点)
```

**关键依赖**：
- Phase 3 依赖 Phase 2（需要 FlowEditorView 骨架）
- Phase 4 依赖 Phase 3（需要 IFlowNode::resolve_port）
- Phase 5 依赖 Phase 2（需要 FlowEditorView）
- Phase 6、7 依赖 Phase 3 + 4（需要 IFlowNode + 端点计算）

**建议实施顺序**：2 → 3 → 4 → 5 → 6 → 7（严格串行，每阶段验证通过再进入下一阶段）

---

## 八、风险与缓解

| 风险 | 缓解措施 |
|---|---|
| GPUI API 与预期不符（git rev 固定） | 实施前先写最小 demo 验证 canvas/PathBuilder/on_scroll_wheel API |
| GPUI 首次编译耗时数分钟 | 提前触发编译，后续迭代用 incremental |
| RefCell 借用冲突（resolved_edges 缓存） | 改为 Entity 字段或 clone 退出 |
| port_calc 与 resolve_port 协调 | 统一在 EdgeView 层：先 resolve_port，失败回退 resolve_endpoints |
| dagre feature 在 CI 编译慢 | demo 默认不启用 dagre，仅手动验证时启用 |
| GPUI 在 Windows 上的字体/渲染问题 | 依赖 gpui-component-assets 提供字体 |
