//! Minimal demo：展示 FlowEditorView 基本功能。
//!
//! 运行：`cargo run -p rust-agent-flow-gpui --example minimal_demo --features demo`
//!
//! 功能：
//! - 显示 5 个节点 + 4 条边（分别使用 4 种连线算法）
//! - 中键拖拽平移视口
//! - 滚轮缩放（鼠标锚点）
//! - 左键拖拽节点
//! - 左键从节点右侧拖拽到另一节点左侧创建连线
//! - 点击节点显示右侧属性面板

use gpui::AppContext;
use rust_agent_flow::{Edge, EdgeType, FlowGraph, PointF};
use rust_agent_flow_gpui::FlowEditorView;

fn main() {
    gpui_platform::application().run(move |cx: &mut gpui::App| {
        rust_agent_flow_gpui::init(cx);

        cx.spawn(async move |cx| {
            cx.open_window(gpui::WindowOptions::default(), |window, cx| {
                let mut graph = FlowGraph::new();
                let n1 = graph.add_node("start", serde_json::json!({}));
                let n2 = graph.add_node("action", serde_json::json!({ "name": "Bezier" }));
                let n3 = graph.add_node("action", serde_json::json!({ "name": "Straight" }));
                let n4 = graph.add_node("action", serde_json::json!({ "name": "Step" }));
                let n5 = graph.add_node("end", serde_json::json!({}));

                // 4 条边分别使用 4 种连线算法
                let mut e1 = Edge::new(n1, n2);
                e1.edge_type = EdgeType::Bezier;
                graph.add_edge(e1);

                let mut e2 = Edge::new(n2, n3);
                e2.edge_type = EdgeType::Straight;
                graph.add_edge(e2);

                let mut e3 = Edge::new(n3, n4);
                e3.edge_type = EdgeType::Step;
                graph.add_edge(e3);

                let mut e4 = Edge::new(n4, n5);
                e4.edge_type = EdgeType::SmoothStep;
                graph.add_edge(e4);

                // 水平排列避免重叠
                graph.node_mut(n1).unwrap().position = PointF::new(80.0, 150.0);
                graph.node_mut(n2).unwrap().position = PointF::new(320.0, 150.0);
                graph.node_mut(n3).unwrap().position = PointF::new(560.0, 150.0);
                graph.node_mut(n4).unwrap().position = PointF::new(800.0, 150.0);
                graph.node_mut(n5).unwrap().position = PointF::new(1040.0, 150.0);

                let view = cx.new(|cx| FlowEditorView::new(graph, cx));
                cx.new(|cx| gpui_component::Root::new(view, window, cx))
            })
            .expect("Failed to open window");
        })
        .detach();
    });
}
