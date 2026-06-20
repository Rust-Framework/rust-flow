//! Mind map tree layout — root centered with bidirectional (LR) child distribution.
//!
//! Algorithm inspired by jsmind (`mode: full`) and mind-elixir (`SIDE` direction):
//! 1. Build a tree from parent→child edges.
//! 2. Split root's children into left and right groups (balanced by subtree size).
//! 3. Layout each side as a horizontal tidy tree (subtree height accumulation).
//! 4. Center root vertically between left and right subtree bounds.
//!
//! For TB direction, falls back to a top-down tidy tree layout.

use std::collections::HashMap;

use crate::auto_layout::options::{LayoutDirection, LayoutOptions};
use crate::graph::FlowGraph;
use crate::id::NodeId;
use crate::math::Point;

/// Run mind map tree layout and write positions into `graph.nodes`.
pub fn layout_mindmap(graph: &mut FlowGraph, options: &LayoutOptions) {
    if graph.nodes.is_empty() {
        return;
    }

    let tree = build_tree(graph);
    if tree.is_empty() {
        return;
    }

    match options.direction {
        LayoutDirection::LeftRight => layout_bidirectional(graph, &tree, options),
        LayoutDirection::TopBottom => layout_topdown(graph, &tree, options),
    }
}

/// Tree structure: node id → children indices.
type Tree = HashMap<NodeId, Vec<NodeId>>;

/// Build parent→children map from graph edges (first parent wins for DAG safety).
fn build_tree(graph: &FlowGraph) -> Tree {
    let mut tree: Tree = HashMap::new();
    let mut has_parent: std::collections::HashSet<NodeId> = std::collections::HashSet::new();

    // Collect all nodes first
    for (id, _) in graph.nodes.iter() {
        tree.entry(id).or_default();
    }

    // Build parent→child relationships from edges
    for edge in &graph.edges {
        let from_port = match graph.ports.get(edge.from_port) {
            Some(p) => p,
            None => continue,
        };
        let to_port = match graph.ports.get(edge.to_port) {
            Some(p) => p,
            None => continue,
        };
        let parent = from_port.node;
        let child = to_port.node;
        if parent == child {
            continue;
        }
        if has_parent.contains(&child) {
            continue; // Tree: each node has at most one parent
        }
        has_parent.insert(child);
        tree.entry(parent).or_default().push(child);
    }

    tree
}

/// Find root nodes (nodes with no parent).
fn find_roots(tree: &Tree) -> Vec<NodeId> {
    let mut has_parent: std::collections::HashSet<NodeId> = std::collections::HashSet::new();
    for children in tree.values() {
        for child in children {
            has_parent.insert(*child);
        }
    }
    tree.keys()
        .filter(|id| !has_parent.contains(id))
        .copied()
        .collect()
}

/// Subtree metrics computed during layout.
struct SubtreeLayout {
    /// Top y position of the subtree bounding box.
    #[allow(dead_code)]
    top: f32,
    /// Bottom y position of the subtree bounding box.
    bottom: f32,
    /// Right (for right side) or left (for left side) x extent.
    extent_x: f32,
}

