# FieldSpec 与字段类型

`FieldSpec` 描述 `node.data` 中的一个字段，`FieldType` 决定属性面板渲染哪种编辑控件。这是「Schema 驱动」最直接的体现：声明字段即获得编辑界面，无需为每种节点写专属面板代码。

## FieldSpec 结构

```rust
pub struct FieldSpec {
    pub key: String,           // node.data 中的键名
    pub label: String,         // 默认标签（gpui 层可 i18n 覆盖）
    pub field_type: FieldType, // 字段类型 → 控件
    pub default: serde_json::Value, // 创建节点时填入 node.data
    pub placeholder: Option<String>, // 占位符（gpui 层可 i18n 覆盖）
}
```

链式构建器：

```rust
impl FieldSpec {
    pub fn new(key, label, field_type) -> Self { /* default=Null, placeholder=None */ }
    pub fn with_default(mut self, default) -> Self { ... }
    pub fn with_placeholder(mut self, placeholder) -> Self { ... }
}
```

`label` 与 `placeholder` 在 core 层只存描述性默认，gpui 层按 `(kind, key)` 查 i18n 表覆盖——这保证 core 不依赖任何语言资源。

## FieldType 与控件映射

```rust
pub enum FieldType {
    Text,
    TextArea,
    CodeEditor,
    CodeBlock,
    Number,
    Switch,
    Dropdown(Vec<DropdownOption>),
    List(ListSpec),
}
```

| FieldType | 控件 | 数据类型 | 说明 |
|-----------|------|----------|------|
| `Text` | 单行 Input | String | 普通文本 |
| `TextArea` | 多行 Input（rows=4） | String | 长文本 |
| `CodeEditor` | 单行代码编辑器 | String | 表达式，无行号、禁止换行 |
| `CodeBlock` | 多行代码编辑器 | String | 带行号、自动缩进 |
| `Number` | 数字 Input | Number | 数值输入 |
| `Switch` | 布尔开关 | Bool | 开关 |
| `Dropdown(Vec)` | 下拉选择 | String | 枚举，值取自 `DropdownOption.value` |
| `List(ListSpec)` | 动态列表 | Array | 可增删条目，每条目是字段集合 |

`Dropdown` 与 `List` 是带参变体，分别携带选项列表与条目字段规格，因此能表达丰富的结构。

## DropdownOption：下拉选项

```rust
pub struct DropdownOption {
    pub value: String, // 存储到 node.data 的值
    pub label: String, // 显示标签（gpui 层 i18n 映射）
}
```

```rust
let field = FieldSpec::new("mode", "Mode", FieldType::Dropdown(vec![
    DropdownOption::new("sync",  "同步"),
    DropdownOption::new("async", "异步"),
])).with_default(json!("sync"));
```

选中后 `node.data["mode"]` 存的是 `value`（`"sync"`），而非显示 label——这保证数据语义稳定，i18n 切换不影响存储。

## ListSpec：动态列表

`List` 是最复杂的字段类型，表达「可增删的条目数组，每个条目有固定字段结构」：

```rust
pub struct ListSpec {
    pub item_fields: Vec<FieldSpec>, // 每个条目的字段定义
    pub min_items: usize,            // 最小条数（如 Condition 的 Else 兜底）
}
```

`item_fields` 本身是 `Vec<FieldSpec>`，因此 List 可以**递归嵌套**（List 的条目里再有 List），不过实践中多数情况是单层。

### Condition 分支列表示例

Condition 节点的条件分支用 List 表达：

```rust
let conditions_list = ListSpec::new(vec![
    FieldSpec::new("name", "条件名", FieldType::Text)
        .with_default(json!("")),
    FieldSpec::new("expr", "表达式", FieldType::CodeEditor)
        .with_default(json!("")),
    FieldSpec::new("desc", "描述",   FieldType::Text)
        .with_placeholder("可选")
        .with_default(json!("")),
]).with_min_items(1); // 至少一个分支

let schema = NodeSchema::new("condition", "Condition")
    .with_field(FieldSpec::new("conditions", "条件列表",
        FieldType::List(conditions_list))
        .with_default(json!([])));
```

对应 `node.data`：

```json
{
  "conditions": [
    { "name": "分支1", "expr": "x > 0", "desc": "" },
    { "name": "else",  "expr": "",      "desc": "" }
  ]
}
```

`min_items` 由属性面板强制——删除到 `min_items` 时禁用删除按钮，保证 Else 兜底分支不被清空。

### 动态端口联动

Condition 的端口数量随 `conditions` 列表长度变化：每个条目对应一个 `if_N` 出口。`IFlowNode::ports_for_node` 读取 `node.data.conditions` 动态生成端口列表（详见 [动态端口](../10-custom-node/dynamic-ports.md)）。这是 List 字段驱动节点拓扑的典型场景。

## default_data 的填充

`NodeSchema::default_data()` 遍历所有 `fields`，把每个 `default` 灌入 `node.data`：

```rust
pub fn default_data(&self) -> serde_json::Value {
    let mut obj = serde_json::Map::new();
    for field in &self.fields {
        obj.insert(field.key.clone(), field.default.clone());
    }
    serde_json::Value::Object(obj)
}
```

各字段类型的典型 default：

| FieldType | 推荐 default |
|-----------|--------------|
| Text/TextArea/CodeEditor/CodeBlock | `json!("")` |
| Number | `json!(0)` |
| Switch | `json!(false)` |
| Dropdown | 选项之一的 `value`，如 `json!("sync")` |
| List | `json!([])`（属性面板按 `min_items` 补齐条目） |

> 注意：`default_data` 只插入顶层字段键，不会递归填充 List 条目的 `item_fields` 默认值——条目默认值由属性面板在新增条目时按 `item_fields` 生成。

## 字段 i18n 策略

core 层的 `label`/`placeholder` 是兜底文案，gpui 层维护一张 `(kind, key) → i18n 文案` 映射表：

```
查找顺序：i18n 表[(kind, key)] → field.label（core 默认）
```

这意味着：

- core 不含任何中文/英文字符串资源，纯结构定义
- 应用侧可自由替换文案，无需改 core
- 同一 schema 可服务于不同语言的 UI

## 字段与面板的解耦

| 关注点 | 归属 |
|--------|------|
| 字段结构（key/type/default） | `FieldSpec`（core） |
| 控件渲染（Input/Switch/...） | `PanelView`（gpui） |
| 文案 | i18n 表（gpui/应用） |
| 数据存储 | `node.data`（core） |

`PanelView` 是**唯一**的通用面板，按 `schema.fields` 顺序渲染对应控件，所有节点共用。新增节点只需声明字段，面板自动跟上——这是消除 per-kind 分发的关键。

## 小结

`FieldSpec` + `FieldType` 把字段结构声明与控件渲染解耦：core 只描述「有什么字段、什么类型、默认值是什么」，gpui 层据此自动生成编辑界面。`Dropdown`/`List` 携带结构化参数，`List` 通过 `item_fields` 实现可递归的条目结构。`default_data` 闭合了从声明到初始数据的循环，`min_items` 保证 List 字段的语义下界。

下一节：[FlowDocument 序列化协议](flowdocument.md)
