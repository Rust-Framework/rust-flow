# SyntaxService 语法高亮扩展

属性面板里的 `CodeEditor`/`CodeBlock` 字段需要语法高亮。rust-agent-flow 的逻辑语言是 rhai，但核心库**不引入** rhai 的 tree-sitter grammar——那是大依赖，且多数项目用不到精确高亮。`SyntaxService` 把「逻辑语言 → 高亮语言」的映射权交给调用方，默认实现用 Rust 近似高亮兜底。

## 为什么不直接绑定 rhai 高亮

考虑两种极端：

| 方案 | 代价 |
|------|------|
| 核心库内置 rhai grammar | 强加 tree-sitter-rhai 大依赖给所有用户 |
| 完全不高亮 | 代码字段体验差 |

`SyntaxService` 选了中间路线：核心库只定义映射接口，默认实现把 `rhai` 映射到 `rust`（语法高度相似：let/fn/if/while/数组/Map），需要精确高亮的项目自行注入。

## 接口与默认实现

```rust
pub trait SyntaxService: Send + Sync {
    /// 返回 code_editor 应使用的语言字符串。
    /// None 表示不支持，调用方回退到普通 multi_line Input。
    fn language_for(&self, kind: &str) -> Option<&str>;
}

pub type SharedSyntaxService = Arc<dyn SyntaxService>;

#[derive(Default, Clone)]
pub struct DefaultSyntaxService;
impl SyntaxService for DefaultSyntaxService {
    fn language_for(&self, kind: &str) -> Option<&str> {
        match kind {
            "rhai" => Some("rust"),   // 近似高亮
            _ => None,
        }
    }
}

pub fn default_syntax_service() -> SharedSyntaxService {
    Arc::new(DefaultSyntaxService)
}
```

接口极简：一个方法，输入逻辑语言标识（如 `"rhai"`、`"javascript"`），输出 gpui-component `code_editor` 认得的语言字符串（如 `"rust"`）。返回 `None` 时调用方回退为普通多行 Input——`new_code_input` 就是这么用的：

```rust
let language = syntax_service.language_for("rhai");
cx.new(|cx| {
    let mut state = InputState::new(window, cx).default_value(text).placeholder(placeholder);
    if let Some(lang) = language {
        state = state.code_editor(lang);          // 有高亮
        if multi_line { state = state.multi_line(true).line_number(true).rows(4); }
        else { state = state.multi_line(false); }
    } else {
        // 无高亮：退化为普通 Input
        if multi_line { state = state.multi_line(true).rows(4); }
    }
    state
})
```

## 注入：替换型 + 销毁重建

`set_syntax_service` 与 `set_data_type_provider` 一样是替换型注入：

```rust
pub fn set_syntax_service(&mut self, service: SharedSyntaxService, cx) {
    self.syntax_service = service;
    self.panel_view = None;   // 销毁现有面板
    cx.notify();
}
```

销毁原因同数据类型：已有面板的 `InputState` 是用旧 `syntax_service` 创建的，`code_editor(lang)` 调用已经固化在 InputState 内部。要切换高亮引擎，只能重建 InputState，而 InputState 是面板的组成部分——所以销毁整个 panel_view，下帧重建。

这是替换型扩展点的铁律：**注入物若被视图在构造时消费并固化，注入即重建视图**。对比累积型的 `add_toolbar_provider`——toolbar provider 在每帧 `render_items` 时动态调用，不固化状态，故无需重建。

## 实战：精确 rhai 高亮

假设你的 crate 已注册了 rhai tree-sitter grammar，实现一个精确高亮服务：

```rust
struct RhaiSyntaxService;
impl SyntaxService for RhaiSyntaxService {
    fn language_for(&self, kind: &str) -> Option<&str> {
        match kind {
            "rhai" => Some("rhai"),      // 假设 grammar 已注册
            "javascript" => Some("javascript"),
            _ => None,
        }
    }
}
```

注入：

```rust
editor.set_syntax_service(Arc::new(RhaiSyntaxService), cx);
```

之后所有 CodeEditor/CodeBlock 字段（Action 节点的 `code`、Loop 节点的 `loop_expr`、List 里的代码列等）都会用 rhai grammar 精确高亮。

## 注入时机与默认值

`FlowEditorView` 初始化时 `syntax_service` 默认是 `default_syntax_service()`。若你的项目从不需要精确高亮，什么都不做即可享受 rhai→rust 近似高亮。只有追求精确体验时才注入自定义服务。

注入时机通常在 `FlowEditorView::new` 之后、首帧前，与 toolbar provider 一起：

```rust
let mut editor = FlowEditorView::new(graph, cx);
editor.set_syntax_service(Arc::new(RhaiSyntaxService), cx);
editor.auto_layout(cx);
```

注意顺序：先注入扩展，再 `auto_layout`，避免重建 panel_view 与布局打架（实际上 `auto_layout` 不触碰 panel_view，但保持「先配置后使用」的习惯更稳妥）。

## 设计权衡：为何是映射而非直接渲染

一个诱人的替代方案是让 `SyntaxService` 直接返回 `AnyElement` 渲染高亮代码。框架没这么做，原因有二：

1. **职责分离**：高亮渲染是 gpui-component `code_editor` 的能力，框架只想复用而非重造
2. **可组合**：`code_editor` 还提供行号、多行、滚动等，框架通过 `InputState` 配置即可获得，不必在 trait 里暴露一堆开关

`SyntaxService` 因此退化成极简的「字符串映射器」，实现成本极低，却能撬动整个 code_editor 能力——这是扩展点设计的典范：**接口最小化，复用最大化**。

## 小结

`SyntaxService` 是个单方法映射 trait，把逻辑语言（rhai）映射到 code_editor 认得的高亮语言；默认实现用 rust 近似高亮兜底，避免引入 grammar 依赖；`set_syntax_service` 是替换型注入，销毁 panel_view 重建以切换 InputState 的 code_editor 配置。接口最小化是它的设计精髓。

下一节：[主题与国际化](theme-i18n.md)