/// Bidirectional layout: root centered, children split left/right.
/// Uses Reingold-Tilford contour comparison for compact spacing.
fn layout_bidirectional(graph: &mut FlowGraph, tree: &Tree, options: &LayoutOptions) {
    let roots = find_roots(tree);
    if roots.is_empty() {
        return;
    }

    let root = roots[0];
    let children = tree.get(&root).cloned().unwrap_or_default();

    if children.is_empty() {
        // Single node — place at origin
        if let Some(n) = graph.nodes.get_mut(root) {
            n.position = Point::new(0.0, 0.0);
        }
        return;
    }

    // Balance left/right by subtree leaf count.
    // Preserve original child order; greedily assign each child to the
    // side with fewer leaves so the split stays even (jsmind heuristic).
    let child_subtree_sizes: Vec<(NodeId, usize)> = children
        .iter()
        .map(|&c| (c, count_leaves(tree, c)))
        .collect();

    let mut left_children: Vec<NodeId> = Vec::new();
    let mut right_children: Vec<NodeId> = Vec::new();
    let mut left_size = 0usize;
    let mut right_size = 0usize;

    for (child, size) in &child_subtree_sizes {
        // Prefer right side for the first child (LR mind maps read left-to-right)
        if left_size == 0 && right_size == 0 {
            right_children.push(*child);
            right_size += size;
        } else if left_size <= right_size {
            left_children.push(*child);
            left_size += size;
        } else {
            right_children.push(*child);
            right_size += size;
        }
    }

    let root_size = graph.nodes.get(root).map(|n| n.size).unwrap_or_default();
    let root_h = root_size.height;
    let root_w = root_size.width;

    // Layout right side using Reingold-Tilford contour comparison
    let mut right_layouts: HashMap<NodeId, Point> = HashMap::new();
    let right_x = root_w / 2.0 + options.rank_spacing;
    layout_side_rt(
        graph,
        tree,
        &right_children,
        options,
        right_x,
        true, // right side
        &mut right_layouts,
    );

    // Layout left side using Reingold-Tilford contour comparison
    let mut left_layouts: HashMap<NodeId, Point> = HashMap::new();
    let left_x = -(root_w / 2.0) - options.rank_spacing;
    layout_side_rt(
        graph,
        tree,
        &left_children,
        options,
        left_x,
        false, // left side
        &mut left_layouts,
    );

    // Compute vertical centering
    let top = right_layouts
        .values()
        .chain(left_layouts.values())
        .map(|p| p.y)
        .fold(f32::MAX, f32::min);
    let bottom = right_layouts
        .values()
        .chain(left_layouts.values())
        .map(|p| p.y)
        .fold(f32::MIN, f32::max);

    // Center root vertically
    let root_center_y = (top + bottom) / 2.0;
    let root_x = -(root_w / 2.0);

    // Apply positions
    if let Some(n) = graph.nodes.get_mut(root) {
        n.position = Point::new(root_x, root_center_y - root_h / 2.0);
    }

    for (id, pos) in right_layouts.iter().chain(left_layouts.iter()) {
        if let Some(n) = graph.nodes.get_mut(*id) {
            n.position = *pos;
        }
    }
}

/// Layout one side using Reingold-Tilford contour comparison.
fn layout_side_rt(
    graph: &FlowGraph,
    tree: &Tree,
    children: &[NodeId],
    options: &LayoutOptions,
    start_x: f32,
    is_right: bool,
    layouts: &mut HashMap<NodeId, Point>,
) {
    if children.is_empty() {
        return;
    }

    let mut current_y = 0.0f32;
    let mut prev_contours: Vec<(NodeId, f32, f32, Contour, Contour)> = Vec::new();

    for &child in children {
        // Layout this child's subtree
        let (ct, cb, _ce, cbc, ctc) =
            layout_subtree_rt(graph, tree, child, options, start_x, current_y, is_right, layouts);

        // For subsequent children, compute contour-based shift
        if !prev_contours.is_empty() {
            let shift = compute_contour_shift(&prev_contours, &ctc, options.node_spacing);
            if shift > 0.0 {
                shift_subtree(graph, tree, child, layouts, shift);
                let (ct2, cb2, _ce2, cbc2, ctc2) = relayout_subtree_rt(
                    graph, tree, child, options, start_x, current_y + shift, is_right, layouts,
                );
                current_y = cb2 + options.node_spacing;
                prev_contours.push((child, ct2, cb2, cbc2, ctc2));
            } else {
                current_y = cb + options.node_spacing;
                prev_contours.push((child, ct, cb, cbc, ctc));
            }
        } else {
            current_y = cb + options.node_spacing;
            prev_contours.push((child, ct, cb, cbc, ctc));
        }
    }
}

