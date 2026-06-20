//! Coordinate assignment — dagre `position/index.js` + `position/bk.js` port.
//!
//! **Y (`positionY`)**: per-rank stacking — `y = prev_y + max_height/2`,
//! `prev_y += max_height + ranksep`.
//!
//! **X (`bk.positionX`)**: Brandes-Köpf "Fast and Simple Horizontal Coordinate
//! Assignment":
//! 1. Find type-1 conflicts (crossings involving inner/dummy segments).
//! 2. Four-direction sweep: upper/lower × left/right → `xss.ul/.ur/.dl/.dr`.
//! 3. `verticalAlignment` (align to upper/lower neighbor, left/right).
//! 4. `horizontalCompaction` (`placeBlock` + `sink`/`shift` overlap resolution).
//! 5. `findSmallestWidthAlignment` + `alignCoordinates`.
//! 6. `balance`: median of the 4 alignments (dagre default when `align` unset).
//!
//! All coordinates are computed in the internal TB orientation (rank → Y,
//! order → X). The pipeline transforms to LR at the end.

use std::collections::{HashMap, HashSet};

use crate::auto_layout::mermaid_dagre::graph::LayoutGraph;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Align {
    UpLeft,
    UpRight,
    DownLeft,
    DownRight,
    /// Median of the four alignments (dagre default).
    Balance,
}

impl Default for Align {
    fn default() -> Self {
        Self::Balance
    }
}

/// Assign final `x`/`y` (center coordinates) to every node.
pub fn run(g: &mut LayoutGraph, align: Align) {
    position_y(g);
    let xs = position_x(g, align);
    for (i, x) in xs.into_iter().enumerate() {
        g.nodes[i].x = x;
    }
}

/// Per-rank Y stacking. Top-aligns each rank (dagre default).
fn position_y(g: &mut LayoutGraph) {
    let max_rank = g.max_rank();
    let layers = g.layers();

    let mut y = g.marginy;
    for rank in 0..=max_rank {
        let layer = &layers[rank as usize];
        if layer.is_empty() {
            continue;
        }
        let max_h = layer
            .iter()
            .map(|&i| g.nodes[i].height)
            .fold(0.0f32, f32::max);
        let center = y + max_h * 0.5;
        for &i in layer {
            g.nodes[i].y = center;
        }
        y += max_h + g.ranksep;
    }
}

/// Brandes-Köpf X assignment with median balance.
fn position_x(g: &LayoutGraph, align: Align) -> Vec<f32> {
    let n = g.nodes.len();

    if align != Align::Balance {
        let (down, right) = align_to_dir(align);
        return bk(g, down, right);
    }

    // Compute all four alignments.
    let ul = bk(g, true, false);
    let ur = bk(g, true, true);
    let dl = bk(g, false, false);
    let dr = bk(g, false, true);

    // Align all four to a common min, then balance by median.
    let alignments = [ul, ur, dl, dr];
    let mins: Vec<f32> = alignments
        .iter()
        .map(|a| a.iter().copied().filter(|v| v.is_finite()).fold(f32::MAX, f32::min))
        .collect();
    let global_min = mins.iter().copied().fold(f32::MAX, f32::min);
    let mut shifted: [Vec<f32>; 4] = [Vec::new(), Vec::new(), Vec::new(), Vec::new()];
    for (k, a) in alignments.iter().enumerate() {
        let delta = global_min - mins[k];
        shifted[k] = a.iter().map(|&x| x + delta).collect();
    }

    let mut xs = vec![0.0f32; n];
    for i in 0..n {
        let mut vals: [f32; 4] = [
            shifted[0][i],
            shifted[1][i],
            shifted[2][i],
            shifted[3][i],
        ];
        vals.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
        // Median of 4 = (vals[1] + vals[2]) / 2 — dagre `balance`.
        xs[i] = (vals[1] + vals[2]) * 0.5;
    }
    xs
}

fn align_to_dir(align: Align) -> (bool, bool) {
    // (down, right)
    match align {
        Align::UpLeft => (true, false),
        Align::UpRight => (true, true),
        Align::DownLeft => (false, false),
        Align::DownRight => (false, true),
        Align::Balance => (true, false),
    }
}

