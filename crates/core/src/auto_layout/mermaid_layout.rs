//! Mermaid `graph TB` / `flowchart` layout — Dagre defaults + feedback-edge semantics.

use std::collections::{HashMap, HashSet, VecDeque};

use dagre::graph::{Graph, GraphOptions};
use dagre::{
    layout, Acyclicer, EdgeLabel, LayoutOptions as DagreLayoutOptions, NodeLabel, RankDir, Ranker,
};

use crate::auto_layout::dagre_layout::{layout_loop_body_regions, schema_key};
use crate::orientation::all_loop_body_nodes;
use crate::auto_layout::options::{LayoutDirection, LayoutOptions};
use crate::dagre_route::DagreEdgeRoute;
use crate::geometry::normalize_dagre_polyline;
use crate::graph::FlowGraph;
use crate::id::NodeId;
use crate::math::Point;
use crate::node_type::LOOP;

const MAIN_EDGE_WEIGHT: i32 = 4;
const FEEDBACK_EDGE_WEIGHT: i32 = 1;

/// Layout like Mermaid `dagre-wrapper`: single Dagre pass, `edge.points` for routing.
pub fn layout_graph_mermaid(graph: &mut FlowGraph, options: &LayoutOptions) {
    if graph.nodes.is_empty() {
        return;
    }

    graph.dagre_edge_routes.clear();

    let loop_body = all_loop_body_nodes(graph);
    let feedback = detect_feedback_edges(graph, &loop_body);

    layout_mermaid_main(graph, options, &loop_body, &feedback);
    layout_loop_body_regions(graph, options);
}

/// Edges that close a cycle (target can reach source without using this edge).
pub fn detect_feedback_edges(graph: &FlowGraph, skip_nodes: &HashSet<NodeId>) -> HashSet<(NodeId, NodeId)> {
    let mut feedback = HashSet::new();

    for edge in &graph.edges {
        let from_port = graph.ports.get(edge.from_port);
        let to_port = graph.ports.get(edge.to_port);
        if from_port.is_none() || to_port.is_none() {
            continue;
        }
        let from = from_port.unwrap().node;
        let to = to_port.unwrap().node;
        if from == to || skip_nodes.contains(&from) || skip_nodes.contains(&to) {
            continue;
        }
        if can_reach(graph, to, from, skip_nodes, edge.from_port, edge.to_port) {
            feedback.insert((from, to));
        }
    }

    feedback
}

fn can_reach(
    graph: &FlowGraph,
    start: NodeId,
    goal: NodeId,
    skip_nodes: &HashSet<NodeId>,
    blocked_from: crate::id::PortId,
    blocked_to: crate::id::PortId,
) -> bool {
    let mut seen = HashSet::from([start]);
    let mut queue = VecDeque::from([start]);

    while let Some(n) = queue.pop_front() {
        if n == goal {
            return true;
        }
        for e in &graph.edges {
            if e.from_port == blocked_from && e.to_port == blocked_to {
                continue;
            }
            let from_p = graph.ports.get(e.from_port);
            let to_p = graph.ports.get(e.to_port);
            if from_p.is_none() || to_p.is_none() {
                continue;
            }
            if from_p.unwrap().node != n {
                continue;
            }
            let next = to_p.unwrap().node;
            if skip_nodes.contains(&next) || seen.contains(&next) {
                continue;
            }
            seen.insert(next);
            queue.push_back(next);
        }
    }
    false
}

fn layout_mermaid_main(
    graph: &mut FlowGraph,
    options: &LayoutOptions,
    loop_body: &HashSet<NodeId>,
    feedback: &HashSet<(NodeId, NodeId)>,
) {
    let (mut g, key_to_id, sizes, edge_bindings) =
        build_dagre_graph(graph, loop_body, feedback);

    let rankdir = match options.direction {
        LayoutDirection::LeftRight => RankDir::LR,
        LayoutDirection::TopBottom => RankDir::TB,
    };

    layout(&mut g, Some(mermaid_dagre_options(options, rankdir)));

    for key in g.nodes() {
        let Some(id) = key_to_id.get(&key) else {
            continue;
        };
        let laid_out = g.node(&key).cloned();
        let (w, h) = sizes.get(&key).copied().unwrap_or((0.0, 0.0));
        if let Some(label) = laid_out {
            let cx = label.x.unwrap_or(0.0) as f32;
            let cy = label.y.unwrap_or(0.0) as f32;
            if let Some(node) = graph.nodes.get_mut(*id) {
                node.position = Point::new(cx - w / 2.0, cy - h / 2.0);
            }
        }
    }

    graph.dagre_edge_routes = extract_dagre_routes(&g, &edge_bindings);
}

