//! Mermaid-style dagre layout engine — a from-scratch Rust port of the
//! `@dagrejs/dagre` 27-step pipeline used by Mermaid flowcharts.
//!
//! This module is self-contained and intentionally independent of the legacy
//! [`crate::auto_layout::layered`] / [`crate::auto_layout::mermaid_layout`]
//! designs. It implements the full pipeline:
//!
//! 1. Build a [`LayoutGraph`] from [`FlowGraph`] (nodes + port edges).
//! 2. `removeSelfEdges` — detach self-loops for later routing.
//! 3. `acyclic.run` — DFS Feedback Arc Set cycle breaking (Mermaid default).
//! 4. `rank` — longest-path + network-simplex rank assignment.
//! 5. `normalize.run` — split long edges with dummy nodes.
//! 6. `order` — barycenter crossing reduction (24 sweeps, keep-best).
//! 7. `position` — Brandes-Köpf coordinate assignment + rank-based Y.
//! 8. `normalize.undo` — collapse dummy chains into `edge.points[]`.
//! 9. `assignNodeIntersects` — clip endpoints to node rectangles.
//! 10. `positionSelfEdges` — route self-loops.
//! 11. `acyclic.undo` — restore edge direction, flip reversed point lists.
//! 12. `adjustCoordinateSystem` — swap X/Y for LR layout.
//! 13. Write node positions (center → top-left) + orthogonal routes back.
//!
//! The result is Mermaid-quality orthogonal edge routing without depending on
//! the external `dagre` crate.

pub mod acyclic;
pub mod graph;
pub mod order;
pub mod position;
pub mod rank;
pub mod route;

use std::collections::{HashMap, HashSet};

use graph::LayoutGraph;

use crate::auto_layout::dagre_layout::layout_loop_body_regions;
use crate::auto_layout::options::{LayoutDirection, LayoutOptions};
use crate::auto_layout::mermaid_layout::detect_feedback_edges;
use crate::dagre_route::DagreEdgeRoute;
use crate::geometry::normalize_dagre_polyline;
use crate::graph::FlowGraph;
use crate::id::NodeId;
use crate::math::Point;
use crate::node_type::LOOP;
use crate::orientation::all_loop_body_nodes;

const MAIN_EDGE_WEIGHT: f32 = 4.0;
const FEEDBACK_EDGE_WEIGHT: f32 = 1.0;

/// Configuration for the Mermaid-style layout.
#[derive(Debug, Clone)]
pub struct MermaidLayoutConfig {
    pub direction: LayoutDirection,
    pub nodesep: f32,
    pub ranksep: f32,
    pub edgesep: f32,
    pub marginx: f32,
    pub marginy: f32,
    pub ranker: rank::Ranker,
    pub acyclicer: acyclic::Acyclicer,
    pub align: position::Align,
}

impl Default for MermaidLayoutConfig {
    fn default() -> Self {
        Self::mermaid_flowchart_tb()
    }
}

impl MermaidLayoutConfig {
    /// Mermaid flowchart defaults: `nodeSpacing`/`rankSpacing` 50, margin 8,
    /// edgesep 20, network-simplex, DFS FAS, balanced BK.
    pub fn mermaid_flowchart_tb() -> Self {
        Self {
            direction: LayoutDirection::TopBottom,
            nodesep: 50.0,
            ranksep: 50.0,
            edgesep: 20.0,
            marginx: 8.0,
            marginy: 8.0,
            ranker: rank::Ranker::NetworkSimplex,
            acyclicer: acyclic::Acyclicer::DfsFas,
            align: position::Align::Balance,
        }
    }

    pub fn mermaid_flowchart_lr() -> Self {
        Self {
            direction: LayoutDirection::LeftRight,
            ..Self::mermaid_flowchart_tb()
        }
    }

    /// Build a config from the legacy [`LayoutOptions`] (used to plug into the
    /// existing `FlowGraph::auto_layout_mermaid` entry point).
    pub fn from_options(options: &LayoutOptions) -> Self {
        let base = match options.direction {
            LayoutDirection::TopBottom => Self::mermaid_flowchart_tb(),
            LayoutDirection::LeftRight => Self::mermaid_flowchart_lr(),
        };
        Self {
            nodesep: options.node_spacing,
            ranksep: options.rank_spacing,
            marginx: options.margin,
            marginy: options.margin,
            ..base
        }
    }
}

/// Run the Mermaid-style dagre pipeline on `graph` and write positions + routes.
///
/// Mirrors [`crate::auto_layout::mermaid_layout::layout_graph_mermaid`] but
/// uses the from-scratch engine in this module instead of the `dagre` crate.
pub fn layout_graph_mermaid_v2(graph: &mut FlowGraph, options: &LayoutOptions) {
    if graph.nodes.is_empty() {
        return;
    }

    graph.dagre_edge_routes.clear();

    let loop_body = all_loop_body_nodes(graph);
    let feedback = detect_feedback_edges(graph, &loop_body);

    let config = MermaidLayoutConfig::from_options(options);

    layout_main(graph, &config, &loop_body, &feedback);
    layout_loop_body_regions(graph, options);
}

