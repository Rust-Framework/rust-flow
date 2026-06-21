# GPUI Agent Flow Designer 开发计划

> 基于 Rust + GPUI + gpui-component 的智能体流程设计器（可视化编辑器）
> 参照 ReactFlow 成熟方案，高内聚低耦合，最简代码实现复杂逻辑

---

## 一、Summary（摘要）

构建一个框架无关的流程图核心库（`rust-agent-flow`）+ GPUI 渲染层（`rust-agent-flow-gpui`），实现类 ReactFlow 的流程设计器：图模型、几何引擎、有向图布局、节点扩展机制（IFlowNode）、智能端点计算、纵横布局切换、条件分支与循环节点。

**范围边界**：本次仅做可视化设计器，不含流程执行引擎（执行调度为后续阶段）。

---

## 二、Current State Analysis（现状分析）

仓库当前为空骨架：
- `Cargo.toml`：workspace 已声明 `crates/core`、`crates/gpui` 两个成员，依赖 `gpui`(zed git)、`gpui-component`(longbridge git)、`slotmap`、`serde`，但两个 crate 目录尚未创建。
- `README.md`：已描述目标架构（core = 图模型+几何+边，gpui = 画布+编辑器视图+交互FSM），MVP 特性（平移/缩放/拖拽/连线/贝塞尔边）。
- `.github/workflows/ci.yml`：CI 已预设 `cargo test -p rust-agent-flow`、`cargo test -p rust-agent-flow-gpui --lib`、`cargo check -p rust-agent-flow-gpui --example minimal_demo --features demo`。
- 无任何 `.rs` 源文件。

**结论**：需从零创建两个 crate 的完整结构，但 workspace 配置、CI、README 已就位，架构方向已明确。

---

## 三、Architecture（架构设计）

遵循高内聚低耦合，严格分层：

```
┌─────────────────────────────────────────────────────────┐
│  crates/gpui (rust-agent-flow-gpui)  — GPUI 渲染+交互层  │
│  ┌──────────────┐ ┌──────────────┐ ┌──────────────────┐ │
│  │ FlowEditor   │ │ NodeView     │ │ EdgeView         │ │
│  │ (主视图+视口) │ │ (节点容器)   │ │ (边渲染 canvas)  │ │
│  ├──────────────┤ ├──────────────┤ ├──────────────────┤ │
│  │ Interaction  │ │ IFlowNode    │ │ PropertyPanel    │ │
│  │ (交互FSM)    │ │ (扩展trait)  │ │ (右侧属性面板)   │ │
│  ├──────────────┤ ├──────────────┤ └──────────────────┤ │
│  │ Viewport     │ │ NodeRegistry │ │ builtin nodes    │ │
│  │ (平移缩放)   │ │ (kind→factory)│ │(Start/Condition/ │ │
│  └──────────────┘ └──────────────┘ │ Loop/Action/End) │ │
│                                     └──────────────────┘ │
├─────────────────────────────────────────────────────────┤
│  crates/core (rust-agent-flow)  — 框架无关核心层         │
│  ┌──────────┐ ┌──────────┐ ┌──────────┐ ┌────────────┐  │
│  │ graph    │ │ schema   │ │ geometry │ │ layout     │  │
│  │(FlowGraph│ │(NodeKind │ │(port_calc│ │(LayoutEngine│ │
│  │ Node Edge│ │ NodeData │ │ edge_path│ │ trait+dagre│ │
│  │ Port)    │ │ PortSpec)│ │ hit_test)│ │ impl)      │  │
│  └──────────┘ └──────────┘ └──────────┘ └────────────┘  │
│  ┌──────────┐                                            │
│  │ viewport │  (offset/scale/transform 纯数学)           │
│  └──────────┘                                            │
└─────────────────────────────────────────────────────────┘
```

