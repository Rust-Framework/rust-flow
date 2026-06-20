//! Post-layout overlap removal and edge route staggering (Dagre / React Flow follow-up).

use crate::auto_layout::layered::rects_overlap;
use crate::graph::FlowGraph;
use crate::id::{NodeId, PortId};
use crate::math::Point;
use crate::node::ResolvedNode;
use crate::orientation::loop_container_overlap_allowed;

/// Per-edge lateral shift along port edges (pixels, screen space).
#[derive(Debug, Clone, Copy, Default)]
pub struct EdgeRouteOffset {
    pub from_shift: f32,
    pub to_shift: f32,
    /// Extra bend-center shift to avoid node obstacles (screen space).
    pub center_nudge_x: f32,
    pub center_nudge_y: f32,
}

/// Push overlapping nodes apart (2D MTV-style separation).
pub fn resolve_graph_overlaps(graph: &mut FlowGraph, padding: f32) {
    let ids: Vec<NodeId> = graph.nodes.iter().map(|(id, _)| id).collect();
    let n = ids.len();
    if n < 2 {
        return;
    }

    for _ in 0..96 {
        let mut moved = false;
        for i in 0..n {
            for j in i + 1..n {
                let a_id = ids[i];
                let b_id = ids[j];
                if loop_container_overlap_allowed(graph, a_id, b_id) {
                    continue;
                }
                let a = graph.nodes.get(a_id).unwrap();
                let b = graph.nodes.get(b_id).unwrap();
                let ax = a.position.x;
                let ay = a.position.y;
                let aw = a.size.width;
                let ah = a.size.height;
                let bx = b.position.x;
                let by = b.position.y;
                let bw = b.size.width;
                let bh = b.size.height;

                if !rects_overlap(ax, ay, aw, ah, bx, by, bw, bh, padding) {
                    continue;
                }

                let push_right = ax + aw + padding - bx;
                let push_left = bx + bw + padding - ax;
                let push_down = ay + ah + padding - by;
                let push_up = by + bh + padding - ay;

                let mut min_sep = f32::MAX;
                let mut axis = 0i8;

                if push_down > 0.0 && push_down < min_sep {
                    min_sep = push_down;
                    axis = 1;
                }
                if push_up > 0.0 && push_up < min_sep {
                    min_sep = push_up;
                    axis = -1;
                }
                if push_right > 0.0 && push_right < min_sep {
                    min_sep = push_right;
                    axis = 2;
                }
                if push_left > 0.0 && push_left < min_sep {
                    axis = -2;
                }

                match axis {
                    1 => graph.nodes.get_mut(b_id).unwrap().position.y += push_down,
                    -1 => graph.nodes.get_mut(a_id).unwrap().position.y += push_up,
                    2 => graph.nodes.get_mut(b_id).unwrap().position.x += push_right,
                    -2 => graph.nodes.get_mut(a_id).unwrap().position.x += push_left,
                    _ => {}
                }
                moved = true;
            }
        }
        if !moved {
            break;
        }
    }
}

/// Stagger parallel edges that share a port (React Flow / yFiles edge bundling lite).
pub fn compute_edge_route_offsets(
    graph: &FlowGraph,
    nodes: &[ResolvedNode],
    node_index: &std::collections::HashMap<NodeId, usize>,
    feedback: &std::collections::HashSet<(NodeId, NodeId)>,
) -> Vec<EdgeRouteOffset> {
    const BASE_STAGGER: f32 = 18.0;
    let zoom = nodes.first().map(|n| n.zoom).unwrap_or(1.0);
    let stagger = BASE_STAGGER * zoom;
    let edge_count = graph.edges.len();
    let mut offsets = vec![EdgeRouteOffset::default(); edge_count];

    stagger_port_group(graph, nodes, node_index, true, stagger, &mut offsets);
    stagger_port_group(graph, nodes, node_index, false, stagger, &mut offsets);
    stagger_node_pair_group(graph, feedback, stagger, &mut offsets);
    apply_mermaid_feedback_channels(graph, nodes, node_index, feedback, &mut offsets);
    refine_routes_avoid_obstacles(graph, nodes, node_index, feedback, &mut offsets);

    offsets
}

