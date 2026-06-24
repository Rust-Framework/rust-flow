//! 自定义数据类型提供程序示例。
//!
//! 实现 [`IDataTypeProvider`] trait，通过
//! [`FlowEditorView::set_data_type_provider`] 注入到编辑器。
//!
//! 提供 `DataModel` 复杂类型（预定义结构，不可编辑），
//! 与内置类型（String/Integer/Float/Boolean/DateTime/Dynamic）
//! 合并后在 Start 节点属性面板的类型下拉中可选。

use rust_agent_flow_gpui::{DataTypeCategory, DataTypeField, IDataType, IDataTypeProvider};

/// 自定义数据类型提供者。
pub struct DemoDataTypeProvider;

/// DataModel 复杂类型：预定义 id + name 结构。
struct DataModelType;

impl IDataType for DataModelType {
    fn name(&self) -> &str {
        "DataModel"
    }
    fn category(&self) -> DataTypeCategory {
        DataTypeCategory::Complex
    }
    fn fields(&self) -> Vec<DataTypeField> {
        vec![
            DataTypeField::new("id", "Integer", "0"),
            DataTypeField::new("name", "String", ""),
        ]
    }
}

/// User 复杂类型：预定义 user_id + email 结构。
struct UserType;

impl IDataType for UserType {
    fn name(&self) -> &str {
        "User"
    }
    fn category(&self) -> DataTypeCategory {
        DataTypeCategory::Complex
    }
    fn fields(&self) -> Vec<DataTypeField> {
        vec![
            DataTypeField::new("user_id", "Integer", "0"),
            DataTypeField::new("email", "String", ""),
        ]
    }
}

impl IDataTypeProvider for DemoDataTypeProvider {
    fn data_types(&self) -> Vec<Box<dyn IDataType>> {
        vec![Box::new(DataModelType), Box::new(UserType)]
    }
}