### 分层原则
- **core 层零 GPUI 依赖**：纯数据结构 + 纯函数算法，可独立测试，定义自己的轻量几何类型（`PointF`/`SizeF`/`RectF`，f32 基础）。
- **gpui 层依赖 core**：在边界处做 `PointF ↔ Point<Pixels>` 转换（一行代码），不污染 core。
- **IFlowNode 属于 gpui 层**：因为 `get_view()/get_panel()` 返回 GPUI 元素，天然 GPUI 绑定。core 只提供 schema 数据标准。

### 模块清单

#### crates/core/src/
| 模块 | 职责 | 关键类型 |
|---|---|---|
| `lib.rs` | crate 入口、re-export | — |
| `graph/mod.rs` | 图结构 | `FlowGraph` |
| `graph/node.rs` | 节点 | `Node`, `NodeId`(slotmap key), `NodeKind`, `NodeData` |
| `graph/edge.rs` | 边 | `Edge`, `EdgeId`, `EdgeType`(Bezier/Straight/Step/SmoothStep), `EdgeKind`(Normal/LoopBack) |
| `graph/port.rs` | 端口 | `PortId`, `PortDirection`(In/Out), `PortSide`(Top/Right/Bottom/Left/Auto) |
| `schema/mod.rs` | schema 标准 | `NodeSchema`, `PortSpec` |
| `schema/node_schema.rs` | 节点 schema 声明 | `NodeSchema { kind, ports, default_size }` |
| `geometry/mod.rs` | 几何类型 | `PointF`, `SizeF`, `RectF` |
| `geometry/port_calc.rs` | 端点智能计算 | `PortResolver`（浮动端点+分布算法） |
| `geometry/edge_path.rs` | 边路径生成（4 种算法） | `bezier_path()`, `straight_path()`, `step_path()`, `smoothstep_path()`, `loop_back_path()` |
| `geometry/hit_test.rs` | 命中测试 | `point_in_rect()`, `point_to_path_distance()` |
| `layout/mod.rs` | 布局抽象 | `LayoutEngine` trait, `LayoutDirection`(Vertical/Horizontal), `LayoutResult` |
| `layout/dagre.rs` | dagre 实现 | `DagreLayout`（feature = "dagre"） |
| `viewport.rs` | 视口数学 | `Viewport { offset, scale }`, `to_screen()`, `to_logical()` |

#### crates/gpui/src/
| 模块 | 职责 | 关键类型 |
|---|---|---|
| `lib.rs` | crate 入口、`init()` | — |
| `editor/flow_editor.rs` | 主编辑器视图 | `FlowEditorView`（Render，持有 graph+viewport+selection） |
| `editor/viewport.rs` | 视口交互 | 平移（中键拖拽）、缩放（滚轮，鼠标锚点） |
| `editor/interaction.rs` | 交互状态机 | `InteractionState`(Idle/Pan/DragNode/DrawEdge/Select) |
| `node/flow_node.rs` | 节点扩展接口 | `IFlowNode` trait（`get_view`, `get_panel`, `resolve_port`） |
| `node/registry.rs` | 节点注册表 | `NodeRegistry`（`HashMap<NodeKind, Box<dyn IFlowNodeFactory>>`） |
| `node/view.rs` | 节点容器视图 | `NodeView`（框架统一容器：标题栏+选中框+端口+内容区） |
| `edge/edge_view.rs` | 边渲染 | `EdgeView`（canvas + PathBuilder 绘制贝塞尔/正交边+箭头） |
| `panel/property_panel.rs` | 右侧属性面板 | `PropertyPanel`（选中节点时显示 `IFlowNode.get_panel()`） |
| `builtin/mod.rs` | 内置节点注册 | `register_builtins(&mut NodeRegistry)` |
| `builtin/start_end.rs` | 开始/结束节点 | — |
| `builtin/action.rs` | 动作节点 | — |
| `builtin/condition.rs` | 条件分支节点 | 标题栏+分支列表，每分支一个出端口 |
| `builtin/loop_node.rs` | 循环节点 | 标题栏+循环条件，双出端口（退出/循环体） |
| `examples/minimal_demo.rs` | 演示 | feature = "demo" |

---

## 四、Proposed Changes（分阶段实施）

按依赖关系排序，每阶段独立可验证。