/// One Brandes-Köpf pass: vertical alignment + horizontal compaction.
/// `down = true` → align to upper neighbor (rank-1); else lower (rank+1).
/// `right = true` → rightmost alignment (place blocks rightward).
fn bk(g: &LayoutGraph, down: bool, right: bool) -> Vec<f32> {
    let n = g.nodes.len();
    let max_rank = g.max_rank();
    let layers = g.layers();

    let conflicts = mark_type1_conflicts(g, &layers);

    // `root[v]`: head of v's aligned block. `align_map[v]`: next node in block.
    let mut root: Vec<usize> = (0..n).collect();
    let mut align_map: Vec<usize> = (0..n).collect();

    let rank_iter: Vec<i32> = if down {
        (0..=max_rank).collect()
    } else {
        (0..=max_rank).rev().collect()
    };

    for rank in rank_iter {
        let layer = &layers[rank as usize];
        if layer.is_empty() {
            continue;
        }
        let mut med = if right {
            usize::MAX
        } else {
            0usize
        };
        // Iterate layer left-to-right (or right-to-left when `right`).
        let positions: Vec<usize> = if right {
            layer.iter().rev().copied().collect()
        } else {
            layer.iter().copied().collect()
        };
        for &v in &positions {
            let neighbors = upper_or_lower_neighbors(g, v, down);
            if neighbors.is_empty() {
                continue;
            }
            let meds = medians(&neighbors, g);
            for &m in &meds {
                if align_map[v] != v {
                    continue; // already aligned
                }
                let m_pos = g.nodes[m].order;
                let ok = if right {
                    m_pos <= med
                } else {
                    m_pos >= med
                };
                if !ok {
                    continue;
                }
                if has_conflict(&conflicts, m, v) {
                    continue;
                }
                // Align: v becomes the "block head" side; m points to v.
                align_map[m] = v;
                root[v] = root_of(&root, m);
                align_map[v] = root_of(&root, m);
                med = m_pos;
            }
        }
    }

    // Horizontal compaction: place each aligned block as a column.
    //
    // We replace dagre's fragile `placeBlock`/`sink`/`shift` mechanism with a
    // direct, provably-non-overlapping block placement:
    //   1. Group nodes into blocks by their alignment root.
    //   2. Order blocks by the min `order` of their members.
    //   3. Place blocks left-to-right (or right-to-left); each block's x is the
    //      max over every rank it occupies of (right edge of the previous block
    //      in that rank + separation). This is the BK separation constraint
    //      applied directly, guaranteeing no same-rank overlap.
    let xs = compact_blocks(g, &root, &layers, right);

    // Normalize: ensure non-negative, apply left margin.
    let min_x = xs
        .iter()
        .copied()
        .filter(|v| v.is_finite())
        .fold(f32::MAX, f32::min);
    let mut xs = xs;
    if min_x.is_finite() {
        for v in 0..n {
            xs[v] = xs[v] - min_x + g.marginx;
        }
    }
    for v in 0..n {
        if !xs[v].is_finite() {
            xs[v] = g.marginx;
        }
    }
    xs
}