fn layout_main(
    graph: &mut FlowGraph,
    config: &MermaidLayoutConfig,
    loop_body: &HashSet<NodeId>,
    feedback: &HashSet<(NodeId, NodeId)>,
) {
    // 1. Build the layout graph.
    let (mut lg, id_to_layout_idx, edge_bindings) =
        build_layout_graph(graph, loop_body, feedback, config);

    // 2. Remove self-edges (stash indices for self-loop routing).
    let self_edges = remove_self_edges(&mut lg);

    // 3. Cycle breaking.
    let reversed = acyclic::run(&mut lg, config.acyclicer);

    // 4. Rank assignment.
    rank::run(&mut lg, config.ranker);

    // 5. Normalize long edges (insert dummies).
    route::normalize(&mut lg);

    // 6. Crossing reduction.
    order::run(&mut lg);

    // 7. Coordinate assignment.
    position::run(&mut lg, config.align);

    // 8. Collapse dummy chains → edge.points[].
    route::undo_normalize(&mut lg);

    // 9. Clip endpoints to node rectangles.
    route::assign_node_intersects(&mut lg);

    // 10. Route self-loops.
    route_self_edges(&mut lg, &self_edges);

    // 11. Restore edge direction (flip reversed point lists).
    acyclic::undo(&mut lg, &reversed);

    // 12. Adjust coordinate system for LR (swap X/Y).
    if config.direction == LayoutDirection::LeftRight {
        swap_xy(&mut lg);
    }

    // 13. Write back to FlowGraph.
    write_back(graph, &lg, &id_to_layout_idx, &edge_bindings);
}

type EdgeBinding = (usize, usize, Option<String>);

/// Build the [`LayoutGraph`] from [`FlowGraph`], skipping loop-body nodes and
/// loop containers (mirroring the legacy `build_dagre_graph`).
fn build_layout_graph(
    graph: &FlowGraph,
    loop_body: &HashSet<NodeId>,
    feedback: &HashSet<(NodeId, NodeId)>,
    config: &MermaidLayoutConfig,
) -> (LayoutGraph, HashMap<NodeId, usize>, Vec<Option<EdgeBinding>>) {
    let mut lg = LayoutGraph::new();
    lg.nodesep = config.nodesep;
    lg.ranksep = config.ranksep;
    lg.edgesep = config.edgesep;
    lg.marginx = config.marginx;
    lg.marginy = config.marginy;

    let mut id_to_idx: HashMap<NodeId, usize> = HashMap::new();
    let mut edge_bindings: Vec<Option<EdgeBinding>> = vec![None; graph.edges.len()];

    // Add real nodes (skip loop-body children and loop containers).
    for (id, node) in graph.nodes.iter() {
        if loop_body.contains(&id) || node.node_type == LOOP {
            continue;
        }
        let idx = lg.add_real_node(id, node.size.width, node.size.height);
        id_to_idx.insert(id, idx);
    }

    // Add edges (skip self-loops here; they're handled by remove_self_edges).
    let mut edge_names: HashMap<(usize, usize), u32> = HashMap::new();
    for (edge_idx, edge) in graph.edges.iter().enumerate() {
        let Some(from_port) = graph.ports.get(edge.from_port) else {
            continue;
        };
        let Some(to_port) = graph.ports.get(edge.to_port) else {
            continue;
        };
        if from_port.node == to_port.node {
            // Self-loop: bind it but don't add to layout graph yet.
            let from_idx = id_to_idx.get(&from_port.node).copied();
            let to_idx = id_to_idx.get(&to_port.node).copied();
            if let (Some(f), Some(t)) = (from_idx, to_idx) {
                edge_bindings[edge_idx] = Some((f, t, None));
            }
            continue;
        }
        if loop_body.contains(&from_port.node) || loop_body.contains(&to_port.node) {
            continue;
        }
        let Some(&from_idx) = id_to_idx.get(&from_port.node) else {
            continue;
        };
        let Some(&to_idx) = id_to_idx.get(&to_port.node) else {
            continue;
        };

        let is_feedback = feedback.contains(&(from_port.node, to_port.node));
        let is_continue = to_port.name == "continue";
        let counter = edge_names.entry((from_idx, to_idx)).or_insert(0);
        let name = if *counter == 0 {
            None
        } else {
            Some(format!("e{}", counter))
        };
        *counter += 1;

        let layout_edge_idx = lg.add_edge(from_idx, to_idx);
        let le = &mut lg.edges[layout_edge_idx];
        le.original_idx = Some(edge_idx);
        le.name = name.clone();
        le.weight = if is_feedback || is_continue {
            FEEDBACK_EDGE_WEIGHT
        } else {
            MAIN_EDGE_WEIGHT
        };
        le.minlen = if is_feedback || is_continue { 2 } else { 1 };
        le.feedback = is_feedback;
        if let Some(label) = edge.label.as_ref() {
            le.label = Some(label.clone());
            // Estimate label dimensions (Mermaid measures via DOM; we approximate).
            let chars = label.chars().count() as f32;
            le.label_width = (chars * 7.0).max(20.0);
            le.label_height = 16.0;
        }

        edge_bindings[edge_idx] = Some((from_idx, to_idx, name));
    }

    (lg, id_to_idx, edge_bindings)
}

