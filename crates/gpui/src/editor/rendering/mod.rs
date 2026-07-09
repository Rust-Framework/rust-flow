//! 渲染层：边、节点、属性面板的渲染方法。
//!
//! ## 模块结构
//!
//! | 子模块 | 职责 | 稳定性 |
//! |--------|------|--------|
//! | [`edge_geometry`] | 边端点计算、Loop 组合边界、渲染指令枚举 | **稳定** — 几何/拓扑计算，被 `hit_test` 跨模块复用 |
//! | [`edges`] | 边 canvas paint + 「+」按钮 + tooltip | 易变 — paint 回调、视觉样式 |
//! | [`nodes`] | 节点 absolute div 渲染 | 易变 — 节点视觉外观 |
//! | [`panel`] | 属性面板生命周期同步 | 易变 — 面板功能演进 |
//!
//! ## Loop 循环体特殊处理
//!
//! 循环体节点（从 `loop_body` 出口可达的节点）始终使用**纵向端口**
//!（上进下出），无论主布局方向是横向还是纵向。回环边（目标端口为
//! `loop_in`）使用 `loop_back_path` 向下绕过 Loop 节点 + 循环体的组合边界。

mod edge_geometry;
mod edges;
mod nodes;
mod panel;

// Re-export stable geometry API for hit_test.rs / routing.rs
// (super::rendering::compute_edge_endpoints)
pub(crate) use edge_geometry::compute_edge_endpoints;

use super::flow_editor::FlowEditorView;

impl FlowEditorView {
    /// 当前视口缩放比例。
    pub(crate) fn scale(&self) -> f32 {
        self.viewport.scale
    }
}
