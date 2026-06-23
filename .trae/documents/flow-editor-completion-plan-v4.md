# Flow Editor 完善计划 v4（剩余阶段 4.2-5）

> 本计划承接 v3，聚焦剩余 4 个子任务的实施。v3 已完成：阶段 2（工具栏重构+数据源）、阶段 3（demo 数据驱动）、阶段 4.1（hit_test 加 EdgePlusButton）。

## Summary（摘要）

完成 rust-agent-flow GPUI 流程编辑器连线「+」按钮添加节点功能的剩余实施：

1. **阶段 4.2**：theme.rs 增加 3 个 plus button 颜色字段；rendering.rs 增加 `render_edge_plus_buttons` 方法，在 `render_edges` 的 canvas 之后叠加 div 覆盖层渲染所有可见边的「+」按钮。
2. **阶段 4.3**：interaction.rs 增加 `AddingNodeFromEdge` 交互状态；`on_mouse_down` 处理 `EdgePlusButton` 命中进入该状态，点击空白退出该状态。
3. **阶段 4.4**：flow_editor.rs 增加 `render_node_picker` 方法渲染节点类型选择浮层；在 `render()` 中集成浮层。
4. **阶段 5**：编译与运行验证。

## Current State Analysis（当前状态分析）

### 已完成（v3 阶段）
| 文件 | 状态 |
|------|------|
| `crates/gpui/src/editor/mod.rs` | 已注册 `mod data_source;` + 导出 `DataSource` |
| `crates/gpui/src/editor/flow_editor.rs` | 已加 `data_source` 字段、`set_data_source()`、`insert_node_at_edge()` |
| `crates/gpui/src/editor/toolbar.rs` | 已重写为 Button+Tooltip+Dropdown（318 行） |
| `demo/src/main.rs` | 已数据驱动（46 行） |
| `crates/gpui/src/editor/hit_test.rs` | 已加 `EdgePlusButton(EdgeId)` + `hit_test_edge_plus()` |
| `crates/gpui/src/editor/interaction.rs` | 已修复非穷尽 match（`EdgePlusButton(_) | Empty => None`） |

### 待完成（本计划范围）

| 文件 | 当前状态 | 需要改动 |
|------|---------|---------|
| `crates/gpui/src/theme.rs` | 82 行结构体字段，无 edge_plus_* | 加 3 个颜色字段 + light()/dark() 初始化 |
| `crates/gpui/src/editor/rendering.rs` | render_edges 在 line 270 以 `.size_full()` 结束，无 plus button | 加 `render_edge_plus_buttons` 方法 + 集成 |
| `crates/gpui/src/editor/interaction.rs` | 4 个状态变体，无 AddingNodeFromEdge | 加状态变体 + on_mouse_down 处理 |
| `crates/gpui/src/editor/flow_editor.rs` | render() 在 line 455 返回 container，无 picker | 加 `render_node_picker` 方法 + render 集成 |

### 关键 API 确认（来自代码探索）

1. **theme.rs 结构**：
   - 结构体字段在 line 16-83，最后一个字段 `toolbar_divider: Rgba`（line 82）
   - `light()` 在 line 93-155，toolbar 段在 line 141-153
   - `dark()` 在 line 158-220，toolbar 段在 line 206-218
   - 无 `panel_hover_bg`/`panel_text` 字段（picker hover 复用 `toolbar_hover_bg`）

2. **rendering.rs render_edges**：
   - line 134-270，返回 `canvas(...).size_full()`
   - 已有 `body_groups: &HashMap<NodeId, HashSet<NodeId>>` 参数
   - 已计算 `hidden_nodes: HashSet<NodeId>`（收起的循环体节点）
   - 已有 `s = self.scale()`、`offset_x/offset_y`

3. **flow_editor.rs render()**：
   - line 384-456，container 结构：edges → content(节点) → toolbar → panel
   - 在 line 455 `container` 返回前可加 picker child

4. **interaction.rs**：
   - `InteractionState` 枚举在 line 22-47，4 个变体（Idle/Panning/DraggingNode/DrawingEdge）
   - `on_mouse_down` 在 line 50-118，match `(event.button, self.hit_test(logical))`
   - 当前 `EdgePlusButton` 命中落在 `_ => {}` 默认分支（无处理）