### Phase 1: Core 图模型 + Schema + 几何基础（无 GPUI）

**目标**：建立框架无关的数据结构和算法，可纯单元测试。

**文件**：`crates/core/Cargo.toml` + `crates/core/src/**`

**关键设计**：

1. **几何类型**（`geometry/mod.rs`）：定义 `PointF{x,y:f32}`、`SizeF{w,h:f32}`、`RectF{origin,size}`，配套 `contains`、`center`、`expand` 方法。不引入 euclid，保持零额外依赖。

2. **图结构**（`graph/`）：
   - `FlowGraph`：持有 `SlotMap<NodeId, Node>` + `SlotMap<EdgeId, Edge>`，提供增删查改。
   - `Node`：`{ id, kind: NodeKind, data: NodeData, position: PointF, size: SizeF }`。
   - `NodeKind`：`String`（非 enum，支持自定义扩展，符合策略模式）。
   - `NodeData`：`serde_json::Value`（或 `HashMap<String, Value>`），业务数据自由承载。
   - `Edge`：`{ id, source: NodeId, source_port: Option<PortId>, target: NodeId, target_port: Option<PortId>, edge_type: EdgeType, kind: EdgeKind }`。
   - `EdgeType`：`enum { Bezier, Straight, Step, SmoothStep }`（连线算法，默认 Bezier）。
   - `EdgeKind`：`enum { Normal, LoopBack }`（语义类型，LoopBack 用于循环节点回环，渲染时调用 `loop_back_path`）。
   - `PortId`：`String`（节点内唯一）。

3. **Schema 标准**（`schema/`）：
   - `NodeSchema`：`{ kind: NodeKind, label: String, ports: Vec<PortSpec>, default_size: SizeF }`。
   - `PortSpec`：`{ id: PortId, direction: PortDirection, side: PortSide, label: Option<String> }`。
   - `PortSide::Auto`：表示由框架智能计算（对应需求3）。
   - 图灵完备覆盖：内置 kind 至少包含 `start`/`end`/`action`/`condition`/`loop`，分别对应顺序、分支、循环三种控制结构，足以表达图灵完备流程。

4. **边路径生成**（`geometry/edge_path.rs`）：提供 4 种连线算法，直接移植 ReactFlow 公式（见研究结论）。统一返回点序列（`Vec<PointF>`），gpui 层用 `PathBuilder` 构建 GPUI Path：
   - `straight_path(src, dst) -> Vec<PointF>`：直线，两点直连，标签在中点。
   - `bezier_path(src, dst, src_side, dst_side, curvature) -> Vec<PointF>`：三次贝塞尔曲线。控制点偏移 `offset = dist>=0 ? 0.5*dist : curvature*25*sqrt(-dist)`（解决反向连接控制点塌缩），默认 curvature=0.25。返回 4 点（起点+2 控制点+终点）供 `curve_to` 使用。
   - `step_path(src, dst, src_side, dst_side) -> Vec<PointF>`：正交直角连线。拐角为尖锐直角（borderRadius=0）。路径为"出端→中段拐点→入端"的折线，中段位置 `step_position=0.5`。处理三大分支：反向（正常 LTR/TTB）、同向（gapOffset 补偿防点重叠）、混合（flipSourceTarget 判断）。
   - `smoothstep_path(src, dst, src_side, dst_side, border_radius) -> Vec<PointF>`：正交圆角连线。路径骨架同 step，但拐角处用圆角曲线替代直角。圆角约束 `bend_size = min(dist(a,b)/2, dist(b,c)/2, border_radius)`，默认 border_radius=5，两段长度悬殊时圆角自动压缩。
   - `loop_back_path(src, dst, direction, node_bounds) -> Vec<PointF>`：循环节点回环专用 U 形路由（Phase 7）。

5. **视口数学**（`viewport.rs`）：`Viewport { offset: PointF, scale: f32 }`，`to_screen(logical) = logical * scale + offset`，`to_logical(screen) = (screen - offset) / scale`。

**验证**：`cargo test -p rust-agent-flow` 覆盖几何公式、图增删、视口变换。

---

