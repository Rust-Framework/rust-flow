//! rust-agent-flow Demo：Agent 编排流程设计器示例。
//!
//! 运行：`cargo run -p rust-agent-flow-demo`
//!
//! 展示一个 Agent 编排场景，覆盖图灵完备控制流：
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

use gpui::AppContext;
use rust_agent_flow::{Edge, EdgeType, FlowGraph, PointF, SizeF};
use rust_agent_flow_gpui::FlowEditorView;

fn main() {
    gpui_platform::application().run(move |cx: &mut gpui::App| {
        rust_agent_flow_gpui::init(cx);

        cx.spawn(async move |cx| {
            cx.open_window(gpui::WindowOptions::default(), |window, cx| {
                let graph = build_agent_flow();
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

/// 构造一个 Agent 编排流程图，展现条件分支和循环编排。
///
/// ```text
/// Start → Planner → Condition ──if_0──→ Search ─────────┐
///                      │──if_1──→ Notify ──────────┐    │
///                      │                            │    │
///                      else                         │    │
///                      ↓                            │    │
///                   ToolCall ───────────────────────┼────┤
///                                                   │    │
///                                                   ↓    ↓
///                                               Loop ──loop_body──→ Process
///                                                │              ↗     │
///                                                Top    Bottom
///                                  loop_in ←─────────────────┘
///                                  done           (回环向下绕过)
///                                   ↓
///                                Summarize → End
/// ```
fn build_agent_flow() -> FlowGraph {
    let mut graph = FlowGraph::new();

    // 节点（尺寸与 schema default_size 对齐）
    let start = graph.add_node_with_size(
        "start",
        serde_json::json!({ "label": "Start" }),
        SizeF::new(120.0, 35.0),
    );
    let planner = graph.add_node_with_size(
        "action",
        serde_json::json!({ "label": "Planner", "desc": "规划下一步" }),
        SizeF::new(180.0, 35.0),
    );
    let condition = graph.add_node_with_size(
        "condition",
        serde_json::json!({
            "label": "Check",
            "conditions": [
                { "id": "if_0", "label": "amount > 100" },
                { "id": "if_1", "label": "user.is_admin" }
            ]
        }),
        SizeF::new(220.0, 144.0),
    );
    let search = graph.add_node_with_size(
        "action",
        serde_json::json!({ "label": "Search", "desc": "检索知识库" }),
        SizeF::new(180.0, 35.0),
    );
    let notify = graph.add_node_with_size(
        "action",
        serde_json::json!({ "label": "Notify", "desc": "发送通知" }),
        SizeF::new(180.0, 35.0),
    );
    let tool = graph.add_node_with_size(
        "action",
        serde_json::json!({ "label": "ToolCall", "desc": "调用外部工具" }),
        SizeF::new(180.0, 35.0),
    );
    let loop_node = graph.add_node_with_size(
        "loop",
        serde_json::json!({ "label": "Loop", "desc": "For each item" }),
        SizeF::new(220.0, 80.0),
    );
    let process = graph.add_node_with_size(
        "action",
        serde_json::json!({ "label": "Process", "desc": "处理当前项" }),
        SizeF::new(180.0, 35.0),
    );
    let summarize = graph.add_node_with_size(
        "action",
        serde_json::json!({ "label": "Summarize", "desc": "汇总结果" }),
        SizeF::new(180.0, 35.0),
    );
    let end = graph.add_node_with_size(
        "end",
        serde_json::json!({ "label": "End" }),
        SizeF::new(120.0, 35.0),
    );

    // 布局：左→右主流程对齐 y=280（端口 y≈298），分支目标紧凑排列
    //
    // 横向布局（默认）：
    //   Condition 出口端口 Y：else=298（标题栏中心）、if_0=343（第一条件行中心）、if_1=397（第二条件行中心）
    //   目标节点中心对齐出口端口 Y → 连线水平、紧凑（间隔 10~19px）
    //   出口顺序（上→下）：else → if_0 → if_1，目标节点同序排列，避免连线交叉
    //
    // 纵向布局：
    //   Condition 底部出口 X 顺序：else（最左）→ if_0 → if_1（最右）
    //   目标节点 Y 顺序（上→下）：tool(else) → search(if_0) → notify(if_1)
    //   Y 顺序与端口 X 顺序一致 → 连线不交叉
    set_position(&mut graph, start, 80.0, 280.0);
    set_position(&mut graph, planner, 320.0, 280.0);
    set_position(&mut graph, condition, 600.0, 280.0);
    set_position(&mut graph, tool, 880.0, 281.0);      // else 分支：中心 y=298.5 ≈ else 端口 298
    set_position(&mut graph, search, 880.0, 326.0);    // if_0 分支：中心 y=343.5 ≈ if_0 端口 343
    set_position(&mut graph, notify, 880.0, 380.0);    // if_1 分支：中心 y=397.5 ≈ if_1 端口 397
    set_position(&mut graph, loop_node, 1160.0, 280.0);
    set_position(&mut graph, process, 1440.0, 400.0);  // 循环体：主线下方，loop_body→process→loop_in 回环
    set_position(&mut graph, summarize, 1720.0, 280.0);
    set_position(&mut graph, end, 2000.0, 280.0);

    // 边：全部使用正交圆角折线（SmoothStep）
    // 主流程
    add_edge(&mut graph, start, planner, None, None, EdgeType::SmoothStep);
    add_edge(&mut graph, planner, condition, None, Some("in"), EdgeType::SmoothStep);

    // 条件分支：Condition 的 if_0 → Search，if_1 → Notify，else → ToolCall（全分支覆盖）
    add_edge(&mut graph, condition, search, Some("if_0"), None, EdgeType::SmoothStep);
    add_edge(&mut graph, condition, notify, Some("if_1"), None, EdgeType::SmoothStep);
    add_edge(&mut graph, condition, tool, Some("else"), None, EdgeType::SmoothStep);

    // 分支汇合：Search/Notify/ToolCall 都连到 Loop 的 in 端口（汇聚点）
    add_edge(&mut graph, search, loop_node, None, Some("in"), EdgeType::SmoothStep);
    add_edge(&mut graph, notify, loop_node, None, Some("in"), EdgeType::SmoothStep);
    add_edge(&mut graph, tool, loop_node, None, Some("in"), EdgeType::SmoothStep);

    // 循环体：Loop 的 loop_body → Process → 回连 Loop 的 loop_in
    add_edge(&mut graph, loop_node, process, Some("loop_body"), None, EdgeType::SmoothStep);
    add_edge(&mut graph, process, loop_node, None, Some("loop_in"), EdgeType::SmoothStep);

    // 循环结束：Loop.done → Summarize
    add_edge(&mut graph, loop_node, summarize, Some("done"), None, EdgeType::SmoothStep);
    add_edge(&mut graph, summarize, end, None, None, EdgeType::SmoothStep);

    graph
}

fn set_position(graph: &mut FlowGraph, id: rust_agent_flow::NodeId, x: f32, y: f32) {
    if let Some(node) = graph.node_mut(id) {
        node.position = PointF::new(x, y);
    }
}

fn add_edge(
    graph: &mut FlowGraph,
    source: rust_agent_flow::NodeId,
    target: rust_agent_flow::NodeId,
    source_port: Option<&str>,
    target_port: Option<&str>,
    edge_type: EdgeType,
) {
    let mut edge = Edge::new(source, target);
    edge.edge_type = edge_type;
    edge.source_port = source_port.map(|s| s.to_string());
    edge.target_port = target_port.map(|s| s.to_string());
    graph.add_edge(edge);
}
