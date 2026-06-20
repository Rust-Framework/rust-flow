//! Integration regression tests - run on every change to nodes, ports, edges, or viewport.
//!
//! ```bash
//! cargo test -p rust-agent-flow regression
//! cargo test --workspace
//! ```

use rust_agent_flow::{
    build_edge_path, check_frame, demo_chain_graph, edge_path_endpoints, FlowEdge, FlowGraph,
    FlowNode, NodeId, Point, PortDirection, PortSide, SceneFrame, Viewport, EPS,
};

const PAN_ZOOM_CASES: &[(f32, f32, f32)] = &[
    (1.0, 0.0, 0.0),
    (1.5, 40.0, 24.0),
    (0.75, -30.0, 18.0),
    (2.0, 100.0, 80.0),
    (0.1, 0.0, 0.0),
    (0.05, 20.0, 10.0),
];

#[test]
fn regression_demo_frame_invariants() {
    let graph = demo_chain_graph();
    for &(zoom, pan_x, pan_y) in PAN_ZOOM_CASES {
        let mut vp = Viewport::new(zoom);
        vp.pan = Point::new(pan_x, pan_y);
        let frame = SceneFrame::resolve(&graph, &vp);
        check_frame(&frame).unwrap_or_else(|e| {
            panic!("demo invariants failed at zoom={zoom} pan=({pan_x},{pan_y}): {}", e.message)
        });
    }
}

#[test]
fn regression_edges_follow_moved_nodes() {
    let mut graph = demo_chain_graph();
    let process = graph
        .nodes
        .iter()
        .find(|(_, n)| n.schema_id == "deduct_stock")
        .map(|(id, _)| id)
        .unwrap();
    graph.nodes.get_mut(process).unwrap().position = Point::new(320.0, 280.0);

    let frame = SceneFrame::resolve(&graph, &Viewport::default());
    check_frame(&frame).expect("moved node frame");
    assert!(frame.edges.len() >= 2);
}

#[test]
fn regression_all_edge_shapes_anchor_to_handles() {
    let mut graph = FlowGraph::new("shapes");
    let n1 = graph.add_node(FlowNode::new(NodeId::default(), "A", Point::new(0.0, 0.0)));
    let n2 = graph.add_node(FlowNode::new(NodeId::default(), "B", Point::new(300.0, 100.0)));
    let out = graph
        .add_port(n1, "out", PortDirection::Output, PortSide::Right)
        .unwrap();
    let inp = graph
        .add_port(n2, "in", PortDirection::Input, PortSide::Left)
        .unwrap();

    let frame = SceneFrame::resolve(&graph, &Viewport::default());
    let a = frame.nodes[0].port_anchors[&out];
    let b = frame.nodes[1].port_anchors[&inp];

    for shape in [
        rust_agent_flow::EdgeShape::SmoothStep,
        rust_agent_flow::EdgeShape::Bezier,
        rust_agent_flow::EdgeShape::Straight,
        rust_agent_flow::EdgeShape::Natural,
    ] {
        let path = build_edge_path(a, PortSide::Right, b, PortSide::Left, shape);
        let (start, end) = edge_path_endpoints(&path);
        assert!((start.x - a.x).abs() < EPS, "shape {shape:?} start.x");
        assert!((start.y - a.y).abs() < EPS, "shape {shape:?} start.y");
        assert!((end.x - b.x).abs() < EPS, "shape {shape:?} end.x");
        assert!((end.y - b.y).abs() < EPS, "shape {shape:?} end.y");
    }

    graph.add_edge(FlowEdge::new(out, inp));
    check_frame(&SceneFrame::resolve(&graph, &Viewport::default())).expect("resolved edge");
}

#[test]
fn regression_handle_dot_origin_on_border() {
    use rust_agent_flow::{handle_dot_origin, handle_position, Size, HANDLE_R};

    let size = Size::new(200.0, 41.0);
    for side in [
        PortSide::Left,
        PortSide::Right,
        PortSide::Top,
        PortSide::Bottom,
    ] {
        let center = handle_position(Point::default(), size, side, 0, 1);
        let origin = handle_dot_origin(center, HANDLE_R);
        match side {
            PortSide::Left => assert!(origin.x < 0.0),
            PortSide::Right => assert!(origin.x + HANDLE_R * 2.0 > size.width),
            PortSide::Top => assert!(origin.y < 0.0),
            PortSide::Bottom => assert!(origin.y + HANDLE_R * 2.0 > size.height),
        }
        assert!((origin.x + HANDLE_R - center.x).abs() < 0.01);
        assert!((origin.y + HANDLE_R - center.y).abs() < 0.01);
    }
}