### Phase 2: GPUI 编辑器骨架（画布+视口+基础渲染+交互FSM）

**目标**：可运行的空画布，支持平移缩放，能渲染静态节点和边。

**文件**：`crates/gpui/Cargo.toml` + `crates/gpui/src/{lib,editor,edge}/**`

**关键设计**：

1. **FlowEditorView**（`editor/flow_editor.rs`）：
   - 实现 GPUI `Render`，持有 `Entity<FlowGraph>`(Model) + `Viewport` + `InteractionState` + `NodeRegistry`。
   - `render()`：`Root` 包裹 → 主区域为 `canvas(prepaint, paint)` 绘制网格+边 + 节点层（`div().absolute()` 叠加）。
   - 初始化调用 `gpui_component::init(cx)`。

2. **视口交互**（`editor/viewport.rs`）：
   - 平移：空白处 `on_mouse_down` 记录起点 → `on_mouse_move` 更新 `offset` → `on_mouse_up` 结束。
   - 缩放：`on_scroll_wheel`，以鼠标为锚点：`new_offset = mouse - (mouse - old_offset) * (new_scale/old_scale)`。
   - 缩放范围 clamp（0.2 ~ 3.0）。

3. **交互状态机**（`editor/interaction.rs`）：
   - `enum InteractionState { Idle, Panning{start}, DraggingNode{node_id, start}, DrawingEdge{from_node, from_port, current} }`。
   - 每个 `on_mouse_*` 根据 state 分派，状态转换集中在一处，避免散落。

4. **节点渲染**（`node/view.rs`，本阶段简化版）：
   - `NodeView`：`div().absolute().left(x).top(y).size(w,h)`，框架统一容器（圆角矩形+边框+标题）。
   - 选中态：边框高亮。
   - 拖拽：`on_drag` 更新节点 `position`。

5. **边渲染**（`edge/edge_view.rs`）：
   - `canvas` paint 回调中遍历 `graph.edges`，用 `PathBuilder` 构建贝塞尔路径，`window.paint_path(stroke_path, color)`。
   - 箭头：路径终点画小三角。

6. **坐标转换**：节点 `position`(logical) → 屏幕坐标用 `viewport.to_screen()`；鼠标事件屏幕坐标 → logical 用 `viewport.to_logical()`。

**验证**：`cargo run -p rust-agent-flow-gpui --example minimal_demo --features demo` 显示画布，可平移缩放，有 2-3 个静态节点和连线。

---

### Phase 3: IFlowNode 扩展接口 + 注册表 + 简单内置节点

**目标**：建立节点扩展机制，实现 Start/End/Action 三个简单节点。

**文件**：`crates/gpui/src/node/{flow_node,registry,view}.rs` + `crates/gpui/src/builtin/{mod,start_end,action}.rs`

**关键设计**：

1. **IFlowNode trait**（`node/flow_node.rs`）：
   ```rust
   pub trait IFlowNode: Send + Sync {
       fn kind(&self) -> &str;
       /// 节点卡片内容区（框架提供外层容器：标题栏+边框+端口）
       fn get_view(&self, node: &Node, ctx: &NodeViewCtx) -> AnyElement;
       /// 右侧属性面板内容
       fn get_panel(&self, node: &Node, ctx: &NodeViewCtx) -> AnyElement;
       /// 自定义端口绝对位置（None 则框架用智能算法计算）
       fn resolve_port(&self, _port: &PortId, _bounds: RectF, _ctx: &NodeViewCtx) -> Option<PointF> { None }
       /// 节点 schema（端口声明、默认尺寸）
       fn schema(&self) -> &NodeSchema;
   }
   ```
   - `NodeViewCtx`：提供 `cx`、`theme`、`emit_data_change(closure)` 回调（编辑 data 时通知 graph 更新并重渲染）。

2. **NodeRegistry**（`node/registry.rs`）：
   - `HashMap<String, Arc<dyn IFlowNode>>`，按 `kind` 查找。
   - `register(node: Arc<dyn IFlowNode>)`、`get(kind) -> Option<&Arc<dyn IFlowNode>>`。
   - 策略模式：渲染节点时 `registry.get(node.kind)` → 调用 `get_view()`。

