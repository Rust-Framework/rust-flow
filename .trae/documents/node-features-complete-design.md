# 节点功能完整设计方案

## 概述

为 rust-agent-flow 流程编辑器设计完整的节点功能：节点删除（含连线自动修复）、条件分支节点展开/收起、循环节点展开/收起、属性面板编辑（CodeEditor + rhai 语法）、生产级节点尺寸优化。

## 当前状态分析

### 架构现状

| 维度    | 现状                                                | 问题                          |
| ----- | ------------------------------------------------- | --------------------------- |
| 节点删除  | `FlowGraph::remove_node` 级联删边，无 UI 入口             | 无删除按钮，无连线修复，无自动重排           |
| 悬停状态  | 无 `hovered` 字段                                    | 无法实现"鼠标移入才显示删除按钮"           |
| 动作回调  | `NodeViewCtx` 无回调能力                               | 节点无法向编辑器发送动作（删除/切换/数据更新）    |
| 展开/收起 | 不存在                                               | 条件/循环节点始终全量渲染               |
| 属性面板  | `render_simple_panel` 只读展示                        | 无任何编辑能力，无 CodeEditor，无 rhai |
| 布局重排  | 仅 `auto_layout()` / `set_layout_direction()` 手动触发 | 结构变化不自动重排                   |

### 关键文件清单

| 文件                                                          | 作用                                      |
| ----------------------------------------------------------- | --------------------------------------- |
| `crates/gpui/src/node/flow_node.rs`                         | `IFlowNode` trait + `NodeViewCtx`（需扩展）  |
| `crates/gpui/src/node/view.rs`                              | `NodeView` 组件 + `render_node_card`（需扩展） |
| `crates/gpui/src/editor/flow_editor.rs`                     | `FlowEditorView` 主视图（需扩展）               |
| `crates/gpui/src/editor/interaction.rs`                     | 交互状态机 + 鼠标事件（需扩展）                       |
| `crates/gpui/src/editor/hit_test.rs`                        | 命中测试（需扩展）                               |
| `crates/gpui/src/editor/rendering.rs`                       | 渲染层（需扩展）                                |
| `crates/gpui/src/builtin/common.rs`                         | 共享辅助函数（需扩展）                             |
| `crates/gpui/src/builtin/condition.rs`                      | Condition 节点（需大改）                       |
| `crates/gpui/src/builtin/loop_node.rs`                      | Loop 节点（需大改）                            |
| `crates/gpui/src/builtin/start.rs` / `end.rs` / `action.rs` | 简单节点（需加删除按钮）                            |
| `crates/gpui/src/panel/mod.rs`                              | `PanelView` 属性面板（需大改）                   |
| `crates/gpui/src/theme.rs`                                  | 主题颜色（需扩展）                               |
| `crates/core/src/graph/mod.rs`                              | `FlowGraph` 图模型（需加桥接方法）                 |

***

## 一、基础设施：动作回调系统 + 悬停追踪

### 1.1 NodeAction 枚举

**文件**: `crates/gpui/src/node/flow_node.rs`

新增动作枚举，统一描述节点可发出的动作：

```rust
/// 节点动作：节点视图/属性面板向编辑器发出的操作请求。
pub enum NodeAction {
    /// 删除此节点。
    Delete,
    /// 切换展开/收起状态。
    ToggleCollapse,
    /// 更新 node.data[key] = value。
    SetData(String, serde_json::Value),
}
```

### 1.2 NodeViewCtx 扩展

**文件**: `crates/gpui/src/node/flow_node.rs`

```rust
pub struct NodeViewCtx<'a> {
    pub window: &'a mut Window,
    pub cx: &'a mut App,
    pub selected: bool,
    /// 当前节点是否被鼠标悬停（用于显示删除按钮等 hover 元素）。
    pub hovered: bool,                    // 新增
    pub scale: f32,
    pub layout: LayoutDirection,
    pub theme: Theme,
    /// 动作回调：节点视图/面板通过此回调向编辑器发送动作。
    /// 闭包已捕获 node_id，调用方无需传入。
    pub on_action: Option<Arc<dyn Fn(NodeAction, &mut App) + Send + Sync>>,  // 新增
}
```

**为何用** **`Arc<dyn Fn + Send + Sync>`**：`NodeView` 需满足 `Send`（GPUI `Component<T>` 要求 `T: Send`），`Arc` 允许低成本克隆，`Send + Sync` 满足跨线程安全要求。

### 1.3 NodeView 扩展

**文件**: `crates/gpui/src/node/view.rs`

```rust
pub struct NodeView {
    // ... 现有字段 ...
    /// 当前节点是否被悬停。
    pub hovered: bool,                    // 新增
    /// 动作回调。
    pub on_action: Option<Arc<dyn Fn(NodeAction, &mut App) + Send + Sync>>,  // 新增
}
```

