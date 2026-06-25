# 主题与国际化

主题（Theme）与国际化（i18n）是两个看似不同的话题，在 rust-agent-flow 里却共享同一套机制：集中定义、随编辑器持有、通过 `NodeViewCtx`/`ToolbarCtx` 传递给渲染层。它们不是传统意义的「扩展点 trait」，但都遵循「能力在框架、呈现归调用方」的精神——框架管配色与文案表，调用方决定何时切换、怎么触发。

## Theme：集中配色

`Theme` 是一个纯数据结构，集中管理编辑器所有颜色：

```rust
#[derive(Clone, Copy, Debug)]
pub struct Theme {
    pub is_dark: bool,
    // 画布
    pub canvas_bg: Rgba,
    pub grid_dot: Rgba,
    // 边
    pub edge_default: Rgba,
    pub edge_loop_back: Rgba,
    // 端口
    pub port_bg: Rgba,
    // 节点（所有节点统一）
    pub node_bg: Rgba, pub node_title_bg: Rgba,
    pub node_border: Rgba, pub node_border_selected: Rgba,
    pub node_text: Rgba, pub node_title_text: Rgba, pub node_subtext: Rgba,
    pub node_in_dot: Rgba, pub node_in_ring: Rgba,
    pub node_out_dot: Rgba, pub node_out_ring: Rgba,
    // 属性面板
    pub panel_bg: Rgba, pub panel_border: Rgba,
    pub panel_title_text: Rgba, pub panel_label_text: Rgba, pub panel_subtext: Rgba,
    // 节点按钮 / 工具栏 / 边「+」按钮 ...
    pub delete_btn_bg: Rgba, pub toggle_btn_bg: Rgba, pub collapse_pill_bg: Rgba,
    pub toolbar_bg: Rgba, pub toolbar_border: Rgba, pub toolbar_accent: Rgba, ...
    pub edge_plus_bg: Rgba, pub edge_plus_hover_bg: Rgba,
}
```

两个要点：

- **所有节点统一颜色**：不为 Start/End/Condition/Loop 定义特殊色，节点类型靠图标和文字区分。这避免了「每加一种节点就要配一套色」的负担。
- **Copy + 按值传递**：`Theme` 是 `Copy`，渲染时直接拷贝进闭包，无需 `Arc`。`NodeViewCtx`/`ToolbarCtx` 持有它的副本。

## 亮暗预设与切换

`Theme::light()` / `Theme::dark()` 提供两套预设，`toggle()` 互换：

```rust
impl Theme {
    pub fn light() -> Self { /* 浅灰底 + indigo 强调 */ }
    pub fn dark() -> Self  { /* 深蓝灰底 + 浅紫强调 */ }
    pub fn toggle(self) -> Self {
        if self.is_dark { Self::light() } else { Self::dark() }
    }
}
```

强调色（节点边框选中、入端口、按钮 accent）在亮色用 `#6366f1`（indigo），暗色用 `#818cf8`（浅紫），保证两种背景下都有足够对比度。

## 切换主题：三重同步

`toggle_theme` 不只是改自己的 `theme` 字段，还要同步三处：

```rust
pub fn toggle_theme(&mut self, cx: &mut Context<Self>) {
    self.theme = self.theme.toggle();
    // 1. 同步 gpui-component 全局主题（Button/DropdownMenu 等组件图标文字跟随）
    let mode = if self.theme.is_dark { ThemeMode::Dark } else { ThemeMode::Light };
    gpui_component::Theme::change(mode, None, cx);
    // 2. 显式通知已存在的 panel_view（它持有 theme 快照）
    if let Some(panel) = &self.panel_view {
        panel.set_theme(self.theme, cx);
    }
    // 3. 刷新窗口 + 通知重绘
    cx.refresh_windows();
    cx.notify();
}
```

为什么要三重同步？

| 同步目标 | 原因 |
|----------|------|
| `self.theme` | 画布、节点、边、工具栏渲染都读它 |
| `gpui_component::Theme` | 框架用的 Button/DropdownMenu 等组件读全局主题，不同步会暗底白字不可见 |
| `panel_view.set_theme` | 面板持有 theme 副本，不通知就用旧色重绘 |

注意 panel_view 这里**不销毁重建**——theme 是纯数据，`set_theme` 直接覆盖字段 + `cx.notify()` 即可。对比 syntax/data-type/language 的销毁重建：那些扩展的产物（InputState 的 code_editor 配置、DataTypeRegistry、i18n 文案）在构造时固化进组件，只能重建；而 theme 只是渲染时读取的参数，热替换无副作用。

## i18n：Language 与 TKey

