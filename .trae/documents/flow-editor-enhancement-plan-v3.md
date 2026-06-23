# Flow Editor Enhancement Plan v3（续作）

> 本计划承接 v2 计划，聚焦剩余 4 个阶段的实施。Phase 1（属性面板 schema 驱动重构）已完成。

## Summary（摘要）

继续完成 rust-agent-flow GPUI 流程编辑器的 5 项功能完善：

1. **工具栏重构**：用 gpui-component `Button` + `Tooltip` + `DropdownMenu` 替换 462 行手写 div 按钮，接入 i18n，新增数据源切换。
2. **数据源集成**：将已创建但未接入的 `data_source.rs` 模块注册到编译系统，为 `FlowEditorView` 增加 `data_source` 字段和切换方法。
3. **Demo 数据驱动**：重写 `demo/main.rs`，用 `DataSource::AgentFlow.to_graph()` 替代 220 行硬编码 `build_agent_flow()`。
4. **连线「+」按钮**：在边中点渲染圆形「+」按钮，点击弹出节点选择面板，选择节点类型后插入到边中间（拆边插入）。
5. **编译与运行验证**。

## Current State Analysis（当前状态分析）

### 已完成
- `crates/gpui/src/panel/mod.rs`：1054 行 schema 驱动面板，FieldState 枚举统一管理 7 种字段类型，已使用 `Switch`/`Input`/`Icon` 等 gpui-component 组件。
- `crates/gpui/src/editor/data_source.rs`：241 行，`DataSource` 枚举（AgentFlow/DataPipeline/SimpleFlow）+ 3 个 FlowDocument 工厂，**文件存在但未注册到 mod.rs**。
- `crates/gpui/src/i18n.rs`：所有需要的 TKey 变体已定义（Tb*、DataSource*、AddNode*、EdgeType*、GridDensity*），中英文翻译齐全，`t(lang, key) -> &'static str` 函数就绪。

### 待完成（本计划范围）
| 文件 | 当前状态 | 需要改动 |
|------|---------|---------|
| `crates/gpui/src/editor/mod.rs` | 8 个子模块，无 data_source | 注册 `mod data_source;` + 导出 |
| `crates/gpui/src/editor/flow_editor.rs` | 15 字段，无 data_source | 加字段 + `set_data_source()` + `insert_node_at_edge()` |
| `crates/gpui/src/editor/toolbar.rs` | 462 行 div 按钮，无 Tooltip | 完全重写为 Button+Tooltip+Dropdown |
| `demo/src/main.rs` | 241 行硬编码 build_agent_flow | 重写为数据驱动 |
| `crates/gpui/src/editor/hit_test.rs` | 148 行，无 EdgePlusButton | 加 `EdgePlusButton(EdgeId)` + 边命中 |
| `crates/gpui/src/editor/rendering.rs` | 444 行，render_edges 无 plus button | 加 plus button 渲染 |
| `crates/gpui/src/editor/interaction.rs` | 224 行，无 plus button 处理 | 加点击处理 + 节点选择面板状态 |

### gpui-component API 确认（来自 panel/mod.rs 实际使用）
- 导入：`use gpui_component::{Icon, IconName, Sizable, StyledExt};`
- Switch：`use gpui_component::switch::Switch;` → `Switch::new(id).checked(bool).on_click(cx.listener(|this, &val, _w, cx| {...}))`
- Icon：`Icon::new(IconName::Close).xsmall()`
- Button（待引入）：`use gpui_component::button::{Button, ButtonGroup};`
- i18n：`use crate::i18n::{t, Language, TKey};` → `t(self.language, TKey::TbZoomIn)`

## Proposed Changes（具体改动）

### 阶段 2：工具栏重构 + 数据源集成