新增 builder 方法 `with_hovered(bool)` 和 `with_on_action(Option<Arc<...>>)`。

在 `RenderOnce::render` 中将 `hovered` 和 `on_action` 传入 `NodeViewCtx`。

### 1.4 PanelView 扩展

**文件**: `crates/gpui/src/panel/mod.rs`

```rust
pub struct PanelView {
    pub node: Node,
    pub flow_node: Option<Arc<dyn IFlowNode>>,
    pub theme: Theme,
    /// 动作回调（与 NodeView 共用同一闭包，已捕获 node_id）。
    pub on_action: Option<Arc<dyn Fn(NodeAction, &mut App) + Send + Sync>>,  // 新增
}
```

在 `RenderOnce::render` 中将 `on_action` 传入 `NodeViewCtx`。

### 1.5 FlowEditorView 扩展

**文件**: `crates/gpui/src/editor/flow_editor.rs`

```rust
pub struct FlowEditorView {
    // ... 现有字段 ...
    /// 当前悬停的节点 ID（用于显示删除按钮等 hover 元素）。
    pub hovered: Option<NodeId>,          // 新增
}
```

**新增方法**：

```rust
/// 处理节点动作（由 NodeView/PanelView 的回调调用）。
fn handle_node_action(
    &mut self,
    node_id: NodeId,
    action: NodeAction,
    cx: &mut Context<Self>,
) {
    match action {
        NodeAction::Delete => self.delete_node(node_id, cx),
        NodeAction::ToggleCollapse => {
            if let Some(node) = self.graph.node_mut(node_id) {
                let collapsed = node.data.get("collapsed")
                    .and_then(|v| v.as_bool())
                    .unwrap_or(false);
                node.data["collapsed"] = serde_json::json!(!collapsed);
            }
            self.relayout();
            cx.notify();
        }
        NodeAction::SetData(key, value) => {
            if let Some(node) = self.graph.node_mut(node_id) {
                node.data[key] = value;
            }
            self.sync_node_sizes();
            self.relayout();
            cx.notify();
        }
    }
}
```

### 1.6 悬停追踪

**文件**: `crates/gpui/src/editor/interaction.rs`

在 `on_mouse_move` 的 `InteractionState::Idle` 分支中增加悬停追踪：

```rust
InteractionState::Idle => {
    let hit = self.hit_test(logical);
    let new_hovered = match &hit {
        HitResult::Node(id)
        | HitResult::DeleteButton(id)
        | HitResult::ToggleButton(id) => Some(*id),
        HitResult::OutPort(id, _) | HitResult::InPort(id, _) => Some(*id),
        HitResult::Empty => None,
    };
    if new_hovered != self.hovered {
        self.hovered = new_hovered;
        cx.notify();
    }
}
```

### 1.7 动作回调注入

**文件**: `crates/gpui/src/editor/rendering.rs`

修改 `render_nodes` 和 `render_panel` 签名，接收 `Entity<Self>` 以创建动作回调：

```rust
pub(crate) fn render_nodes(
    &self,
    entity: gpui::Entity<Self>,
) -> Vec<gpui::AnyElement> {
    // ...
    self.graph.nodes().map(|node| {
        let node_id = node.id;
        let entity = entity.clone();
        let on_action: Arc<dyn Fn(NodeAction, &mut App) + Send + Sync> =
            Arc::new(move |action, cx| {
                cx.update_entity(entity.clone(), |view, cx| {
                    view.handle_node_action(node_id, action, cx);
                });
            });
        let view = NodeView::new(node.clone())
            // ... 现有 builder ...
            .with_hovered(self.hovered == Some(node_id))
            .with_on_action(Some(on_action));
        // ...
    }).collect()
}

pub(crate) fn render_panel(
    &self,
    entity: gpui::Entity<Self>,
) -> Option<gpui::AnyElement> {
    let node = self.selected.and_then(|id| self.graph.node(id).cloned())?;
    let node_id = node.id;
    let flow_node = self.registry.get(&node.kind);
    let on_action: Arc<dyn Fn(NodeAction, &mut App) + Send + Sync> =
        Arc::new(move |action, cx| {
            cx.update_entity(entity.clone(), |view, cx| {
                view.handle_node_action(node_id, action, cx);
            });
        });
    let panel = PanelView::new(node)
        .with_flow_node_opt(flow_node)
        .with_theme(self.theme)
        .with_on_action(Some(on_action));
    // ...
}
```

**文件**: `crates/gpui/src/editor/flow_editor.rs` 的 `Render::render`：

```rust
fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
    let entity = cx.entity();  // 获取 Entity<Self>
    let edges = self.render_edges();
    let nodes = self.render_nodes(entity.clone());
    let panel = self.render_panel(entity);
    // ...
}
```