国际化由 `Language` 枚举 + `TKey` 枚举 + `t()` 函数构成：

```rust
#[derive(Clone, Copy, Default, PartialEq, Eq)]
pub enum Language { #[default] Zh, En }

impl Language {
    pub fn toggle(self) -> Self { match self { Zh => En, En => Zh } }
    pub fn is_zh(self) -> bool { matches!(self, Zh) }
}

pub fn t(lang: Language, key: TKey) -> &'static str {
    match lang { Language::Zh => t_zh(key), Language::En => t_en(key) }
}
```

`TKey` 枚举穷举了框架所有需国际化的文字：

| 分组 | 示例 TKey |
|------|-----------|
| 节点类型标签 | Start/End/Action/Condition/Loop/Variable/DataAdapter/Agent |
| Condition | If/Else |
| Loop 模式 | LoopForEach/LoopForLoop/LoopWhile/LoopParallel |
| 面板 | PanelTitle/PanelKind/PanelNodeName/PanelConditions/PanelAddBranch/PanelLoopMode... |
| Start/End 面板 | PanelParams/PanelVariables/PanelReturns/PanelAddParam... |
| 工具栏 tooltip | TbZoomIn/TbFitView/TbToggleDrag/TbToggleTheme/TbToggleLanguage/TbDataSource |
| 数据源名称 | DataSourceAgentFlow/DataSourceDataPipeline/DataSourceSimpleFlow |
| 内置数据类型 | TypeString/TypeInteger/TypeFloat/TypeBoolean/TypeDateTime/TypeDynamic |

## 关键边界：节点 label 不受 i18n 影响

这是一条容易混淆的规则：

| 文字类型 | 来源 | 是否随语言切换 |
|----------|------|----------------|
| 节点 label（如 "规划"） | `node.data["label"]` | 否，业务数据 |
| 节点类型标签（如 "动作"） | `t(lang, TKey::Action)` | 是 |
| 字段标签（如 "条件分支"） | `field_label` 映射 `t(lang, TKey::PanelConditions)` | 是 |
| 按钮 tooltip | `t(lang, TKey::TbXxx)` | 是 |

节点 label 是用户起的业务名字，存 JSON，换语言不该改它；类型标签/字段标签/tooltip 是框架 UI 文字，必须跟随语言。`kind_label` 和 `field_label` 负责把 kind/field_key 映射到 TKey，再 `t()` 取文案。

## 切换语言：销毁重建

`set_language` 是替换型注入，销毁 panel_view：

```rust
pub fn set_language(&mut self, language: Language, cx) {
    self.language = language;
    self.panel_view = None;   // 销毁现有面板
    cx.notify();
}

pub fn toggle_language(&mut self, cx) {
    self.set_language(self.language.toggle(), cx);
}
```

为什么语言切换要重建面板，而主题不用？因为面板构造时把 `t(lang, TKey::Xxx)` 的结果固化进了多处：字段标签、placeholder、添加按钮文字……这些都是 `String`，存在 div 里，不会随 `language` 字段变化而重渲染。销毁重建确保所有文案重新取值。

对比三类扩展的注入语义：

| 扩展 | 注入类型 | 销毁 panel_view |
|------|----------|-----------------|
| ToolbarProvider | 累积（push） | 否（每帧动态渲染） |
| IDataTypeProvider | 替换 | 是（registry 固化进 ItemState） |
| SyntaxService | 替换 | 是（code_editor 配置固化进 InputState） |
| Language | 替换 | 是（文案固化进 div） |
| Theme | 替换 | 否（纯数据热替换） |

## 辅助映射函数

i18n 模块还提供两个映射函数：

```rust
// kind → 本地化类型标签
pub fn kind_label(lang, kind: &str) -> &'static str { ... }

// 内置数据类型名 → 本地化显示名（provider 类型原样返回）
pub fn data_type_label(lang, type_name: &str) -> &str { ... }
```

`data_type_label` 对内置类型翻译（"String"→"文本"），对 provider 的 Complex 类型原样返回类型名——因为 provider 类型名是业务定义的，框架无从翻译。若需翻译 provider 类型名，可在 provider 侧自行做本地化。

## 小结

`Theme` 集中管理配色，亮暗预设 + `toggle()`，切换时三重同步（self + gpui-component 全局 + panel_view 副本），纯数据热替换无需重建面板。i18n 由 `Language`/`TKey`/`t()` 构成，节点 label 不受影响；语言切换是替换型注入，因文案固化进 div 而销毁重建 panel_view。理解「哪些扩展需重建、哪些不需」的关键在于：注入物是否在构造时被消费固化。

下一章：[项目组织与集成](../16-best-practices/project-structure.md)