/// Detach self-loop edges from the layout graph, returning their (edge_idx,
/// owner_node_idx) so they can be re-routed after positioning.
fn remove_self_edges(lg: &mut LayoutGraph) -> Vec<(usize, usize)> {
    let mut self_edges: Vec<(usize, usize)> = Vec::new();
    let mut to_remove: Vec<usize> = Vec::new();
    for (eidx, e) in lg.edges.iter().enumerate() {
        if e.from == e.to {
            self_edges.push((e.original_idx.unwrap_or(usize::MAX), e.from));
            to_remove.push(eidx);
        }
    }
    to_remove.sort_unstable_by(|a, b| b.cmp(a));
    for idx in to_remove {
        lg.edges.remove(idx);
    }
    self_edges
}

/// Route self-loops as 5-point curves to the right of each owner.
fn route_self_edges(lg: &mut LayoutGraph, self_edges: &[(usize, usize)]) {
    for &(orig_idx, owner) in self_edges {
        let center = Point::new(lg.nodes[owner].x, lg.nodes[owner].y);
        let w = lg.nodes[owner].width;
        let h = lg.nodes[owner].height;
        let dummy_x = center.x + w * 0.5 + lg.nodesep.max(40.0);
        let pts = route::route_self_edge(center, w, h, dummy_x);
        // Attach to the original edge by finding it via original_idx.
        for e in &mut lg.edges {
            if e.original_idx == Some(orig_idx) {
                e.points = pts.clone();
            }
        }
    }
}

/// Swap X/Y for every node and edge point (TB → LR coordinate transform).
fn swap_xy(lg: &mut LayoutGraph) {
    for n in &mut lg.nodes {
        std::mem::swap(&mut n.x, &mut n.y);
    }
    for e in &mut lg.edges {
        for p in &mut e.points {
            std::mem::swap(&mut p.x, &mut p.y);
        }
        if let Some(p) = &mut e.label_pos {
            std::mem::swap(&mut p.x, &mut p.y);
        }
    }
}