> **注意**：`Context::entity()` 的确切 API 需在实现时验证。若不可用，可在 `FlowEditorView::new` 中返回 `(Self, Entity<Self>)` 或使用 `WeakEntity`。

***

## 二、节点删除 + 连线自动修复

### 2.1 删除按钮 UI

**文件**: `crates/gpui/src/builtin/common.rs`

新增共享辅助函数：

```rust
/// 删除按钮尺寸（逻辑坐标）。
pub(crate) const DELETE_BTN_SIZE: f32 = 20.0;

/// 渲染删除按钮（×图标），仅在 hover 时由调用方决定是否渲染。
pub(crate) fn render_delete_button(
    node_w: f32,
    scale: f32,
    theme: &Theme,
) -> AnyElement {
    let btn_size = DELETE_BTN_SIZE * scale;
    // 绝对定位在节点右上角：left = node_w - btn_size - padding, top = padding
    let left = node_w - btn_size - 4.0 * scale;
    let top = 4.0 * scale;
    div()
        .absolute()
        .left(px(left))
        .top(px(top))
        .w(px(btn_size))
        .h(px(btn_size))
        .rounded_md()
        .bg(theme.delete_btn_bg)
        .flex()
        .items_center()
        .justify_center()
        .text_size(px(12.0 * scale))
        .text_color(theme.delete_btn_text)
        .child("×")
        .into_any_element()
}

/// 展开/收起切换按钮尺寸（逻辑坐标）。
pub(crate) const TOGGLE_BTN_SIZE: f32 = 20.0;

/// 渲染展开/收起切换按钮。
pub(crate) fn render_toggle_button(
    node_w: f32,
    scale: f32,
    collapsed: bool,
    theme: &Theme,
) -> AnyElement {
    let btn_size = TOGGLE_BTN_SIZE * scale;
    // 位于删除按钮左侧
    let left = node_w - btn_size - 4.0 * scale - btn_size - 4.0 * scale;
    let top = (36.0 * scale - btn_size) * 0.5; // 标题栏垂直居中
    let icon = if collapsed { "▷" } else { "▽" };
    div()
        .absolute()
        .left(px(left))
        .top(px(top))
        .w(px(btn_size))
        .h(px(btn_size))
        .rounded_md()
        .bg(theme.toggle_btn_bg)
        .flex()
        .items_center()
        .justify_center()
        .text_size(px(12.0 * scale))
        .text_color(theme.toggle_btn_text)
        .child(icon)
        .into_any_element()
}
```

### 2.2 各节点渲染删除按钮

**简单节点（Action）**：在 `get_view` 中，当 `ctx.hovered` 为 true 时，在 `render_node_card` 返回的容器上叠加删除按钮。

修改 `render_node_card` 签名，增加 `hovered: bool` 和 `deletable: bool` 参数，内部条件渲染删除按钮。

**结构化节点（Condition/Loop）**：在 `get_view` 中，当 `ctx.hovered` 为 true 时，在容器中添加删除按钮子元素。

**Start/End 节点**：`deletable = false`，不渲染删除按钮。

### 2.3 命中测试扩展

**文件**: `crates/gpui/src/editor/hit_test.rs`

```rust
pub(crate) enum HitResult {
    Empty,
    Node(NodeId),
    OutPort(NodeId, PortId),
    InPort(NodeId, PortId),
    DeleteButton(NodeId),    // 新增
    ToggleButton(NodeId),    // 新增
}
```

在 `hit_test` 方法中，端口命中检查之后、节点主体命中检查之前，增加按钮命中检查：

```rust
// 2. 检查删除按钮命中（仅可删除节点）
if node.kind != "start" && node.kind != "end" {
    let btn_size = DELETE_BTN_SIZE;
    let btn_rect = RectF::new(
        PointF::new(
            node.position.x + node.size.w - btn_size - 4.0,
            node.position.y + 4.0,
        ),
        SizeF::new(btn_size, btn_size),
    );
    if point_in_rect(logical, btn_rect) {
        return HitResult::DeleteButton(node.id);
    }
}

// 3. 检查切换按钮命中（仅条件/循环节点）
if node.kind == "condition" || node.kind == "loop" {
    let btn_size = TOGGLE_BTN_SIZE;
    let btn_left = node.position.x + node.size.w - btn_size - 4.0 - btn_size - 4.0;
    let btn_top = node.position.y + (36.0 - btn_size) * 0.5;
    let btn_rect = RectF::new(
        PointF::new(btn_left, btn_top),
        SizeF::new(btn_size, btn_size),
    );
    if point_in_rect(logical, btn_rect) {
        return HitResult::ToggleButton(node.id);
    }
}
```

### 2.4 鼠标事件处理

