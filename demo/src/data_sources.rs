//! Demo 数据源：预置流程示例。
//!
//! 每个数据源返回 [`FlowDocument`]，通过 [`FlowGraph::from_document`] 转换为
//! 可编辑的流程图。数据驱动设计：节点/边定义与渲染逻辑解耦。

use rust_agent_flow::{
    EdgeDef, EdgeType, FlowDocument, FlowGraph, NodeDef, PointF, SizeF,
};
use rust_agent_flow_gpui::{Language, TKey, t};

/// Demo 预置数据源枚举。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum DemoDataSource {
    /// Agent 编排流程（默认）：条件分支 + 循环回环。
    #[default]
    AgentFlow,
    /// 数据处理管道：数据清洗 → 分流 → 处理 → 汇合。
    DataPipeline,
    /// 简单线性流程：Start → Action → End。
    SimpleFlow,
}

impl DemoDataSource {
    /// 返回数据源对应的 FlowGraph（通过 from_document 转换）。
    pub fn to_graph(&self) -> FlowGraph {
        FlowGraph::from_document(&self.to_document())
    }

    /// 返回数据源的 FlowDocument 定义。
    pub fn to_document(&self) -> FlowDocument {
        match self {
            Self::AgentFlow => agent_flow_doc(),
            Self::DataPipeline => data_pipeline_doc(),
            Self::SimpleFlow => simple_flow_doc(),
        }
    }

    /// 返回数据源的显示标签（根据语言国际化）。
    pub fn label(&self, lang: Language) -> &'static str {
        let key = match self {
            Self::AgentFlow => TKey::DataSourceAgentFlow,
            Self::DataPipeline => TKey::DataSourceDataPipeline,
            Self::SimpleFlow => TKey::DataSourceSimpleFlow,
        };
        t(lang, key)
    }

    /// 返回所有数据源变体。
    pub fn all() -> &'static [DemoDataSource] {
        &[Self::AgentFlow, Self::DataPipeline, Self::SimpleFlow]
    }
}

