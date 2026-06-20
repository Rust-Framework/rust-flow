pub mod auto_layout;
pub mod dagre_route;
pub mod edge;
pub mod geometry;
pub mod graph;
pub mod id;
pub mod invariants;
pub mod layout;
pub mod math;
pub mod node;
pub mod node_layout;
pub mod node_sync;
pub mod node_type;
pub mod orientation;
pub mod port;
pub mod scene;
pub mod schema;
pub mod viewport;

pub use auto_layout::{layout_graph, LayoutDirection, LayoutOptions};
pub use edge::{EdgeShape, EdgeStroke, FlowEdge};
pub use geometry::{
    build_edge_path, bezier_control_points, catmull_rom_segments, edge_path_endpoints,
    edge_stroke_dash, get_bezier_path, get_bezier_edge_center, get_edge_center,
    get_smooth_step_path, get_smooth_step_label_center, get_step_points, handle_position,
    hit_node_at, hit_port_at, BezierPath, CatmullSegment, EdgePath, SmoothStepSegment,
    PORT_HIT_RADIUS, DEFAULT_BORDER_RADIUS, DEFAULT_CURVATURE, DEFAULT_OFFSET,
};
pub use invariants::{check_edge, check_frame, check_node_anchors, InvariantViolation, EPS};
pub use layout::{
    handle_dot_origin, scaled, ACCENT_H, DOT_R, HANDLE_R, MIN_W as NODE_MIN_W, MIN_SCREEN_H,
    MIN_SCREEN_W, NODE_PAD, TITLE_H, VISUAL_HEIGHT,
};
pub use node::{FlowNode, ResolvedNode};
pub use node_layout::{
    branch_collapsed, branch_node_size, common_node_size, mindmap_node_size, parse_branch_items,
    BranchItem,
    BRANCH_COLLAPSED_H, BRANCH_HEADER, BRANCH_PAD, BRANCH_ROW, BRANCH_WIDTH, COMMON_HEIGHT,
    COMMON_WIDTH, HTTP_HEIGHT, LOOP_BODY_ZONE, LOOP_FOOTER, LOOP_HEADER, LOOP_HEIGHT, LOOP_WIDTH,
    TRIGGER_HEIGHT,
};
pub use schema::{
    apply_template, builtin_type_registry, demo_document, demo_document_json,
    demo_graph_from_document, mindmap_document_json, document_from_graph, document_needs_layout,
    document_to_graph, graph_from_document, graph_to_document, viewport_from_document,
    viewport_to_document, mermaid_layout_direction, mermaid_to_flow_document, FlowDocument,
    FlowDocumentEdge, FlowDocumentNode, FlowFieldDef, FlowFieldType, FlowNodeTypeDef,
    FlowPortDef, FlowRenderDef, FlowTypeRegistry, FLOW_DOCUMENT_VERSION,
    FLOW_TYPE_REGISTRY_VERSION, MindMapDocument, MindMapFlatNode, MindMapNode,
    MINDMAP_DOCUMENT_VERSION, document_from_any_json, is_mindmap_json,
    mindmap_layout_direction_from_json, mindmap_to_flow_document,
};
pub use node_sync::{sync_all_structured_nodes, sync_structured_node, toggle_branch_collapsed};
pub use orientation::{
    all_loop_body_nodes, apply_flow_orientation, collect_loop_body_nodes, flow_input_side,
    flow_output_side, loop_container_overlap_allowed,
};
pub use node_type::{BRANCH, COMMON, HTTP, LOOP, TRIGGER};
pub use scene::{ResolvedEdge, SceneFrame};
pub use graph::{demo_chain_graph, FlowGraph, GraphStats};
pub use id::{EdgeId, NodeId, PortId};
pub use math::{Point, Size};
pub use port::{FlowPort, PortDirection, PortSide};
pub use viewport::Viewport;
