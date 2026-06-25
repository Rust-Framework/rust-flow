# Variable / Adapter / Agent

## 配置型三件套

Variable/Adapter/Agent 三种节点都是「In + Out」双端口、标题栏 + 主体的简单结构——拓扑上与 Action 完全一致。它们的差异在于**主体显示什么**和**面板编辑什么**：分别承载变量定义、数据适配、智能体配置三类配置语义。

| 节点 | kind | 主体高度 | 主体显示 | 面板 |
|------|------|----------|----------|------|
| Variable | `"variable"` | 28 | 变量数量（「N 个变量」/「无变量」） | 由 PanelView 接管 |
| Adapter | `"adapter"` | 28 | desc 或「Data Adapter」 | 由 PanelView 接管 |
| Agent | `"agent"` | 28 | model 或「Agent」 | 由 PanelView 接管 |

三者的 `get_panel` 都返回空 div（实际由 `PanelView` 根据 schema 渲染）——这是 gpui 层面板系统的统一入口，`IFlowNode::get_panel` 在内置节点中只有 Action/Condition/Loop 用 `render_simple_panel`。

## VariableNode：变量定义

```rust
schema: NodeSchema::new("variable", "Variable")
    .with_size(SizeF::new(200.0, TITLE_H + BODY_H))  // 200 × 64
    .with_port(PortSpec::new("in", PortDirection::In, PortSide::Auto))
    .with_port(PortSpec::new("out", PortDirection::Out, PortSide::Auto))
    .with_field(FieldSpec::new("label", "Label", FieldType::Text)
        .with_default(serde_json::json!("Variable")))
    .with_field(FieldSpec::new("variables", "Variables",
        FieldType::List(ListSpec::new(vec![
            FieldSpec::new("name", "Name", FieldType::Text)...,
            FieldSpec::new("type", "Type", FieldType::Text)
                .with_default(serde_json::json!("string"))),
            FieldSpec::new("value", "Value", FieldType::Text)...,
        ]))
        .with_default(serde_json::json!([])))
```

`variables` 是 List 字段，每项三子字段（name/type/value）。主体显示变量数量：

```rust
let body_text = if n_vars > 0 {
    format!("{} {}", n_vars, t(lang, TKey::VariableCount))  // "3 个变量"
} else {
    t(lang, TKey::VariableCount).to_string()  // "无变量"
};
```

Variable 的语义是「在流程中定义/修改变量」——上游传入数据，节点定义新变量供下游使用。`type` 字段默认 `"string"`，支持任意类型字符串（由 `IDataTypeProvider` 扩展）。

## AdapterNode：数据适配

```rust
schema: NodeSchema::new("adapter", "Data Adapter")
    .with_size(SizeF::new(200.0, TITLE_H + BODY_H))
    .with_port(/* in */).with_port(/* out */)
    .with_field(FieldSpec::new("label", "Label", FieldType::Text)...)
    .with_field(FieldSpec::new("desc", "Description", FieldType::Text)...)
```

Adapter 的 schema 极简：只有 label 和 desc。主体显示 desc，为空时回退到 i18n 的「Data Adapter」。

**语义**：数据适配/转换节点——上游数据格式不匹配下游需求时，用 Adapter 做转换。desc 描述转换规则（如「JSON → CSV」「camelCase → snake_case」）。实际转换逻辑由运行时引擎解释，框架只负责编辑期配置。

`desc` 是 Text 字段（单行描述），不是 CodeBlock——适配规则的代码实现属于运行时引擎职责，编辑器只存配置。

## AgentNode：智能体配置

```rust
schema: NodeSchema::new("agent", "Agent")
    .with_size(SizeF::new(200.0, TITLE_H + BODY_H))
    .with_port(/* in */).with_port(/* out */)
    .with_field(FieldSpec::new("label", "Label", FieldType::Text)...)
    .with_field(FieldSpec::new("model", "Model", FieldType::Text)
        .with_default(serde_json::json!("gpt-4")))
    .with_field(FieldSpec::new("prompt", "System Prompt", FieldType::TextArea)...)
```

Agent 的字段：

| 字段 | 类型 | 默认值 | 用途 |
|------|------|--------|------|
| `label` | Text | `""` | 节点显示名 |
| `model` | Text | `"gpt-4"` | 模型名 |
| `prompt` | TextArea | `""` | 系统提示词 |