#### 2.1 注册 data_source 模块
**文件**：`crates/gpui/src/editor/mod.rs`
**What**：添加 `mod data_source;` 声明和 `pub use data_source::DataSource;` 导出
**Why**：data_source.rs 当前是孤儿模块，未纳入编译系统
**How**：
```rust
mod data_source;  // 新增
mod flow_editor;
// ... 其他模块
pub use data_source::DataSource;  // 新增
pub use flow_editor::{FlowEditorView, LayoutDirection};
pub use interaction::InteractionState;
```

#### 2.2 FlowEditorView 增加 data_source 字段
**文件**：`crates/gpui/src/editor/flow_editor.rs`
**What**：
1. 结构体增加 `pub data_source: DataSource` 字段（放在 `language` 字段后）
2. `new()` 初始化 `data_source: DataSource::default()`
3. 新增 `set_data_source()` 方法
4. 新增 `insert_node_at_edge()` 方法（阶段 4 使用）

**Why**：工具栏数据源切换 Dropdown 需要读写编辑器的当前数据源；切换时需重建图并重排
**How**：
```rust
use super::data_source::DataSource;

pub struct FlowEditorView {
    // ... 现有字段
    pub language: Language,
    pub data_source: DataSource,  // 新增
}

// new() 中：
language: Language::default(),
data_source: DataSource::default(),

// 新增方法：
pub fn set_data_source(&mut self, ds: DataSource, cx: &mut Context<Self>) {
    if self.data_source == ds {
        return;
    }
    self.data_source = ds;
    self.graph = ds.to_graph();
    self.selected = None;
    self.hovered = None;
    self.panel_view = None;
    self.viewport = Viewport::default();
    self.relayout();
    cx.notify();
}

/// 在边中间插入新节点：拆边 → 插入节点 → 连两条新边
pub(crate) fn insert_node_at_edge(
    &mut self,
    edge_id: rust_agent_flow::EdgeId,
    kind: &str,
    cx: &mut Context<Self>,
) {
    // 1. 读取原边信息
    let (src, src_port, dst, dst_port, edge_type) = match self.graph.edge(edge_id) {
        Some(e) => (e.source, e.source_port.clone(), e.target, e.target_port.clone(), e.edge_type),
        None => return,
    };
    // 2. 删除原边
    self.graph.remove_edge(edge_id);
    // 3. 创建新节点（用 schema default_data + default_size）
    let schema = self.registry.get(kind);
    let data = schema.map(|s| s.default_data()).unwrap_or(serde_json::json!({"label": kind}));
    let size = schema.and_then(|s| s.default_size).unwrap_or(rust_agent_flow::SizeF::new(180.0, 60.0));
    let new_id = self.graph.add_node_with_size(kind, data, size);
    // 4. 连接 src → new → dst
    let mut e1 = rust_agent_flow::Edge::new(src, new_id);
    e1.source_port = src_port;
    e1.edge_type = edge_type;
    self.graph.add_edge(e1);
    let mut e2 = rust_agent_flow::Edge::new(new_id, dst);
    e2.target_port = dst_port;
    e2.edge_type = edge_type;
    self.graph.add_edge(e2);
    // 5. 选中新节点 + 重排
    self.selected = Some(new_id);
    self.relayout();
    cx.notify();
}
```

#### 2.3 工具栏完全重写
**文件**：`crates/gpui/src/editor/toolbar.rs`
**What**：完全重写 462 行 div 按钮为 gpui-component Button + Tooltip + DropdownMenu
**Why**：任务 1 要求工具栏所有操作提供 tooltip 提示并本地化；任务 2 要求增加数据源切换
**How**：

**导入**：
```rust
use gpui_component::button::{Button, ButtonGroup};
use gpui_component::{Icon, IconName, Sizable, StyledExt};
use gpui_component::dropdown_menu::DropdownMenu;
use crate::i18n::{t, TKey};
use super::data_source::DataSource;
```

**按钮映射表**（IconName + TKey + 行为）：

