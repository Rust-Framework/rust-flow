//! 数据类型系统：IDataType / IDataTypeProvider 接口 + 内置类型 + 注册表。
//!
//! **设计参考**：与 [`ToolbarProvider`](crate::editor::toolbar_ext::ToolbarProvider)
//! 相同的 trait + `Arc<dyn Trait>` + setter 注入模式。
//!
//! 类型分类：
//! - **Basic**：基础标量类型（Boolean/String/Number/DateTime），直接存储 value
//! - **Complex**：复杂类型，预定义结构，结构**不可编辑**（由 provider 提供）
//! - **Dynamic**：动态类型（DynamicObject），结构**可手动编辑**（增删改字段）
//!
//! 注入方式：
//! - 调用方通过 [`FlowEditorView::set_data_type_provider`] 注入自定义类型
//! - 不注入时仅有内置类型可用
//! - 内置类型始终可用，与 provider 类型合并

use std::sync::Arc;

/// 数据类型分类。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DataTypeCategory {
    /// 基础标量类型：直接存储 value（Boolean/String/Number/DateTime）。
    Basic,
    /// 复杂类型：预定义结构，结构不可编辑（由 provider 提供）。
    Complex,
    /// 动态类型：结构可手动编辑（DynamicObject）。
    Dynamic,
}

/// 数据类型字段定义（复杂/动态类型的子字段）。
#[derive(Debug, Clone)]
pub struct DataTypeField {
    /// 字段名称。
    pub name: String,
    /// 字段类型名（引用注册表中的类型名，如 "String"、"Number"）。
    pub field_type: String,
    /// 默认值。
    pub default_value: String,
}

impl DataTypeField {
    pub fn new(name: impl Into<String>, field_type: impl Into<String>, default_value: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            field_type: field_type.into(),
            default_value: default_value.into(),
        }
    }
}

/// 数据类型接口（扩展点基础）。
///
/// 提供数据结构定义。内置类型和外部 provider 类型均实现此接口。
pub trait IDataType: Send + Sync {
    /// 类型名称（唯一标识，如 "String"、"DataModel"、"DynamicObject"）。
    fn name(&self) -> &str;
    /// 类型分类。
    fn category(&self) -> DataTypeCategory;
    /// 字段定义（仅 Complex/Dynamic 类型有字段，Basic 返回空）。
    fn fields(&self) -> Vec<DataTypeField>;
}

/// 数据类型提供程序接口（扩展点）。
///
/// 调用方实现此 trait，通过
/// [`FlowEditorView::set_data_type_provider`](crate::editor::FlowEditorView::set_data_type_provider)
/// 注入自定义数据类型。不注入代表没有自定义类型。
///
/// **示例**（demo 自定义类型提供者）：
/// ```ignore
/// use rust_agent_flow_gpui::{IDataType, IDataTypeProvider, DataTypeCategory, DataTypeField};
///
/// pub struct MyDataTypeProvider;
///
/// struct DataModelType;
/// impl IDataType for DataModelType {
///     fn name(&self) -> &str { "DataModel" }
///     fn category(&self) -> DataTypeCategory { DataTypeCategory::Complex }
///     fn fields(&self) -> Vec<DataTypeField> {
///         vec![
///             DataTypeField::new("id", "Number", "0"),
///             DataTypeField::new("name", "String", ""),
///         ]
///     }
/// }
///
/// impl IDataTypeProvider for MyDataTypeProvider {
///     fn data_types(&self) -> Vec<Box<dyn IDataType>> {
///         vec![Box::new(DataModelType)]
///     }
/// }
/// ```
pub trait IDataTypeProvider: Send + Sync {
    /// 提供自定义数据类型列表。
    fn data_types(&self) -> Vec<Box<dyn IDataType>>;
}

/// 共享数据类型提供程序类型（`Arc<dyn IDataTypeProvider>` 的别名）。
pub type SharedDataTypeProvider = Arc<dyn IDataTypeProvider>;

