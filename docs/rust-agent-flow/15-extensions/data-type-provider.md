# IDataTypeProvider 数据类型扩展

Start 节点的参数/变量需要选择「类型」。框架内置了 String/Integer/Float/Boolean/DateTime/Dynamic 六种，但真实业务里常有领域类型：`User`、`Order`、`Document`……这些结构化类型由谁定义？`IDataTypeProvider` 把这个口子交给调用方。

## 类型分类

`DataTypeCategory` 把所有类型归为三类，决定编辑形态：

```rust
pub enum DataTypeCategory {
    Basic,    // 基础标量，直接 value
    Complex,  // provider 预定义结构，结构不可编辑
    Dynamic,  // 结构可手动增删改
}
```

| 分类 | 结构来源 | 结构可编辑 | 典型类型 |
|------|----------|-----------|----------|
| Basic | 无字段 | 否 | String/Integer/Float/Boolean/DateTime |
| Complex | provider | 否（只读） | User、Order 等领域类型 |
| Dynamic | 用户运行时定义 | 是 | Dynamic（自定义结构） |

Basic 与 Dynamic 是内置的，Complex 必须由 provider 提供——这正是 `IDataTypeProvider` 的用武之地。

## 两个 trait：类型与提供者

接口分两层：单个类型实现 `IDataType`，提供者实现 `IDataTypeProvider` 汇总一批类型：

```rust
pub trait IDataType: Send + Sync {
    fn name(&self) -> &str;                       // 唯一标识，如 "User"
    fn category(&self) -> DataTypeCategory;       // Basic/Complex/Dynamic
    fn fields(&self) -> Vec<DataTypeField>;       // 子字段（Basic 返回空）
}

pub trait IDataTypeProvider: Send + Sync {
    fn data_types(&self) -> Vec<Box<dyn IDataType>>;
}

pub type SharedDataTypeProvider = Arc<dyn IDataTypeProvider>;
```

`DataTypeField` 描述子字段：

```rust
pub struct DataTypeField {
    pub name: String,            // 字段名
    pub field_type: String,      // 类型名（引用注册表中的类型，如 "Integer"）
    pub default_value: String,   // 默认值
}
```

注意 `field_type` 是**字符串引用**而非枚举——子字段类型可以是注册表里的任意类型（包括另一个 Complex），支持类型组合。

## DataTypeRegistry：合并内置与 provider

`DataTypeRegistry` 是类型的运行时查询入口，构造时合并内置类型与 provider 类型：

```rust
impl DataTypeRegistry {
    pub fn new(provider: Option<SharedDataTypeProvider>) -> Self {
        let mut types: Vec<Box<dyn IDataType>> = vec![
            Box::new(BuiltinBasicType::new("String")),
            Box::new(BuiltinBasicType::new("Integer")),
            Box::new(BuiltinBasicType::new("Float")),
            Box::new(BuiltinBasicType::new("Boolean")),
            Box::new(BuiltinBasicType::new("DateTime")),
            Box::new(DynamicType),    // Dynamic 始终内置
        ];
        if let Some(p) = provider {
            types.extend(p.data_types());   // 追加 provider 类型
        }
        Self { types }
    }
}
```

关键设计：

- 内置类型**始终可用**，不注入 provider 也能工作
- provider 类型是**追加**而非替换，不会覆盖内置
- `Dynamic` 永远内置，作为「自定义结构」的兜底出口

注册表提供一组查询方法，面板据此决定编辑形态：

| 方法 | 用途 |
|------|------|
| `type_names()` | 类型下拉菜单选项 |
| `get(name)` | 按名查类型 |
| `is_basic/is_complex/is_dynamic(name)` | 判断分类 |
| `has_fields(name)` | 是否有子字段 |
| `fields(name)` | 取子字段定义 |
| `is_structure_editable(name)` | 结构是否可编辑（仅 Dynamic） |

## 注入：替换型 + 销毁重建

与 `ToolbarProvider` 的累积注入不同，`set_data_type_provider` 是**替换型**：

```rust
pub fn set_data_type_provider(&mut self, provider: SharedDataTypeProvider, cx) {
    self.data_type_provider = Some(provider);
    self.panel_view = None;   // 销毁现有面板
    cx.notify();
}
```

为什么必须销毁 panel_view？因为已有面板持有旧 `DataTypeRegistry` 的引用，类型下拉、子字段渲染都基于旧类型集。若不重建，用户会看到「注册表已变、面板还显示旧类型」的撕裂状态。销毁后下一帧 `ensure_panel_view` 用新 provider 重建 `StartPanelView`，其 `build` 会用新 `DataTypeRegistry` 重新构建所有 `ItemState`。

这是替换型扩展点的统一语义：**注入即重建依赖它的视图**。本章的 SyntaxService 与下一节的 Language 都遵循此规则。

## 实战：自定义复杂类型

假设业务有 `User` 类型，含 id/name/email 三个字段：

```rust
use rust_agent_flow_gpui::{
    DataTypeCategory, DataTypeField, IDataType, IDataTypeProvider, SharedDataTypeProvider,
};

struct UserType;
impl IDataType for UserType {
    fn name(&self) -> &str { "User" }
    fn category(&self) -> DataTypeCategory { DataTypeCategory::Complex }
    fn fields(&self) -> Vec<DataTypeField> {
        vec![
            DataTypeField::new("id",    "Integer", "0"),
            DataTypeField::new("name",  "String",  ""),
            DataTypeField::new("email", "String",  ""),
        ]
    }
}

struct OrderType;
impl IDataType for OrderType {
    fn name(&self) -> &str { "Order" }
    fn category(&self) -> DataTypeCategory { DataTypeCategory::Complex }
    fn fields(&self) -> Vec<DataTypeField> {
        vec![
            DataTypeField::new("order_id", "String", ""),
            DataTypeField::new("user",     "User",   ""),  // 引用另一个 Complex
            DataTypeField::new("amount",   "Float",  "0"),
        ]
    }
}

pub struct DomainDataTypeProvider;
impl IDataTypeProvider for DomainDataTypeProvider {
    fn data_types(&self) -> Vec<Box<dyn IDataType>> {
        vec![Box::new(UserType), Box::new(OrderType)]
    }
}
```

注入：

```rust
editor.set_data_type_provider(Arc::new(DomainDataTypeProvider), cx);
```

之后 Start 面板的类型下拉里就多了 `User`、`Order`，选中后展开只读子字段。`Order.user` 引用 `User` 体现了 `field_type` 字符串引用的威力——类型可任意组合嵌套。

## Demo 的空 provider

Demo 的 `DemoDataTypeProvider` 当前返回空 vec，仅作扩展点参考：

```rust
pub struct DemoDataTypeProvider;
impl IDataTypeProvider for DemoDataTypeProvider {
    fn data_types(&self) -> Vec<Box<dyn IDataType>> { vec![] }
}
```

Demo 数据均用内置类型，故无需注入。真实项目按上节示例填充即可。

## 小结

`IDataType`/`IDataTypeProvider` 两层接口让调用方注入领域 Complex 类型；`DataTypeRegistry` 合并内置与 provider 类型，内置始终可用；`set_data_type_provider` 是替换型注入，销毁 panel_view 以避免新旧类型集撕裂。`field_type` 字符串引用支持类型嵌套组合。

下一节：[SyntaxService 语法高亮扩展](syntax-service.md)
