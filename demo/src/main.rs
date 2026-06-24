//! rust-agent-flow Demo：Agent 编排流程设计器示例。
//!
//! 运行：`cargo run -p rust-agent-flow-demo`
//!
//! 使用数据驱动方式加载预置流程（[`DemoDataSource`]），展示图灵完备控制流：
//! - Start → Planner（规划）→ Condition（条件判断）
//! - Condition 分支：if_0 → Search（检索），if_1 → Notify（通知），else → ToolCall（工具调用）
//! - 三路汇合到 Loop（循环）
//! - Loop 的 loop_body → Process（循环体）→ 回连 Loop 的 loop_in（循环回环）
//! - Loop 的 done → Summarize（汇总）→ End
//!
//! 交互：
//! - 中键拖拽：平移视口
//! - 滚轮：缩放（鼠标锚点）
//! - 左键拖拽节点：移动节点
//! - 左键从节点出端口拖到另一节点入端口：创建连线
//! - 点击节点：显示右侧属性面板
//! - 点击边中点「+」按钮：弹出节点选择面板，选择类型后插入到边中间

use std::sync::Arc;

use gpui::AppContext;
use rust_agent_flow_gpui::{CombinedAssets, FlowEditorView, SharedToolbarProvider};

mod data_sources;
mod toolbar_provider;

use data_sources::DemoDataSource;
use toolbar_provider::{AppControlsToolbar, DataSourceToolbar};

fn main() {
    gpui_platform::application()
        .with_assets(CombinedAssets)
        .run(move |cx: &mut gpui::App| {
            rust_agent_flow_gpui::init(cx);

            cx.spawn(async move |cx| {
                cx.open_window(gpui::WindowOptions::default(), |window, cx| {
                    // 数据驱动：从 DemoDataSource 加载预置流程图
                    let initial_ds = DemoDataSource::AgentFlow;
                    let graph = initial_ds.to_graph();
                    let view = cx.new(|cx| {
                        let mut editor = FlowEditorView::new(graph, cx);
                        // 使用 dagre 自动排版（ReactFlow 同款 Sugiyama 算法）
                        editor.auto_layout(cx);
                        // 注入数据源选择器工具栏扩展（调用侧自定义工具项）
                        let provider: SharedToolbarProvider =
                            Arc::new(DataSourceToolbar::new(initial_ds));
                        editor.add_toolbar_provider(provider, cx);
                        // 注入应用控件工具栏（拖拽开关/主题切换/语言切换）
                        // 这些控件跟随目标系统，由调用侧决定如何呈现
                        let app_controls: SharedToolbarProvider =
                            Arc::new(AppControlsToolbar::new());
                        editor.add_toolbar_provider(app_controls, cx);
                        editor
                    });
                    cx.new(|cx| gpui_component::Root::new(view, window, cx))
                })
                .expect("Failed to open window");
            })
            .detach();
        });
}
