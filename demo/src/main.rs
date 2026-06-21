//! rust-agent-flow Demo：Agent 编排流程设计器示例。
//!
//! 运行：`cargo run -p rust-agent-flow-demo`
//!
//! 展示一个 Agent 编排场景：
//! - Start → Planner（规划）→ 分支到 Search / ToolCall 两个 Action
//! - 两条分支汇合到 Summarize → End
//! - 4 种连线算法分别在不同边上展示
//!
//! 交互：
//! - 中键拖拽：平移视口
//! - 滚轮：缩放（鼠标锚点）
//! - 左键拖拽节点：移动节点
//! - 左键从节点右侧拖到另一节点左侧：创建连线
//! - 点击节点：显示右侧属性面板

use gpui::AppContext;
use rust_agent_flow::{Edge, EdgeType, FlowGraph, PointF};
use rust_agent_flow_gpui::FlowEditorView;

fn main() {
    gpui_platform::application().run(move |cx: &mut gpui::App| {
        rust_agent_flow_gpui::init(cx);

        cx.spawn(async move |cx| {
            cx.open_window(gpui::WindowOptions::default(), |window, cx| {
                let graph = build_agent_flow();
                let view = cx.new(|cx| FlowEditorView::new(graph, cx));
                cx.new(|cx| gpui_component::Root::new(view, window, cx))
            })
            .expect("Failed to open window");
        })
        .detach();
    });
}

/// 构造一个 Agent 编排流程图。
fn build_agent_flow() -> FlowGraph {
    let mut graph = FlowGraph::new();

    // 节点
    let start = graph.add_node("start", serde_json::json!({ "label": "Start" }));
    let planner = graph.add_node(
        "action",
        serde_json::json!({ "label": "Planner", "desc": "规划下一步动作" }),
    );
    let search = graph.add_node(
        "action",
        serde_json::json!({ "label": "Search", "desc": "检索知识库" }),
    );
    let tool = graph.add_node(
        "action",
        serde_json::json!({ "label": "ToolCall", "desc": "调用外部工具" }),
    );
    let summarize = graph.add_node(
        "action",
        serde_json::json!({ "label": "Summarize", "desc": "汇总结果" }),
    );
    let end = graph.add_node("end", serde_json::json!({ "label": "End" }));

    // 布局：左→右，分支上下展开
    set_position(&mut graph, start, 80.0, 200.0);
    set_position(&mut graph, planner, 320.0, 200.0);
    set_position(&mut graph, search, 600.0, 80.0);
    set_position(&mut graph, tool, 600.0, 320.0);
    set_position(&mut graph, summarize, 880.0, 200.0);
    set_position(&mut graph, end, 1160.0, 200.0);

    // 边：4 种算法各展示一次
    add_edge(&mut graph, start, planner, EdgeType::Bezier);
    add_edge(&mut graph, planner, search, EdgeType::SmoothStep);
    add_edge(&mut graph, planner, tool, EdgeType::SmoothStep);
    add_edge(&mut graph, search, summarize, EdgeType::Step);
    add_edge(&mut graph, tool, summarize, EdgeType::Step);
    add_edge(&mut graph, summarize, end, EdgeType::Straight);

    graph
}

fn set_position(graph: &mut FlowGraph, id: rust_agent_flow::NodeId, x: f32, y: f32) {
    if let Some(node) = graph.node_mut(id) {
        node.position = PointF::new(x, y);
    }
}

fn add_edge(graph: &mut FlowGraph, source: rust_agent_flow::NodeId, target: rust_agent_flow::NodeId, edge_type: EdgeType) {
    let mut edge = Edge::new(source, target);
    edge.edge_type = edge_type;
    graph.add_edge(edge);
}