/// Direct block compaction. Each aligned block (chain of nodes sharing a root)
/// is placed as a vertical column. Blocks are ordered by their topmost member's
/// `order` and placed so that same-rank nodes never overlap.
fn compact_blocks(
    g: &LayoutGraph,
    root: &[usize],
    layers: &[Vec<usize>],
    right: bool,
) -> Vec<f32> {
    let n = g.nodes.len();
    let max_rank = layers.len() as i32 - 1;

    // Group members by block root.
    let mut blocks: HashMap<usize, Vec<usize>> = HashMap::new();
    for v in 0..n {
        let r = root_of(root, v);
        blocks.entry(r).or_default().push(v);
    }

    // For each block, record per-rank max half-width and the block's ordering
    // key (min member order).
    let mut block_info: Vec<(usize, Vec<(i32, f32)>)> = Vec::new();
    for (&r, members) in &blocks {
        let mut rank_halfw: HashMap<i32, f32> = HashMap::new();
        let mut min_order = usize::MAX;
        for &v in members {
            let rank = g.nodes[v].rank;
            let halfw = if g.nodes[v].is_dummy {
                0.0
            } else {
                g.nodes[v].width * 0.5
            };
            let entry = rank_halfw.entry(rank).or_insert(0.0);
            if halfw > *entry {
                *entry = halfw;
            }
            if g.nodes[v].order < min_order {
                min_order = g.nodes[v].order;
            }
        }
        let ranks: Vec<(i32, f32)> = rank_halfw.into_iter().collect();
        block_info.push((r, ranks));
    }

    // Order blocks by min member order (stable). For right placement, reverse.
    block_info.sort_by_key(|(r, _)| {
        let key = blocks
            .get(r)
            .map(|m| m.iter().map(|&v| g.nodes[v].order).min().unwrap_or(0))
            .unwrap_or(0);
        key
    });
    if right {
        block_info.reverse();
    }

    let mut x = vec![f32::NAN; n];
    // Per-rank right edge (for left placement) / left edge (for right placement)
    // of the most recently placed block occupying that rank.
    let mut rank_edge: HashMap<i32, f32> = HashMap::new();

    for (r, ranks) in &block_info {
        // Compute this block's x as the max constraint over its ranks.
        let mut bx = if right {
            f32::INFINITY
        } else {
            f32::NEG_INFINITY
        };
        for &(rank, halfw) in ranks {
            let gap = block_gap(g, rank);
            if right {
                let edge = rank_edge.get(&rank).copied().unwrap_or(f32::INFINITY);
                let candidate = edge - gap - halfw;
                if candidate < bx {
                    bx = candidate;
                }
            } else {
                let edge = rank_edge.get(&rank).copied().unwrap_or(f32::NEG_INFINITY);
                let candidate = edge + gap + halfw;
                if candidate > bx {
                    bx = candidate;
                }
            }
        }
        if !bx.is_finite() {
            bx = if right { 0.0 } else { 0.0 };
        }
        // Assign x to every member.
        if let Some(members) = blocks.get(r) {
            for &v in members {
                x[v] = bx;
            }
        }
        // Update rank edges.
        for &(rank, halfw) in ranks {
            if right {
                let entry = rank_edge.entry(rank).or_insert(f32::INFINITY);
                if bx - halfw < *entry {
                    *entry = bx - halfw;
                }
            } else {
                let entry = rank_edge.entry(rank).or_insert(f32::NEG_INFINITY);
                if bx + halfw > *entry {
                    *entry = bx + halfw;
                }
            }
        }
    }

    // Map every node to its root's x.
    let mut xs = vec![0.0f32; n];
    for v in 0..n {
        let r = root_of(root, v);
        xs[v] = x[r];
    }
    let _ = max_rank;
    xs
}

/// Gap between adjacent nodes in a rank: nodesep for real-real, edgesep when
/// dummies are involved (dagre `sep()` semantics).
fn block_gap(g: &LayoutGraph, _rank: i32) -> f32 {
    // Conservative: use nodesep (real nodes dominate flowcharts). Dummies use
    // edgesep but nodesep >= edgesep typically, so nodesep is a safe bound.
    g.nodesep.max(g.edgesep)
}

fn root_of(root: &[usize], v: usize) -> usize {
    let mut r = v;
    while root[r] != r {
        r = root[r];
    }
    r
}

/// Neighbors on the alignment side: rank-1 (upper) if `down`, else rank+1 (lower).
fn upper_or_lower_neighbors(g: &LayoutGraph, v: usize, down: bool) -> Vec<usize> {
    if down {
        g.preds(v)
    } else {
        g.succs(v)
    }
}

/// Median neighbor(s) by position. Returns 1 or 2 elements.
fn medians(neighbors: &[usize], g: &LayoutGraph) -> Vec<usize> {
    if neighbors.is_empty() {
        return Vec::new();
    }
    let mut sorted: Vec<usize> = neighbors.to_vec();
    sorted.sort_by_key(|&u| g.nodes[u].order);
    let m = sorted.len();
    if m % 2 == 1 {
        vec![sorted[m / 2]]
    } else {
        vec![sorted[m / 2 - 1], sorted[m / 2]]
    }
}

