//! 自定义数据类型提供程序示例（当前为空）。
//!
//! Demo 示例数据均已使用内置类型（String/Integer/Float/Boolean/DateTime/Dynamic），
//! 无需注入额外复杂类型。保留此文件作为扩展点参考。
//!
//! 如需添加自定义复杂类型，可参考以下写法：
//! ```ignore
//! use rust_agent_flow_gpui::{DataTypeCategory, DataTypeField, IDataType, IDataTypeProvider};
//!
//! struct MyType;
//! impl IDataType for MyType {
//!     fn name(&self) -> &str { "MyType" }
//!     fn category(&self) -> DataTypeCategory { DataTypeCategory::Complex }
//!     fn fields(&self) -> Vec<DataTypeField> { vec![] }
//! }
//! ```

use rust_agent_flow_gpui::{IDataTypeProvider, SharedDataTypeProvider};

/// 自定义数据类型提供者（当前不提供额外类型）。
pub struct DemoDataTypeProvider;

impl IDataTypeProvider for DemoDataTypeProvider {
    fn data_types(&self) -> Vec<Box<dyn rust_agent_flow_gpui::IDataType>> {
        vec![]
    }
}