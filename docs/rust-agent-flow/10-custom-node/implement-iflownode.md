# 实现 IFlowNode

## 最小可工作节点

实现一个自定义节点只需 4 个必须方法 + 一次注册。下面以「HttpCall」节点为例，演示完整流程。

### 第一步：定义结构体与 schema

```rust
use gpui::{div, px, AnyElement, IntoElement, ParentElement, Styled};
use gpui_component::{Icon, Sizable, StyledExt};
use rust_agent_flow::{
    FieldSpec, FieldType, LayoutDirection, Node, NodeSchema, PortDirection, PortId,
    PortSide, PortSpec, SizeF, PointF,
};
use crate::node::{IFlowNode, NodeViewCtx};
use crate::builtin::common::{label_of_localized, node_icon, TITLE_H};

const BODY_H: f32 = 28.0;

pub struct HttpCallNode {
    schema: NodeSchema,
}

impl Default for HttpCallNode {
    fn default() -> Self { Self::new() }
}

impl HttpCallNode {
    pub fn new() -> Self {
        Self {
            schema: NodeSchema::new("http_call", "HTTP Call")
                .with_size(SizeF::new(200.0, TITLE_H + BODY_H))
                .with_port(PortSpec::new("in", PortDirection::In, PortSide::Auto))
                .with_port(PortSpec::new("out", PortDirection::Out, PortSide::Auto))
                .with_field(
                    FieldSpec::new("label", "Label", FieldType::Text)
                        .with_default(serde_json::json!("")),
                )
                .with_field(
                    FieldSpec::new("url", "URL", FieldType::Text)
                        .with_default(serde_json::json!("https://")),
                )
                .with_field(
                    FieldSpec::new("method", "Method",
                        FieldType::Dropdown(vec![
                            DropdownOption::new("GET", "GET"),
                            DropdownOption::new("POST", "POST"),
                        ]))
                        .with_default(serde_json::json!("GET")),
                ),
        }
    }
}
```

schema 定义端口（in/out）、字段（label/url/method）和默认尺寸。FieldType 决定面板渲染控件——Text 单行输入，Dropdown 下拉选择。

### 第二步：实现 IFlowNode

```rust
impl IFlowNode for HttpCallNode {
    fn kind(&self) -> &str { "http_call" }

    fn get_view(&self, node: &Node, ctx: &mut NodeViewCtx) -> AnyElement {
        let s = ctx.scale;
        let w = node.size.w * s;
        let h = (TITLE_H + BODY_H) * s;
        let title_h = TITLE_H * s;
        let body_h = BODY_H * s;
        let t = &ctx.theme;
        let layout = ctx.layout;

        let label = label_of_localized(node, ctx.language);
        let url = node.data.get("url").and_then(|v| v.as_str()).unwrap_or("");
        let method = node.data.get("method").and_then(|v| v.as_str()).unwrap_or("GET");
        let body_text = format!("{} {}", method, url);

        let border_color = if ctx.selected { t.node_border_selected } else { t.node_border };

        let mut container = div().relative().w(px(w)).h(px(h));

        // 标题栏
        container = container.child(
            div().absolute().top_0().left_0().w(px(w)).h(px(title_h))
                .bg(t.node_title_bg).rounded_t_lg().border_1()
                .border_color(border_color).border_b_0()
                .flex().items_center().px(px(12.0 * s)).gap(px(6.0 * s))
                .child(Icon::new(node_icon("http_call")).with_size(px(16.0 * s))
                    .text_color(t.node_title_text))
                .child(div().text_size(px(14.0 * s)).font_semibold()
                    .text_color(t.node_title_text).child(label)),
        );

        // 主体
        container = container.child(
            div().absolute().left_0().top(px(title_h)).w(px(w)).h(px(body_h))
                .bg(t.node_bg).rounded_b_lg().border_1()
                .border_color(border_color).border_t_0()
                .flex().items_center().px(px(12.0 * s))
                .child(div().text_size(px(12.0 * s)).text_color(t.node_subtext)
                    .child(body_text)),
        );

        // 端口（横向：左右中心；纵向：顶/底中心）
        // ... 用 make_port 渲染，参考 Action 节点实现

        container.into_any_element()
    }

    fn get_panel(&self, node: &Node, ctx: &mut NodeViewCtx) -> AnyElement {
        // 用通用 Schema 面板（PanelView 会根据 schema 字段渲染）
        // 也可以返回空 div，让 PanelView 接管
        crate::builtin::common::render_simple_panel(node, ctx.language, &ctx.theme)
    }

    fn schema(&self) -> &NodeSchema { &self.schema }
}
```