fn extract_dagre_routes(
    g: &Graph<NodeLabel, EdgeLabel>,
    edge_bindings: &[Option<EdgeBinding>],
) -> Vec<Option<DagreEdgeRoute>> {
    let mut routes = vec![None; edge_bindings.len()];
    for (edge_idx, binding) in edge_bindings.iter().enumerate() {
        if let Some((from_key, to_key, name)) = binding {
            if let Some(label) = g.edge(from_key, to_key, name.as_deref()) {
                if label.points.is_empty() {
                    continue;
                }
                let points: Vec<Point> = label
                    .points
                    .iter()
                    .map(|p| Point::new(p.x as f32, p.y as f32))
                    .collect();
                let points = normalize_dagre_polyline(&points);
                let label_pos = label
                    .x
                    .zip(label.y)
                    .map(|(x, y)| Point::new(x as f32, y as f32));
                routes[edge_idx] = Some(DagreEdgeRoute {
                    points,
                    label_pos,
                });
            }
        }
    }
    routes
}

fn mermaid_dagre_options(options: &LayoutOptions, rankdir: RankDir) -> DagreLayoutOptions {
    DagreLayoutOptions {
        rankdir,
        align: None,
        nodesep: options.node_spacing as f64,
        ranksep: options.rank_spacing as f64,
        edgesep: 20.0,
        marginx: options.margin as f64,
        marginy: options.margin as f64,
        ranker: Ranker::NetworkSimplex,
        acyclicer: Some(Acyclicer::Greedy),
        ..Default::default()
    }
}

type EdgeBinding = (String, String, Option<String>);