/// Type-1 conflict set: pairs (u, v) of nodes in adjacent ranks whose incident
/// inner edge segments cross. Inner = both endpoints dummy.
fn mark_type1_conflicts(g: &LayoutGraph, layers: &[Vec<usize>]) -> HashSet<(usize, usize)> {
    let mut conflicts: HashSet<(usize, usize)> = HashSet::new();
    let max_rank = layers.len() as i32 - 1;
    for rank in 0..max_rank {
        let upper = &layers[rank as usize];
        // Collect segments (u in rank, v in rank+1) with their orders.
        let mut segs: Vec<(usize, usize, usize, usize)> = Vec::new(); // (u_order, u, v_order, v)
        for &u in upper {
            for (_, e) in g.out_edges(u) {
                let v = e.to;
                if g.nodes[v].rank == rank + 1 {
                    segs.push((g.nodes[u].order, u, g.nodes[v].order, v));
                }
            }
            for (_, e) in g.in_edges(u) {
                let v = e.from;
                if g.nodes[v].rank == rank + 1 {
                    segs.push((g.nodes[u].order, u, g.nodes[v].order, v));
                }
            }
        }
        for i in 0..segs.len() {
            for j in (i + 1)..segs.len() {
                let (au_ox, au, av_ox, av) = segs[i];
                let (bu_ox, bu, bv_ox, bv) = segs[j];
                let crosses = (au_ox < bu_ox && av_ox > bv_ox)
                    || (au_ox > bu_ox && av_ox < bv_ox);
                if !crosses {
                    continue;
                }
                let inner_i = g.nodes[au].is_dummy && g.nodes[av].is_dummy;
                let inner_j = g.nodes[bu].is_dummy && g.nodes[bv].is_dummy;
                if inner_i && !inner_j {
                    conflicts.insert((bu, bv));
                    conflicts.insert((bv, bu));
                } else if inner_j && !inner_i {
                    conflicts.insert((au, av));
                    conflicts.insert((av, au));
                }
            }
        }
    }
    conflicts
}

fn has_conflict(conflicts: &HashSet<(usize, usize)>, a: usize, b: usize) -> bool {
    conflicts.contains(&(a, b))
}

/// dagre `placeBlock`. Places block rooted at `v` at minimum x, linking to
/// adjacent blocks via `sink`/`shift` to enforce separation.
fn place_block(
    v: usize,
    g: &LayoutGraph,
    root: &[usize],
    align_map: &[usize],
    x: &mut [f32],
    sink: &mut [usize],
    shift: &mut [f32],
    right: bool,
) {
    if !x[v].is_nan() {
        return;
    }
    x[v] = 0.0;
    let mut w = v;
    loop {
        // The neighbor of w in its layer on the placement side.
        let neighbor = layer_neighbor(g, w, right);
        if let Some(pred) = neighbor {
            let u = root_of(root, pred);
            place_block(u, g, root, align_map, x, sink, shift, right);
            // Link blocks: if sink[v] == v, point it at u.
            if sink[v] == v {
                sink[v] = u;
            }
            if sink[v] != sink[u] {
                // Blocks are in different chains: record a shift to separate them.
                let sep = separation(g, u, w);
                if right {
                    // Placing rightward: u is to the right of v's block.
                    let delta = x[v] - x[u] - sep;
                    if delta < shift[sink[u]] {
                        shift[sink[u]] = delta;
                    }
                } else {
                    let delta = x[v] - x[u] - sep;
                    if -delta < shift[sink[u]] {
                        shift[sink[u]] = -delta;
                    }
                    // Ensure v stays to the right of u.
                    let min_x = x[u] + sep;
                    if x[v] < min_x {
                        x[v] = min_x;
                    }
                }
            } else {
                // Same chain: enforce separation directly.
                let sep = separation(g, u, w);
                if right {
                    let max_x = x[u] - sep;
                    if x[v] > max_x {
                        x[v] = max_x;
                    }
                } else {
                    let min_x = x[u] + sep;
                    if x[v] < min_x {
                        x[v] = min_x;
                    }
                }
            }
        }
        w = align_map[w];
        if w == v {
            break;
        }
    }
}

/// The immediate layer-neighbor of `w` on the placement side:
/// left placement → the node immediately to w's left in its layer;
/// right placement → the node immediately to w's right.
fn layer_neighbor(g: &LayoutGraph, w: usize, right: bool) -> Option<usize> {
    let rank = g.nodes[w].rank;
    if rank < 0 {
        return None;
    }
    let mut layer: Vec<usize> = g
        .nodes
        .iter()
        .enumerate()
        .filter(|(_, n)| n.rank == rank)
        .map(|(i, _)| i)
        .collect();
    layer.sort_by_key(|&i| g.nodes[i].order);
    let pos = layer.iter().position(|&i| i == w)?;
    if right {
        if pos + 1 < layer.len() {
            Some(layer[pos + 1])
        } else {
            None
        }
    } else if pos > 0 {
        Some(layer[pos - 1])
    } else {
        None
    }
}