// ====== 内置类型实现 ======

/// 内置基础类型（Boolean/String/Number/DateTime）。
struct BuiltinBasicType {
    name: &'static str,
}

impl BuiltinBasicType {
    fn new(name: &'static str) -> Self {
        Self { name }
    }
}

impl IDataType for BuiltinBasicType {
    fn name(&self) -> &str {
        self.name
    }
    fn category(&self) -> DataTypeCategory {
        DataTypeCategory::Basic
    }
    fn fields(&self) -> Vec<DataTypeField> {
        Vec::new()
    }
}

/// 内置动态类型 DynamicObject：结构可手动编辑。
struct DynamicObjectType;

impl IDataType for DynamicObjectType {
    fn name(&self) -> &str {
        "DynamicObject"
    }
    fn category(&self) -> DataTypeCategory {
        DataTypeCategory::Dynamic
    }
    fn fields(&self) -> Vec<DataTypeField> {
        // DynamicObject 默认无字段，由用户手动添加
        Vec::new()
    }
}

/// 数据类型注册表：合并内置类型 + provider 提供的类型。
///
/// 由 [`FlowEditorView`] 持有，传递给属性面板用于类型选择和结构定义。
pub struct DataTypeRegistry {
    types: Vec<Box<dyn IDataType>>,
}

impl DataTypeRegistry {
    /// 创建注册表，合并内置类型和 provider 类型。
    pub fn new(provider: Option<SharedDataTypeProvider>) -> Self {
        let mut types: Vec<Box<dyn IDataType>> = vec![
            Box::new(BuiltinBasicType::new("Boolean")),
            Box::new(BuiltinBasicType::new("String")),
            Box::new(BuiltinBasicType::new("Number")),
            Box::new(BuiltinBasicType::new("DateTime")),
            Box::new(DynamicObjectType),
        ];
        if let Some(p) = provider {
            types.extend(p.data_types());
        }
        Self { types }
    }

    /// 创建仅含内置类型的空注册表。
    pub fn builtin() -> Self {
        Self::new(None)
    }

    /// 返回所有类型名（用于下拉菜单）。
    pub fn type_names(&self) -> Vec<&str> {
        self.types.iter().map(|t| t.name()).collect()
    }

    /// 按名称查找类型。
    pub fn get(&self, name: &str) -> Option<&dyn IDataType> {
        self.types
            .iter()
            .find(|t| t.name() == name)
            .map(|t| t.as_ref())
    }

    /// 判断类型是否为基础类型。
    pub fn is_basic(&self, name: &str) -> bool {
        self.get(name)
            .map(|t| t.category() == DataTypeCategory::Basic)
            .unwrap_or(false)
    }

    /// 判断类型是否为复杂类型（结构不可编辑）。
    pub fn is_complex(&self, name: &str) -> bool {
        self.get(name)
            .map(|t| t.category() == DataTypeCategory::Complex)
            .unwrap_or(false)
    }

    /// 判断类型是否为动态类型（结构可编辑）。
    pub fn is_dynamic(&self, name: &str) -> bool {
        self.get(name)
            .map(|t| t.category() == DataTypeCategory::Dynamic)
            .unwrap_or(false)
    }

    /// 判断类型是否有字段（复杂/动态类型）。
    pub fn has_fields(&self, name: &str) -> bool {
        self.get(name)
            .map(|t| t.category() != DataTypeCategory::Basic)
            .unwrap_or(false)
    }

    /// 获取类型的字段定义（基础类型返回空）。
    pub fn fields(&self, name: &str) -> Vec<DataTypeField> {
        self.get(name)
            .map(|t| t.fields())
            .unwrap_or_default()
    }

    /// 判断类型结构是否可编辑（仅动态类型）。
    pub fn is_structure_editable(&self, name: &str) -> bool {
        self.is_dynamic(name)
    }
}