fn stagger_port_group(
    graph: &FlowGraph,
    nodes: &[ResolvedNode],
    node_index: &std::collections::HashMap<NodeId, usize>,
    by_from: bool,
    spacing: f32,
    offsets: &mut [EdgeRouteOffset],
) {
    use std::collections::HashMap;

    let mut groups: HashMap<PortId, Vec<usize>> = HashMap::new();
    for (idx, edge) in graph.edges.iter().enumerate() {
        let port = if by_from {
            edge.from_port
        } else {
            edge.to_port
        };
        groups.entry(port).or_default().push(idx);
    }

    for (_port_id, indices) in groups {
        if indices.len() <= 1 {
            continue;
        }
        let mut sorted: Vec<(usize, f32, f32)> = indices
            .iter()
            .map(|&idx| {
                let edge = &graph.edges[idx];
                let port_id = if by_from {
                    edge.from_port
                } else {
                    edge.to_port
                };
                let other_port = if by_from {
                    edge.to_port
                } else {
                    edge.from_port
                };
                let (primary, secondary) =
                    sort_key_for_edge(graph, nodes, node_index, port_id, other_port);
                (idx, primary, secondary)
            })
            .collect();
        sorted.sort_by(|a, b| {
            a.1.partial_cmp(&b.1)
                .unwrap_or(std::cmp::Ordering::Equal)
                .then_with(|| a.2.partial_cmp(&b.2).unwrap_or(std::cmp::Ordering::Equal))
        });

        let n = sorted.len() as f32;
        for (i, (idx, _, _)) in sorted.iter().enumerate() {
            let t = i as f32 - (n - 1.0) / 2.0;
            let shift = t * spacing;
            if by_from {
                offsets[*idx].from_shift = shift;
            } else {
                offsets[*idx].to_shift = shift;
            }
        }
    }
}

fn sort_key_for_edge(
    graph: &FlowGraph,
    nodes: &[ResolvedNode],
    node_index: &std::collections::HashMap<NodeId, usize>,
    _port_id: PortId,
    other_port_id: PortId,
) -> (f32, f32) {
    let other_node = graph
        .ports
        .get(other_port_id)
        .and_then(|p| node_index.get(&p.node))
        .and_then(|&i| nodes.get(i));
    let center = other_node.map(|n| {
        Point::new(
            n.screen_pos.x + n.screen_size.width * 0.5,
            n.screen_pos.y + n.screen_size.height * 0.5,
        )
    });
    center.map(|c| (c.x, c.y)).unwrap_or((0.0, 0.0))
}

/// Stagger multiple edges between the same two nodes (skip feedback pairs).
fn stagger_node_pair_group(
    graph: &FlowGraph,
    feedback: &std::collections::HashSet<(NodeId, NodeId)>,
    spacing: f32,
    offsets: &mut [EdgeRouteOffset],
) {
    use std::collections::HashMap;

    let mut groups: HashMap<(NodeId, NodeId), Vec<usize>> = HashMap::new();
    for (idx, edge) in graph.edges.iter().enumerate() {
        let from_node = graph.ports.get(edge.from_port).map(|p| p.node);
        let to_node = graph.ports.get(edge.to_port).map(|p| p.node);
        if let (Some(a), Some(b)) = (from_node, to_node) {
            if feedback.contains(&(a, b)) {
                continue;
            }
            groups.entry((a, b)).or_default().push(idx);
        }
    }

    for indices in groups.values() {
        if indices.len() <= 1 {
            continue;
        }
        let n = indices.len() as f32;
        for (i, &idx) in indices.iter().enumerate() {
            let t = i as f32 - (n - 1.0) / 2.0;
            offsets[idx].center_nudge_x += t * spacing * 0.6;
            offsets[idx].center_nudge_y += t * spacing * 0.6;
        }
    }
}

/// Mermaid-style feedback loops: dedicated vertical channel on the left of the graph.
fn apply_mermaid_feedback_channels(
    graph: &FlowGraph,
    nodes: &[ResolvedNode],
    node_index: &std::collections::HashMap<NodeId, usize>,
    feedback: &std::collections::HashSet<(NodeId, NodeId)>,
    offsets: &mut [EdgeRouteOffset],
) {
    if feedback.is_empty() || nodes.is_empty() {
        return;
    }

    let zoom = nodes.first().map(|n| n.zoom).unwrap_or(1.0);
    let margin = 36.0 * zoom;
    let channel_spacing = 16.0 * zoom;

    let min_x = nodes
        .iter()
        .map(|n| n.screen_pos.x)
        .fold(f32::MAX, f32::min);

    let mut feedback_indices: Vec<usize> = Vec::new();
    for (idx, edge) in graph.edges.iter().enumerate() {
        let from_node = graph.ports.get(edge.from_port).map(|p| p.node);
        let to_node = graph.ports.get(edge.to_port).map(|p| p.node);
        if let (Some(a), Some(b)) = (from_node, to_node) {
            if feedback.contains(&(a, b)) {
                feedback_indices.push(idx);
            }
        }
    }

    let n = feedback_indices.len() as f32;
    for (i, &idx) in feedback_indices.iter().enumerate() {
        let edge = &graph.edges[idx];
        let from = match anchor_point(nodes, node_index, graph, edge.from_port) {
            Some(p) => p,
            None => continue,
        };
        let to = match anchor_point(nodes, node_index, graph, edge.to_port) {
            Some(p) => p,
            None => continue,
        };
        let t = i as f32 - (n - 1.0) / 2.0;
        let channel_x = min_x - margin + t * channel_spacing;
        offsets[idx].center_nudge_x = channel_x - (from.x + to.x) * 0.5;
    }
}