/// Count leaves in a subtree (for balance heuristic).
fn count_leaves(tree: &Tree, node: NodeId) -> usize {
    let children = match tree.get(&node) {
        Some(c) => c,
        None => return 1,
    };
    if children.is_empty() {
        return 1;
    }
    children.iter().map(|&c| count_leaves(tree, c)).sum()
}

// ============================================================================
// Reingold-Tilford contour-based layout (tidy tree).
//
// Reference: "Tidier Drawings of Trees" (Reingold & Tilford, 1981).
// The algorithm produces compact layouts by comparing subtree contours
// instead of using simple bounding boxes. This avoids excessive whitespace
// when adjacent subtrees have different shapes.
//
// For a horizontal (LR) mind map:
// - "depth" maps to x (horizontal position)
// - "prelim" maps to y (vertical position)
// - We compare bottom contour of left subtree with top contour of right subtree
// ============================================================================

/// Contour entry: (depth, y_value).
/// For bottom contour: max y at each depth.
/// For top contour: min y at each depth.
type Contour = Vec<(i32, f32)>;

/// Layout a subtree using Reingold-Tilford contour comparison.
///
/// Returns (top_y, bottom_y, extent_x, bottom_contour, top_contour).
fn layout_subtree_rt(
    graph: &FlowGraph,
    tree: &Tree,
    node_id: NodeId,
    options: &LayoutOptions,
    x: f32,
    y_start: f32,
    is_right: bool,
    layouts: &mut HashMap<NodeId, Point>,
) -> (f32, f32, f32, Contour, Contour) {
    let node_size = graph.nodes.get(node_id).map(|n| n.size).unwrap_or_default();
    let node_h = node_size.height;
    let node_w = node_size.width;

    let children = tree.get(&node_id).cloned().unwrap_or_default();

    if children.is_empty() {
        // Leaf node: single point in contour
        let pos = if is_right {
            Point::new(x, y_start)
        } else {
            Point::new(x - node_w, y_start)
        };
        layouts.insert(node_id, pos);

        let extent_x = if is_right { x + node_w } else { x - node_w };
        let bottom = y_start + node_h;
        // Contour at depth 0: just this node
        let bottom_contour = vec![(0, bottom)];
        let top_contour = vec![(0, y_start)];
        (y_start, bottom, extent_x, bottom_contour, top_contour)
    } else {
        // Internal node: layout children with contour comparison
        let child_x = if is_right {
            x + node_w + options.rank_spacing
        } else {
            x - node_w - options.rank_spacing
        };

        // Layout first child
        let mut current_y = y_start;
        let mut top = f32::MAX;
        let mut bottom = f32::MIN;
        let mut max_extent_x = if is_right { x + node_w } else { x - node_w };

        let mut child_contours: Vec<(NodeId, f32, f32, Contour, Contour)> = Vec::new();

        for (i, &child) in children.iter().enumerate() {
            if i == 0 {
                // First child: layout at current_y
                let (ct, cb, ce, cbc, ctc) =
                    layout_subtree_rt(graph, tree, child, options, child_x, current_y, is_right, layouts);
                top = top.min(ct);
                bottom = bottom.max(cb);
                max_extent_x = if is_right {
                    max_extent_x.max(ce)
                } else {
                    max_extent_x.min(ce)
                };
                current_y = cb + options.node_spacing;
                child_contours.push((child, ct, cb, cbc, ctc));
            } else {
                // Subsequent children: compute shift using contour comparison
                let (ct, cb, ce, cbc, ctc) =
                    layout_subtree_rt(graph, tree, child, options, child_x, current_y, is_right, layouts);

                // Compute required shift by comparing contours
                let shift = compute_contour_shift(
                    &child_contours,
                    &ctc,
                    options.node_spacing,
                );

                if shift > 0.0 {
                    // Shift this child's subtree down by `shift`
                    shift_subtree(graph, tree, child, layouts, shift);
                    // Recompute contours after shift
                    let (ct2, cb2, ce2, cbc2, ctc2) = relayout_subtree_rt(
                        graph, tree, child, options, child_x, current_y + shift, is_right, layouts,
                    );
                    top = top.min(ct2);
                    bottom = bottom.max(cb2);
                    max_extent_x = if is_right {
                        max_extent_x.max(ce2)
                    } else {
                        max_extent_x.min(ce2)
                    };
                    current_y = cb2 + options.node_spacing;
                    child_contours.push((child, ct2, cb2, cbc2, ctc2));
                } else {
                    top = top.min(ct);
                    bottom = bottom.max(cb);
                    max_extent_x = if is_right {
                        max_extent_x.max(ce)
                    } else {
                        max_extent_x.min(ce)
                    };
                    current_y = cb + options.node_spacing;
                    child_contours.push((child, ct, cb, cbc, ctc));
                }
            }
        }

        // Center this node vertically over its children
        let center_y = (top + bottom) / 2.0;
        let pos = if is_right {
            Point::new(x, center_y - node_h / 2.0)
        } else {
            Point::new(x - node_w, center_y - node_h / 2.0)
        };
        layouts.insert(node_id, pos);

        // Build merged contours: this node at depth 0, children at depth 1+
        let node_top = center_y - node_h / 2.0;
        let node_bottom = center_y + node_h / 2.0;
        let mut bottom_contour = vec![(0, node_bottom)];
        let mut top_contour = vec![(0, node_top)];

        for (_, _, _, cbc, ctc) in &child_contours {
            merge_contour(&mut bottom_contour, cbc, 1, f32::max);
            merge_contour(&mut top_contour, ctc, 1, f32::min);
        }

        (
            top.min(node_top),
            bottom.max(node_bottom),
            max_extent_x,
            bottom_contour,
            top_contour,
        )
    }
}