主体显示 `model`，为空时回退到 i18n 的「Agent」。

**TextArea vs Text**：`prompt` 用 TextArea（多行），`model` 用 Text（单行）——FieldType 决定面板渲染控件类型。TextArea 支持换行，适合长提示词；Text 单行输入，适合短标识符。

## 三者的端口位置算法

三者都覆写 `port_position`，算法与 Action 完全一致：

```rust
fn port_position(&self, node: &Node, port_id: &PortId, layout: LayoutDirection) -> Option<PointF> {
    let left = node.position.x;
    let right = node.position.x + node.size.w;
    let top = node.position.y;
    let mid_x = node.position.x + node.size.w * 0.5;
    let node_mid_y = node.position.y + node.size.h * 0.5;
    let bottom = node.position.y + TITLE_H + BODY_H;

    match port_id.as_str() {
        "in" => match layout {
            LayoutDirection::Horizontal => Some(PointF::new(left, node_mid_y)),
            LayoutDirection::Vertical => Some(PointF::new(mid_x, top)),
        },
        "out" => match layout {
            LayoutDirection::Horizontal => Some(PointF::new(right, node_mid_y)),
            LayoutDirection::Vertical => Some(PointF::new(mid_x, bottom)),
        },
        _ => None,
    }
}
```

| 布局 | In 位置 | Out 位置 |
|------|---------|----------|
| 横向 | 左中心 `(left, node_mid_y)` | 右中心 `(right, node_mid_y)` |
| 纵向 | 顶中心 `(mid_x, top)` | 底中心 `(mid_x, bottom)` |

`node_mid_y` 是节点几何中心，不是标题栏中心——这保证端口在节点垂直方向居中，与实际渲染的端口圆圈位置对齐。

## content_size 的固定值

```rust
fn content_size(&self, node: &Node) -> SizeF {
    SizeF::new(node.size.w, TITLE_H + BODY_H)  // BODY_H = 28
}
```

三者的高度都是固定的 `TITLE_H + BODY_H = 64`——变量数量、desc 长度、model 名都不影响节点高度。主体文案超长时由 GPUI 的文本裁剪处理，不撑高节点。

这与 Condition 的动态高度形成对比：Condition 的高度随条件数变化，Variable 的高度固定——因为 Variable 的变量列表在面板编辑，不在节点主体显示。

## 三者不覆写的方法

| 方法 | 覆写？ | 原因 |
|------|--------|------|
| `ports_for_node` | 否 | 端口静态固定（in + out） |
| `plus_button_at_target` | 否 | 单出口，无按钮聚集问题 |
| `ports_for_node` | 否 | 同上 |

三者都是「静态端口、固定尺寸、单出口」的简单节点——只需覆写 `port_position` 和 `content_size` 显式对齐，其余走默认实现。

## 内置 8 节点覆写总览

| 节点 | ports_for_node | port_position | content_size | plus_button_at_target |
|------|----------------|---------------|--------------|----------------------|
| Start | 否 | 是 | 是（固定） | 否 |
| End | 否 | 是 | 是（固定） | 否 |
| Action | 否 | 是 | 是（固定） | 否 |
| Condition | **是** | 是 | **是（动态）** | 否 |
| Loop | 否 | 是 | 是（固定） | **是** |
| Variable | 否 | 是 | 是（固定） | 否 |
| Adapter | 否 | 是 | 是（固定） | 否 |
| Agent | 否 | 是 | 是（固定） | 否 |

只有 Condition 覆写 `ports_for_node`（动态端口），只有 Loop 覆写 `plus_button_at_target`（按钮避让），只有 Condition 的 `content_size` 是真正动态的。其余 7 种节点的高度都是固定常量——但都显式覆写 `content_size` 以声明渲染高度，避免依赖 `node.size.h` 的隐式正确性。

## 小结

Variable/Adapter/Agent 是配置型三件套，拓扑与 Action 一致（In+Out），差异在主体显示与面板字段。Variable 用 List 字段编辑变量表，Adapter 用 desc 描述转换规则，Agent 用 model+prompt 配置智能体。三者都覆写 `port_position` 和 `content_size`（固定值），不覆写 `ports_for_node`/`plus_button_at_target`——简单节点的标准模式。本章 8 种内置节点的覆写模式总览表是设计自定义节点时的决策参考。

下一节：[实现 IFlowNode](../10-custom-node/implement-iflownode.md)