/// Write node positions (center → top-left) and orthogonal edge routes back
/// into the [`FlowGraph`].
fn write_back(
    graph: &mut FlowGraph,
    lg: &LayoutGraph,
    id_to_idx: &HashMap<NodeId, usize>,
    _edge_bindings: &[Option<EdgeBinding>],
) {
    // Node positions.
    for (id, &idx) in id_to_idx {
        let ln = &lg.nodes[idx];
        if let Some(node) = graph.nodes.get_mut(*id) {
            node.position = Point::new(ln.x - ln.width * 0.5, ln.y - ln.height * 0.5);
        }
    }

    // Edge routes: build a map original_idx -> route.
    let mut routes_by_idx: HashMap<usize, DagreEdgeRoute> = HashMap::new();
    for e in &lg.edges {
        if let Some(oidx) = e.original_idx {
            if e.points.len() >= 2 {
                let points = normalize_dagre_polyline(&e.points);
                routes_by_idx.insert(
                    oidx,
                    DagreEdgeRoute {
                        points,
                        label_pos: e.label_pos,
                    },
                );
            }
        }
    }

    // Write into dagre_edge_routes aligned with graph.edges order.
    graph.dagre_edge_routes = (0..graph.edges.len())
        .map(|i| routes_by_idx.get(&i).cloned())
        .collect();
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::auto_layout::options::LayoutOptions;
    use crate::apply_flow_orientation;
    use crate::builtin_type_registry;
    use crate::mermaid_to_flow_document;
    use crate::FlowGraph;
    use crate::SceneFrame;
    use crate::viewport::Viewport;

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

    fn layout_orchestrator(direction: LayoutDirection) -> FlowGraph {
        let types = builtin_type_registry();
        let doc = mermaid_to_flow_document(ORCHESTRATOR).unwrap();
        let mut graph = FlowGraph::from_document(&doc, &types);
        let options = match direction {
            LayoutDirection::TopBottom => LayoutOptions::mermaid_flowchart_tb(),
            LayoutDirection::LeftRight => LayoutOptions {
                direction: LayoutDirection::LeftRight,
                ..LayoutOptions::mermaid_flowchart_tb()
            },
        };
        apply_flow_orientation(&mut graph, direction);
        layout_graph_mermaid_v2(&mut graph, &options);
        graph
    }

    fn center(graph: &FlowGraph, schema_id: &str) -> (f32, f32) {
        let n = graph
            .nodes
            .iter()
            .find(|(_, n)| n.schema_id == schema_id)
            .map(|(_, n)| n)
            .unwrap();
        (
            n.position.x + n.size.width * 0.5,
            n.position.y + n.size.height * 0.5,
        )
    }

    #[test]
    fn v2_orchestrator_no_node_overlap() {
        let graph = layout_orchestrator(LayoutDirection::TopBottom);
        let ids: Vec<NodeId> = graph.nodes.keys().collect();
        for i in 0..ids.len() {
            for j in (i + 1)..ids.len() {
                let a = graph.nodes.get(ids[i]).unwrap();
                let b = graph.nodes.get(ids[j]).unwrap();
                let overlap_x =
                    a.position.x < b.position.x + b.size.width && b.position.x < a.position.x + a.size.width;
                let overlap_y =
                    a.position.y < b.position.y + b.size.height && b.position.y < a.position.y + a.size.height;
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
    fn v2_orchestrator_fanout_below_orchestrator() {
        let graph = layout_orchestrator(LayoutDirection::TopBottom);
        let (_, oy) = center(&graph, "O");
        for id in ["P", "E", "CA", "CB", "T", "R"] {
            let (_, cy) = center(&graph, id);
            assert!(cy > oy, "{id} should be below O (oy={oy}, cy={cy})");
        }
    }

    #[test]
    fn v2_orchestrator_delivery_below_reviewers() {
        let graph = layout_orchestrator(LayoutDirection::TopBottom);
        let (_, dy) = center(&graph, "D");
        let (_, ty) = center(&graph, "T");
        let (_, ry) = center(&graph, "R");
        assert!(dy > ty, "D below T");
        assert!(dy > ry, "D below R");
    }

    #[test]
    fn v2_orchestrator_dagre_routes_populated() {
        let graph = layout_orchestrator(LayoutDirection::TopBottom);
        let populated = graph.dagre_edge_routes.iter().filter(|r| r.is_some()).count();
        assert!(populated >= 10, "expected >=10 routes, got {populated}");
    }

    #[test]
    fn v2_orchestrator_frame_invariants() {
        let graph = layout_orchestrator(LayoutDirection::TopBottom);
        let frame = SceneFrame::resolve(&graph, &Viewport::default());
        crate::invariants::check_frame(&frame).expect("v2 layout frame invariants");
    }

    #[test]
    fn v2_orchestrator_lr_main_chain_increases_x() {
        let graph = layout_orchestrator(LayoutDirection::LeftRight);
        let (ux, _) = center(&graph, "U");
        let (ox, _) = center(&graph, "O");
        assert!(ux < ox, "U left of O (LR)");
    }

    #[test]
    fn v2_orchestrator_polyline_edges() {
        let graph = layout_orchestrator(LayoutDirection::TopBottom);
        let frame = SceneFrame::resolve(&graph, &Viewport::default());
        let mut polyline = 0;
        for edge in &frame.edges {
            if matches!(edge.path, crate::EdgePath::Polyline(_)) {
                polyline += 1;
            }
        }
        assert!(polyline >= 8, "expected >=8 polyline edges, got {polyline}");
    }

    #[test]
    fn v2_simple_chain_no_crossings() {
        let types = builtin_type_registry();
        let text = "graph TB\n A[Start] --> B[Mid] --> C[End]";
        let doc = mermaid_to_flow_document(text).unwrap();
        let mut graph = FlowGraph::from_document(&doc, &types);
        apply_flow_orientation(&mut graph, LayoutDirection::TopBottom);
        layout_graph_mermaid_v2(&mut graph, &LayoutOptions::mermaid_flowchart_tb());
        let populated = graph.dagre_edge_routes.iter().filter(|r| r.is_some()).count();
        let frame = SceneFrame::resolve(&graph, &Viewport::default());
        // 2 edges, both should resolve.
        assert!(frame.edges.len() >= 2, "edges={} routes_populated={} total_routes={} graph_edges={}",
            frame.edges.len(), populated, graph.dagre_edge_routes.len(), graph.edges.len());
        let (ax, _) = center(&graph, "A");
        let (bx, _) = center(&graph, "B");
        let (cx, _) = center(&graph, "C");
        // Aligned vertically (TB): x centers close.
        assert!((ax - bx).abs() < 5.0, "A/B not aligned: {ax} vs {bx}");
        assert!((bx - cx).abs() < 5.0, "B/C not aligned: {bx} vs {cx}");
    }
}