**文件**: `crates/gpui/src/editor/interaction.rs`

在 `on_mouse_down` 中增加：

```rust
(MouseButton::Left, HitResult::DeleteButton(node_id)) => {
    self.delete_node(node_id, cx);
}
(MouseButton::Left, HitResult::ToggleButton(node_id)) => {
    self.handle_node_action(node_id, NodeAction::ToggleCollapse, cx);
}
```

### 2.5 删除节点 + 线性桥接

**文件**: `crates/gpui/src/editor/flow_editor.rs`

```rust
/// 删除节点：线性桥接 + 级联删边 + 自动重排。
///
/// 桥接策略（行业标准，参考 n8n/ReactFlow）：
/// - 仅当节点恰好有 1 条入边和 1 条出边时，自动桥接前驱→后继
/// - 多端口节点（条件/循环）删除时直接删除所有关联边，不做桥接
fn delete_node(&mut self, node_id: NodeId, cx: &mut Context<Self>) {
    // 收集边信息（避免借用冲突）
    let in_edges: Vec<(NodeId, Option<PortId>, EdgeType)> = self.graph
        .in_edges(node_id)
        .map(|e| (e.source, e.source_port.clone(), e.edge_type))
        .collect();
    let out_edges: Vec<(NodeId, Option<PortId>)> = self.graph
        .out_edges(node_id)
        .map(|e| (e.target, e.target_port.clone()))
        .collect();

    // 线性桥接：1 入 1 出 → 创建桥接边
    if in_edges.len() == 1 && out_edges.len() == 1 {
        let (src, src_port, edge_type) = &in_edges[0];
        let (dst, dst_port) = &out_edges[0];
        let mut bridge = rust_agent_flow::Edge::new(*src, *dst);
        bridge.source_port = src_port.clone();
        bridge.target_port = dst_port.clone();
        bridge.edge_type = *edge_type;
        self.graph.add_edge(bridge);
    }

    // 删除节点（级联删除所有关联边）
    self.graph.remove_node(node_id);

    // 清理选中/悬停状态
    if self.selected == Some(node_id) {
        self.selected = None;
    }
    if self.hovered == Some(node_id) {
        self.hovered = None;
    }

    // 自动重排
    self.relayout();
    cx.notify();
}
```

### 2.6 主题扩展

**文件**: `crates/gpui/src/theme.rs`

新增颜色字段：

```rust
// ====== 节点按钮 ======
/// 删除按钮背景色。
pub delete_btn_bg: Rgba,
/// 删除按钮文字色。
pub delete_btn_text: Rgba,
/// 切换按钮背景色。
pub toggle_btn_bg: Rgba,
/// 切换按钮文字色。
pub toggle_btn_text: Rgba,
/// 收起状态"..."胶囊背景色。
pub collapse_pill_bg: Rgba,
/// 收起状态"..."胶囊文字色。
pub collapse_pill_text: Rgba,
```

亮色/暗色主题分别设置合理颜色值。

***

## 三、条件分支节点展开/收起

### 3.1 收起状态数据存储

在 `node.data["collapsed"]: bool` 中存储收起状态（默认 `false`）。

### 3.2 收起状态渲染

**文件**: `crates/gpui/src/builtin/condition.rs`

当 `node.data["collapsed"] == true` 时：

```
高度 = TITLE_H (36px)
┌───────────────────────────────────────┐
│[In]  ◆ Condition    [▽] [×]   [merged]→│  标题栏 h=36
└───────────────────────────────────────┘
                                        ↑ 所有出口合并到此位置
```

* 标题栏：label（左）+ toggle按钮 + delete按钮(hover) + 合并出口端口（右边缘垂直居中）

* 不渲染条件项行和 else 行

* 所有 out 端口（if\_0, if\_1, ..., else）合并到标题栏右边缘，垂直居中

* 渲染一个"..."小胶囊在标题栏中（label 右侧），提示收起状态

### 3.3 展开状态渲染（现有逻辑 + toggle按钮）

保持现有条件项行 + else 行渲染逻辑，额外添加：

* toggle 按钮（标题栏右侧，显示 "▽"）

* delete 按钮（hover 时显示）

* "..." 胶囊不显示

### 3.4 content\_size 覆写

```rust
fn content_size(&self, node: &Node) -> SizeF {
    let collapsed = node.data.get("collapsed")
        .and_then(|v| v.as_bool())
        .unwrap_or(false);
    let h = if collapsed {
        TITLE_H
    } else {
        content_height(node)
    };
    SizeF::new(node.size.w, h)
}
```

### 3.5 port\_position 覆写

收起状态下，所有 out 端口返回同一位置（标题栏右边缘垂直居中）：

