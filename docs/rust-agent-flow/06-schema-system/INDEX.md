# 第六章 Schema 与字段系统

第五章讲了图的数据骨架，本章讲节点的「元数据」——`NodeSchema`。Schema 声明节点的端口规格、默认尺寸与字段结构，驱动属性面板**自动生成**编辑界面，消除 per-kind 面板分发。这是「Schema 驱动」原则落地最直接的体现。

## 本章小节

| 小节 | 内容 |
|------|------|
| [NodeSchema 与 PortSpec](node-schema-port.md) | 节点声明、端口规格、默认尺寸与构建器 |
| [FieldSpec 与字段类型](fieldspec-types.md) | Text/Code/Dropdown/List 等控件映射与默认值填充 |
| [FlowDocument 序列化协议](flowdocument.md) | 版本、元数据、NodeDef/EdgeDef 与 JSON 结构 |

## 学习目标

读完本章，你应能：

- 用 `NodeSchema::new(...).with_port(...).with_field(...)` 链式声明一个完整节点
- 说出 `PortSpec` 中 `side` 为 `Auto` 与固定值的区别
- 列出全部 `FieldType` 变体及其对应的属性面板控件
- 解释 `default_data()` 如何从 `fields` 推导出初始 `node.data`
- 写出一份合法的 `FlowDocument` JSON 并描述每个字段含义

## 前置知识

- 已阅读 [第五章 流程图数据模型](../05-graph-model/INDEX.md)
- 了解 `serde_json::Value` 与 Rust 枚举

## 下一步

从 [NodeSchema 与 PortSpec](node-schema-port.md) 开始。
