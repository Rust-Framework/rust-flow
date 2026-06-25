# Start 节点专属面板

通用 `PanelView` 能处理 8 种字段类型，为何 Start 节点要单独实现一套 `StartPanelView`？因为 Start 节点的参数/变量是**结构化树形数据**——每项有名称、类型、可选/数组标记、默认值，复杂/动态类型还带子字段。这远超 `FieldType::List` 的扁平二维表，必须用 Tree 控件 + 详情编辑器呈现。

## 模块拆分

`crates/gpui/src/panel/start/` 按职责拆成 8 个文件，避免单文件膨胀：

| 文件 | 职责 |
|------|------|
| `mod.rs` | `StartPanelView` 结构、`build`、`Render` 实现 |
| `common.rs` | 公共类型 `Selection`、`RowInputs`、`label_of` 等 |
| `data_types.rs` | JSON 读写辅助（`item_name`/`item_type`/`item_fields`...） |
| `item.rs` | `ItemState` + `FieldState` 单项状态管理 |
| `sync.rs` | node.data ↔ 面板状态同步、Input 订阅 |
| `handlers.rs` | 增删改、类型切换等事件处理 |
| `tree_render.rs` | Tree 控件渲染（内联 Input/Select/Switch） |
| `detail_editor.rs` | 浮层详情编辑面板 + 头部渲染 |

这种「按职责切文件、mod.rs 做聚合」是 rust-agent-flow 处理复杂模块的统一风格。

## StartPanelView 结构

```rust
pub struct StartPanelView {
    pub node: Node,
    pub flow_node: Option<Arc<dyn IFlowNode>>,
    pub theme: Theme,
    pub on_action: Option<ActionCallback>,
    pub syntax_service: SharedSyntaxService,
    pub language: Language,

    registry: DataTypeRegistry,                    // 类型注册表
    label_input: Entity<InputState>,               // 节点名称
    params_state: Vec<ItemState>,                  // 参数项
    variables_state: Vec<ItemState>,               // 变量项
    params_tree: Entity<TreeState>,                // 参数 Tree 控件
    variables_tree: Entity<TreeState>,             // 变量 Tree 控件
    selected: Option<Selection>,                   // 选中项（驱动详情浮层）
    syncing: bool,
    scroll_handle: ScrollHandle,
    _subscriptions: Vec<Subscription>,
}
```

与 `PanelView` 相比，多了 `registry`（数据类型注册表）、两套 `ItemState`、两个 `TreeState` 和 `selected`。

## 数据类型注册表

`DataTypeRegistry` 合并内置类型与 provider 注入类型：

```
DataTypeRegistry::new(provider)
   │
   ├── 内置 Basic：String / Integer / Float / Boolean / DateTime
   ├── 内置 Dynamic：Dynamic（结构可手动编辑）
   └── provider.data_types()（调用方注入的 Complex 类型）
```

类型分三类，决定编辑形态：

| 分类 | 结构 | 值编辑 |
|------|------|--------|
| Basic | 无字段，直接 value | 参数模式只读、变量模式可编辑 |
| Complex | provider 预定义，**结构只读** | 按模式可编辑 |
| Dynamic | **结构可手动增删改** | 按模式可编辑 |

`registry` 提供 `is_basic` / `is_complex` / `is_dynamic` / `has_fields` / `fields` / `is_structure_editable` 等查询，Tree 渲染与详情编辑器据此决定哪些控件可编辑。

## 树形编辑模型

参数区与变量区各用一个 `TreeState`，`build_section_tree_items` 把 `Vec<ItemState>` 转成 `TreeItem` 列表。每行的 `TreeItem` 内联渲染控件，而非纯文字：

```
┌ 输入参数 ──────────────── + 添加参数 ┐
│ ▾ param1 : String        [value]    │  ← 基础类型：单行
│ ▾ param2 : User          (展开子字段)│  ← 复杂类型：可展开子字段
│     ├ id : Integer       [0]        │
│     └ name : String      [""]       │
│ ▾ param3 : Dynamic       (展开子字段)│  ← 动态类型：子字段可增删
└─────────────────────────────────────┘
```

Tree 选中变化通过 `cx.observe(&params_tree, ...)` 监听，更新 `selected`，驱动右侧浮层详情编辑器。展开/收起状态通过 `cx.subscribe(&params_tree, |_, _, event: &TreeEvent, ...|)` 同步回 `ItemState`。

## 参数 vs 变量

二者结构相同，区别在**可编辑性**：

| 维度 | 参数（params） | 变量（variables） |
|------|---------------|------------------|
| 语义 | 流程入参，外部传入 | 流程内部定义 |
| 子字段值 | **只读**（由调用方提供） | **可编辑**（默认值） |
| 结构 | 可增删项、可改类型 | 可增删项、可改类型 |

`ItemState::from_value(item_val, is_variable, &registry, ...)` 的第二个参数控制值是否可编辑——参数传 `false`，变量传 `true`。

## 三种类型形态的编辑

### Basic 类型

```
name [Input]  type [Select]  optional [Switch]  array [Switch]  value [Input/Switch]
```

值控件随类型变化：Boolean → Switch，其余 → Input。`is_optional=true` 时默认值可省略。

### Complex 类型

结构由 provider 定义，**不可编辑**：子字段名、类型只读展示。值在变量模式下可编辑。

### Dynamic 类型

结构**可手动编辑**：用户可增删改子字段（字段名、字段类型、字段值），是低代码场景的「自定义结构」出口。

## 同步机制

`StartPanelView` 同样用 `syncing` 标记防回环。`subscribe_item_inputs` 在 `build` 阶段为每个 `ItemState` 的 Input 订阅变化，回调里检查 `syncing` 后写回 node.data。整体结构与 `PanelView` 一致，只是数据形状从「扁平字段」变成「树形项」。

## 为什么不并入通用面板

曾考虑用 `FieldType::List` 配合嵌套结构表达参数/变量，但很快碰壁：

- 参数项的「类型」是动态下拉，切换类型要联动改变值控件形态（Boolean→Switch）
- Complex/Dynamic 类型要展开任意层子字段，List 的二维表表达不了
- 选中项需要浮层详情编辑器（描述、字段摘要等），通用面板没有这个交互层

这些需求决定了 Start 节点必须走 `PanelEntity::Start` 特例通道，独立实现。这也是 `PanelEntity` 枚举存在的意义——为「schema 驱动表达不了」的少数节点留出逃生舱。

## 小结

`StartPanelView` 用 Tree 控件 + 详情浮层处理参数/变量的结构化树形编辑；`DataTypeRegistry` 合并内置与 provider 类型，按 Basic/Complex/Dynamic 决定编辑形态；参数只读、变量可编辑。它证明了 `PanelEntity` 特例通道的必要性——schema 驱动覆盖多数，特例覆盖少数。

下一章：[扩展点体系：ToolbarProvider 工具栏扩展](../15-extensions/toolbar-provider.md)
