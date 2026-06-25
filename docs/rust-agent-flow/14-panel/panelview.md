# PanelView 面板实体

## 面板的诞生与销毁

属性面板不是常驻组件。它随选中节点**创建**、随取消选中或切换扩展**销毁**。`FlowEditorView` 用一个 `panel_view: Option<PanelEntity>` 持有它：

```
用户点击节点 ──→ ensure_panel_view ──→ PanelView::new(...) ──→ panel_view = Some(...)
用户点空白   ──→ panel_view = None
注入新扩展   ──→ panel_view = None（下次 render 重建）
```

不选中时不占资源——这是属性面板的核心生命周期特征。

## PanelEntity：按 kind 分发

并非所有节点都走通用面板。`PanelEntity` 是一个二选一枚举，按节点 `kind` 决定使用哪种面板实体：

```rust
#[derive(Clone)]
pub enum PanelEntity {
    Generic(Entity<PanelView>),            // schema 驱动通用面板
    Start(Entity<start::StartPanelView>),  // Start 节点专属
}
```

分发规则简单：`kind == "start"` 走 `Start`，其余全部走 `Generic`。`PanelEntity` 提供统一接口 `render_element` / `set_theme`，让 `FlowEditorView` 不必关心具体类型：

```rust
pub fn render_element(self, window: &mut Window, cx: &mut App) -> gpui::AnyElement {
    match self {
        PanelEntity::Generic(entity) => entity.update(cx, |v, cx| v.render(window, cx).into_any_element()),
        PanelEntity::Start(entity)   => entity.update(cx, |v, cx| v.render(window, cx).into_any_element()),
    }
}
```

这种「枚举 + 统一接口」是 rust-agent-flow 处理「少数特例 + 多数通用」的典型手法，避免了为特例引入 trait object 的开销。

## PanelView 结构

`PanelView` 是一个有状态的 GPUI 实体，实现 `Render` trait：

```rust
pub struct PanelView {
    pub node: Node,                              // 当前编辑的节点快照
    pub flow_node: Option<Arc<dyn IFlowNode>>,   // 节点逻辑（提供 schema）
    pub theme: Theme,
    pub on_action: Option<ActionCallback>,       // 回写到编辑器的通道
    pub syntax_service: SharedSyntaxService,     // 代码字段高亮
    pub language: Language,

    label_input: Entity<InputState>,             // 节点名称（label 字段单独处理）
    field_states: Vec<FieldState>,               // 其余字段状态，与 schema.fields 对齐
    syncing: bool,                               // 同步标记，避免回环
    scroll_handle: ScrollHandle,                 // 内容区滚动
    _subscriptions: Vec<Subscription>,
}
```

几个关键字段的含义：

| 字段 | 作用 |
|------|------|
| `node` | 节点数据快照，`sync_from_node` 据此判断是否变化 |
| `flow_node` | 提供 `schema()`，决定渲染哪些字段、什么类型 |
| `on_action` | 闭包通道，字段变化时发 `NodeAction::SetData` 回编辑器 |
| `label_input` | label 字段单独一个 InputState，不进 `field_states` |
| `field_states` | 与 `schema.fields` **一一对应**（label 位置用占位） |
| `syncing` | 同步期间置 true，拦截 InputEvent::Change 防止回环 |

## build：schema 即蓝图

构造 `PanelView` 时，`build` 按 `flow_node.schema().fields` 逐个构建 `FieldState`：

```rust
fn build(...) -> Self {
    // 1. label 字段单独建 InputState
    let label_input = cx.new(|cx| InputState::new(...).default_value(label.as_str()));
    let sub_label = cx.subscribe_in(&label_input, window, Self::on_label_change);

    let mut field_states: Vec<FieldState> = Vec::new();
    let mut subscriptions = vec![sub_label];

    if let Some(ref fn_) = flow_node {
        for (idx, field) in fn_.schema().fields.iter().enumerate() {
            if field.key == "label" {
                field_states.push(FieldState::Switch(false)); // 占位，不渲染
                continue;
            }
            let default_value = node.data.get(&field.key)
                .cloned().unwrap_or_else(|| field.default.clone());
            let state = Self::build_field_state(idx, field, &default_value, ...);
            field_states.push(state);
        }
    }
    ...
}
```

注意 `label` 字段被**跳过**——它由 `label_input` 专门管理。为了保持 `field_states` 与 `schema.fields` 索引对齐（便于后续按 `field_idx` 反查 schema），label 位置塞入一个不会被渲染的占位 `FieldState::Switch(false)`。这种「索引对齐」是面板多处逻辑的前提。

## render：schema 驱动分发

`render_schema_panel` 遍历 `schema.fields`，跳过 label，按 `FieldType` 分发到不同渲染函数：

```rust
fn render_field(&mut self, field_idx, field, label, theme, cx) -> AnyElement {
    match &field.field_type {
        FieldType::Text | FieldType::Number        => self.render_input_field(field_idx, label, None, theme),
        FieldType::TextArea                         => self.render_input_field(field_idx, label, Some(px(80.0)), theme),
        FieldType::CodeEditor                       => self.render_input_field(field_idx, label, None, theme),
        FieldType::CodeBlock                        => self.render_input_field(field_idx, label, Some(px(120.0)), theme),
        FieldType::Switch                           => self.render_switch_field(field_idx, label, theme, cx),
        FieldType::Dropdown(options)                => self.render_dropdown_field(field_idx, label, options, theme, cx),
        FieldType::List(list_spec)                  => self.render_list_field(field_idx, label, list_spec, theme, cx),
    }
}
```

`field_label` 负责把 `(kind, field_key)` 映射到本地化文案（如 `("condition","conditions")` → `PanelConditions`），找不到则回退到 schema 中的 `field.label`。

整个面板布局是定宽 300px 的纵向滚动容器：

```
┌──────── 300px ────────┐
│ 头部：图标 + 类型标签  │
├───────────────────────┤
│ 节点名称 [Input]       │  ← label_input
│ ───────────────       │
│ 字段1 标签             │
│ [控件]                 │
│ 字段2 标签             │
│ [控件]                 │
│ ...（可滚动）          │
└───────────────────────┘
```

## 为什么是 schema 驱动

对比两种方案：

| 方案 | 新增节点工作量 | 维护成本 |
|------|----------------|----------|
| per-kind 面板 | 写一整个面板文件 | 每种节点独立维护 |
| schema 驱动 | 只声明 `NodeSchema.fields` | 面板代码集中维护 |

schema 驱动的代价是字段控件类型受限（Text/Number/TextArea/CodeEditor/CodeBlock/Switch/Dropdown/List）。但实践证明这 8 种已覆盖绝大多数配置场景，极少数需要复杂交互的节点（如 Start 的参数/变量树）则走 `PanelEntity` 的特例通道。

## 小结

`PanelEntity` 用枚举区分通用与特例；`PanelView` 持有节点快照、schema 来源、回写通道与字段状态。`build` 按 schema 构建状态、订阅变化，`render` 按 `FieldType` 分发控件——新增节点只需声明 schema，面板自动生成。

下一节：[FieldState 与同步机制](fieldstate-sync.md)