```rust
fn port_position(&self, node: &Node, port_id: &PortId, layout: LayoutDirection) -> Option<PointF> {
    let collapsed = node.data.get("collapsed")
        .and_then(|v| v.as_bool())
        .unwrap_or(false);

    if collapsed {
        // 收起状态：所有端口在标题栏边缘
        let title_mid_y = node.position.y + TITLE_H * 0.5;
        return match port_id.as_str() {
            "in" => match layout {
                LayoutDirection::Horizontal => Some(PointF::new(node.position.x, title_mid_y)),
                LayoutDirection::Vertical => Some(PointF::new(node.position.x + node.size.w * 0.5, node.position.y)),
            },
            // 所有 out 端口合并到标题栏右边缘
            _ => match layout {
                LayoutDirection::Horizontal => Some(PointF::new(node.position.x + node.size.w, title_mid_y)),
                LayoutDirection::Vertical => Some(PointF::new(node.position.x + node.size.w * 0.5, node.position.y + TITLE_H)),
            },
        };
    }

    // 展开状态：保持现有逻辑
    // ... 现有 port_position 代码 ...
}
```

### 3.6 收起状态视觉布局（横向）

```
┌──────────────────────────────────────────┐
│[In]  ◆ Condition  ...  [▽] [×]    [●]→│
│                                      ↑   │  h=36
└──────────────────────────────────────────┘
                                         ↑ 合并出口（if_0/if_1/else 共用）
```

* `[In]`: 入端口，左边缘垂直居中

* `◆ Condition`: 标题文字

* `...`: 收起指示胶囊（小圆角矩形，灰色背景）

* `[▽]`: toggle 按钮（点击展开）

* `[×]`: delete 按钮（hover 显示）

* `[●]→`: 合并出口端口，右边缘垂直居中

### 3.7 条件分支新增

在属性面板中提供"新增分支"按钮，点击后：

1. 生成新分支 id（`if_2`, `if_3`, ... 基于现有数量递增）
2. 通过 `NodeAction::SetData("conditions", new_conditions_json)` 更新 `node.data["conditions"]`
3. 同时需要在 schema 中注册新端口（或动态端口支持）

> **注意**：当前 `NodeSchema` 的端口是静态的（`Vec<PortSpec>`）。新增分支需要动态端口。两种方案：
>
> * A) 修改 `NodeSchema` 支持动态端口（基于 `node.data` 推导端口列表）
>
> * B) 预定义足够多的端口（if\_0 \~ if\_9），按条件数量激活
>
> 推荐方案 A：在 `ConditionNode` 中覆写一个新方法 `dynamic_ports(node) -> Vec<PortSpec>`，命中测试和端口解析时优先使用动态端口。

***

## 四、循环节点展开/收起

### 4.1 收起状态数据存储

在 `node.data["collapsed"]: bool` 中存储收起状态（默认 `false`）。

### 4.2 收起状态渲染

**文件**: `crates/gpui/src/builtin/loop_node.rs`

当 `node.data["collapsed"] == true` 时：

```
高度 = TITLE_H (36px)
┌──────────────────────────────────────────┐
│[In]  ⟳ Loop  ...  [▽] [×]               │
│[loop_in]↑              [loop_body]↓[done]│  h=36
└──────────────────────────────────────────┘
```

* 4 个端口全部放在标题栏边缘，垂直堆叠：

  * `in`: 左边缘，Y = 12（上）

  * `loop_in`: 左边缘，Y = 24（下）

  * `done`: 右边缘，Y = 12（上）

  * `loop_body`: 右边缘，Y = 24（下）

* "..." 胶囊在标题栏中部

* toggle 按钮 + delete 按钮

### 4.3 content\_size 覆写

```rust
fn content_size(&self, node: &Node) -> SizeF {
    let collapsed = node.data.get("collapsed")
        .and_then(|v| v.as_bool())
        .unwrap_or(false);
    let h = if collapsed {
        TITLE_H
    } else {
        TITLE_H + BODY_H
    };
    SizeF::new(node.size.w, h)
}
```

### 4.4 port\_position 覆写

收起状态下，端口垂直堆叠在标题栏两侧：