### 第三步：注册

```rust
use std::sync::Arc;
use crate::node::NodeRegistry;

// 在 FlowEditorView 构造后注册
let mut view = FlowEditorView::new(window, cx);
view.registry.register(Arc::new(HttpCallNode::new()));
```

注册后，画布上添加 kind=`"http_call"` 的节点即可渲染。

## 必须方法的实现要点

| 方法 | 要点 |
|------|------|
| `kind` | 返回与 schema 一致的字符串，作为注册表键 |
| `get_view` | 用 `ctx.scale` 乘所有尺寸；颜色从 `ctx.theme` 取；selected 切换边框色 |
| `get_panel` | 简单节点用 `render_simple_panel`；复杂面板返回空 div 由 PanelView 接管 |
| `schema` | 返回 `&self.schema`，schema 在 `new()` 时构造一次 |

## 主题与缩放的正确处理

**颜色**：永远从 `ctx.theme` 取，不要硬编码 `rgba(...)`。这样主题切换（亮/暗）自动生效。

**缩放**：所有尺寸乘 `ctx.scale`：

```rust
let w = node.size.w * s;        // 节点宽
let title_h = TITLE_H * s;      // 标题栏高
let font_size = 14.0 * s;       // 字号
let port_size = 6.0 * s;        // 端口圆点
```

`node.size` 是逻辑坐标，渲染时乘 scale 转屏幕坐标。框架的视口变换只负责节点位置（`left = pos.x * s`），节点内部元素由节点实现自己乘 scale。

## label 的本地化回退

```rust
let label = label_of_localized(node, ctx.language);
```

`label_of_localized` 优先用 `node.data["label"]`，为空时回退到 `kind_label(lang, &node.kind)`——i18n 的类型名。这保证用户未自定义 label 时显示本地化文案，切换语言自动同步。

如果自定义节点有专属 i18n key，需在 `i18n` 模块的 `kind_label` 函数里追加映射：

```rust
pub fn kind_label(lang: Language, kind: &str) -> &'static str {
    match kind {
        "http_call" => match lang {
            Language::Zh => "HTTP 调用",
            Language::En => "HTTP Call",
        },
        // ... 其他 kind
        _ => kind,
    }
}
```

## 图标的扩展

`common::node_icon` 是按 kind 字符串匹配的函数：

```rust
pub(crate) fn node_icon(kind: &str) -> NodeIcon {
    match kind {
        "start" => NodeIcon::Builtin(IconName::Play),
        // ...
        "http_call" => NodeIcon::Builtin(IconName::Globe),  // 追加自定义映射
        _ => NodeIcon::Builtin(IconName::Settings),
    }
}
```

`NodeIcon::Builtin` 用 gpui-component 内置图标，`NodeIcon::Custom` 用 assets/ 下的自定义 SVG。优先用内置图标（语义匹配时），无合适图标时用 Custom。

## 错误示范：硬编码颜色

```rust
// 错误：主题切换后颜色不变
.bg(gpui::rgb(0xffffff))

// 正确：从 theme 取
.bg(t.node_bg)
```

硬编码颜色是自定义节点最常见的坑——开发时在亮色主题下看起来正常，切到暗色主题后文字与背景同色。所有颜色必须从 `ctx.theme` 取。

## 错误示范：忘记乘 scale

```rust
// 错误：缩放时节点内部元素不缩放
let font_size = 14.0;

// 正确：乘 scale
let font_size = 14.0 * s;
```

忘记乘 scale 会导致缩放时节点内部元素大小不变——视口缩小时节点变小但文字仍 14px，溢出节点边界；视口放大时文字相对节点变小。

## 小结

实现自定义节点的最小步骤：定义结构体与 schema（端口+字段+尺寸）、实现 4 个必须方法（kind/get_view/get_panel/schema）、注册到 NodeRegistry。关键规范：颜色从 theme 取、尺寸乘 scale、label 用 `label_of_localized` 回退、icon 在 `node_icon` 追加映射。简单节点无需覆写可选方法——默认实现已够用。

下一节：[动态端口与 ports_for_node](dynamic-ports.md)