struct ObstacleRect {
    x: f32,
    y: f32,
    w: f32,
    h: f32,
}

fn node_obstacle(node: &ResolvedNode) -> ObstacleRect {
    ObstacleRect {
        x: node.screen_pos.x,
        y: node.screen_pos.y,
        w: node.screen_size.width,
        h: node.screen_size.height,
    }
}

fn point_in_rect(px: f32, py: f32, r: &ObstacleRect, pad: f32) -> bool {
    px >= r.x - pad
        && px <= r.x + r.w + pad
        && py >= r.y - pad
        && py <= r.y + r.h + pad
}

fn cross(ox: f32, oy: f32, ax: f32, ay: f32, bx: f32, by: f32) -> f32 {
    (ax - ox) * (by - oy) - (ay - oy) * (bx - ox)
}

fn segments_intersect(ax: f32, ay: f32, bx: f32, by: f32, cx: f32, cy: f32, dx: f32, dy: f32) -> bool {
    let d1 = cross(cx, cy, dx, dy, ax, ay);
    let d2 = cross(cx, cy, dx, dy, bx, by);
    let d3 = cross(ax, ay, bx, by, cx, cy);
    let d4 = cross(ax, ay, bx, by, dx, dy);
    if ((d1 > 0.0 && d2 < 0.0) || (d1 < 0.0 && d2 > 0.0))
        && ((d3 > 0.0 && d4 < 0.0) || (d3 < 0.0 && d4 > 0.0))
    {
        return true;
    }
  false
}

fn segment_hits_rect(ax: f32, ay: f32, bx: f32, by: f32, r: &ObstacleRect, pad: f32) -> bool {
    if point_in_rect(ax, ay, r, pad) || point_in_rect(bx, by, r, pad) {
        return true;
    }
    let x2 = r.x + r.w;
    let y2 = r.y + r.h;
    segments_intersect(ax, ay, bx, by, r.x, r.y, x2, r.y)
        || segments_intersect(ax, ay, bx, by, x2, r.y, x2, y2)
        || segments_intersect(ax, ay, bx, by, x2, y2, r.x, y2)
        || segments_intersect(ax, ay, bx, by, r.x, y2, r.x, r.y)
}

fn path_hits_obstacle(points: &[Point], obstacle: &ObstacleRect, pad: f32) -> bool {
    for window in points.windows(2) {
        if segment_hits_rect(
            window[0].x,
            window[0].y,
            window[1].x,
            window[1].y,
            obstacle,
            pad,
        ) {
            return true;
        }
    }
    false
}