```rust
fn port_position(&self, node: &Node, port_id: &PortId, layout: LayoutDirection) -> Option<PointF> {
    let collapsed = node.data.get("collapsed")
        .and_then(|v| v.as_bool())
        .unwrap_or(false);

    if collapsed {
        let left = node.position.x;
        let right = node.position.x + node.size.w;
        let y_upper = node.position.y + 12.0;  // 上端口
        let y_lower = node.position.y + 24.0;  // 下端口
        return match port_id.as_str() {
            "in" => match layout {
                LayoutDirection::Horizontal => Some(PointF::new(left, y_upper)),
                LayoutDirection::Vertical => Some(PointF::new(node.position.x + node.size.w * 0.3, node.position.y)),
            },
            "loop_in" => match layout {
                LayoutDirection::Horizontal => Some(PointF::new(left, y_lower)),
                LayoutDirection::Vertical => Some(PointF::new(node.position.x + node.size.w * 0.1, node.position.y + TITLE_H * 0.5)),
            },
            "done" => match layout {
                LayoutDirection::Horizontal => Some(PointF::new(right, y_upper)),
                LayoutDirection::Vertical => Some(PointF::new(node.position.x + node.size.w * 0.7, node.position.y + TITLE_H)),
            },
            "loop_body" => match layout {
                LayoutDirection::Horizontal => Some(PointF::new(right, y_lower)),
                LayoutDirection::Vertical => Some(PointF::new(node.position.x + node.size.w * 0.9, node.position.y + TITLE_H * 0.5)),
            },
            _ => None,
        };
    }

    // 展开状态：保持现有逻辑
    // ... 现有 port_position 代码 ...
}
```

### 4.5 循环模式数据存储

在 `node.data` 中新增字段：

```json
{
    "label": "Loop",
    "loop_mode": "for_each",        // "for_each" | "while" | "for_loop" | "batch_parallel"
    "condition_expr": "i < 10",      // rhai 表达式（while/for_loop 模式使用）
    "collapsed": false
}
```

循环模式说明：

| 模式    | 值                | 说明            | 条件表达式             |
| ----- | ---------------- | ------------- | ----------------- |
| 数组循环  | `for_each`       | 遍历数组每个元素      | 不需要               |
| 条件循环  | `while`          | while 条件成立时循环 | 需要（rhai bool 表达式） |
| 计次循环  | `for_loop`       | for i in 0..n | 需要（rhai 范围表达式）    |
| 批量/并行 | `batch_parallel` | 批量并行处理        | 不需要               |

***

## 五、属性面板编辑（CodeEditor + rhai）

### 5.1 依赖添加

**文件**: `crates/gpui/Cargo.toml`

```toml
[dependencies]
# ... 现有依赖 ...
rhai = "1"          # rhai 表达式引擎（用于语法校验，后续可扩展为求值）
```

### 5.2 属性面板状态管理

**问题**：`PanelView` 是 `RenderOnce`（无状态），每次渲染重建。文本输入和代码编辑器需要持久状态。

**方案**：在 `FlowEditorView` 中存储面板输入状态：

```rust
pub struct FlowEditorView {
    // ... 现有字段 ...
    /// 属性面板输入状态（按节点 ID + 字段名索引）。
    /// 用于在面板重建时恢复文本输入/代码编辑器的未提交内容。
    panel_input_cache: HashMap<NodeId, HashMap<String, String>>,
}
```

当 `NodeAction::SetData` 被触发时，同步更新 `panel_input_cache`。面板渲染时从 cache 读取初始值。

> **备选方案**：若 `gpui-component` 的 `Editor`/`TextInput` 支持自身 `Entity` 状态管理（GPUI entity），可将 editor entity 存入 `panel_input_cache`，避免每次重建。实现时需查阅 `gpui-component` API 确定最佳方案。

### 5.3 Condition 节点属性面板

**文件**: `crates/gpui/src/builtin/condition.rs` 的 `get_panel` 方法

替换 `render_simple_panel`，实现可编辑面板：

```
┌─────────────────────────────────┐
│ Condition 节点（条件分支）        │  标题
├─────────────────────────────────┤
│ 节点名称                         │
│ ┌─────────────────────────────┐ │
│ │ Check                       │ │  TextInput
│ └─────────────────────────────┘ │
│                                 │
│ 条件分支列表                     │
│ ┌─────────────────────────────┐ │
│ │ if_0: amount > 100      [×] │ │  CodeEditor (rhai) + 删除
│ ├─────────────────────────────┤ │
│ │ if_1: user.is_admin      [×] │ │  CodeEditor (rhai) + 删除
│ └─────────────────────────────┘ │
│ ┌─────────┐                     │
│ │ + 添加  │                     │  新增分支按钮
│ └─────────┘                     │
│                                 │
│ ┌─────────────────────────────┐ │
│ │ Else（兜底）                 │ │  只读
│ └─────────────────────────────┘ │
│                                 │
│ ┌─────────┐                     │
│ │ 收起/展开 │                     │  Toggle 按钮
│ └─────────┘                     │
└─────────────────────────────────┘
```

每个条件分支项的 CodeEditor 配置：

* 语言：rhai

* 单行模式（条件表达式通常单行）

* 值来自 `node.data["conditions"][i]["label"]`

* 编辑完成（blur）时触发 `NodeAction::SetData("conditions", updated_json)`

### 5.4 Loop 节点属性面板