/// Agent 编排流程：13 节点 + 15 边，覆盖条件分支和循环回环。
fn agent_flow_doc() -> FlowDocument {
    let mut doc = FlowDocument::new("Agent Orchestration")
        .with_description("Agent 编排流程：规划 → 条件分支 → 循环处理 → 汇总");

    let start = doc.add_node(NodeDef::new("start", serde_json::json!({
        "label": "Start",
        "params": [
            { "name": "query", "type": "string", "value": "" },
            { "name": "context", "type": "object", "value": "{}" }
        ],
        "variables": [{ "name": "turn", "type": "int", "value": "0" }]
    })).with_size(SizeF::new(160.0, 56.0)).with_position(PointF::new(80.0, 280.0)));

    let variable = doc.add_node(NodeDef::new("variable", serde_json::json!({
        "label": "Vars",
        "variables": [
            { "name": "threshold", "type": "float", "value": "0.8" },
            { "name": "max_retry", "type": "int", "value": "3" }
        ]
    })).with_size(SizeF::new(200.0, 64.0)).with_position(PointF::new(300.0, 200.0)));

    let agent = doc.add_node(NodeDef::new("agent", serde_json::json!({
        "label": "Agent",
        "model": "gpt-4",
        "prompt": "You are a helpful assistant."
    })).with_size(SizeF::new(200.0, 64.0)).with_position(PointF::new(300.0, 360.0)));

    let planner = doc.add_node(NodeDef::new("action", serde_json::json!({
        "label": "Planner", "desc": "规划下一步"
    })).with_size(SizeF::new(180.0, 35.0)).with_position(PointF::new(540.0, 280.0)));

    let condition = doc.add_node(NodeDef::new("condition", serde_json::json!({
        "label": "Check",
        "conditions": [
            { "id": "if_0", "label": "needs_search" },
            { "id": "if_1", "label": "needs_tool" }
        ]
    })).with_size(SizeF::new(220.0, 144.0)).with_position(PointF::new(820.0, 280.0)));

    let tool = doc.add_node(NodeDef::new("action", serde_json::json!({
        "label": "ToolCall", "desc": "调用外部工具"
    })).with_size(SizeF::new(180.0, 35.0)).with_position(PointF::new(1100.0, 281.0)));

    let search = doc.add_node(NodeDef::new("action", serde_json::json!({
        "label": "Search", "desc": "检索知识库"
    })).with_size(SizeF::new(180.0, 35.0)).with_position(PointF::new(1100.0, 326.0)));

    let notify = doc.add_node(NodeDef::new("action", serde_json::json!({
        "label": "Notify", "desc": "发送通知"
    })).with_size(SizeF::new(180.0, 35.0)).with_position(PointF::new(1100.0, 380.0)));

    let adapter = doc.add_node(NodeDef::new("adapter", serde_json::json!({
        "label": "Adapter", "desc": "JSON → Struct"
    })).with_size(SizeF::new(200.0, 64.0)).with_position(PointF::new(1380.0, 326.0)));

    let loop_node = doc.add_node(NodeDef::new("loop", serde_json::json!({
        "label": "Loop", "desc": "For each item"
    })).with_size(SizeF::new(220.0, 80.0)).with_position(PointF::new(1660.0, 280.0)));

    let process = doc.add_node(NodeDef::new("action", serde_json::json!({
        "label": "Process", "desc": "处理当前项"
    })).with_size(SizeF::new(180.0, 35.0)).with_position(PointF::new(1940.0, 400.0)));

    let summarize = doc.add_node(NodeDef::new("action", serde_json::json!({
        "label": "Summarize", "desc": "汇总结果"
    })).with_size(SizeF::new(180.0, 35.0)).with_position(PointF::new(2220.0, 280.0)));

    let end = doc.add_node(NodeDef::new("end", serde_json::json!({
        "label": "End",
        "returns": [
            { "name": "answer", "type": "string", "value": "" },
            { "name": "status", "type": "int", "value": "0" }
        ]
    })).with_size(SizeF::new(160.0, 56.0)).with_position(PointF::new(2500.0, 280.0)));

    // 主流程（顺序初始化：Start → Vars → Agent → Planner → Check）
    // 消除原 Start 同时扇出 Vars/Agent 的并行依赖，Vars 先就绪再驱动 Agent
    doc.add_edge(EdgeDef::new(start, variable).with_edge_type(EdgeType::SmoothStep));
    doc.add_edge(EdgeDef::new(variable, agent).with_edge_type(EdgeType::SmoothStep));
    doc.add_edge(EdgeDef::new(agent, planner).with_edge_type(EdgeType::SmoothStep));
    doc.add_edge(EdgeDef::new(planner, condition).with_target_port("in").with_edge_type(EdgeType::SmoothStep));
    // 条件分支（任务意图维度：needs_search / needs_tool / else needs_notify）
    doc.add_edge(EdgeDef::new(condition, search).with_source_port("if_0").with_edge_type(EdgeType::SmoothStep));
    doc.add_edge(EdgeDef::new(condition, tool).with_source_port("if_1").with_edge_type(EdgeType::SmoothStep));
    doc.add_edge(EdgeDef::new(condition, notify).with_source_port("else").with_edge_type(EdgeType::SmoothStep));
    // 对称适配：Search 和 ToolCall 都经 Adapter 归一化再入 Loop，保证 Loop 输入类型一致
    doc.add_edge(EdgeDef::new(search, adapter).with_edge_type(EdgeType::SmoothStep));
    doc.add_edge(EdgeDef::new(tool, adapter).with_edge_type(EdgeType::SmoothStep));
    doc.add_edge(EdgeDef::new(adapter, loop_node).with_target_port("in").with_edge_type(EdgeType::SmoothStep));
    // 副作用分支隔离：Notify 不入 Loop（无迭代对象），直接汇入 Summarize
    doc.add_edge(EdgeDef::new(notify, summarize).with_edge_type(EdgeType::SmoothStep));
    // 循环体
    doc.add_edge(EdgeDef::new(loop_node, process).with_source_port("loop_body").with_edge_type(EdgeType::SmoothStep));
    doc.add_edge(EdgeDef::new(process, loop_node).with_target_port("loop_in").with_edge_type(EdgeType::SmoothStep));
    // 结束
    doc.add_edge(EdgeDef::new(loop_node, summarize).with_source_port("done").with_edge_type(EdgeType::SmoothStep));
    doc.add_edge(EdgeDef::new(summarize, end).with_edge_type(EdgeType::SmoothStep));

    doc
}

