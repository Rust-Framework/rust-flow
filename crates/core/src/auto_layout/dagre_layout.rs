//! Dagre.js-compatible layout via the [`dagre`] Rust port (React Flow standard).
//!
//! - Global graph: LR or TB (toolbar switch)
//! - Loop body regions: always TB (Mermaid `subgraph direction TB`)

use std::collections::{HashMap, HashSet};

use dagre::graph::{Graph, GraphOptions};
use dagre::{
    layout, Acyclicer, Align, EdgeLabel, LayoutOptions as DagreLayoutOptions, NodeLabel, RankAlign,
    RankDir, Ranker,
};
use slotmap::Key;

use crate::auto_layout::options::{LayoutDirection, LayoutOptions};
use crate::graph::FlowGraph;
use crate::id::NodeId;
use crate::math::{Point, Size};
use crate::node_layout::{LOOP_BODY_ZONE, LOOP_FOOTER, LOOP_HEADER, LOOP_WIDTH};
use crate::node_type::LOOP;
use crate::auto_layout::overlap::resolve_graph_overlaps;
use crate::orientation::{all_loop_body_nodes, collect_loop_body_nodes};

const MAIN_EDGE_WEIGHT: i32 = 4;
const FEEDBACK_EDGE_WEIGHT: i32 = 1;
const LOOP_BODY_PAD: f32 = 16.0;

/// Run Dagre layout and write top-left positions into `graph.nodes`.
pub fn layout_graph_dagre(graph: &mut FlowGraph, options: &LayoutOptions) {
    if graph.nodes.is_empty() {
        return;
    }

    let loop_body = all_loop_body_nodes(graph);
    layout_main_graph(graph, options, &loop_body);
    layout_loop_body_regions(graph, options);
    resolve_graph_overlaps(graph, options.node_spacing * 0.45);
}

fn layout_main_graph(
    graph: &mut FlowGraph,
    options: &LayoutOptions,
    loop_body: &HashSet<NodeId>,
) {
    let mut g = Graph::with_options(GraphOptions {
        directed: true,
        multigraph: true,
        compound: false,
    });

    let mut id_to_key: HashMap<NodeId, String> = HashMap::new();
    let mut key_to_id: HashMap<String, NodeId> = HashMap::new();
    let mut sizes: HashMap<String, (f32, f32)> = HashMap::new();

    for (id, node) in graph.nodes.iter() {
        if loop_body.contains(&id) {
            continue;
        }
        let key = schema_key(id, node.schema_id.as_str());
        id_to_key.insert(id, key.clone());
        key_to_id.insert(key.clone(), id);
        sizes.insert(key.clone(), (node.size.width, node.size.height));
        g.set_node(
            &key,
            Some(NodeLabel {
                width: node.size.width as f64,
                height: node.size.height as f64,
                ..Default::default()
            }),
        );
    }

    let mut edge_names: HashMap<(String, String), u32> = HashMap::new();
    for edge in &graph.edges {
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
        let Some(from_key) = id_to_key.get(&from_port.node).cloned() else {
            continue;
        };
        let Some(to_key) = id_to_key.get(&to_port.node).cloned() else {
            continue;
        };

        let is_continue = to_port.name == "continue";
        let is_body = from_port.name == "body";
        let counter = edge_names.entry((from_key.clone(), to_key.clone())).or_insert(0);
        let name = if *counter == 0 {
            None
        } else {
            Some(format!("e{}", counter))
        };
        *counter += 1;

        let weight = if is_continue {
            FEEDBACK_EDGE_WEIGHT
        } else {
            MAIN_EDGE_WEIGHT
        };
        let minlen = if is_continue || is_body { 2 } else { 1 };

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
    }

    let rankdir = match options.direction {
        LayoutDirection::LeftRight => RankDir::LR,
        LayoutDirection::TopBottom => RankDir::TB,
    };

    let dagre_opts = DagreLayoutOptions {
        rankdir: rankdir,
        align: Some(Align::UL),
        rank_align: RankAlign::Top,
        nodesep: options.node_spacing as f64,
        ranksep: options.rank_spacing as f64,
        edgesep: options.node_spacing.max(12.0) as f64 / 3.0,
        marginx: options.margin as f64,
        marginy: options.margin as f64,
        ranker: Ranker::NetworkSimplex,
        acyclicer: Some(Acyclicer::Greedy),
        ..Default::default()
    };

    layout(&mut g, Some(dagre_opts));

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
}