5. **i18n.rs AddNode* 键**（已定义，可直接用）：
   - `AddNodeTitle`("添加节点"/"Add Node") - 浮层标题
   - `AddNodeAction`/`AddNodeCondition`/`AddNodeLoop`/`AddNodeVariable`/`AddNodeAdapter`/`AddNodeAgent` - 6 种节点类型标签

6. **kind_label_str**：private 函数在 `panel/mod.rs` line 1012，返回简短类型名（"Action"等）。本计划**不复用**此函数，改用 `AddNode*` 键（更语义化，"Action Node"）。

7. **insert_node_at_edge**：已在 flow_editor.rs line 259 实现，签名为 `pub(crate) fn insert_node_at_edge(&mut self, edge_id: EdgeId, kind: &str, cx: &mut Context<Self>)`，可直接调用。

8. **hit_test_edge_plus**：已在 hit_test.rs line 164 实现，跳过回环边（`target_port == "loop_in"`），半径 12px 逻辑坐标，中点用节点中心中点。

## Proposed Changes（具体改动）

### 阶段 4.2：theme.rs + rendering.rs 加 plus button 渲染

#### 4.2.1 theme.rs 加 3 个颜色字段
**文件**：`crates/gpui/src/theme.rs`
**What**：在 `toolbar_divider` 字段后增加 3 个 edge_plus_* 字段；在 `light()` 和 `dark()` 中初始化
**Why**：plus button 需要独立的背景/边框/hover 颜色，与节点/工具栏视觉区分
**How**：

结构体（line 82 后追加）：
```rust
pub toolbar_divider: Rgba,

// ====== 边「+」按钮 ======
pub edge_plus_bg: Rgba,
pub edge_plus_border: Rgba,
pub edge_plus_hover_bg: Rgba,
}
```

`light()`（line 153 后追加）：
```rust
toolbar_divider: gpui::rgb(0xe2e8f0),

// 边「+」按钮
edge_plus_bg: gpui::rgb(0xffffff),
edge_plus_border: gpui::rgb(0x94a3b8),
edge_plus_hover_bg: gpui::rgb(0x6366f1),
}
```

`dark()`（line 218 后追加）：
```rust
toolbar_divider: gpui::rgb(0x475569),

// 边「+」按钮
edge_plus_bg: gpui::rgb(0x334155),
edge_plus_border: gpui::rgb(0x94a3b8),
edge_plus_hover_bg: gpui::rgb(0x818cf8),
}
```

#### 4.2.2 rendering.rs 加 render_edge_plus_buttons 方法
**文件**：`crates/gpui/src/editor/rendering.rs`
**What**：
1. 新增 `render_edge_plus_buttons` 方法：遍历可见边，计算中点屏幕坐标，渲染 div 覆盖层
2. 修改 `render_edges`：在 canvas 之后返回包含 canvas + plus button 层的容器

**Why**：canvas 不便处理 hover 样式，用 absolute div 覆盖层承载 plus button 视觉
**How**：

**导入追加**（line 16-17 附近）：
```rust
use gpui::{canvas, div, px, App, AppContext, Entity, IntoElement, ParentElement, Point, Styled};
use rust_agent_flow::{Edge, EdgeId, EdgeType, FlowGraph, NodeId, PointF, PortSide, RectF};
use gpui_component::{Icon, IconName, Sizable};
```

**render_edges 修改**（line 134-270）：将返回值从单个 canvas 改为 `div().size_full().child(canvas).child(plus_buttons)`：

```rust
pub(crate) fn render_edges(
    &self,
    body_groups: &HashMap<NodeId, HashSet<NodeId>>,
) -> impl IntoElement {
    // ... 现有逻辑保留（edge_renders、drawing、offset 等）...

    let canvas_el = canvas(
        |bounds, _window, _cx| bounds.size,
        move |bounds, _size, window, _cx| {
            // ... 现有 paint 逻辑保留 ...
        },
    )
    .size_full();

    // plus button 覆盖层
    let plus_buttons = self.render_edge_plus_buttons(body_groups);

    div().size_full().child(canvas_el).child(plus_buttons)
}
```

