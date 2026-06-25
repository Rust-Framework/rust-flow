# ToolbarProvider 工具栏扩展

工具栏是编辑器顶部的一排按钮。rust-agent-flow 内置了一组（缩放、布局、边类型、点阵等），但「数据源切换」「拖拽开关」「主题/语言切换」这类**与具体业务相关**的控件，核心库既不该也不便硬编码。`ToolbarProvider` 就是把这些决定权交还给调用方的扩展点。

## 扩展点四件套

rust-agent-flow 所有扩展点遵循同一形态：

```rust
// 1. trait 定义能力
pub trait ToolbarProvider: Send + Sync {
    fn render_items(&self, ctx: &ToolbarCtx) -> Vec<AnyElement>;
}
// 2. Arc<dyn Trait> 别名，便于跨线程共享
pub type SharedToolbarProvider = Arc<dyn ToolbarProvider>;
// 3. setter 注入到 FlowEditorView
editor.add_toolbar_provider(provider, cx);
// 4. 渲染时按需调用
```

记住这个模板，本章其余三个扩展点（IDataTypeProvider / SyntaxService / Theme）都是它的变体。

## ToolbarCtx：渲染上下文

`render_items` 收到一个 `ToolbarCtx`，里面是构建工具项所需的全部信息：

```rust
pub struct ToolbarCtx {
    pub entity: Entity<FlowEditorView>,  // 编辑器句柄，回调中 update 它
    pub theme: Theme,                     // 当前主题颜色
    pub language: Language,               // 当前语言（决定 tooltip 文案）
    pub drag_enabled: bool,               // 拖拽开关状态（决定按钮 selected 态）
}
```

`entity` 是关键：provider 在按钮 `on_click` 回调里 `entity.update(cx, |this, cx| { ... })` 调用编辑器方法（`set_graph` / `toggle_drag` / `toggle_theme`...）。框架不规定你调什么，只把句柄递给你。

## 注入与渲染

`add_toolbar_provider` 把 provider push 进 `custom_toolbar: Vec<SharedToolbarProvider>`：

```rust
pub fn add_toolbar_provider(&mut self, provider: SharedToolbarProvider, cx) {
    self.custom_toolbar.push(provider);
    cx.notify();
}
```

**累积注入**：多次调用可注入多个 provider，按注入顺序渲染。工具栏渲染时，内置项结束后画一条竖线分隔符，再依次调用各 provider 的 `render_items`：

```
[内置: 放大 缩小 适应 布局 边类型 点阵] │ [provider1 项...] │ [provider2 项...]
                                          ▲ 竖线分隔
```

注意：每个 provider 的元素之间不画分隔符，provider 之间也不画——分隔符仅出现在「内置 ↔ 自定义」边界。

## 实战：数据源选择器

Demo 的 `DataSourceToolbar` 是典型用法。它持有一份当前数据源状态，切换时调用 `set_graph` 重建图：

```rust
pub struct DataSourceToolbar {
    current: Arc<Mutex<DemoDataSource>>,
}

impl ToolbarProvider for DataSourceToolbar {
    fn render_items(&self, ctx: &ToolbarCtx) -> Vec<AnyElement> {
        let lang = ctx.language;
        let current = *self.current.lock().unwrap();
        let entity = ctx.entity.clone();
        let current_state = self.current.clone();

        let btn = Button::new("demo-data-source")
            .icon(IconName::ALargeSmall)
            .small().ghost()
            .tooltip(t(lang, TKey::TbDataSource))
            .dropdown_menu(move |menu, _w, _cx| {
                let mut menu = menu;
                for &ds in DemoDataSource::all() {
                    let label = ds.label(lang);
                    let entity = entity.clone();
                    let current_state = current_state.clone();
                    menu = menu.item(
                        PopupMenuItem::new(label)
                            .checked(ds == current)
                            .on_click(move |_, _, cx| {
                                *current_state.lock().unwrap() = ds;
                                let graph = ds.to_graph();
                                entity.update(cx, |this, cx| this.set_graph(graph, cx));
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

要点：

- `current` 用 `Arc<Mutex<...>>` 是因为闭包被 `move` 进 `DropdownMenu`，需在多份闭包间共享可变状态
- 切换时先更新自身 `current`，再 `set_graph` 重建图——保证下次渲染时 `checked` 正确
- `entity.clone()` 给每个菜单项一份句柄，回调里 `entity.update` 改编辑器

## 实战：应用控件工具栏

`AppControlsToolbar` 展示「框架提供能力方法、调用方决定 UI 呈现」的分工：

```rust
impl ToolbarProvider for AppControlsToolbar {
    fn render_items(&self, ctx: &ToolbarCtx) -> Vec<AnyElement> {
        let lang = ctx.language;
        // 拖拽开关：调 toggle_drag
        let drag_btn = Button::new("app-toggle-drag")
            .icon(FlowIcon::Drag).small().ghost()
            .selected(ctx.drag_enabled)
            .tooltip(t(lang, TKey::TbToggleDrag))
            .on_click(move |_, _, cx| {
                drag_entity.update(cx, |this, cx| this.toggle_drag(cx));
            });
        // 主题切换：调 toggle_theme，图标随当前主题变
        let theme_btn = Button::new("app-toggle-theme")
            .icon(if ctx.theme.is_dark { IconName::Sun } else { IconName::Moon })
            .on_click(move |_, _, cx| {
                theme_entity.update(cx, |this, cx| this.toggle_theme(cx));
            });
        // 语言切换：调 toggle_language
        let lang_btn = Button::new("app-toggle-language")
            .icon(IconName::Globe)
            .on_click(move |_, _, cx| {
                lang_entity.update(cx, |this, cx| this.toggle_language(cx));
            });
        vec![drag_btn, theme_btn, lang_btn]
    }
}
```

框架只提供 `toggle_drag` / `toggle_theme` / `toggle_language` 三个能力方法；按钮放哪、用什么图标、是否显示文字，全由调用方在 provider 里决定。这就是「能力在框架、呈现归调用方」的边界。

## 注入时机

通常在 `FlowEditorView::new` 后、首帧渲染前注入：

```rust
let view = cx.new(|cx| {
    let mut editor = FlowEditorView::new(graph, cx);
    editor.auto_layout(cx);
    editor.add_toolbar_provider(Arc::new(DataSourceToolbar::new(initial_ds)), cx);
    editor.add_toolbar_provider(Arc::new(AppControlsToolbar::new()), cx);
    editor
});
```

注入后 `cx.notify()` 触发重绘，工具栏即带上自定义项。

## 小结

`ToolbarProvider` 是扩展点四件套的范本：trait 定义 `render_items`、`SharedToolbarProvider` 别名共享、`add_toolbar_provider` 累积注入、`ToolbarCtx` 递送编辑器句柄与上下文。Demo 的数据源选择器与应用控件工具栏展示了「能力在框架、呈现归调用方」的分工。

下一节：[IDataTypeProvider 数据类型扩展](data-type-provider.md)
