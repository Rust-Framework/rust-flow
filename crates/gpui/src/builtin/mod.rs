//! Builtin 模块：内置图灵完备控制流节点实现。
//!
//! 每个节点独立文件，共享辅助函数在 [`common`] 中：
//! - [`start`]: StartNode — 流程起点（仅 Out 端口）
//! - [`end`]: EndNode — 流程终点（仅 In 端口）
//! - [`action`]: ActionNode — 顺序执行步骤（In + Out）
//! - [`condition`]: ConditionNode — 条件分支（In + Out，出边可多条）
//! - [`loop_node`]: LoopNode — 循环体（In + Out，出边可回连）

use std::sync::Arc;

use crate::node::NodeRegistry;

mod action;
mod common;
mod condition;
mod end;
mod loop_node;
mod start;

pub use action::ActionNode;
pub use condition::ConditionNode;
pub use end::EndNode;
pub use loop_node::LoopNode;
pub use start::StartNode;

/// 注册所有内置节点。
pub fn register_all(registry: &mut NodeRegistry) {
    registry.register(Arc::new(StartNode::new()));
    registry.register(Arc::new(EndNode::new()));
    registry.register(Arc::new(ActionNode::new()));
    registry.register(Arc::new(ConditionNode::new()));
    registry.register(Arc::new(LoopNode::new()));
}