**新增 render_edge_plus_buttons 方法**（放在 render_edges 之后）：
```rust
/// 渲染所有可见边中点的「+」按钮（div 覆盖层）。
///
/// 按钮位置 = viewport.offset + edge_midpoint × scale
/// 跳过回环边（target_port == "loop_in"）和连接到隐藏循环体节点的边。
fn render_edge_plus_buttons(
    &self,
    body_groups: &HashMap<NodeId, HashSet<NodeId>>,
) -> impl IntoElement {
    let s = self.scale();
    let offset_x = self.viewport.offset.x;
    let offset_y = self.viewport.offset.y;
    let bg = self.theme.edge_plus_bg;
    let border = self.theme.edge_plus_border;
    let text_color = self.theme.toolbar_text;

    // 收集隐藏节点（收起的循环体）
    let mut hidden_nodes: HashSet<NodeId> = HashSet::new();
    for (loop_node, body_nodes) in body_groups {
        if let Some(ln) = self.graph.node(*loop_node) {
            let body_collapsed = ln.data.get("body_collapsed")
                .and_then(|v| v.as_bool()).unwrap_or(false);
            if body_collapsed {
                hidden_nodes.extend(body_nodes.iter().copied());
            }
        }
    }

    let buttons: Vec<_> = self.graph.edges()
        .filter(|edge| edge.target_port.as_deref() != Some("loop_in"))
        .filter(|edge| !hidden_nodes.contains(&edge.source) && !hidden_nodes.contains(&edge.target))
        .filter_map(|edge| {
            let src = self.graph.node(edge.source)?;
            let dst = self.graph.node(edge.target)?;
            let src_center = PointF::new(
                src.position.x + src.size.w * 0.5,
                src.position.y + src.size.h * 0.5,
            );
            let dst_center = PointF::new(
                dst.position.x + dst.size.w * 0.5,
                dst.position.y + dst.size.h * 0.5,
            );
            let mid = PointF::new(
                (src_center.x + dst_center.x) * 0.5,
                (src_center.y + dst_center.y) * 0.5,
            );
            let screen_x = offset_x + mid.x * s;
            let screen_y = offset_y + mid.y * s;
            Some((edge.id, screen_x, screen_y))
        })
        .map(|(edge_id, x, y)| {
            div()
                .absolute()
                .left(px(x - 10.0))
                .top(px(y - 10.0))
                .w(px(20.0))
                .h(px(20.0))
                .rounded_full()
                .bg(bg)
                .border_1()
                .border_color(border)
                .flex()
                .items_center()
                .justify_center()
                .text_color(text_color)
                .child(Icon::new(IconName::Plus).xsmall())
        })
        .collect();

    div().size_full().children(buttons)
}
```

### 阶段 4.3：interaction.rs 加 AddingNodeFromEdge 状态

**文件**：`crates/gpui/src/editor/interaction.rs`
**What**：
1. `InteractionState` 增加 `AddingNodeFromEdge { edge_id, anchor }` 变体
2. `on_mouse_down` 处理 `EdgePlusButton` 命中：进入 `AddingNodeFromEdge` 状态
3. `on_mouse_down` 处理 `Empty` 命中：若当前在 `AddingNodeFromEdge` 状态，退出回 Idle
4. `on_mouse_move` 的 Idle 分支 hover 追踪：`EdgePlusButton` 不更新 hovered（已在 v3 修复）

**Why**：点击 plus button 后需弹出节点选择浮层，浮层显示期间处于 `AddingNodeFromEdge` 状态；点击浮层外取消
**How**：

**InteractionState 枚举增加变体**（line 47 后）：
```rust
/// 点击边「+」按钮后：等待用户在浮层中选择节点类型。
/// `anchor` 为点击时的屏幕坐标，用于浮层定位。
AddingNodeFromEdge {
    edge_id: rust_agent_flow::EdgeId,
    anchor: PointF,
}
```

**on_mouse_down 增加 EdgePlusButton 分支**（在 `HitResult::OutPort` 分支前插入）：
```rust
(MouseButton::Left, HitResult::EdgePlusButton(edge_id)) => {
    // 点击边「+」按钮：进入 AddingNodeFromEdge 状态，显示节点选择浮层
    let anchor = PointF::new(
        event.position.x.as_f32(),
        event.position.y.as_f32(),
    );
    self.interaction = InteractionState::AddingNodeFromEdge {
        edge_id,
        anchor,
    };
}
```