/// Push bend centers so forward orthogonal segments avoid unrelated nodes.
fn refine_routes_avoid_obstacles(
    graph: &FlowGraph,
    nodes: &[ResolvedNode],
    node_index: &std::collections::HashMap<NodeId, usize>,
    feedback: &std::collections::HashSet<(NodeId, NodeId)>,
    offsets: &mut [EdgeRouteOffset],
) {
    use crate::geometry::edge_step_polyline;
    use crate::port::PortSide;

    let zoom = nodes.first().map(|n| n.zoom).unwrap_or(1.0);
    let pad = 10.0 * zoom;
    let step = 22.0 * zoom;

    for (edge_idx, edge) in graph.edges.iter().enumerate() {
        let from_port = match graph.ports.get(edge.from_port) {
            Some(p) => p,
            None => continue,
        };
        let to_port = match graph.ports.get(edge.to_port) {
            Some(p) => p,
            None => continue,
        };
        let from_node_id = from_port.node;
        let to_node_id = to_port.node;
        if feedback.contains(&(from_node_id, to_node_id)) {
            continue;
        }
        let from = match anchor_point(nodes, node_index, graph, edge.from_port) {
            Some(p) => p,
            None => continue,
        };
        let to = match anchor_point(nodes, node_index, graph, edge.to_port) {
            Some(p) => p,
            None => continue,
        };
        let from_side = from_port.side;
        let to_side = to_port.side;

        let from_idx = match node_index.get(&from_node_id) {
            Some(i) => *i,
            None => continue,
        };
        let _ = from_idx;

        let obstacles: Vec<ObstacleRect> = nodes
            .iter()
            .filter(|n| n.id != from_node_id && n.id != to_node_id)
            .map(node_obstacle)
            .collect();

        for _ in 0..8 {
            let path = edge_step_polyline(from, from_side, to, to_side, offsets[edge_idx], zoom);
            let mut hit: Option<&ObstacleRect> = None;
            for obstacle in &obstacles {
                if path_hits_obstacle(&path, obstacle, pad) {
                    hit = Some(obstacle);
                    break;
                }
            }
            if hit.is_none() {
                break;
            }
            let r = hit.unwrap();
            let rcx = r.x + r.w * 0.5;
            let rcy = r.y + r.h * 0.5;
            let mid_x = (from.x + to.x) * 0.5;
            let mid_y = (from.y + to.y) * 0.5;

            if matches!(
                (from_side, to_side),
                (PortSide::Bottom, PortSide::Top)
                    | (PortSide::Top, PortSide::Bottom)
                    | (PortSide::Bottom, PortSide::Bottom)
                    | (PortSide::Top, PortSide::Top)
            ) {
                if rcx >= mid_x {
                    offsets[edge_idx].center_nudge_x += step;
                } else {
                    offsets[edge_idx].center_nudge_x -= step;
                }
                if rcy >= mid_y {
                    offsets[edge_idx].center_nudge_y += step;
                } else {
                    offsets[edge_idx].center_nudge_y -= step;
                }
            } else {
                if rcy >= mid_y {
                    offsets[edge_idx].center_nudge_y += step;
                } else {
                    offsets[edge_idx].center_nudge_y -= step;
                }
                if rcx >= mid_x {
                    offsets[edge_idx].center_nudge_x += step;
                } else {
                    offsets[edge_idx].center_nudge_x -= step;
                }
            }
        }
    }
}

fn anchor_point(
    nodes: &[ResolvedNode],
    node_index: &std::collections::HashMap<NodeId, usize>,
    graph: &FlowGraph,
    port_id: PortId,
) -> Option<Point> {
    let port = graph.ports.get(port_id)?;
    let idx = *node_index.get(&port.node)?;
    nodes[idx].port_anchors.get(&port_id).copied()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::auto_layout::{LayoutDirection, LayoutOptions};
    use crate::apply_flow_orientation;
    use crate::auto_layout::dagre_layout::layout_graph_dagre;
    use crate::builtin_type_registry;
    use crate::demo_document;
    use crate::orientation::loop_container_overlap_allowed;
    use crate::FlowGraph;

    #[test]
    fn overlap_resolver_separates_boxes() {
        let mut graph = FlowGraph::new("t");
        let a = graph.add_node(crate::node::FlowNode::new(
            crate::id::NodeId::default(),
            "A",
            Point::new(0.0, 0.0),
        ));
        let b = graph.add_node(crate::node::FlowNode::new(
            crate::id::NodeId::default(),
            "B",
            Point::new(10.0, 10.0),
        ));
        graph.nodes.get_mut(a).unwrap().size = crate::math::Size::new(100.0, 40.0);
        graph.nodes.get_mut(b).unwrap().size = crate::math::Size::new(100.0, 40.0);
        resolve_graph_overlaps(&mut graph, 16.0);
        let pa = graph.nodes.get(a).unwrap().position;
        let pb = graph.nodes.get(b).unwrap().position;
        assert!(
            !rects_overlap(pa.x, pa.y, 100.0, 40.0, pb.x, pb.y, 100.0, 40.0, 8.0)
        );
    }

    #[test]
    fn demo_no_overlap_after_post_process() {
        let types = builtin_type_registry();
        let doc = demo_document();
        let mut graph = FlowGraph::from_document(&doc, &types);
        let options = LayoutOptions {
            direction: LayoutDirection::LeftRight,
            ..LayoutOptions::comfortable()
        };
        apply_flow_orientation(&mut graph, options.direction);
        layout_graph_dagre(&mut graph, &options);
        resolve_graph_overlaps(&mut graph, options.node_spacing * 0.5);

        let ids: Vec<_> = graph.nodes.iter().map(|(id, _)| id).collect();
        for i in 0..ids.len() {
            for j in i + 1..ids.len() {
                if loop_container_overlap_allowed(&graph, ids[i], ids[j]) {
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
                        6.0,
                    ),
                    "overlap {} vs {}",
                    a.label,
                    b.label
                );
            }
        }
    }
}
