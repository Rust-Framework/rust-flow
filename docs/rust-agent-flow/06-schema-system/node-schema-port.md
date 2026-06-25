# NodeSchema 与 PortSpec

`NodeSchema` 是节点的声明式元数据：它告诉框架「这种 kind 的节点有哪些端口、默认多大、有哪些字段」。`IFlowNode` 实现通过 `schema()` 方法返回它，`NodeRegistry` 按 `kind` 索引。本节聚焦 schema 主体与端口规格，字段系统见下一节。

## NodeSchema 结构

```rust
pub struct NodeSchema {
    pub kind: NodeKind,        // 匹配 IFlowNode::kind
    pub label: String,         // 显示标签（gpui 层可 i18n 覆盖）
    pub ports: Vec<PortSpec>,  // 端口规格
    pub default_size: SizeF,   // 默认尺寸
    pub fields: Vec<FieldSpec>,// 字段定义（驱动属性面板）
}
```

| 字段 | 用途 |
|------|------|
| `kind` | 与 `Node.kind`、`IFlowNode::kind` 三方匹配，是策略分发的键 |
| `label` | 工具栏/树形面板的默认显示文案，gpui 层可按 `(kind, key)` 做 i18n 覆盖 |
| `ports` | 声明端口，决定边端点计算与渲染 |
| `default_size` | 创建节点时的初始尺寸（未指定时 180×64） |
| `fields` | 字段定义，驱动属性面板自动生成（详见下一节） |

## 链式构建器

`NodeSchema` 用链式 builder 构造，避免长结构体字面量：

```rust
impl NodeSchema {
    pub fn new(kind, label) -> Self { /* ports/fields 空, default_size 180×64 */ }
    pub fn with_port(mut self, port: PortSpec) -> Self { self.ports.push(port); self }
    pub fn with_size(mut self, size: SizeF) -> Self { self.default_size = size; self }
    pub fn with_field(mut self, field: FieldSpec) -> Self { self.fields.push(field); self }
}
```

一个 Action 节点的完整声明：

```rust
let schema = NodeSchema::new("action", "Action")
    .with_port(PortSpec::new("in",  PortDirection::In,  PortSide::Auto))
    .with_port(PortSpec::new("out", PortDirection::Out, PortSide::Auto))
    .with_field(FieldSpec::new("label", "Label", FieldType::Text)
        .with_default(json!("Action")))
    .with_field(FieldSpec::new("code", "Code", FieldType::CodeBlock)
        .with_default(json!("")));
```

## PortSpec：端口规格

```rust
pub struct PortSpec {
    pub id: PortId,                // String，节点内唯一
    pub direction: PortDirection,  // In / Out
    pub side: PortSide,            // Top/Right/Bottom/Left/Auto
    pub label: Option<String>,     // 端口标签（可选）
}
```

构建器同样链式：

```rust
impl PortSpec {
    pub fn new(id, direction, side) -> Self { /* label = None */ }
    pub fn with_label(mut self, label) -> Self { self.label = Some(label); self }
}
```

### side 的两种策略

| side 值 | 行为 | 适用节点 |
|---------|------|----------|
| `Auto`（默认） | 框架根据相连节点相对位置推导 | Start/Action/End 等普通节点 |
| 固定（Top/Right/Bottom/Left） | 始终在该侧 | Condition/Loop 等结构化节点 |

结构化节点需要固定 side 以保证语义清晰：

```rust
// Condition：if_N 出口固定在右侧不同高度
PortSpec::new("if_0", PortDirection::Out, PortSide::Right).with_label("分支1")
PortSpec::new("else", PortDirection::Out, PortSide::Right).with_label("Else")

// Loop：done 出口固定右侧，loop_body 出口固定右侧
PortSpec::new("done",      PortDirection::Out, PortSide::Right)
PortSpec::new("loop_body", PortDirection::Out, PortSide::Right)
```

固定 side 让 `resolve_endpoints`（见 [端口端点计算](../07-geometry-layout/port-calc.md)）跳过位置推导，直接按声明分配——这对结构化节点的可读性至关重要。

## ports_by_direction：按方向过滤

```rust
pub fn ports_by_direction(&self, dir: PortDirection) -> impl Iterator<Item = &PortSpec> {
    self.ports.iter().filter(move |p| p.direction == dir)
}
```

渲染层用它分别绘制入边端口与出边端口，端口分布算法也按方向分组收集边（见 `distribute_on_side`）。

## default_data：从 fields 推导初始数据

```rust
pub fn default_data(&self) -> serde_json::Value {
    let mut obj = serde_json::Map::new();
    for field in &self.fields {
        obj.insert(field.key.clone(), field.default.clone());
    }
    serde_json::Value::Object(obj)
}
```

创建新节点时，框架调用 `schema.default_data()` 生成 `Node.data`，保证每个字段都有合法初值，属性面板不会读到 `undefined`。这是 Schema 驱动的关键闭环：

```
NodeSchema.fields ──default_data()──> Node.data
     ↑                                       ↓
     └────── 属性面板读取字段渲染 ◄──────────┘
```

`default_data` 只填 `fields` 声明过的键，**不会**清掉 `data` 里多出的字段——这允许老数据携带新版本 schema 未识别的字段（向前兼容）。

## default_size 的使用时机

| 场景 | size 来源 |
|------|-----------|
| `FlowDocument.from_document` 中 `NodeDef.size = None` | `schema.default_size`（实际由 gpui 层 `sync_node_sizes` 修正） |
| 工具栏拖入新节点 | `schema.default_size` |
| 结构化节点（Condition/Loop） | `IFlowNode` 实现里覆盖 `with_size` 给出准确尺寸 |

`default_size` 默认 `180×64`，结构化节点通常更大（如 Loop 的 `180×120` 以容纳标题栏与多个端口）。

## 内置节点 kind 全景

内置 8 种节点构成图灵完备控制流：

| kind | 语义 | 端口特征 |
|------|------|----------|
| `start` | 流程起点 | 仅 Out |
| `end` | 流程终点 | 仅 In |
| `action` | 步骤 | In + Out |
| `condition` | 条件分支 | In + 多个 if_N/else Out |
| `loop` | 循环迭代 | In + done/loop_body Out + loop_in In |
| `variable` | 变量声明 | In + Out |
| `adapter` | 数据适配 | In + Out |
| `agent` | 智能体 | In + Out |

它们的 schema 在 `crates/gpui/src/builtin/` 各自模块中声明，注册到 `NodeRegistry`。自定义节点只需声明新 `kind` 的 schema 并注册，无需修改框架。

## 小结

`NodeSchema` 用链式 builder 声明节点的端口、尺寸与字段。`PortSpec.side` 的 Auto/固定两种策略分别服务普通节点与结构化节点。`default_data()` 把 `fields` 的 default 值灌入 `Node.data`，闭合 Schema 驱动的属性面板循环。`kind` 是贯穿 schema、`IFlowNode`、`Node` 三方的策略分发键。

下一节：[FieldSpec 与字段类型](fieldspec-types.md)
