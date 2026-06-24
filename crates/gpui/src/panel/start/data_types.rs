//! Start 节点数据 JSON 辅助函数。
//!
//! 与 [`DataTypeRegistry`](crate::data_type::DataTypeRegistry) 配合使用：
//! - 注册表提供类型元信息（名称、分类、字段定义）
//! - 本模块提供 JSON 序列化/反序列化辅助
//!
//! JSON 结构约定（低代码变量模型）：
//! - 基础类型：`{ "name": "x", "type": "String", "is_optional": false, "is_array": false, "value": "abc" }`
//! - 复杂/动态类型：`{ "name": "x", "type": "DataModel", "is_optional": false, "is_array": false, "fields": [...] }`
//!   - 子字段：`{ "name": "id", "type": "Integer", "value": "0" }`
//!
//! 规则：
//! - `is_optional=true` 时 `value` 可省略（可选参数/变量可不设默认值）
//! - `is_array=true` 表示该变量为数组/集合类型

use serde_json::{json, Value};

use crate::data_type::{DataTypeField, DataTypeRegistry};

/// 构建基础类型项的默认 JSON：`{ name, type, is_optional, is_array, value }`。
pub fn build_basic_item(
    name: &str,
    type_name: &str,
    value: &str,
    is_optional: bool,
    is_array: bool,
) -> Value {
    json!({
        "name": name,
        "type": type_name,
        "is_optional": is_optional,
        "is_array": is_array,
        "value": value,
    })
}

/// 构建复杂/动态类型项的默认 JSON：`{ name, type, is_optional, is_array, fields: [...] }`。
///
/// 复杂类型：字段结构由注册表定义自动填充。
/// 动态类型（Dynamic）：默认无字段，由用户手动添加。
pub fn build_structured_item(
    name: &str,
    type_name: &str,
    registry: &DataTypeRegistry,
    is_optional: bool,
    is_array: bool,
) -> Value {
    let fields: Vec<Value> = registry
        .fields(type_name)
        .iter()
        .map(|d| {
            json!({
                "name": d.name,
                "type": d.field_type,
                "value": d.default_value,
            })
        })
        .collect();
    json!({
        "name": name,
        "type": type_name,
        "is_optional": is_optional,
        "is_array": is_array,
        "fields": fields,
    })
}

/// 构建新项的默认 JSON（基础类型 String，空名称，非可选非数组）。
pub fn build_default_item() -> Value {
    build_basic_item("", "String", "", false, false)
}

/// 根据类型名构建项 JSON（自动判断基础/复杂/动态）。
///
/// 保留原 `is_optional`/`is_array` 状态（类型切换时不丢失标志位）。
pub fn build_item_for_type(
    name: &str,
    type_name: &str,
    registry: &DataTypeRegistry,
    is_optional: bool,
    is_array: bool,
) -> Value {
    if registry.has_fields(type_name) {
        build_structured_item(name, type_name, registry, is_optional, is_array)
    } else {
        build_basic_item(name, type_name, "", is_optional, is_array)
    }
}

/// 读取项的 name 字段。
pub fn item_name(item: &Value) -> String {
    item.get("name")
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .to_string()
}

/// 读取项的 type 字段。
pub fn item_type(item: &Value) -> String {
    item.get("type")
        .and_then(|v| v.as_str())
        .unwrap_or("String")
        .to_string()
}

/// 读取项的 is_optional 字段（默认 false）。
pub fn item_is_optional(item: &Value) -> bool {
    item.get("is_optional")
        .and_then(|v| v.as_bool())
        .unwrap_or(false)
}

/// 读取项的 is_array 字段（默认 false）。
pub fn item_is_array(item: &Value) -> bool {
    item.get("is_array")
        .and_then(|v| v.as_bool())
        .unwrap_or(false)
}

/// 读取基础类型的 value 字段。
pub fn item_value(item: &Value) -> String {
    item.get("value")
        .map(value_to_string)
        .unwrap_or_default()
}

/// 读取复杂/动态类型的子字段数组。
pub fn item_fields(item: &Value) -> Vec<Value> {
    item.get("fields")
        .and_then(|v| v.as_array())
        .cloned()
        .unwrap_or_default()
}

/// JSON Value → String。
pub fn value_to_string(v: &Value) -> String {
    match v {
        Value::String(s) => s.clone(),
        Value::Number(n) => n.to_string(),
        Value::Bool(b) => b.to_string(),
        Value::Null => String::new(),
        _ => v.to_string(),
    }
}

/// 从子字段 JSON 构建 DataTypeField（用于动态类型序列化）。
pub fn field_value_to_def(field: &Value) -> Option<DataTypeField> {
    let name = field.get("name").and_then(|v| v.as_str())?.to_string();
    let field_type = field
        .get("type")
        .and_then(|v| v.as_str())
        .unwrap_or("String")
        .to_string();
    let default_value = field
        .get("value")
        .map(value_to_string)
        .unwrap_or_default();
    Some(DataTypeField::new(name, field_type, default_value))
}