/// Separation between two adjacent nodes in a block: half-widths + nodesep
/// (real nodes) or edgesep (dummies), matching dagre's `sep()`.
fn separation(g: &LayoutGraph, a: usize, b: usize) -> f32 {
    let half_a = if g.nodes[a].is_dummy {
        0.0
    } else {
        g.nodes[a].width * 0.5
    };
    let half_b = if g.nodes[b].is_dummy {
        0.0
    } else {
        g.nodes[b].width * 0.5
    };
    let gap = if g.nodes[a].is_dummy && g.nodes[b].is_dummy {
        g.edgesep
    } else {
        g.nodesep
    };
    half_a + half_b + gap
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::auto_layout::mermaid_dagre::graph::LayoutGraph;
    use crate::id::NodeId;

    #[test]
    fn position_assigns_increasing_y_per_rank() {
        let mut g = LayoutGraph::new();
        g.ranksep = 50.0;
        g.marginy = 8.0;
        let a = g.add_real_node(NodeId::default(), 40.0, 30.0);
        let b = g.add_real_node(NodeId::default(), 40.0, 30.0);
        let c = g.add_real_node(NodeId::default(), 40.0, 30.0);
        g.nodes[a].rank = 0;
        g.nodes[b].rank = 1;
        g.nodes[c].rank = 2;
        g.add_edge(a, b);
        g.add_edge(b, c);
        g.nodes[a].order = 0;
        g.nodes[b].order = 0;
        g.nodes[c].order = 0;
        run(&mut g, Align::Balance);
        assert!(g.nodes[a].y < g.nodes[b].y);
        assert!(g.nodes[b].y < g.nodes[c].y);
    }

    #[test]
    fn position_separates_siblings_horizontally() {
        let mut g = LayoutGraph::new();
        g.nodesep = 50.0;
        g.edgesep = 20.0;
        g.marginx = 8.0;
        let a = g.add_real_node(NodeId::default(), 40.0, 30.0);
        let b = g.add_real_node(NodeId::default(), 40.0, 30.0);
        let c = g.add_real_node(NodeId::default(), 40.0, 30.0);
        let d = g.add_real_node(NodeId::default(), 40.0, 30.0);
        g.nodes[a].rank = 0;
        g.nodes[b].rank = 0;
        g.nodes[c].rank = 1;
        g.nodes[d].rank = 1;
        g.add_edge(a, c);
        g.add_edge(b, d);
        g.nodes[a].order = 0;
        g.nodes[b].order = 1;
        g.nodes[c].order = 0;
        g.nodes[d].order = 1;
        run(&mut g, Align::Balance);
        let gap = (g.nodes[a].x - g.nodes[b].x).abs();
        assert!(gap >= 40.0 + 50.0 - 1.0, "a.x={} b.x={} gap={}", g.nodes[a].x, g.nodes[b].x, gap);
        let gap2 = (g.nodes[c].x - g.nodes[d].x).abs();
        assert!(gap2 >= 40.0 + 50.0 - 1.0, "c.x={} d.x={} gap={}", g.nodes[c].x, g.nodes[d].x, gap2);
    }

    #[test]
    fn position_no_overlap_within_rank() {
        let mut g = LayoutGraph::new();
        g.nodesep = 50.0;
        g.marginx = 8.0;
        // 4 nodes in one rank, no edges.
        let mut ids = Vec::new();
        for i in 0..4 {
            let id = g.add_real_node(NodeId::default(), 60.0, 30.0);
            g.nodes[id].rank = 0;
            g.nodes[id].order = i;
            ids.push(id);
        }
        run(&mut g, Align::Balance);
        let mut xs: Vec<f32> = ids.iter().map(|&i| g.nodes[i].x).collect();
        xs.sort_by(|a, b| a.partial_cmp(b).unwrap());
        for w in xs.windows(2) {
            assert!(w[1] - w[0] >= 60.0 + 50.0 - 1.0, "xs={:?}", xs);
        }
    }
}
