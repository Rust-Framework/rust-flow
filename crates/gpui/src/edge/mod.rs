//! Edge 模块：边渲染组件，支持 4 种连线算法 + 箭头 + Loop 回环。

mod edge_view;

pub use edge_view::EdgeView;
pub(crate) use edge_view::{paint_edge_routed, paint_edge_scaled, paint_loop_back_edge};
