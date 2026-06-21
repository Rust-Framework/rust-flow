//! Builtin 模块：内置节点实现。
//!
//! Phase 2 仅提供注册入口空壳，Phase 3 起逐步添加 Start/End/Action/Condition/Loop。

use crate::node::NodeRegistry;

/// 注册所有内置节点。
///
/// Phase 2：无内置节点（使用 NodeView 的 fallback 渲染）。
/// Phase 3：注册 Start/End/Action。
/// Phase 6：注册 Condition。
/// Phase 7：注册 Loop。
pub fn register_all(_registry: &mut NodeRegistry) {
    // Phase 3 起实现
}
