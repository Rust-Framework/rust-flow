//! Builtin control-flow node type definitions.

use crate::node_type::{BRANCH, COMMON, LOOP};
use crate::port::{PortDirection, PortSide};
use crate::schema::types::{
    FlowFieldDef, FlowFieldType, FlowNodeTypeDef, FlowPortDef, FlowRenderDef, FlowSizeDef,
    FlowTypeRegistry,
};

pub fn builtin_type_registry() -> FlowTypeRegistry {
    let mut registry = FlowTypeRegistry::new();

    registry.register(
        COMMON,
        FlowNodeTypeDef {
            label: "常用节点".into(),
            category: "common".into(),
            description: Some("通用处理节点".into()),
            default_data: serde_json::json!({ "expression": "" }),
            default_size: FlowSizeDef {
                width: 200.0,
                height: 45.0,
            },
            ports: vec![
                FlowPortDef {
                    id: "in".into(),
                    label: Some("in".into()),
                    direction: PortDirection::Input,
                    side: PortSide::Left,
                },
                FlowPortDef {
                    id: "out".into(),
                    label: Some("out".into()),
                    direction: PortDirection::Output,
                    side: PortSide::Right,
                },
            ],
            fields: vec![
                FlowFieldDef {
                    key: "label".into(),
                    label: "标签".into(),
                    field_type: FlowFieldType::Text,
                    default: None,
                    placeholder: None,
                },
                FlowFieldDef {
                    key: "expression".into(),
                    label: "表达式".into(),
                    field_type: FlowFieldType::Expression,
                    default: Some(serde_json::json!("")),
                    placeholder: Some("e.g. process(input)".into()),
                },
            ],
            render: FlowRenderDef {
                accent: Some("#8C8C8E".into()),
                title: Some("{{label}}".into()),
                subtitle: None,
                body: vec![],
                footer: None,
            },
        },
    );

    registry.register(
        BRANCH,
        FlowNodeTypeDef {
            label: "条件分支".into(),
            category: "control".into(),
            description: Some("If / Else 条件分支".into()),
            default_data: serde_json::json!({
                "collapsed": false,
                "branches": [
                    { "id": "true", "label": "满足", "condition": "" },
                    { "id": "false", "label": "否则", "condition": "else" }
                ]
            }),
            default_size: FlowSizeDef {
                width: 180.0,
                height: 72.0,
            },
            ports: vec![
                FlowPortDef {
                    id: "in".into(),
                    label: None,
                    direction: PortDirection::Input,
                    side: PortSide::Left,
                },
                FlowPortDef {
                    id: "true".into(),
                    label: Some("true".into()),
                    direction: PortDirection::Output,
                    side: PortSide::Right,
                },
                FlowPortDef {
                    id: "false".into(),
                    label: Some("false".into()),
                    direction: PortDirection::Output,
                    side: PortSide::Right,
                },
            ],
            fields: vec![
                FlowFieldDef {
                    key: "label".into(),
                    label: "标签".into(),
                    field_type: FlowFieldType::Text,
                    default: None,
                    placeholder: None,
                },
                FlowFieldDef {
                    key: "condition".into(),
                    label: "条件表达式".into(),
                    field_type: FlowFieldType::Expression,
                    default: Some(serde_json::json!("")),
                    placeholder: Some("e.g. x > 0".into()),
                },
            ],
            render: FlowRenderDef {
                accent: Some("#8B5AD8".into()),
                title: Some("If / Else".into()),
                subtitle: Some("if ({{data.condition}})".into()),
                body: vec![],
                footer: Some("true · false".into()),
            },
        },
    );

    registry.register(
        LOOP,
        FlowNodeTypeDef {
            label: "循环遍历".into(),
            category: "control".into(),
            description: Some("For-each 循环节点".into()),
            default_data: serde_json::json!({
                "iterator": "item",
                "collection": "",
                "max_iterations": 1000
            }),
            default_size: FlowSizeDef {
                width: 200.0,
                height: 64.0,
            },
            ports: vec![
                FlowPortDef {
                    id: "in".into(),
                    label: None,
                    direction: PortDirection::Input,
                    side: PortSide::Left,
                },
                FlowPortDef {
                    id: "out".into(),
                    label: Some("out".into()),
                    direction: PortDirection::Output,
                    side: PortSide::Right,
                },
                FlowPortDef {
                    id: "body".into(),
                    label: Some("body".into()),
                    direction: PortDirection::Output,
                    side: PortSide::Bottom,
                },
            ],
            fields: vec![
                FlowFieldDef {
                    key: "label".into(),
                    label: "标签".into(),
                    field_type: FlowFieldType::Text,
                    default: None,
                    placeholder: None,
                },
                FlowFieldDef {
                    key: "iterator".into(),
                    label: "迭代变量".into(),
                    field_type: FlowFieldType::Text,
                    default: Some(serde_json::json!("item")),
                    placeholder: None,
                },
                FlowFieldDef {
                    key: "collection".into(),
                    label: "集合".into(),
                    field_type: FlowFieldType::Expression,
                    default: Some(serde_json::json!("")),
                    placeholder: Some("e.g. items".into()),
                },
                FlowFieldDef {
                    key: "max_iterations".into(),
                    label: "最大迭代次数".into(),
                    field_type: FlowFieldType::Number,
                    default: Some(serde_json::json!(1000)),
                    placeholder: None,
                },
            ],
            render: FlowRenderDef {
                accent: Some("#E68C33".into()),
                title: Some("{{label}}".into()),
                subtitle: Some("for {{data.iterator}} in {{data.collection}}".into()),
                body: vec!["body ↓".into()],
                footer: None,
            },
        },
    );

    registry
}
