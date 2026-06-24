//! Start 节点数据类型系统：简单类型 + 复杂类型注册表。
//!
//! 简单类型（string/int/float/bool）直接存储 value；
//! 复杂类型（DataModel 等）具有预定义的子字段结构，树形展示。
//!
//! 类型切换时自动重建数据结构：
//! - simple → complex：移除 value，按类型定义填充 fields
//! - complex → simple：移除 fields，添加空 value

use serde_json::{json, Value};

/// 简单数据类型列表（下拉可选）。
pub const SIMPLE_TYPES: &[&str] = &["string", "int", "float", "bool"];

/// 复杂数据类型列表（下拉可选，展开后显示子字段）。
pub const COMPLEX_TYPES: &[&str] = &["DataModel"];

/// 所有可选类型（简单 + 复杂），用于下拉菜单。
pub fn all_types() -> Vec<&'static str> {
    let mut types: Vec<&'static str> = SIMPLE_TYPES.to_vec();
    types.extend_from_slice(COMPLEX_TYPES);
    types
}

/// 判断类型是否为复杂类型。
pub fn is_complex_type(type_name: &str) -> bool {
    COMPLEX_TYPES.contains(&type_name)
}

/// 复杂类型的子字段定义。
pub struct ComplexFieldDef {
    pub name: &'static str,
    pub field_type: &'static str,
    pub default_value: &'static str,
}

/// 返回复杂类型的子字段定义。
///
/// 用于类型切换时自动填充子字段结构。
pub fn complex_type_fields(type_name: &str) -> Option<Vec<ComplexFieldDef>> {
    match type_name {
        "DataModel" => Some(vec![
            ComplexFieldDef {
                name: "id",
                field_type: "int",
                default_value: "0",
            },
            ComplexFieldDef {
                name: "name",
                field_type: "string",
                default_value: "",
            },
        ]),
        _ => None,
    }
}

/// 构建简单类型项的默认 JSON：`{ name, type, value }`。
pub fn build_simple_item(name: &str, type_name: &str, value: &str) -> Value {
    json!({
        "name": name,
        "type": type_name,
        "value": value,
    })
}

/// 构建复杂类型项的默认 JSON：`{ name, type, fields: [...] }`。
///
/// 子字段结构由 [`complex_type_fields`] 定义，自动填充。
pub fn build_complex_item(name: &str, type_name: &str) -> Value {
    let fields: Vec<Value> = complex_type_fields(type_name)
        .map(|defs| {
            defs.iter()
                .map(|d| {
                    json!({
                        "name": d.name,
                        "type": d.field_type,
                        "value": d.default_value,
                    })
                })
                .collect()
        })
        .unwrap_or_default();
    json!({
        "name": name,
        "type": type_name,
        "fields": fields,
    })
}

/// 构建新项的默认 JSON（简单类型，空名称）。
pub fn build_default_item() -> Value {
    build_simple_item("", "string", "")
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
        .unwrap_or("string")
        .to_string()
}

/// 读取简单类型的 value 字段。
pub fn item_value(item: &Value) -> String {
    item.get("value")
        .map(value_to_string)
        .unwrap_or_default()
}

/// 读取复杂类型的子字段数组。
pub fn item_fields(item: &Value) -> Vec<Value> {
    item.get("fields")
        .and_then(|v| v.as_array())
        .map(|a| a.clone())
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