pub(crate) fn layout_loop_body_regions(graph: &mut FlowGraph, options: &LayoutOptions) {
    let loop_ids: Vec<NodeId> = graph
        .nodes
        .iter()
        .filter(|(_, n)| n.node_type == LOOP)
        .map(|(id, _)| id)
        .collect();

    for loop_id in loop_ids {
        let children = collect_loop_body_nodes(graph, loop_id);
        if children.is_empty() {
            continue;
        }

        let positions = layout_subset_tb(graph, &children, options);
        let loop_pos = graph.nodes.get(loop_id).map(|n| n.position).unwrap_or_default();

        let mut min_x = f32::MAX;
        let mut min_y = f32::MAX;
        let mut max_x = f32::MIN;
        let mut max_y = f32::MIN;

        for child_id in &children {
            let (cx, cy) = positions.get(child_id).copied().unwrap_or((0.0, 0.0));
            let child = graph.nodes.get(*child_id).unwrap();
            let x = cx - child.size.width / 2.0;
            let y = cy - child.size.height / 2.0;
            min_x = min_x.min(x);
            min_y = min_y.min(y);
            max_x = max_x.max(x + child.size.width);
            max_y = max_y.max(y + child.size.height);
        }

        let content_w = max_x - min_x;
        let content_h = max_y - min_y;
        let body_origin_x = loop_pos.x + LOOP_BODY_PAD;
        let body_origin_y = loop_pos.y + LOOP_HEADER + LOOP_BODY_PAD;

        for child_id in &children {
            let (cx, cy) = positions.get(child_id).copied().unwrap_or((0.0, 0.0));
            let child = graph.nodes.get(*child_id).unwrap();
            let rel_x = cx - child.size.width / 2.0 - min_x;
            let rel_y = cy - child.size.height / 2.0 - min_y;
            if let Some(node) = graph.nodes.get_mut(*child_id) {
                node.position = Point::new(body_origin_x + rel_x, body_origin_y + rel_y);
            }
        }

        let body_w = content_w + LOOP_BODY_PAD * 2.0;
        let body_h = content_h.max(LOOP_BODY_ZONE - LOOP_BODY_PAD) + LOOP_BODY_PAD * 2.0;
        if let Some(loop_node) = graph.nodes.get_mut(loop_id) {
            loop_node.size = Size::new(
                body_w.max(LOOP_WIDTH),
                LOOP_HEADER + body_h + LOOP_FOOTER,
            );
        }
    }
}