**文件**: `crates/gpui/src/builtin/loop_node.rs` 的 `get_panel` 方法

```
┌─────────────────────────────────┐
│ Loop 节点（循环）                │  标题
├─────────────────────────────────┤
│ 节点名称                         │
│ ┌─────────────────────────────┐ │
│ │ Loop                        │ │  TextInput
│ └─────────────────────────────┘ │
│                                 │
│ 循环模式                         │
│ ○ 数组循环 (For Each)            │  RadioGroup
│ ● 条件循环 (While)               │
│ ○ 计次循环 (For Loop)            │
│ ○ 批量/并行循环                  │
│                                 │
│ 条件表达式 (rhai)                │  仅 while/for_loop 模式显示
│ ┌─────────────────────────────┐ │
│ │ i < 10                      │ │  CodeEditor (rhai, 多行)
│ └─────────────────────────────┘ │
│                                 │
│ ┌─────────┐                     │
│ │ 收起/展开 │                     │  Toggle 按钮
│ └─────────┘                     │
└─────────────────────────────────┘
```

### 5.5 gpui-component 集成

需查阅 `gpui-component` 文档确认以下 API：

1. **TextInput**：单行文本输入，用于节点名称编辑

   * 是否支持 controlled 模式（value + on\_change 回调）

   * 是否需要 `Entity<TextInput>` 持久状态

2. **Editor / CodeEditor**：多行代码编辑器，用于 rhai 表达式

   * 如何设置语言（rhai 语法高亮）

   * 如何设置单行/多行模式

   * 如何获取/设置内容

   * 是否支持 `Entity<Editor>` 持久状态

3. **RadioGroup / Dropdown**：循环模式选择

   * 是否有现成组件

> **实现时注意**：`gpui-component` 的组件通常需要 `Entity` 状态管理。在 `RenderOnce` 组件中使用时，需将 `Entity` 存储在 `FlowEditorView` 或通过 `cx.new_entity` 创建（但需解决重建问题）。

### 5.6 rhai 语法支持

1. 添加 `rhai = "1"` 依赖
2. 在属性面板中，条件表达式编辑器配置为 rhai 语言
3. 可选：在编辑完成时用 `rhai::AST::parse` 校验语法合法性，无效时显示错误提示
4. 后续可扩展：实际执行 rhai 表达式进行条件判断（本方案不包含求值，仅编辑+校验）

### 5.7 简单节点属性面板增强

Action/Start/End 节点的属性面板也增加节点名称编辑能力（TextInput），保持与 Condition/Loop 一致的编辑体验。

***

## 六、生产级节点尺寸与视觉优化

### 6.1 尺寸调整

| 节点        | 现有尺寸        | 调整后      | 说明                |
| --------- | ----------- | -------- | ----------------- |
| Start     | 120×35      | 120×36   | 高度对齐 TITLE\_H     |
| End       | 120×35      | 120×36   | 同上                |
| Action    | 180×35      | 200×44   | 加宽加高，容纳 desc + 按钮 |
| Condition | 220×144(动态) | 240×(动态) | 加宽，容纳按钮           |
| Loop      | 220×80      | 240×80   | 加宽，容纳按钮           |

### 6.2 标题栏布局规范

所有节点标题栏统一 36px 高度，右侧布局（从右到左）：

```
[合并端口/出端口] [delete×(hover)] [toggle▽(条件/循环)] ... [label] [in端口]
```

* delete 按钮：20×20，距右边缘 4px，距顶 4px

* toggle 按钮：20×20，在 delete 左侧 4px，垂直居中

* "..." 胶囊（收起时）：在 label 右侧，小圆角矩形（高 16px，宽 24px）

### 6.3 文本溢出处理

节点 label 过长时截断显示省略号：

* 使用 GPUI 的 `text_ellipsis()` 或手动截断

* 最大显示宽度 = 节点宽度 - 按钮区域宽度 - padding

### 6.4 视觉细节

* 删除按钮 hover 时背景加深（`delete_btn_bg` → 更深色）

* toggle 按钮 hover 时背景加深

* "..." 胶囊使用半透明背景，与标题栏颜色协调

* 选中状态边框宽度增加到 2px

* 端口 hover 时外环放大（视觉反馈，可选）

***

## 实现顺序

按依赖关系分阶段实施：

### 阶段 1：基础设施

1. 新增 `NodeAction` 枚举（`flow_node.rs`）
2. 扩展 `NodeViewCtx`（`hovered` + `on_action`）
3. 扩展 `NodeView` 和 `PanelView`（存储 `hovered` + `on_action`）
4. `FlowEditorView` 新增 `hovered` 字段 + `handle_node_action` 方法
5. 悬停追踪（`interaction.rs` 的 `on_mouse_move`）
6. 动作回调注入（`rendering.rs` 的 `render_nodes` / `render_panel`）
7. 主题扩展（`theme.rs` 新增按钮颜色）