3. **NodeView 容器**（`node/view.rs`，完整版）：
   - 框架统一渲染：外层 `div().absolute()` → 标题栏（kind label）→ 内容区（`IFlowNode.get_view()`）→ 端口标记（小圆点，按 schema + resolve_port 定位）→ 选中边框。
   - 端口标记是可交互的（`on_mouse_down` 发起连线）。
   - 内容区由 IFlowNode 提供，容器由框架提供——避免每个节点重复实现容器。

4. **内置节点**：
   - `StartNode`/`EndNode`：单端口（出/入），内容区显示图标+文字。
   - `ActionNode`：入端口+出端口，内容区显示动作名称，属性面板可编辑名称/参数。

**验证**：demo 中显示 Start→Action→End 流程，选中 Action 右侧出现属性面板，可编辑名称并实时更新节点卡片。

---

### Phase 4: 智能端点计算 + 边渲染完善

**目标**：实现需求3的智能端点算法，边渲染使用动态端点。

**文件**：`crates/core/src/geometry/port_calc.rs` + `crates/gpui/src/edge/edge_view.rs`

**关键设计**：

1. **PortResolver**（`port_calc.rs`）：核心算法，输入 `FlowGraph` + 节点 bounds，输出每条边的实际端点坐标。

   **算法步骤**（参照 ReactFlow floating edges + 智能分布）：
   - **Step 1 - 选边**：对每条边，计算 `delta = target_center - source_center`：
     - `|dx| >= |dy|`：源端用 `Right`(dx>0) 或 `Left`(dx<0)，目标端用对侧。
     - `|dy| > |dx|`：源端用 `Bottom`(dy>0) 或 `Top`(dy<0)，目标端用对侧。
   - **Step 2 - 分布**：对每个节点的每一边，收集该边上所有端口（区分 In/Out），沿边均匀分布：
     - 单端口：居中（50%），稍向外偏移（+2px）。
     - 多端口：均分，如 3 个端口在右边 → 25%/50%/75% 位置。
   - **Step 3 - 防重叠**（需求3.3）：若同一边同时有 In 和 Out 端口，In 和 Out 各占一半区域分别分布，互不重叠。如右边：Out 占 25%~50%，In 占 50%~75%。
   - **Step 4 - 计算绝对坐标**：`port_abs = node_origin + side_anchor + offset_along_side`。

2. **边路径算法选择**：端点确定后，根据 `Edge.kind`（`EdgeType` 枚举：`Bezier`/`Straight`/`Step`/`SmoothStep`）选择路径算法：
   - `Bezier`：调用 `bezier_path`，适合自由流向的曲线（默认）。
   - `Straight`：调用 `straight_path`，两点直连，适合短距离或对角连接。
   - `Step`：调用 `step_path`，正交直角折线，拐角尖锐，适合规整网格布局。
   - `SmoothStep`：调用 `smoothstep_path`，正交圆角折线，拐角平滑，适合规整布局且视觉柔和。
   - 端点 side 由 PortResolver 确定，传入各算法函数处理对向/同向/混合三大分支。
   - `EdgeType` 可由用户在属性面板或全局设置切换，默认 `Bezier`。

3. **缓存**：`PortResolver` 结果缓存于 `FlowGraph` 版本号，节点移动时失效重算。

4. **resolve_port 优先级**：若 `IFlowNode.resolve_port()` 返回 `Some`，覆盖 PortResolver 的默认计算（用于条件分支节点的分支项端口）。

**验证**：拖动节点时连线端点动态切换边（左/右/上/下），多连线同边时端点均匀分布不重叠。

---

### Phase 5: 布局引擎 + 方向切换

**目标**：集成 dagre 自动布局，支持纵向/横向切换。

**文件**：`crates/core/src/layout/{mod,dagre}.rs` + `crates/gpui/src/editor/flow_editor.rs`

**关键设计**：