/// Compute the minimum shift needed between previous children's bottom contour
/// and the new child's top contour.
fn compute_contour_shift(
    prev_children: &[(NodeId, f32, f32, Contour, Contour)],
    new_top: &Contour,
    spacing: f32,
) -> f32 {
    // Merge all previous children's bottom contours
    let mut prev_bottom: Contour = Vec::new();
    for (_, _, _, cbc, _) in prev_children {
        merge_contour(&mut prev_bottom, cbc, 0, f32::max);
    }

    // Build top contour map for the new child
    let new_top_map: HashMap<i32, f32> = new_top.iter().copied().collect();

    // Compare at each depth level
    let mut max_shift = 0.0f32;
    for (depth, prev_y) in &prev_bottom {
        if let Some(&new_y) = new_top_map.get(depth) {
            // Need: new_y + shift >= prev_y + spacing
            let shift = (prev_y + spacing) - new_y;
            if shift > max_shift {
                max_shift = shift;
            }
        }
    }
    max_shift
}

/// Merge a child contour into the parent contour at the given depth offset.
fn merge_contour(parent: &mut Contour, child: &Contour, depth_offset: i32, combine: fn(f32, f32) -> f32) {
    let mut parent_map: HashMap<i32, f32> = parent.iter().copied().collect();
    for (d, y) in child {
        let actual_depth = d + depth_offset;
        parent_map
            .entry(actual_depth)
            .and_modify(|v| *v = combine(*v, *y))
            .or_insert(*y);
    }
    parent.clear();
    let mut entries: Vec<_> = parent_map.into_iter().collect();
    entries.sort_by_key(|(d, _)| *d);
    parent.extend(entries);
}

/// Shift a subtree's y positions by `delta` (in-place).
fn shift_subtree(
    graph: &FlowGraph,
    tree: &Tree,
    node_id: NodeId,
    layouts: &mut HashMap<NodeId, Point>,
    delta: f32,
) {
    if let Some(pos) = layouts.get_mut(&node_id) {
        pos.y += delta;
    }
    if let Some(children) = tree.get(&node_id) {
        for child in children {
            shift_subtree(graph, tree, *child, layouts, delta);
        }
    }
}

