//! Demo 数据源：预置流程示例。
//!
//! 每个数据源对应一个 JSON 文件（[`FlowDocument`] 协议），通过
//! [`FlowGraph::from_document`] 转换为可编辑的流程图。
//!
//! 数据驱动设计：流程定义以 JSON 文件形式存储在 `demo/data/` 目录下，
//! 编译时通过 `include_str!` 嵌入二进制，运行时直接反序列化加载。
//! 节点/边定义与渲染逻辑完全解耦，新增流程只需添加 JSON 文件。

use rust_agent_flow::{FlowDocument, FlowGraph};
use rust_agent_flow_gpui::{Language, TKey, t};

/// Agent 编排流程 JSON（编译时嵌入）。
const AGENT_FLOW_JSON: &str = include_str!("../data/agent_flow.json");
/// 数据处理管道 JSON（编译时嵌入）。
const DATA_PIPELINE_JSON: &str = include_str!("../data/data_pipeline.json");
/// 简单线性流程 JSON（编译时嵌入）。
const SIMPLE_FLOW_JSON: &str = include_str!("../data/simple_flow.json");

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
    /// 返回数据源对应的 FlowGraph（从 JSON 反序列化后转换）。
    pub fn to_graph(&self) -> FlowGraph {
        FlowGraph::from_document(&self.to_document())
    }

    /// 从 JSON 反序列化为 FlowDocument。
    ///
    /// JSON 解析失败时 panic（编译时嵌入的静态数据，不应出错）。
    pub fn to_document(&self) -> FlowDocument {
        let json = match self {
            Self::AgentFlow => AGENT_FLOW_JSON,
            Self::DataPipeline => DATA_PIPELINE_JSON,
            Self::SimpleFlow => SIMPLE_FLOW_JSON,
        };
        serde_json::from_str(json).expect("内置 JSON 数据解析失败")
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