| 按钮 | IconName | TKey (tooltip) | 行为 |
|------|----------|----------------|------|
| 放大 | Plus | TbZoomIn | `zoom_in(window, cx)` |
| 缩小 | Minus | TbZoomOut | `zoom_out(window, cx)` |
| 适应视图 | Maximize | TbFitView | `fit_view(cx)` |
| 重置视图 | Undo | TbResetView | `reset_view(cx)` |
| 横向布局 | ArrowRight | TbLayoutHorizontal | `set_layout_direction(Horizontal, cx)` |
| 纵向布局 | ArrowDown | TbLayoutVertical | `set_layout_direction(Vertical, cx)` |
| 点阵开关 | LayoutDashboard | TbToggleGrid | toggle show_grid |
| 拖拽开关 | Edit (fallback: 用文字) | TbToggleDrag | `toggle_drag(cx)` |
| 主题切换 | Sun/Moon | TbToggleTheme | `toggle_theme(cx)` |
| 语言切换 | 用文字 "En"/"中" | TbToggleLanguage | `toggle_language(cx)` |

**Dropdown 按钮**（用 `Button.dropdown_menu`）：

1. **边类型 Dropdown**（tooltip: TbEdgeType）：
   - 4 个菜单项：EdgeBezier/EdgeStraight/EdgeStep/EdgeSmoothStep
   - 点击设置 `default_edge_type` + 更新所有边

2. **点阵密度 Dropdown**（tooltip: TbGridDensity）：
   - 3 个菜单项：GridDensityCompact(20)/GridDensityNormal(28)/GridDensitySparse(40)
   - 点击调用 `set_grid_spacing()`

3. **数据源 Dropdown**（tooltip: TbDataSource）：
   - 3 个菜单项：DataSourceAgentFlow/DataSourceDataPipeline/DataSourceSimpleFlow
   - 点击调用 `set_data_source()`

**缩放百分比显示**：保留 40px 宽度的文本 div（非按钮）

**Button 模式**：
- 图标按钮：`Button::new(("tb-zoom-in",)).icon(IconName::Plus).tooltip(t(lang, TKey::TbZoomIn)).on_click(cx.listener(|this, _, window, cx| this.zoom_in(window, cx)))`
- 激活态按钮（布局方向/点阵/拖拽）：用 `.selected(bool)` 或条件样式
- Dropdown 按钮：`Button::new(("tb-edge-type",)).icon(IconName::Network).tooltip(...).dropdown_menu(|menu, window, cx| menu.menu(...))`

**布局结构**：
```
[+][100%][−] | [fit][reset] | [↔][↕] | [edge-type▾] | [grid][density▾] | [drag] | [theme][lang] | [data-source▾]
```

**保留的辅助方法**：`zoom_in`/`zoom_out`/`reset_view`/`fit_view`/`toggle_drag` 不变

**删除**：`divider()` 函数（改用 ButtonGroup 分组或 margin）

### 阶段 3：Demo 数据驱动

**文件**：`demo/src/main.rs`
**What**：重写 241 行，删除 `build_agent_flow()`/`set_position()`/`add_edge()` 三个硬编码函数
**Why**：任务 2 要求将 demo 改为数据驱动
**How**：
```rust
use rust_agent_flow_gpui::{DataSource, FlowEditorView};

fn main() {
    gpui_platform::application()
        .with_assets(Assets)
        .run(move |cx: &mut gpui::App| {
            rust_agent_flow_gpui::init(cx);
            cx.spawn(async move |cx| {
                cx.open_window(gpui::WindowOptions::default(), |window, cx| {
                    let graph = DataSource::AgentFlow.to_graph();
                    let view = cx.new(|cx| {
                        let mut editor = FlowEditorView::new(graph, cx);
                        editor.auto_layout(cx);
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

**导出调整**：`crates/gpui/src/lib.rs` 需确认 `DataSource` 已通过 `pub use editor::DataSource;` 导出（若未导出则添加）。

### 阶段 4：连线「+」按钮添加节点

#### 4.1 hit_test.rs 增加 EdgePlusButton 命中
**文件**：`crates/gpui/src/editor/hit_test.rs`
**What**：
1. `HitResult` 枚举增加 `EdgePlusButton(rust_agent_flow::EdgeId)` 变体
2. 新增 `hit_test_edges()` 方法：遍历所有可见边，计算每条边中点，若鼠标在 plus button 半径内（10px 逻辑距离）则命中
3. `hit_test()` 优先级调整：**端口 > 边 plus button > 删除按钮 > 切换按钮 > 节点主体 > 空白**

**Why**：plus button 需要可点击，必须先能命中
**How**：
```rust
pub enum HitResult {
    Empty,
    Node(NodeId),
    OutPort(NodeId, PortId),
    InPort(NodeId, PortId),
    DeleteButton(NodeId),
    ToggleButton(NodeId),
    EdgePlusButton(rust_agent_flow::EdgeId),  // 新增
}

