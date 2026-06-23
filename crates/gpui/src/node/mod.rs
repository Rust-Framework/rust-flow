//! Node 模块：IFlowNode 扩展接口 + NodeRegistry 注册表 + NodeView 渲染 + 语法高亮服务。

mod flow_node;
mod registry;
mod syntax;
mod view;

pub use flow_node::{ActionCallback, IFlowNode, NodeAction, NodeViewCtx};
pub use registry::NodeRegistry;
pub use syntax::{default_syntax_service, DefaultSyntaxService, SharedSyntaxService, SyntaxService};
pub use view::{render_node_card, NodeView, NodeVisual};