1. **LayoutEngine trait**（`layout/mod.rs`）：
   ```rust
   pub trait LayoutEngine: Send + Sync {
       fn layout(&self, graph: &FlowGraph, direction: LayoutDirection) -> LayoutResult;
   }
   pub enum LayoutDirection { Vertical, Horizontal }  // TB / LR
   pub struct LayoutResult { pub positions: HashMap<NodeId, PointF> }
   ```

2. **DagreLayout**（`layout/dagre.rs`，feature = "dagre"）：
   - 依赖 `mermaid-dagre` crate（1:1 dagre Rust 移植）。
   - 将 `FlowGraph` 转为 dagre 输入（节点+边），设置 `RankDir::TB`(Vertical) 或 `LR`(Horizontal)。
   - 取 dagre 输出坐标写回 `LayoutResult`。
   - 手动触发（如点击"自动排版"按钮），非每次拖拽重算。

3. **方向切换**（`flow_editor.rs`）：
   - `FlowEditorView` 持有 `layout_direction: LayoutDirection`。
   - 切换时：调用 `DagreLayout.layout(graph, direction)` → 更新所有节点 position → `cx.notify()`。
   - 方向影响 PortResolver 的默认 side 偏好（Vertical 偏好 Top-in/Bottom-out，Horizontal 偏好 Left-in/Right-out）。

**验证**：点击"横向布局"按钮，节点重排为左→右流向；点击"纵向布局"，重排为上→下；连线端点随方向自适应。

---

### Phase 6: 条件分支节点

**目标**：实现需求5.1的条件分支节点。

**文件**：`crates/gpui/src/builtin/condition.rs`

**关键设计**：

1. **节点结构**：
   - 标题栏（"条件分支" + 删除按钮）。
   - 分支列表：每项 = `{ label, condition_expr }`，纵向排列。
   - 1 个入端口（标题栏左侧/顶部），N 个出端口（每分支项右侧/底部）。
   - `NodeData` 存储 `branches: Vec<{label, expr}>`。

2. **端口位置精确计算**（`resolve_port` 覆写）：
   - 入端口：标题栏中部。
   - 每个分支出端口：该分支项行的右侧中部（横向）或底部中部（纵向）。
   - `resolve_port(port_id, bounds, ctx)`：根据 port_id 匹配分支索引，计算 `y = bounds.top + title_height + branch_index * branch_height + branch_height/2`，`x = bounds.right`。

3. **属性面板**（`get_panel`）：
   - 分支列表编辑器：增删分支、编辑 label 和 condition_expr。
   - 编辑后 `emit_data_change` 更新 `NodeData.branches`，触发节点重渲染+端口重算。

4. **布局尺寸**：节点高度 = title_height + branches.len() * branch_height；宽度固定或按内容自适应。

**验证**：添加条件节点，默认 2 分支（true/false），连线从分支项右出端点连出，位置精确对齐分支项；属性面板增删分支后节点高度和端口位置同步更新。

---

### Phase 7: 循环节点

**目标**：实现需求5.2的循环节点，含回环连线。

**文件**：`crates/gpui/src/builtin/loop_node.rs` + `crates/core/src/geometry/edge_path.rs`

**关键设计**：

1. **节点结构**：
   - 标题栏（"循环" + 循环条件编辑）。
   - 横向模式：标题栏左入端点、右出端点（退出）；循环条件区域右出端点（循环体）。
   - 纵向模式：标题栏顶入端点、底出端点（退出）；循环条件区域底出端点（循环体）。
   - `NodeData` 存储 `loop_condition: String`。

2. **双出端口**：
   - `exit` 端口：不符合循环条件时退出（标题栏右/底）。
   - `body` 端口：符合循环条件时进入循环体（循环条件区右/底）。
   - `resolve_port` 精确计算两个出端口位置，避免重叠。