fn layout_subset_tb(
    graph: &FlowGraph,
    node_ids: &[NodeId],
    options: &LayoutOptions,
) -> HashMap<NodeId, (f32, f32)> {
    let mut g = Graph::with_options(GraphOptions {
        directed: true,
        multigraph: true,
        compound: false,
    });

    let mut id_to_key: HashMap<NodeId, String> = HashMap::new();
    let mut key_to_id: HashMap<String, NodeId> = HashMap::new();

    for id in node_ids {
        let node = graph.nodes.get(*id).unwrap();
        let key = schema_key(*id, node.schema_id.as_str());
        id_to_key.insert(*id, key.clone());
        key_to_id.insert(key.clone(), *id);
        g.set_node(
            &key,
            Some(NodeLabel {
                width: node.size.width as f64,
                height: node.size.height as f64,
                ..Default::default()
            }),
        );
    }

    let id_set: HashSet<NodeId> = node_ids.iter().copied().collect();
    let mut edge_names: HashMap<(String, String), u32> = HashMap::new();

    for edge in &graph.edges {
        let from_port = match graph.ports.get(edge.from_port) {
            Some(p) => p,
            None => continue,
        };
        let to_port = match graph.ports.get(edge.to_port) {
            Some(p) => p,
            None => continue,
        };
        if !id_set.contains(&from_port.node) || !id_set.contains(&to_port.node) {
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
        let counter = edge_names.entry((from_key.clone(), to_key.clone())).or_insert(0);
        let name = if *counter == 0 {
            None
        } else {
            Some(format!("lb{}", counter))
        };
        *counter += 1;

        let is_continue_target = graph
            .ports
            .get(edge.to_port)
            .is_some_and(|p| p.name == "continue");

        g.set_edge(
            &from_key,
            &to_key,
            Some(EdgeLabel {
                minlen: if is_continue_target { 2 } else { 1 },
                weight: if is_continue_target {
                    FEEDBACK_EDGE_WEIGHT
                } else {
                    MAIN_EDGE_WEIGHT
                },
                ..Default::default()
            }),
            name.as_deref(),
        );
    }

    let subset_opts = DagreLayoutOptions {
        rankdir: RankDir::TB,
        align: Some(Align::UL),
        rank_align: RankAlign::Top,
        nodesep: (options.node_spacing * 0.75) as f64,
        ranksep: (options.rank_spacing * 0.5) as f64,
        marginx: LOOP_BODY_PAD as f64,
        marginy: LOOP_BODY_PAD as f64,
        ranker: Ranker::NetworkSimplex,
        ..Default::default()
    };

    layout(&mut g, Some(subset_opts));

    let mut out = HashMap::new();
    for key in g.nodes() {
        if let Some(id) = key_to_id.get(&key) {
            let label = g.node(&key).unwrap();
            out.insert(
                *id,
                (
                    label.x.unwrap_or(0.0) as f32,
                    label.y.unwrap_or(0.0) as f32,
                ),
            );
        }
    }
    out
}

pub(crate) fn schema_key(id: NodeId, schema_id: &str) -> String {
    if !schema_id.is_empty() {
        schema_id.to_string()
    } else {
        format!("n{}", id.data().as_ffi())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::auto_layout::layered::rects_overlap;
    use crate::apply_flow_orientation;
    use crate::check_frame;
    use crate::scene::SceneFrame;
    use crate::viewport::Viewport;

    fn layout_demo_with(direction: LayoutDirection) -> FlowGraph {
        use crate::builtin_type_registry;
        use crate::demo_document;
        let types = builtin_type_registry();
        let doc = demo_document();
        let mut graph = FlowGraph::from_document(&doc, &types);
        let options = LayoutOptions {
            direction,
            ..LayoutOptions::comfortable()
        };
        apply_flow_orientation(&mut graph, direction);
        layout_graph_dagre(&mut graph, &options);
        graph
    }

    #[test]
    fn dagre_demo_lr_main_chain_increases_x() {
        let graph = layout_demo_with(LayoutDirection::LeftRight);
        let pos = |id: &str| {
            graph
                .nodes
                .iter()
                .find(|(_, n)| n.schema_id == id)
                .map(|(_, n)| n.position.x)
                .unwrap_or(0.0)
        };
        assert!(pos("trigger") < pos("fetch_order"));
        assert!(pos("fetch_order") < pos("check_stock"));
        assert!(pos("check_stock") < pos("route_customer"));
    }

    #[test]
    fn dagre_demo_tb_main_chain_increases_y() {
        let graph = layout_demo_with(LayoutDirection::TopBottom);
        let pos = |id: &str| {
            graph
                .nodes
                .iter()
                .find(|(_, n)| n.schema_id == id)
                .map(|(_, n)| n.position.y)
                .unwrap_or(0.0)
        };
        assert!(pos("trigger") < pos("fetch_order"));
        assert!(pos("fetch_order") < pos("check_stock"));
        assert!(pos("check_stock") < pos("route_customer"));
    }

    #[test]
    fn dagre_loop_body_below_header_tb() {
        let graph = layout_demo_with(LayoutDirection::LeftRight);
        let loop_node = graph
            .nodes
            .iter()
            .find(|(_, n)| n.schema_id == "loop_lines")
            .map(|(_, n)| n)
            .unwrap();
        let deduct = graph
            .nodes
            .iter()
            .find(|(_, n)| n.schema_id == "deduct_stock")
            .map(|(_, n)| n)
            .unwrap();
        assert!(deduct.position.y > loop_node.position.y + LOOP_HEADER);
        assert!(deduct.position.x >= loop_node.position.x);
        assert!(deduct.position.x + deduct.size.width <= loop_node.position.x + loop_node.size.width + 1.0);
    }

    #[test]
    fn dagre_demo_no_overlap() {
        let graph = layout_demo_with(LayoutDirection::LeftRight);
        let padding = 8.0;
        let ids: Vec<_> = graph.nodes.iter().map(|(id, _)| id).collect();
        for i in 0..ids.len() {
            for j in i + 1..ids.len() {
                if crate::orientation::loop_container_overlap_allowed(&graph, ids[i], ids[j]) {
                    continue;
                }
                let a = &graph.nodes[ids[i]];
                let b = &graph.nodes[ids[j]];
                assert!(
                    !rects_overlap(
                        a.position.x,
                        a.position.y,
                        a.size.width,
                        a.size.height,
                        b.position.x,
                        b.position.y,
                        b.size.width,
                        b.size.height,
                        padding,
                    ),
                    "overlap between '{}' and '{}'",
                    a.label,
                    b.label
                );
            }
        }
    }

    #[test]
    fn dagre_demo_frame_invariants() {
        let graph = layout_demo_with(LayoutDirection::LeftRight);
        let frame = SceneFrame::resolve(&graph, &Viewport::default());
        check_frame(&frame).expect("dagre layout invariants");
    }
}
