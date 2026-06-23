//! rust-agent-flow Demo：Agent 编排流程设计器示例。
//!
//! 运行：`cargo run -p rust-agent-flow-demo`
//!
//! 使用数据驱动方式加载预置流程（[`DataSource`]），展示图灵完备控制流：
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

use gpui::AppContext;
use rust_agent_flow_gpui::{CombinedAssets, DataSource, FlowEditorView};

fn main() {
    gpui_platform::application()
        .with_assets(CombinedAssets)
        .run(move |cx: &mut gpui::App| {
            rust_agent_flow_gpui::init(cx);

            cx.spawn(async move |cx| {
                cx.open_window(gpui::WindowOptions::default(), |window, cx| {
                    // 数据驱动：从 DataSource 加载预置流程图
                    let graph = DataSource::AgentFlow.to_graph();
                    let view = cx.new(|cx| {
                        let mut editor = FlowEditorView::new(graph, cx);
                        // 使用 dagre 自动排版（ReactFlow 同款 Sugiyama 算法）
                        editor.auto_layout(cx);
                        editor
                    });
                    cx.new(|cx| gpui_component::Root::new(view, window, cx))
                })
                .expect("Failed to open window");
            })
            .detach();
        });
}