// hit_test() 中，在 DeleteButton 之前插入：
if let Some(edge_id) = self.hit_test_edge_plus(logical) {
    return HitResult::EdgePlusButton(edge_id);
}

fn hit_test_edge_plus(&self, logical: PointF) -> Option<rust_agent_flow::EdgeId> {
    let radius = 12.0; // 逻辑坐标半径
    for edge in self.graph.edges() {
        // 跳过隐藏边（连接到收起循环体的边）
        if self.is_edge_hidden(edge) { continue; }
        let mid = self.edge_midpoint(edge);
        let dx = logical.x - mid.x;
        let dy = logical.y - mid.y;
        if dx * dx + dy * dy <= radius * radius {
            return Some(edge.id);
        }
    }
    None
}
```

**边中点计算**：用源节点出端口和目标节点入端口的中点（简化版，不做完整路径采样）

#### 4.2 rendering.rs 增加 plus button 渲染
**文件**：`crates/gpui/src/editor/rendering.rs`
**What**：在 `render_edges` 的 canvas paint 之后，叠加一个 div 层渲染所有可见边的 plus button
**Why**：canvas 不便处理点击事件，用 div 覆盖层承载 plus button 的视觉和交互
**How**：

由于 hit_test 已处理点击，rendering 只需绘制视觉。在 canvas 之后添加一个 absolute div 层：
```rust
// render_edges 返回的容器末尾，追加 plus button 层
let plus_buttons = self.render_edge_plus_buttons(entity, body_groups);
container = container.child(plus_buttons);
```

`render_edge_plus_buttons` 实现：
- 遍历可见边，计算中点屏幕坐标 = `offset + midpoint * scale`
- 每个按钮：`div().absolute().left(px(screen_x - 10)).top(px(screen_y - 10)).w(px(20)).h(px(20)).rounded_full().bg(theme.edge_plus_bg).border_1().border_color(theme.edge_plus_border).flex().items_center().justify_center().child(Icon::new(IconName::Plus).xsmall())`
- 颜色取自 theme（新增 `edge_plus_bg`/`edge_plus_border`/`edge_plus_hover_bg` 字段，或复用现有 toolbar 字段）

**Theme 扩展**（`crates/gpui/src/theme.rs`）：
- 新增 `edge_plus_bg: Rgba`（默认白色）
- 新增 `edge_plus_border: Rgba`（默认灰色）
- 新增 `edge_plus_hover_bg: Rgba`（默认浅蓝）
- 在 `light()`/`dark()` 中初始化

#### 4.3 interaction.rs 增加 plus button 点击处理
**文件**：`crates/gpui/src/editor/interaction.rs`
**What**：
1. `InteractionState` 增加 `AddingNodeFromEdge { edge_id, screen_pos }` 变体
2. `on_mouse_down` 处理 `EdgePlusButton` 命中：进入 `AddingNodeFromEdge` 状态
3. `on_mouse_move` 在该状态下无特殊行为（或追踪鼠标）
4. 新增 `render_node_picker` 方法：在 `AddingNodeFromEdge` 状态下渲染节点类型选择浮层

**Why**：点击 plus button 后需弹出节点选择面板
**How**：

`InteractionState` 新增：
```rust
AddingNodeFromEdge {
    edge_id: rust_agent_flow::EdgeId,
    anchor: PointF,  // 屏幕坐标，浮层定位用
}
```

`on_mouse_down` 分支：
```rust
HitResult::EdgePlusButton(edge_id) => {
    self.interaction = InteractionState::AddingNodeFromEdge {
        edge_id,
        anchor: screen_pos,
    };
    cx.notify();
    return;
}
```

**节点选择浮层**（在 flow_editor.rs 的 render 中渲染）：
- 列出 registry 中所有节点 kind（start/end/action/condition/loop/variable/adapter/agent）
- 每项显示 kind_label_str（i18n）+ 图标
- 点击某项 → 调用 `insert_node_at_edge(edge_id, kind, cx)` → 退出 `AddingNodeFromEdge` 状态

**浮层渲染**（新增方法 `render_node_picker`）：
```rust
fn render_node_picker(&self, cx: &mut Context<Self>) -> Option<impl IntoElement> {
    let InteractionState::AddingNodeFromEdge { edge_id, anchor } = &self.interaction else {
        return None;
    };
    let edge_id = *edge_id;
    let kinds = ["action", "condition", "loop", "variable", "adapter", "agent"];
    let lang = self.language;
    let theme = self.theme;
    
    Some(
        div()
            .absolute()
            .left(px(anchor.x + 10.0))
            .top(px(anchor.y + 10.0))
            .w(px(160.0))
            .bg(theme.panel_bg)
            .border_1()
            .border_color(theme.panel_border)
            .rounded_md()
            .shadow_lg()
            .p_1()
            .flex()
            .flex_col()
            .gap_1()
            .children(kinds.iter().map(|&kind| {
                div()
                    .id(("node-picker-", kind))
                    .px_2()
                    .py_1()
                    .rounded_md()
                    .hover(|s| s.bg(theme.panel_hover_bg))
                    .text_sm()
                    .text_color(theme.panel_text)
                    .on_mouse_down(MouseButton::Left, cx.listener(move |this, _, _, cx| {
                        this.insert_node_at_edge(edge_id, kind, cx);
                        this.interaction = InteractionState::Idle;
                        cx.notify();
                    }))
                    .child(kind_label_str(lang, kind))
            }))
    )
}
```

**退出浮层**：点击浮层外区域 → 回到 Idle（在 `on_mouse_down` 的 Empty 命中分支中，若当前是 `AddingNodeFromEdge` 则退出）

#### 4.4 flow_editor.rs render 集成
**文件**：`crates/gpui/src/editor/flow_editor.rs`
**What**：在 `render()` 中调用 `render_node_picker()`，将浮层加入容器
**How**：
```rust
// 在 toolbar 之后、panel 之前
if let Some(picker) = self.render_node_picker(cx) {
    container = container.child(picker);
}
```

### 阶段 5：编译与运行验证

**验证步骤**：
1. `cargo build -p rust-agent-flow-gpui` — 确认 gpui crate 编译通过
2. `cargo build -p rust-agent-flow-demo` — 确认 demo 编译通过
3. `cargo run -p rust-agent-flow-demo` — 运行验证：
   - 工具栏按钮 hover 显示 tooltip（中英文）
   - 数据源 Dropdown 切换 3 个流程正常加载
   - 边中点显示「+」按钮，点击弹出节点选择浮层
   - 选择节点类型后边被拆分，新节点插入中间
   - 属性面板编辑实时同步节点数据
4. 切换语言验证 tooltip/浮层文案切换
5. 切换主题验证 plus button 颜色适配

## Assumptions & Decisions（假设与决策）

### 决策
1. **边 plus button 用 div 覆盖层而非 canvas 绘制**：div 便于处理 hover 样式和未来扩展，canvas 仅画连线。
2. **plus button 命中优先级高于节点删除/切换按钮**：plus button 在边中点，通常不在节点上，但优先级高避免被节点 body 遮挡。
3. **节点选择浮层用简单 div 列表**：不引入 PopupMenu/ContextMenu 组件（避免额外复杂度），6 种节点类型直接列出。
4. **insert_node_at_edge 复用 schema default_data**：新节点用 schema 默认数据，用户可后续通过属性面板编辑。
5. **边中点用端口中点简化计算**：不做完整路径采样，源出端口和目标入端口的中点足够准确（dagre 布局下边通常水平/垂直）。
6. **工具栏 Button 激活态**：用 `.selected(bool)` 或条件背景色，保持与现有 toggle 视觉一致。
7. **数据源切换时重置视口和选中状态**：避免新图坐标系不一致导致节点飞出视口。

### 假设
1. `gpui_component::button::Button` 支持 `.icon(IconName).tooltip(text).on_click(handler)` 链式调用（来自 v2 计划确认）。
2. `Button.dropdown_menu()` 闭包签名为 `|menu, window, cx| menu.menu(label, action)` 或 `menu.item(...)`（需在实施时验证确切 API）。
3. `EdgeId` 类型已由 core crate 导出（`rust_agent_flow::EdgeId`）。
4. `FlowGraph` 有 `edge(EdgeId) -> Option<&Edge>`、`remove_edge(EdgeId)`、`edges()` 方法（来自现有代码模式）。
5. `NodeRegistry` 有 `get(kind) -> Option<&NodeSchema>` 或类似方法（来自 flow_editor.rs sync_node_sizes 中的 `registry.get(&node.kind)`）。

### 风险与回退
- **DropdownMenu API 不确定**：若 `Button.dropdown_menu` 签名不符，回退为循环切换按钮（保留现有交互模式，仅加 Tooltip）。
- **Button tooltip 不生效**：若 `.tooltip()` 方法不存在于当前 gpui-component 版本，回退为 `title()` 或在按钮旁加文字标签。
- **边中点计算偏差**：若端口中点与实际路径中点偏差大（如回环边），回退为只对 `EdgeRender::Normal` 边显示 plus button，回环边不显示。

## 文件改动清单

| 文件 | 改动类型 | 行数估计 |
|------|---------|---------|
| `crates/gpui/src/editor/mod.rs` | 编辑（+2 行） | 25 |
| `crates/gpui/src/editor/flow_editor.rs` | 编辑（+40 行：字段+方法+render集成） | 435 |
| `crates/gpui/src/editor/toolbar.rs` | 重写 | ~350 |
| `crates/gpui/src/editor/hit_test.rs` | 编辑（+30 行） | 178 |
| `crates/gpui/src/editor/rendering.rs` | 编辑（+40 行） | 484 |
| `crates/gpui/src/editor/interaction.rs` | 编辑（+25 行） | 249 |
| `crates/gpui/src/theme.rs` | 编辑（+6 行：3 个颜色字段×2 套主题） | 236 |
| `crates/gpui/src/lib.rs` | 编辑（+1 行：导出 DataSource，若需要） | - |
| `demo/src/main.rs` | 重写 | ~30 |

## 实施顺序

1. **阶段 2.1**：mod.rs 注册 data_source → 编译确认模块接入
2. **阶段 2.2**：flow_editor.rs 加字段+方法 → 编译确认
3. **阶段 2.3**：toolbar.rs 重写 → 编译确认工具栏
4. **阶段 3**：demo/main.rs 重写 + lib.rs 导出 → 编译确认 demo
5. **阶段 4.1**：hit_test.rs 加 EdgePlusButton → 编译确认
6. **阶段 4.2**：rendering.rs + theme.rs 加 plus button 渲染 → 编译确认
7. **阶段 4.3**：interaction.rs 加状态+处理 → 编译确认
8. **阶段 4.4**：flow_editor.rs render 集成浮层 → 编译确认
9. **阶段 5**：cargo run 运行验证
