# GPUI 惯用法与所有权

## 为什么用命中测试而非 per-element 闭包

GPUI 的 `listener` 闭包（如 `cx.listener(|this, event, window, cx| {...})`）**无法捕获外部变量**如 `node_id`。这意味着不能这样写：

```rust
// ❌ 不可行：闭包无法捕获 node_id
for node in nodes {
    div().on_click(cx.listener(move |this, _, _, cx| {
        this.select_node(node_id, cx); // 编译错误：闭包捕获了 node_id
    }))
}
```

rust-agent-flow 的解法是**命中测试（hit-test）**方案：画布统一处理鼠标事件，用几何计算确定点击目标：

```rust
fn on_mouse_down(&mut self, event: &MouseDownEvent, _, cx: &mut Context<Self>) {
    let logical = self.to_logical(event.position);
    match self.hit_test(logical) {
        HitResult::Node(node_id) => { /* 选中 */ }
        HitResult::OutPort(node_id, port) => { /* 开始连线 */ }
        HitResult::Empty => { /* 取消选中 */ }
        // ...
    }
}
```

`hit_test` 逐层判断：删除按钮 → 切换按钮 → 边「+」按钮 → 端口 → 节点矩形 → 空白。

## Entity 与 Context 模型

GPUI 采用 **Entity + Context** 所有权模型：

```rust
// 创建实体（返回 Entity<T>，即 Arc 语义的共享句柄）
let view = cx.new(|cx| FlowEditorView::new(graph, cx));

// 更新实体（闭包内获得 &mut T）
entity.update(cx, |this, cx| {
    this.do_something(cx);
});
```

框架内的关键实践：

- `FlowEditorView` 实现 `Render` trait，是主实体
- `PanelView` 是独立的 `Entity<PanelView>`，作为子实体
- `FlowEditorView.panel_view: Option<PanelEntity>` 持有面板实体句柄

## Entity 在扩展点中的传递

`ToolbarProvider::render_items` 通过 `ToolbarCtx.entity` 传递编辑器句柄，让扩展能在回调中更新编辑器：

```rust
impl ToolbarProvider for MyToolbar {
    fn render_items(&self, ctx: &ToolbarCtx) -> Vec<AnyElement> {
        let entity = ctx.entity.clone();
        vec![
            Button::new("my-btn")
                .on_click(move |_, _, cx| {
                    entity.update(cx, |this, cx| {
                        this.toggle_theme(cx); // 通过句柄调用编辑器方法
                    });
                })
                .into_any_element()
        ]
    }
}
```

这是「能力与 UI 分离」的实现基础——框架提供方法，扩展通过句柄调用。

## 借用冲突与解决

GPUI 的 `Render::render` 签名是 `&mut self`，容易与内部借用冲突。框架的典型处理模式：

### 模式一：先取可变借用，释放后再取不可变借用

```rust
fn render(&mut self, window, cx) -> impl IntoElement {
    // 先调用 &mut self 方法（如 ensure_panel_view）
    let panel = self.ensure_panel_view(entity, window, cx);

    // 借用释放后，再引用不可变缓存
    let body_groups = &self.cached_body_groups;
    let edges = self.render_edges(body_groups);
    // ...
}
```

### 模式二：克隆数据避免持有借用

```rust
// 克隆 fields（Vec<FieldSpec>）而非持有 flow_node 借用
let fields: Vec<FieldSpec> = fn_.schema().fields.clone();
for (i, field) in fields.iter().enumerate() {
    col = col.child(self.render_field(i, field, ...)); // &mut self 调用
}
```

### 模式三：ActionCallback 闭包捕获 node_id

节点视图/面板通过 `ActionCallback`（`Arc<dyn Fn(NodeAction, &mut App)>`）向编辑器发动作，闭包在创建时捕获 `node_id`：

```rust
let on_action: ActionCallback = Arc::new({
    let node_id = node.id;
    move |action: NodeAction, cx: &mut App| {
        entity.update(cx, |this, cx| {
            this.handle_node_action(node_id, action, cx);
        });
    }
});
```

## notify 与刷新

GPUI 的 `cx.notify()` 标记实体为脏，触发下次 `render`。框架在状态变更后必须调用：

```rust
pub fn set_drag_enabled(&mut self, enabled: bool, cx: &mut Context<Self>) {
    self.drag_enabled = enabled;
    cx.notify(); // 不调用则界面不更新
}
```

## 主题同步的特殊处理

切换主题时，`FlowEditorView` 持有的 `panel_view` 内部有 `theme` 快照，必须**显式通知**：

```rust
pub fn toggle_theme(&mut self, cx: &mut Context<Self>) {
    self.theme = self.theme.toggle();
    gpui_component::Theme::change(mode, None, cx);
    if let Some(panel) = &self.panel_view {
        panel.set_theme(self.theme, cx); // 显式同步到面板
    }
    cx.refresh_windows();
    cx.notify();
}
```

`set_data_type_provider` / `set_language` / `set_syntax_service` 注入新扩展时，会销毁现有 `panel_view`（置 None），下次 render 时用新配置重建——避免面板持有过期引用。

## 小结

GPUI 的闭包约束决定了命中测试方案；Entity + Context 模型决定了扩展点通过句柄传递；借用冲突需要「先可变后不可变」或克隆数据的处理模式。理解这些惯用法，才能在自定义节点与扩展中避免借用错误。

下一节：[渐进式披露与框架边界](progressive-disclosure.md)