**on_mouse_down Empty 分支增加退出逻辑**（修改 line 103-114）：
```rust
(MouseButton::Left, HitResult::Empty) => {
    // 点击空白：若当前在 AddingNodeFromEdge 状态，仅退出浮层（不平移）
    if matches!(self.interaction, InteractionState::AddingNodeFromEdge { .. }) {
        self.interaction = InteractionState::Idle;
        cx.notify();
        return;
    }
    // 否则：左键拖拽空白区域 → 平移画布
    let start_screen = PointF::new(
        event.position.x.as_f32(),
        event.position.y.as_f32(),
    );
    self.selected = None;
    self.interaction = InteractionState::Panning {
        start_screen,
        origin: self.viewport.offset,
    };
}
```

**注意**：`on_mouse_down` 末尾已有 `cx.notify()`（line 117），新增分支无需额外调用。但 Empty 分支的早退需要 `cx.notify()` + `return`。

### 阶段 4.4：flow_editor.rs 加 render_node_picker + render 集成

**文件**：`crates/gpui/src/editor/flow_editor.rs`
**What**：
1. 新增 `render_node_picker` 方法：在 `AddingNodeFromEdge` 状态下渲染节点类型选择浮层
2. 在 `render()` 中调用 `render_node_picker`，将浮层加入 container

**Why**：用户点击 plus button 后需选择要插入的节点类型
**How**：

**导入追加**（line 28-31 附近）：
```rust
use gpui::{
    div, px, Context, CursorStyle, InteractiveElement, IntoElement, MouseButton, ParentElement,
    Pixels, Point, Render, Styled, Window,
};
use crate::i18n::{t, TKey};
```

**新增 render_node_picker 方法**（放在 render 方法之前或之后）：
```rust
/// 渲染节点类型选择浮层（仅在 AddingNodeFromEdge 状态下显示）。
///
/// 浮层定位 = anchor + (10, 10) 偏移，列出 6 种可插入节点类型。
/// 点击某项 → 调用 insert_node_at_edge → 退出 AddingNodeFromEdge。
fn render_node_picker(&self, cx: &mut Context<Self>) -> Option<impl IntoElement> {
    let InteractionState::AddingNodeFromEdge { edge_id, anchor } = &self.interaction else {
        return None;
    };
    let edge_id = *edge_id;
    let lang = self.language;
    let theme = self.theme;
    let bg = theme.panel_bg;
    let border = theme.panel_border;
    let title_text = theme.panel_title_text;
    let text_color = theme.panel_label_text;
    let hover_bg = theme.toolbar_hover_bg;
    let subtext = theme.panel_subtext;

    // 6 种可插入节点类型 + 对应 i18n 键
    let kinds: [(&str, TKey); 6] = [
        ("action", TKey::AddNodeAction),
        ("condition", TKey::AddNodeCondition),
        ("loop", TKey::AddNodeLoop),
        ("variable", TKey::AddNodeVariable),
        ("adapter", TKey::AddNodeAdapter),
        ("agent", TKey::AddNodeAgent),
    ];

    Some(
        div()
            .absolute()
            .left(px(anchor.x + 10.0))
            .top(px(anchor.y + 10.0))
            .w(px(160.0))
            .bg(bg)
            .border_1()
            .border_color(border)
            .rounded_md()
            .shadow_lg()
            .p_2()
            .flex()
            .flex_col()
            .gap_1()
            .on_mouse_down(MouseButton::Left, |_, _, _| {
                // 拦截浮层内点击，防止冒泡到画布的 Empty 分支导致浮层关闭
            })
            .child(
                div()
                    .text_sm()
                    .text_color(subtext)
                    .child(t(lang, TKey::AddNodeTitle).to_string()),
            )
            .children(kinds.iter().map(|&(kind, key)| {
                div()
                    .id(("node-picker", kind))
                    .px_2()
                    .py_1()
                    .rounded_md()
                    .text_sm()
                    .text_color(text_color)
                    .hover(|s| s.bg(hover_bg))
                    .on_mouse_down(MouseButton::Left, cx.listener(move |this, _, _, cx| {
                        this.insert_node_at_edge(edge_id, kind, cx);
                        this.interaction = InteractionState::Idle;
                        cx.notify();
                    }))
                    .child(t(lang, key).to_string())
            })),
    )
}
```

**render() 集成**（在 line 453 `if let Some(panel_view) = panel { ... }` 之后、line 455 `container` 返回之前）：
```rust
// ====== 节点选择浮层：仅在 AddingNodeFromEdge 状态下显示 ======
if let Some(picker) = self.render_node_picker(cx) {
    container = container.child(picker);
}

container
```

### 阶段 5：编译与运行验证