3. **回环连线**（`edge_path.rs` 新增 `loop_back_path`）：
   - 循环体末节点 → 循环节点入端口的回环边，用专用 U 形路由：
     - 横向：末节点右出 → 向右延伸 → 向下 → 向左绕过循环节点 → 向上 → 循环节点左入。控制点确保绕开节点本体。
     - 纵向：末节点底出 → 向下延伸 → 向左 → 向上绕过循环节点 → 向右 → 循环节点顶入。
   - `loop_back_path(src, dst, direction, node_bounds) -> Vec<PointF>`：生成 U 形路径点序列，绕行间距固定（如 40px）。
   - 边类型标记 `EdgeKind::LoopBack`，EdgeView 渲染时调用 `loop_back_path`。

4. **属性面板**：编辑循环条件表达式。

**验证**：循环节点横向布局，exit 端口连到下一节点，body 端口连到循环体首节点，循环体末节点回环连回循环节点入端点，回环连线 U 形绕行不穿过节点。

---

## 五、Assumptions & Decisions（假设与决策）

| # | 决策 | 理由 |
|---|---|---|
| 1 | core 层零 GPUI 依赖，自定义 `PointF/SizeF/RectF` | 高内聚低耦合，core 可独立测试复用 |
| 2 | `NodeKind` 用 `String` 非 enum | 支持自定义节点扩展，策略模式按 kind 匹配 |
| 3 | `NodeData` 用 `serde_json::Value` | 业务数据自由承载，schema 只约束端口声明 |
| 4 | IFlowNode 属于 gpui 层 | `get_view/get_panel` 返回 GPUI 元素，天然 GPUI 绑定 |
| 5 | 端口位置默认智能计算，`resolve_port` 可覆写 | 兼顾简单节点（自动）和复杂节点（条件/循环精确控制） |
| 6 | 布局用 `mermaid-dagre` crate | 1:1 dagre 移植，最省力，符合"尽可能抄袭" |
| 7 | 边路径返回点序列非字符串 | gpui 层用 PathBuilder 构建，解耦格式 |
| 8 | 节点用声明式 `div().absolute()`，边用 canvas 命令式 | 节点需丰富交互用声明式，边数量多用 canvas 高效 |
| 9 | 本次不含执行引擎 | 范围确认，执行调度为后续阶段 |
| 10 | 图灵完备 = 节点类型覆盖顺序/分支/循环 | schema 可表达图灵完备控制流，非计算执行 |

---

## 六、Verification（验证步骤）

### 单元测试（core 层）
- `cargo test -p rust-agent-flow`
  - 几何：bezier/straight/step/smoothstep 四种路径点正确性、port_calc 分布算法、hit_test
  - 图：FlowGraph 增删查改、slotmap ID 稳定性
  - 视口：to_screen/to_logical 互逆
  - 布局：DagreLayout 输出节点数匹配、方向正确

### 单元测试（gpui 层）
- `cargo test -p rust-agent-flow-gpui --lib`
  - NodeRegistry 注册/查找
  - 交互状态机转换

### 集成验证（demo）
- `cargo run -p rust-agent-flow-gpui --example minimal_demo --features demo`
  - Phase 2：画布平移缩放、静态节点连线
  - Phase 3：Start/Action/End 节点、属性面板编辑
  - Phase 4：拖节点时端点动态切边、多连线同边不重叠、切换 Bezier/Straight/Step/SmoothStep 四种连线样式
  - Phase 5：纵横布局切换、自动排版
  - Phase 6：条件分支节点多分支出端口精确对齐
  - Phase 7：循环节点回环连线 U 形绕行

### CI
- `cargo test -p rust-agent-flow`
- `cargo test -p rust-agent-flow-gpui --lib`
- `cargo check -p rust-agent-flow-gpui --example minimal_demo --features demo`

---

## 七、实施顺序与依赖

```
Phase 1 (core 基础)
   ↓
Phase 2 (gpui 骨架) ──→ Phase 3 (IFlowNode + 简单节点)
                           ↓
Phase 5 (布局+方向) ←── Phase 4 (智能端点)
                           ↓
Phase 6 (条件分支) → Phase 7 (循环)
```

Phase 1→2 强依赖；Phase 3 依赖 2；Phase 4、5 可并行但建议 4 先（端点是边渲染基础）；Phase 6、7 依赖 3+4，可并行。