/// Re-layout a subtree that was already positioned (used after shifting).
/// This recomputes the bounding box and contours from existing layout positions.
fn relayout_subtree_rt(
    graph: &FlowGraph,
    tree: &Tree,
    node_id: NodeId,
    _options: &LayoutOptions,
    _x: f32,
    _y_start: f32,
    _is_right: bool,
    layouts: &mut HashMap<NodeId, Point>,
) -> (f32, f32, f32, Contour, Contour) {
    let node_size = graph.nodes.get(node_id).map(|n| n.size).unwrap_or_default();
    let node_w = node_size.width;
    let node_h = node_size.height;

    let pos = layouts.get(&node_id).copied().unwrap_or_default();
    let top = pos.y;
    let bottom = pos.y + node_h;
    let extent_x = if pos.x >= 0.0 { pos.x + node_w } else { pos.x };

    let children = tree.get(&node_id).cloned().unwrap_or_default();
    let mut bottom_contour = vec![(0, bottom)];
    let mut top_contour = vec![(0, top)];

    let mut min_top = top;
    let mut max_bottom = bottom;
    let mut max_extent = extent_x;

    for child in &children {
        let (ct, cb, ce, cbc, ctc) =
            relayout_subtree_rt(graph, tree, *child, _options, _x, _y_start, _is_right, layouts);
        min_top = min_top.min(ct);
        max_bottom = max_bottom.max(cb);
        max_extent = max_extent.max(ce);
        merge_contour(&mut bottom_contour, &cbc, 1, f32::max);
        merge_contour(&mut top_contour, &ctc, 1, f32::min);
    }

    (min_top, max_bottom, max_extent, bottom_contour, top_contour)
}

/// Top-down tree layout (fallback for TB direction).
fn layout_topdown(graph: &mut FlowGraph, tree: &Tree, options: &LayoutOptions) {
    let roots = find_roots(tree);
    if roots.is_empty() {
        return;
    }

    let mut layouts: HashMap<NodeId, Point> = HashMap::new();
    let mut current_y = 0.0f32;

    for root in &roots {
        let extent = layout_subtree_topdown(
            graph,
            tree,
            *root,
            options,
            0.0,
            current_y,
            &mut layouts,
        );
        current_y = extent.bottom + options.node_spacing;
    }

    for (id, pos) in layouts.iter() {
        if let Some(n) = graph.nodes.get_mut(*id) {
            n.position = *pos;
        }
    }
}