### 阶段 2：节点删除

1. 删除按钮渲染函数（`common.rs`）
2. 各节点 `get_view` 添加删除按钮（hover 时）
3. 命中测试扩展（`hit_test.rs` 新增 `DeleteButton`）
4. 鼠标事件处理（`interaction.rs`）
5. `delete_node` 方法 + 线性桥接（`flow_editor.rs`）

### 阶段 3：条件节点展开/收起

1. `ConditionNode` 收起状态渲染（`condition.rs`）
2. `content_size` 覆写（收起/展开高度）
3. `port_position` 覆写（收起状态合并端口）
4. toggle 按钮渲染 + 命中测试
5. "..." 胶囊渲染

### 阶段 4：循环节点展开/收起

1. `LoopNode` 收起状态渲染（`loop_node.rs`）
2. `content_size` 覆写
3. `port_position` 覆写（收起状态端口堆叠）
4. toggle 按钮复用条件节点的命中测试

### 阶段 5：属性面板编辑

1. 添加 rhai 依赖（`Cargo.toml`）
2. 面板输入状态管理（`FlowEditorView.panel_input_cache`）
3. 查阅 `gpui-component` API（TextInput / Editor / RadioGroup）
4. Condition 节点属性面板（名称 + 分支列表 + CodeEditor）
5. Loop 节点属性面板（名称 + 模式 + 条件表达式 CodeEditor）
6. 简单节点属性面板（名称 TextInput）
7. 动态端口支持（条件分支新增/删除）

### 阶段 6：生产级优化

1. 节点尺寸调整
2. 文本溢出处理
3. 视觉细节打磨（hover 效果、间距、对齐）
4. 整体测试 + 排版验证

***

## 假设与决策

| 决策点       | 选择                                 | 理由                       |
| --------- | ---------------------------------- | ------------------------ |
| 收起范围      | 仅收起节点内部显示                          | 用户确认，不破坏现有 dagre 布局      |
| 删除桥接策略    | 线性桥接 + 多端口直接删除                     | 行业标准（n8n/ReactFlow），用户确认 |
| 收起状态存储    | `node.data["collapsed"]`           | 随图持久化，简单直接               |
| 动作回调机制    | `Arc<dyn Fn + Send + Sync>`        | 满足 GPUI `Send` 要求，低成本克隆  |
| 按钮交互      | 命中测试方案（非 GPUI listener）            | 与现有架构一致，避免闭包捕获限制         |
| 面板状态管理    | `FlowEditorView.panel_input_cache` | 解决 `RenderOnce` 无状态问题    |
| rhai 集成深度 | 语法编辑 + 校验，不求值                      | 用户要求"rhai嵌入语法"，求值为后续扩展   |

## 风险与注意事项

1. **`Context::entity()`** **API**：需验证 GPUI 中获取 `Entity<Self>` 的确切方法。若不可用，改用 `new_entity` + 存储方案。
2. **`gpui-component`** **Editor API**：需查阅文档确认 TextInput/Editor 的状态管理方式（controlled vs Entity-based）。
3. **动态端口**：条件分支新增需要动态端口支持。当前 `NodeSchema` 端口是静态的，需扩展为基于 `node.data` 推导。
4. **排版稳定性**：收起/展开触发 `relayout()`，需确保 dagre 布局在节点尺寸变化后保持稳定，不产生跳跃。
5. **回环边兼容**：Loop 节点收起后，`loop_body` 和 `loop_in` 端口位置变化，需确保 `loop_back_path` 和 `compute_loop_bounds` 仍能正确计算。
6. **命中测试优先级**：按钮命中必须在节点主体命中之前检查，否则按钮区域会被节点主体"吞掉"。

## 验证步骤

1. **删除功能**：创建 A→B→C 链，删除 B，验证 A→C 自动桥接，排版无断裂
2. **多端口删除**：删除条件/循环节点，验证所有关联边被清除，无残留连线
3. **hover 显示**：鼠标移入节点，删除按钮出现；移出，消失
4. **条件收起**：点击 toggle，条件行隐藏，出口合并到标题栏，整体重排
5. **条件展开**：再次点击 toggle，条件行恢复，出口回到各行，整体重排
6. **循环收起**：点击 toggle，循环体区域隐藏，端口堆叠到标题栏，回环边正确路由
7. **属性编辑**：选中节点，面板中编辑名称，验证节点标题实时更新
8. **分支编辑**：在面板中编辑条件表达式（rhai），验证节点显示更新
9. **新增分支**：点击"添加"，验证新分支行出现，新端口可连线
10. **排版质量**：收起/展开后，dagre 布局保持高质量分层，无重叠/跳跃