**验证步骤**：
1. `cargo build -p rust-agent-flow-gpui` — 确认 gpui crate 编译通过
2. `cargo build -p rust-agent-flow-demo` — 确认 demo 编译通过
3. `cargo run -p rust-agent-flow-demo` — 运行验证：
   - 边中点显示圆形「+」按钮（白色背景/灰色边框/Plus 图标）
   - 点击「+」按钮弹出节点选择浮层（6 种类型）
   - 点击浮层中某类型 → 边被拆分，新节点插入中间，自动选中
   - 点击浮层外空白 → 浮层关闭
   - 切换主题验证 plus button 颜色适配（暗色主题深色背景）
   - 数据源切换 3 个流程均能正常显示「+」按钮
4. 切换语言验证浮层文案切换（中英文）

## Assumptions & Decisions（假设与决策）

### 决策
1. **plus button 用 div 覆盖层而非 canvas 绘制**：div 便于未来扩展 hover/click 交互，canvas 仅画连线。
2. **plus button 中点用节点中心中点**：与 hit_test_edge_plus 保持一致（hit_test 已用此算法），避免命中与视觉错位。
3. **节点选择浮层用简单 div 列表**：不引入 PopupMenu 组件，6 种类型直接列出，简洁高效。
4. **浮层 i18n 用 AddNode* 键**：不复用 panel/mod.rs 的私有 `kind_label_str`，改用语义更明确的 `AddNodeAction`("Action Node") 等。
5. **浮层定位 = anchor + (10, 10)**：简单偏移，避免覆盖 plus button 本身。
6. **浮层内点击拦截冒泡**：浮层根 div 加 `on_mouse_down` 空闭包，防止点击浮层项时冒泡到画布 Empty 分支导致浮层提前关闭。
7. **AddingNodeFromEdge 状态下点击空白仅退出浮层**：不平移画布，避免用户误操作。
8. **plus button 颜色独立于工具栏**：新增 `edge_plus_*` 字段，便于独立调色（hover 时可改背景，当前实现未做 hover 切换，预留字段）。

### 假设
1. `gpui_component::Icon::new(IconName::Plus).xsmall()` 可用（panel/mod.rs 已用 `Icon::new(IconName::Close).xsmall()` 模式）。
2. `div().children(impl Iterator<Item = impl IntoElement>)` 可用（GPUI 标准模式）。
3. `div().shadow_lg()` 可用（GPUI 标准样式方法）。
4. `EdgeId` 是 `Copy` 类型（hit_test.rs 中 `return Some(edge.id)` 已隐式 Copy）。
5. `InteractionState` 已派生 `Debug, Clone, Default`（line 22），新增变体无需额外派生。

### 风险与回退
- **`div().shadow_lg()` 不存在**：回退为 `.shadow_sm()` 或删除阴影。
- **`Icon::new(IconName::Plus)` 不存在**：回退为用文字 "+"。
- **浮层点击拦截不生效**：若 `on_mouse_down` 空闭包无法阻止冒泡，回退为在 Empty 分支检查 `event.position` 是否在浮层矩形内。
- **plus button 视觉与节点重叠**：若边中点恰好落在节点上，plus button 会被节点遮挡。回退为提高 plus button 的 z-index（GPUI 中后添加的 child 在上层，已自然解决）。

## 文件改动清单

| 文件 | 改动类型 | 行数估计 |
|------|---------|---------|
| `crates/gpui/src/theme.rs` | 编辑（+9 行：3 字段 + 3×2 初始化） | 239 |
| `crates/gpui/src/editor/rendering.rs` | 编辑（+70 行：方法 + 集成） | 514 |
| `crates/gpui/src/editor/interaction.rs` | 编辑（+20 行：状态变体 + 分支） | 244 |
| `crates/gpui/src/editor/flow_editor.rs` | 编辑（+60 行：render_node_picker + 集成） | 516 |

## 实施顺序

1. **阶段 4.2.1**：theme.rs 加 3 个颜色字段 → 编译确认
2. **阶段 4.2.2**：rendering.rs 加 render_edge_plus_buttons + 集成 → 编译确认
3. **阶段 4.3**：interaction.rs 加 AddingNodeFromEdge 状态 + on_mouse_down 分支 → 编译确认
4. **阶段 4.4**：flow_editor.rs 加 render_node_picker + render 集成 → 编译确认
5. **阶段 5**：cargo run 运行验证