fn build_dagre_graph(
    graph: &FlowGraph,
    loop_body: &HashSet<NodeId>,
    feedback: &HashSet<(NodeId, NodeId)>,
) -> (
    Graph<NodeLabel, EdgeLabel>,
    HashMap<String, NodeId>,
    HashMap<String, (f32, f32)>,
    Vec<Option<EdgeBinding>>,
) {
    let mut g: Graph<NodeLabel, EdgeLabel> = Graph::with_options(GraphOptions {
        directed: true,
        multigraph: true,
        compound: false,
    });

    let mut id_to_key: HashMap<NodeId, String> = HashMap::new();
    let mut key_to_id: HashMap<String, NodeId> = HashMap::new();
    let mut sizes: HashMap<String, (f32, f32)> = HashMap::new();

    for (id, node) in graph.nodes.iter() {
        if loop_body.contains(&id) || node.node_type == LOOP {
            continue;
        }
        let key = schema_key(id, node.schema_id.as_str());
        id_to_key.insert(id, key.clone());
        key_to_id.insert(key.clone(), id);
        sizes.insert(key.clone(), (node.size.width, node.size.height));

        let order = node
            .data
            .get("layout_order")
            .and_then(|v| v.as_u64())
            .map(|n| n as usize);

        g.set_node(
            &key,
            Some(NodeLabel {
                width: node.size.width as f64,
                height: node.size.height as f64,
                order,
                ..Default::default()
            }),
        );
    }

    let mut edge_names: HashMap<(String, String), u32> = HashMap::new();
    let mut edge_bindings: Vec<Option<EdgeBinding>> = vec![None; graph.edges.len()];

    for (edge_idx, edge) in graph.edges.iter().enumerate() {
        let from_port = graph.ports.get(edge.from_port);
        let to_port = graph.ports.get(edge.to_port);
        if from_port.is_none() || to_port.is_none() {
            continue;
        }
        let from_port = from_port.unwrap();
        let to_port = to_port.unwrap();
        if from_port.node == to_port.node {
            continue;
        }
        if loop_body.contains(&from_port.node) || loop_body.contains(&to_port.node) {
            continue;
        }

        let from_key = match id_to_key.get(&from_port.node) {
            Some(k) => k.clone(),
            None => continue,
        };
        let to_key = match id_to_key.get(&to_port.node) {
            Some(k) => k.clone(),
            None => continue,
        };

        let is_feedback = feedback.contains(&(from_port.node, to_port.node));
        let is_continue = to_port.name == "continue";
        let counter = edge_names.entry((from_key.clone(), to_key.clone())).or_insert(0);
        let name = if *counter == 0 {
            None
        } else {
            Some(format!("e{}", counter))
        };
        *counter += 1;

        let weight = if is_feedback || is_continue {
            FEEDBACK_EDGE_WEIGHT
        } else {
            MAIN_EDGE_WEIGHT
        };
        let minlen = if is_feedback || is_continue { 2 } else { 1 };

        g.set_edge(
            &from_key,
            &to_key,
            Some(EdgeLabel {
                minlen,
                weight,
                ..Default::default()
            }),
            name.as_deref(),
        );

        edge_bindings[edge_idx] = Some((from_key, to_key, name));
    }

    (g, key_to_id, sizes, edge_bindings)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::apply_flow_orientation;
    use crate::builtin_type_registry;
    use crate::mermaid_to_flow_document;
    use crate::FlowGraph;

    const ORCHESTRATOR: &str = r#"
graph TB
    U[用户任务] --> O[Orchestrator 主编排]
    O --> P[planner 规划]
    O --> E[explorer 探索]
    O --> CA[coder-alpha 并行 A]
    O --> CB[coder-beta 并行 B]
    O --> T[tester 验证]
    O --> R[reviewer 审查]
    T -->|FAIL| O
    R -->|阻塞项| O
    T -->|PASS| D[交付]
    R -->|通过| D
"#;

    fn layout_orchestrator() -> FlowGraph {
        let types = builtin_type_registry();
        let doc = mermaid_to_flow_document(ORCHESTRATOR).unwrap();
        let mut graph = FlowGraph::from_document(&doc, &types);
        apply_flow_orientation(&mut graph, LayoutDirection::TopBottom);
        layout_graph_mermaid(&mut graph, &LayoutOptions::mermaid_flowchart_tb());
        graph
    }

    fn pos(graph: &FlowGraph, id: &str) -> (f32, f32) {
        let n = graph
            .nodes
            .iter()
            .find(|(_, n)| n.schema_id == id)
            .map(|(_, n)| n)
            .unwrap();
        (
            n.position.x + n.size.width * 0.5,
            n.position.y + n.size.height * 0.5,
        )
    }

    #[test]
    fn mermaid_orchestrator_dagre_routes_populated() {
        let graph = layout_orchestrator();
        let populated = graph.dagre_edge_routes.iter().filter(|r| r.is_some()).count();
        assert!(populated >= 10, "expected dagre routes, got {populated}");
    }

    #[test]
    fn mermaid_orchestrator_feedback_detected() {
        let graph = layout_orchestrator();
        let feedback = detect_feedback_edges(&graph, &HashSet::new());
        let t = graph.nodes.iter().find(|(_, n)| n.schema_id == "T").map(|(id, _)| id).unwrap();
        let o = graph.nodes.iter().find(|(_, n)| n.schema_id == "O").map(|(id, _)| id).unwrap();
        let r = graph.nodes.iter().find(|(_, n)| n.schema_id == "R").map(|(id, _)| id).unwrap();
        assert!(feedback.contains(&(t, o)));
        assert!(feedback.contains(&(r, o)));
    }

    #[test]
    fn mermaid_orchestrator_no_node_overlap() {
        let graph = layout_orchestrator();
        let ids: Vec<NodeId> = graph.nodes.keys().collect();
        for i in 0..ids.len() {
            for j in (i + 1)..ids.len() {
                let a = graph.nodes.get(ids[i]).unwrap();
                let b = graph.nodes.get(ids[j]).unwrap();
                let overlap_x =
                    a.position.x < b.position.x + b.size.width
                        && b.position.x < a.position.x + a.size.width;
                let overlap_y =
                    a.position.y < b.position.y + b.size.height
                        && b.position.y < a.position.y + a.size.height;
                assert!(
                    !overlap_x || !overlap_y,
                    "nodes {:?} and {:?} overlap",
                    a.schema_id,
                    b.schema_id
                );
            }
        }
    }

    #[test]
    fn mermaid_orchestrator_fanout_below_orchestrator() {
        let graph = layout_orchestrator();
        let (_, oy) = pos(&graph, "O");
        for id in ["P", "E", "CA", "CB", "T", "R"] {
            let (_, cy) = pos(&graph, id);
            assert!(cy > oy, "{id} should be below O");
        }
    }

    #[test]
    fn mermaid_orchestrator_delivery_below_reviewers() {
        let graph = layout_orchestrator();
        let (_, dy) = pos(&graph, "D");
        let (_, ty) = pos(&graph, "T");
        let (_, ry) = pos(&graph, "R");
        assert!(dy > ty);
        assert!(dy > ry);
        let (dx, _) = pos(&graph, "D");
        let (tx, _) = pos(&graph, "T");
        let (rx, _) = pos(&graph, "R");
        let mid = (tx + rx) * 0.5;
        assert!((dx - mid).abs() < 40.0, "D at {dx} vs T/R mid {mid}");
    }
}