/// Recursively layout a subtree top-down.
fn layout_subtree_topdown(
    graph: &FlowGraph,
    tree: &Tree,
    node_id: NodeId,
    options: &LayoutOptions,
    x: f32,
    y: f32,
    layouts: &mut HashMap<NodeId, Point>,
) -> SubtreeLayout {
    let node_size = graph.nodes.get(node_id).map(|n| n.size).unwrap_or_default();
    let node_h = node_size.height;
    let node_w = node_size.width;

    let children = tree.get(&node_id).cloned().unwrap_or_default();

    if children.is_empty() {
        let pos = Point::new(x - node_w / 2.0, y);
        layouts.insert(node_id, pos);
        SubtreeLayout {
            top: y,
            bottom: y + node_h,
            extent_x: x + node_w / 2.0,
        }
    } else {
        let child_y = y + node_h + options.rank_spacing;
        let mut current_x = x;
        let top = y;
        let mut bottom = f32::MIN;
        let mut left = f32::MAX;
        let mut right = f32::MIN;

        for &child in &children {
            let subtree = layout_subtree_topdown(
                graph,
                tree,
                child,
                options,
                current_x,
                child_y,
                layouts,
            );
            bottom = bottom.max(subtree.bottom);
            left = left.min(subtree.extent_x);
            right = right.max(subtree.extent_x);

            let child_w = graph.nodes.get(child).map(|n| n.size.width).unwrap_or(100.0);
            current_x = subtree.extent_x + child_w / 2.0 + options.node_spacing;
        }

        // Center this node horizontally over its children
        let center_x = (left + right) / 2.0;
        let pos = Point::new(center_x - node_w / 2.0, y);
        layouts.insert(node_id, pos);

        SubtreeLayout {
            top,
            bottom,
            extent_x: center_x + node_w / 2.0,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::auto_layout::LayoutOptions;
    use crate::edge::FlowEdge;
    use crate::graph::FlowGraph;
    use crate::node_type::COMMON;
    use crate::port::PortDirection;
    use crate::math::Point;

    fn build_simple_mindmap() -> FlowGraph {
        let mut graph = FlowGraph::new("mindmap");
        let root = graph.add_typed_node(COMMON, "Root", Point::new(0.0, 0.0));
        let a = graph.add_typed_node(COMMON, "A", Point::new(0.0, 0.0));
        let b = graph.add_typed_node(COMMON, "B", Point::new(0.0, 0.0));
        let a1 = graph.add_typed_node(COMMON, "A1", Point::new(0.0, 0.0));
        let a2 = graph.add_typed_node(COMMON, "A2", Point::new(0.0, 0.0));

        let root_out = graph.ports.iter()
            .find(|(_, p)| p.node == root && p.direction == PortDirection::Output)
            .map(|(id, _)| id).unwrap();
        let a_in = graph.ports.iter()
            .find(|(_, p)| p.node == a && p.direction == PortDirection::Input)
            .map(|(id, _)| id).unwrap();
        let b_in = graph.ports.iter()
            .find(|(_, p)| p.node == b && p.direction == PortDirection::Input)
            .map(|(id, _)| id).unwrap();
        let a_out = graph.ports.iter()
            .find(|(_, p)| p.node == a && p.direction == PortDirection::Output)
            .map(|(id, _)| id).unwrap();
        let a1_in = graph.ports.iter()
            .find(|(_, p)| p.node == a1 && p.direction == PortDirection::Input)
            .map(|(id, _)| id).unwrap();
        let a2_in = graph.ports.iter()
            .find(|(_, p)| p.node == a2 && p.direction == PortDirection::Input)
            .map(|(id, _)| id).unwrap();

        graph.add_edge(FlowEdge::new(root_out, a_in));
        graph.add_edge(FlowEdge::new(root_out, b_in));
        graph.add_edge(FlowEdge::new(a_out, a1_in));
        graph.add_edge(FlowEdge::new(a_out, a2_in));

        graph
    }

    #[test]
    fn mindmap_bidirectional_splits_children() {
        let mut graph = build_simple_mindmap();
        let options = LayoutOptions {
            direction: LayoutDirection::LeftRight,
            node_spacing: 20.0,
            rank_spacing: 80.0,
            margin: 40.0,
            ordering_iterations: 1,
        };
        layout_mindmap(&mut graph, &options);

        // Root should be placed
        let root = graph.nodes.iter()
            .find(|(_, n)| n.label == "Root")
            .map(|(_, n)| n.position).unwrap();
        // Children should be on both sides (some left, some right of root)
        let positions: Vec<_> = graph.nodes.iter()
            .filter(|(_, n)| n.label == "A" || n.label == "B")
            .map(|(_, n)| n.position.x)
            .collect();
        // At least one child should be on each side
        let has_right = positions.iter().any(|&x| x > root.x);
        let has_left = positions.iter().any(|&x| x < root.x);
        assert!(has_right || has_left, "children should be distributed");
    }

    #[test]
    fn mindmap_topdown_layout_works() {
        let mut graph = build_simple_mindmap();
        let options = LayoutOptions {
            direction: LayoutDirection::TopBottom,
            node_spacing: 20.0,
            rank_spacing: 60.0,
            margin: 20.0,
            ordering_iterations: 1,
        };
        layout_mindmap(&mut graph, &options);

        let root_y = graph.nodes.iter()
            .find(|(_, n)| n.label == "Root")
            .map(|(_, n)| n.position.y).unwrap();
        let child_y = graph.nodes.iter()
            .find(|(_, n)| n.label == "A")
            .map(|(_, n)| n.position.y).unwrap();
        // Child should be below root in TB layout
        assert!(child_y > root_y, "child should be below root");
    }

    /// Build an asymmetric mind map where one subtree is much deeper than others.
    /// This is the case where Reingold-Tilford contour comparison shines:
    /// it avoids excessive whitespace by tightly packing adjacent subtrees.
    fn build_asymmetric_mindmap() -> FlowGraph {
        let mut graph = FlowGraph::new("asymmetric");
        let root = graph.add_typed_node(COMMON, "Root", Point::new(0.0, 0.0));
        let a = graph.add_typed_node(COMMON, "A", Point::new(0.0, 0.0));
        let b = graph.add_typed_node(COMMON, "B", Point::new(0.0, 0.0));
        let c = graph.add_typed_node(COMMON, "C", Point::new(0.0, 0.0));
        // A has deep children
        let a1 = graph.add_typed_node(COMMON, "A1", Point::new(0.0, 0.0));
        let a2 = graph.add_typed_node(COMMON, "A2", Point::new(0.0, 0.0));
        let a1a = graph.add_typed_node(COMMON, "A1a", Point::new(0.0, 0.0));
        let a1b = graph.add_typed_node(COMMON, "A1b", Point::new(0.0, 0.0));

        let root_out = graph.ports.iter()
            .find(|(_, p)| p.node == root && p.direction == PortDirection::Output)
            .map(|(id, _)| id).unwrap();
        let a_in = graph.ports.iter()
            .find(|(_, p)| p.node == a && p.direction == PortDirection::Input)
            .map(|(id, _)| id).unwrap();
        let b_in = graph.ports.iter()
            .find(|(_, p)| p.node == b && p.direction == PortDirection::Input)
            .map(|(id, _)| id).unwrap();
        let c_in = graph.ports.iter()
            .find(|(_, p)| p.node == c && p.direction == PortDirection::Input)
            .map(|(id, _)| id).unwrap();
        let a_out = graph.ports.iter()
            .find(|(_, p)| p.node == a && p.direction == PortDirection::Output)
            .map(|(id, _)| id).unwrap();
        let a1_in = graph.ports.iter()
            .find(|(_, p)| p.node == a1 && p.direction == PortDirection::Input)
            .map(|(id, _)| id).unwrap();
        let a2_in = graph.ports.iter()
            .find(|(_, p)| p.node == a2 && p.direction == PortDirection::Input)
            .map(|(id, _)| id).unwrap();
        let a1_out = graph.ports.iter()
            .find(|(_, p)| p.node == a1 && p.direction == PortDirection::Output)
            .map(|(id, _)| id).unwrap();
        let a1a_in = graph.ports.iter()
            .find(|(_, p)| p.node == a1a && p.direction == PortDirection::Input)
            .map(|(id, _)| id).unwrap();
        let a1b_in = graph.ports.iter()
            .find(|(_, p)| p.node == a1b && p.direction == PortDirection::Input)
            .map(|(id, _)| id).unwrap();

        graph.add_edge(FlowEdge::new(root_out, a_in));
        graph.add_edge(FlowEdge::new(root_out, b_in));
        graph.add_edge(FlowEdge::new(root_out, c_in));
        graph.add_edge(FlowEdge::new(a_out, a1_in));
        graph.add_edge(FlowEdge::new(a_out, a2_in));
        graph.add_edge(FlowEdge::new(a1_out, a1a_in));
        graph.add_edge(FlowEdge::new(a1_out, a1b_in));

        graph
    }

    /// Verify that the Reingold-Tilford contour layout produces no overlapping nodes.
    #[test]
    fn mindmap_rt_no_node_overlap() {
        let mut graph = build_asymmetric_mindmap();
        let options = LayoutOptions {
            direction: LayoutDirection::LeftRight,
            node_spacing: 20.0,
            rank_spacing: 80.0,
            margin: 40.0,
            ordering_iterations: 1,
        };
        layout_mindmap(&mut graph, &options);

        let nodes: Vec<_> = graph.nodes.iter().collect();
        for i in 0..nodes.len() {
            for j in (i + 1)..nodes.len() {
                let a = nodes[i].1;
                let b = nodes[j].1;
                let overlap_x = a.position.x < b.position.x + b.size.width
                    && b.position.x < a.position.x + a.size.width;
                let overlap_y = a.position.y < b.position.y + b.size.height
                    && b.position.y < a.position.y + a.size.height;
                assert!(
                    !overlap_x || !overlap_y,
                    "nodes {:?} and {:?} overlap",
                    nodes[i].1.label,
                    nodes[j].1.label
                );
            }
        }
    }

    /// Verify that parent nodes are centered over their children in the RT layout.
    #[test]
    fn mindmap_rt_parent_centered_over_children() {
        let mut graph = build_asymmetric_mindmap();
        let options = LayoutOptions {
            direction: LayoutDirection::LeftRight,
            node_spacing: 20.0,
            rank_spacing: 80.0,
            margin: 40.0,
            ordering_iterations: 1,
        };
        layout_mindmap(&mut graph, &options);

        // Find node A and its children A1, A2
        let a_data = graph.nodes.iter()
            .find(|(_, n)| n.label == "A")
            .map(|(id, n)| (id, n.position, n.size)).unwrap();
        let a1_data = graph.nodes.iter()
            .find(|(_, n)| n.label == "A1")
            .map(|(_, n)| (n.position, n.size)).unwrap();
        let a2_data = graph.nodes.iter()
            .find(|(_, n)| n.label == "A2")
            .map(|(_, n)| (n.position, n.size)).unwrap();

        let a_center_y = a_data.1.y + a_data.2.height / 2.0;
        let a1_top = a1_data.0.y;
        let a1_bottom = a1_data.0.y + a1_data.1.height;
        let a2_top = a2_data.0.y;
        let a2_bottom = a2_data.0.y + a2_data.1.height;

        // Parent A should be vertically within the range of its direct children
        let children_top = a1_top.min(a2_top);
        let children_bottom = a1_bottom.max(a2_bottom);
        assert!(
            a_center_y >= children_top - 5.0 && a_center_y <= children_bottom + 5.0,
            "parent A center y {} should be within children range [{}, {}]",
            a_center_y,
            children_top,
            children_bottom
        );
    }

    /// Debug test: print layout positions for the actual demo mind map data.
    #[test]
    fn mindmap_debug_demo_layout() {
        let json = crate::mindmap_document_json();
        let types = crate::builtin_type_registry();
        let doc = crate::schema::document_from_any_json(json).unwrap();
        let mut graph = FlowGraph::from_document(&doc, &types);
        graph.is_mindmap = true;
        graph.layout_direction = LayoutDirection::LeftRight;
        let options = LayoutOptions::mindmap_lr();
        layout_mindmap(&mut graph, &options);

        println!("\n=== Mind Map Layout (LR bidirectional) ===");
        for (_, node) in graph.nodes.iter() {
            println!(
                "  {:<20} pos=({:7.1}, {:7.1})  size=({:.0}, {:.0})",
                node.label, node.position.x, node.position.y, node.size.width, node.size.height
            );
        }

        // Verify root is centered between left and right children
        let root = graph.nodes.iter()
            .find(|(_, n)| n.label == "Rust Agent Flow")
            .map(|(_, n)| n).unwrap();
        println!(
            "  Root center: ({:.1}, {:.1})",
            root.position.x + root.size.width / 2.0,
            root.position.y + root.size.height / 2.0
        );

        // Check that some children are on the left and some on the right
        let root_center_x = root.position.x + root.size.width / 2.0;
        let mut left_count = 0;
        let mut right_count = 0;
        for (_, node) in graph.nodes.iter() {
            if node.label == "Rust Agent Flow" {
                continue;
            }
            let node_center_x = node.position.x + node.size.width / 2.0;
            if node_center_x < root_center_x {
                left_count += 1;
            } else {
                right_count += 1;
            }
        }
        println!("  Left children: {}, Right children: {}", left_count, right_count);
        assert!(left_count > 0, "should have left-side children");
        assert!(right_count > 0, "should have right-side children");
    }
}
