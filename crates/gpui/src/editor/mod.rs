//! Editor 模块：FlowEditorView 主视图 + 视口交互 + 交互状态机。
//!
//! 按职责拆分到子模块：
//! - [`flow_editor`]：核心结构体 + 构造 + 布局 + Render 实现
//! - [`interaction`]：交互状态机 + 鼠标事件处理
//! - [`hit_test`]：命中测试
//! - [`rendering`]：边/节点/面板渲染
//! - [`toolbar`]：工具栏
//! - [`grid`]：点阵背景
//! - [`ports`]：端口位置计算
//! - [`viewport`]：视口数学映射

mod flow_editor;
mod grid;
mod hit_test;
mod interaction;
mod ports;
mod rendering;
mod toolbar;
mod viewport;

pub use flow_editor::{FlowEditorView, LayoutDirection};
pub use interaction::InteractionState;