/// 数据处理管道：8 节点，数据清洗 → 分流 → 处理 → 汇合。
fn data_pipeline_doc() -> FlowDocument {
    let mut doc = FlowDocument::new("Data Pipeline")
        .with_description("数据处理管道：清洗 → 分流 → 并行处理 → 汇合");

    let start = doc.add_node(NodeDef::new("start", serde_json::json!({
        "label": "Source",
        "params": [{ "name": "source", "type": "string", "value": "db" }]
    })).with_size(SizeF::new(160.0, 56.0)).with_position(PointF::new(80.0, 280.0)));

    let clean = doc.add_node(NodeDef::new("adapter", serde_json::json!({
        "label": "Clean", "desc": "数据清洗"
    })).with_size(SizeF::new(200.0, 64.0)).with_position(PointF::new(300.0, 280.0)));

    let vars = doc.add_node(NodeDef::new("variable", serde_json::json!({
        "label": "Config",
        "variables": [
            { "name": "batch_size", "type": "int", "value": "100" },
            { "name": "timeout", "type": "int", "value": "30" }
        ]
    })).with_size(SizeF::new(200.0, 64.0)).with_position(PointF::new(560.0, 200.0)));

    let condition = doc.add_node(NodeDef::new("condition", serde_json::json!({
        "label": "Route",
        "conditions": [{ "id": "if_0", "label": "data.size > 1000" }]
    })).with_size(SizeF::new(220.0, 108.0)).with_position(PointF::new(560.0, 320.0)));

    let batch = doc.add_node(NodeDef::new("action", serde_json::json!({
        "label": "Batch Process", "desc": "批量处理"
    })).with_size(SizeF::new(180.0, 35.0)).with_position(PointF::new(860.0, 280.0)));

    let single = doc.add_node(NodeDef::new("action", serde_json::json!({
        "label": "Single Process", "desc": "单条处理"
    })).with_size(SizeF::new(180.0, 35.0)).with_position(PointF::new(860.0, 360.0)));

    let merge = doc.add_node(NodeDef::new("adapter", serde_json::json!({
        "label": "Merge", "desc": "结果合并"
    })).with_size(SizeF::new(200.0, 64.0)).with_position(PointF::new(1100.0, 320.0)));

    let end = doc.add_node(NodeDef::new("end", serde_json::json!({
        "label": "Sink",
        "returns": [{ "name": "count", "type": "int", "value": "0" }]
    })).with_size(SizeF::new(160.0, 56.0)).with_position(PointF::new(1380.0, 320.0)));

    doc.add_edge(EdgeDef::new(start, clean).with_edge_type(EdgeType::SmoothStep));
    doc.add_edge(EdgeDef::new(clean, vars).with_edge_type(EdgeType::SmoothStep));
    doc.add_edge(EdgeDef::new(clean, condition).with_target_port("in").with_edge_type(EdgeType::SmoothStep));
    doc.add_edge(EdgeDef::new(condition, batch).with_source_port("if_0").with_edge_type(EdgeType::SmoothStep));
    doc.add_edge(EdgeDef::new(condition, single).with_source_port("else").with_edge_type(EdgeType::SmoothStep));
    doc.add_edge(EdgeDef::new(batch, merge).with_edge_type(EdgeType::SmoothStep));
    doc.add_edge(EdgeDef::new(single, merge).with_edge_type(EdgeType::SmoothStep));
    doc.add_edge(EdgeDef::new(merge, end).with_edge_type(EdgeType::SmoothStep));

    doc
}

/// 简单线性流程：4 节点。
fn simple_flow_doc() -> FlowDocument {
    let mut doc = FlowDocument::new("Simple Flow")
        .with_description("简单线性流程：开始 → 处理 → 结束");

    let start = doc.add_node(NodeDef::new("start", serde_json::json!({
        "label": "Start"
    })).with_size(SizeF::new(160.0, 56.0)).with_position(PointF::new(100.0, 200.0)));

    let action1 = doc.add_node(NodeDef::new("action", serde_json::json!({
        "label": "Step 1", "desc": "第一步"
    })).with_size(SizeF::new(180.0, 35.0)).with_position(PointF::new(340.0, 200.0)));

    let action2 = doc.add_node(NodeDef::new("action", serde_json::json!({
        "label": "Step 2", "desc": "第二步"
    })).with_size(SizeF::new(180.0, 35.0)).with_position(PointF::new(600.0, 200.0)));

    let end = doc.add_node(NodeDef::new("end", serde_json::json!({
        "label": "End"
    })).with_size(SizeF::new(160.0, 56.0)).with_position(PointF::new(860.0, 200.0)));

    doc.add_edge(EdgeDef::new(start, action1).with_edge_type(EdgeType::SmoothStep));
    doc.add_edge(EdgeDef::new(action1, action2).with_edge_type(EdgeType::SmoothStep));
    doc.add_edge(EdgeDef::new(action2, end).with_edge_type(EdgeType::SmoothStep));

    doc
}
