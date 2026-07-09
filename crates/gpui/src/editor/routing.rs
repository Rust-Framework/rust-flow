//! 路由编排：在 relayout 时为需要避障的边计算 A* 路径并缓存。
//!
//! **混合路由策略**（整齐优先）：
//! 对每条非 LoopBack 边先用 `smoothstep_path` 计算 ReactFlow 几何路径
//! （进出段垂直于节点面 20px，箭头自然与入面垂直），再用 Liang-Barsky
//! 线段-矩形相交检测路径是否穿过任何其他节点：
//! - **不穿节点**：跳过该边（不写入缓存），渲染层回退 `EdgeRender::Normal`
//!   → `paint_edge_scaled` → `smoothstep_path`，保持整齐 + 垂直进出 + 规范箭头。
//! - **穿节点**：调用 `route_edge` Grid A* 避障路由，waypoints 写入缓存，
//!   渲染层用 `EdgeRender::Routed` 绘制。
//!
//! 这样大部分边走 smoothstep（整齐、箭头垂直），少数穿节点的边走 A*（避障），
//! 兼顾视觉规范与障碍规避。
//!
//! 渲染层（[`super::rendering::edges`]）优先使用缓存的路由 waypoints，
//! 命中测试（[`super::hit_test`]）复用同一份缓存。未缓存的边回退到
//! ReactFlow 几何算法（[`EdgeRender::Normal`]）。
//!
//! [`EdgeRender::Normal`]: super::rendering::edge_geometry::EdgeRender::Normal

use std::collections::{HashMap, HashSet};

use rust_agent_flow::{
    EdgeId, FlowGraph, NodeId, PointF, PortSide, RectF, SizeF, GRID_CELL_SIZE, OBSTACLE_MARGIN,
    route_edge, smoothstep_path,
};

use crate::node::NodeRegistry;
use super::flow_editor::LayoutDirection;
use super::rendering::{compute_edge_endpoints, compute_loop_bounds};

/// smoothstep_path 检测用的圆角半径，与渲染层 `paint_edge_scaled` 保持一致，
/// 保证碰撞检测路径与实际渲染路径吻合。
const SMOOTHSTEP_BORDER_RADIUS: f32 = 12.0;

/// 每个 Loop 节点的预计算障碍数据。
struct LoopObstacleData {
    /// Loop + 所有 body 节点的 union bounds（最强障碍，堵住整个 loop 区域）。
    union_bounds: RectF,
    /// Loop 与每个 body 节点之间的缝隙矩形（次强障碍，只堵缝）。
    gaps: Vec<RectF>,
}

/// 为所有非 LoopBack 边计算路由路径（混合策略）。
///
/// - LoopBack 边（`target_port == "loop_in"`）跳过，使用专用 `loop_back_path`
/// - 连接到隐藏节点（收起的循环体）的边跳过
/// - smoothstep 几何路径不穿节点的边跳过（渲染层回退 Normal，保持整齐）
/// - 穿节点的边用 A* 避障路由；路由失败不写入缓存，渲染层回退 Normal
///
/// **loop 障碍策略**（union bounds 优先 + 缝隙矩形回退）：
/// - 优先用 union bounds（Loop + 所有 body 的凸包围盒）作为障碍，堵住整个
///   loop 区域，A* 必须从整体外围绕行，不会从 Loop 与 body 之间的上方/下方
///   空隙穿过（视觉上"穿 loop body"）。
/// - 如果 union bounds 与当前边的 src/dst 节点相交（如 Summarize 紧邻 Process
///   时被包进 union bounds），则回退到缝隙矩形——只堵 Loop 与 body 之间的缝，
///   不阻挡 src/dst 端口方向。
/// - 排除 src/dst 所在的 loop，保证 loop 内部边（loop_body/done）不被自身障碍阻挡。
pub(crate) fn route_all_edges(
    graph: &FlowGraph,
    registry: &NodeRegistry,
    body_nodes: &HashSet<NodeId>,
    body_groups: &HashMap<NodeId, HashSet<NodeId>>,
    hidden_nodes: &HashSet<NodeId>,
    layout: LayoutDirection,
    src_side_default: PortSide,
    dst_side_default: PortSide,
) -> HashMap<EdgeId, Vec<PointF>> {
    // 收集所有节点原始 bounds（携带 NodeId 以便逐边排除 src/dst）。
    // smoothstep 碰撞检测用原始 bounds（用户要"不穿节点"即可），
    // A* 路由时再外扩 OBSTACLE_MARGIN（保证路径离节点有间距）。
    let all_node_bounds: Vec<(NodeId, RectF)> =
        graph.nodes().map(|n| (n.id, n.bounds())).collect();

    // 预计算每个 loop 的障碍数据：union bounds + 缝隙矩形。
    let loop_data: HashMap<NodeId, LoopObstacleData> = body_groups
        .iter()
        .map(|(loop_id, body)| {
            let union_bounds = compute_loop_bounds(graph, *loop_id, body);
            let gaps = compute_loop_gaps(graph, *loop_id, body);
            (*loop_id, LoopObstacleData { union_bounds, gaps })
        })
        .collect();

    // 预计算所有非 LoopBack 边的 smoothstep 路径，用于检测 A* 路径是否与
    // 其他边视觉交叉。当 A* 路径与 done 边等 smoothstep 边在 dst 附近重叠时，
    // 触发 dst 偏移重新 A* 路由，选择不交叉且最短的方向。
    let edge_smooth_paths: HashMap<EdgeId, Vec<PointF>> = graph
        .edges()
        .filter(|e| e.target_port.as_deref() != Some("loop_in"))
        .filter(|e| !hidden_nodes.contains(&e.source) && !hidden_nodes.contains(&e.target))
        .map(|e| {
            let (s, ss, d, ds) = compute_edge_endpoints(
                e,
                graph,
                registry,
                layout,
                src_side_default,
                dst_side_default,
                body_nodes,
            );
            (e.id, smoothstep_path(s, d, ss, ds, SMOOTHSTEP_BORDER_RADIUS))
        })
        .collect();

    let mut routes = HashMap::new();

    for edge in graph.edges() {
        // 跳过 LoopBack 边：专用 loop_back_path 保持 U 形绕行语义。
        if edge.target_port.as_deref() == Some("loop_in") {
            #[cfg(debug_assertions)]
            eprintln!(
                "[route] edge {:?}→{:?} (loop_in) SKIPPED →专用 loop_back_path",
                edge.source, edge.target
            );
            continue;
        }
        // 跳过隐藏节点相关边：这些边不渲染，路由它们无意义。
        if hidden_nodes.contains(&edge.source) || hidden_nodes.contains(&edge.target) {
            continue;
        }

        let (src, src_side, dst, dst_side) = compute_edge_endpoints(
            edge,
            graph,
            registry,
            layout,
            src_side_default,
            dst_side_default,
            body_nodes,
        );

        // 排除 src/dst 节点自身：路径需要从 src 端口出、从 dst 端口入。
        let other_bounds: Vec<RectF> = all_node_bounds
            .iter()
            .filter(|(nid, _)| *nid != edge.source && *nid != edge.target)
            .map(|(_, r)| *r)
            .collect();

        // loop 障碍：排除 src/dst 所在的 loop（即 src 或 dst 是该 loop 的
        // Loop 节点或 body 节点时，不把该 loop 的障碍作为障碍）。
        // 这样 loop 内部边（loop_body/done/in）不被自身障碍阻挡，
        // 而外部边（如分支→Summarize）会被 loop 障碍挡住，A* 绕行而非穿缝。
        //
        // 优先用 union bounds（堵住整个 loop 区域）；如果 union bounds 与
        // src/dst 节点相交，回退到缝隙矩形（只堵缝，不阻挡 src/dst）。
        let src_bounds = all_node_bounds
            .iter()
            .find(|(n, _)| *n == edge.source)
            .map(|(_, r)| *r);
        let dst_bounds = all_node_bounds
            .iter()
            .find(|(n, _)| *n == edge.target)
            .map(|(_, r)| *r);

        let loop_obstacles: Vec<RectF> = loop_data
            .iter()
            .filter(|(loop_id, _)| {
                let body = &body_groups[*loop_id];
                **loop_id != edge.source
                    && **loop_id != edge.target
                    && !body.contains(&edge.source)
                    && !body.contains(&edge.target)
            })
            .flat_map(|(loop_id, data)| {
                // union bounds 与 src/dst 相交 → 回退到缝隙矩形
                let union_safe = src_bounds.map_or(true, |b| !data.union_bounds.intersects(b))
                    && dst_bounds.map_or(true, |b| !data.union_bounds.intersects(b));
                if union_safe {
                    vec![data.union_bounds]
                } else {
                    #[cfg(debug_assertions)]
                    eprintln!(
                        "[route] edge {:?}→{:?} loop {:?} union_bounds {:?} intersects src/dst → fallback to gaps",
                        edge.source, edge.target, loop_id, data.union_bounds
                    );
                    data.gaps.clone()
                }
            })
            .collect();

        // 合并障碍：单个节点 bounds + loop 障碍。
        let mut all_obstacles = other_bounds.clone();
        all_obstacles.extend(loop_obstacles.iter().copied());

        // 混合策略第一步：先用 smoothstep 几何路径检测是否穿其他节点或 loop 区域。
        // 不穿 → 跳过（渲染层回退 Normal smoothstep，保持整齐 + 垂直进出 + 规范箭头）。
        let smooth_path =
            smoothstep_path(src, dst, src_side, dst_side, SMOOTHSTEP_BORDER_RADIUS);
        let smooth_hits = path_intersects_obstacles(&smooth_path, &all_obstacles);
        #[cfg(debug_assertions)]
        eprintln!(
            "[route] edge {:?}→{:?} (src_port={:?} dst_port={:?}) src={:?} dst={:?} smooth_hits={}",
            edge.source, edge.target, edge.source_port, edge.target_port, src, dst, smooth_hits
        );
        if !smooth_hits {
            continue;
        }

        // 穿节点 → A* 避障路由（obstacles 外扩 margin 保证路径离节点有间距）。
        let obstacles: Vec<RectF> = all_obstacles
            .iter()
            .map(|r| r.expand(OBSTACLE_MARGIN))
            .collect();

        let routed = route_edge(src, dst, src_side, dst_side, &obstacles, GRID_CELL_SIZE);
        #[cfg(debug_assertions)]
        {
            eprintln!(
                "[route]   → A* result: {} waypoints {:?}",
                routed.as_ref().map(|w| w.len()).unwrap_or(0),
                routed.as_ref()
            );
            log_to_file(&format!(
                "edge {:?}→{:?} (src_port={:?} dst_port={:?}) src={:?} dst={:?} src_side={:?} dst_side={:?}\n  A* routed: {:?}\n  loop_obstacles: {:?}\n",
                edge.source, edge.target, edge.source_port, edge.target_port, src, dst, src_side, dst_side, routed, loop_obstacles
            ));
        }

        // 检测 A* 路径是否与其他边 smoothstep 路径视觉交叉。
        // 交叉发生在多条边连到同一 dst 端口时（如 Notify→Summarize 与 done 边
        // 在 Summarize 左侧重叠）。此时对 dst 沿垂直于 dst_side 方向偏移，重新
        // A* 路由，使两条边在 dst 附近有间距，避免重叠交叉。
        let final_path = resolve_edge_crossing(
            src,
            dst,
            src_side,
            dst_side,
            &obstacles,
            routed,
            &edge_smooth_paths,
            edge.id,
        );

        if let Some(waypoints) = final_path {
            routes.insert(edge.id, waypoints);
        }
        // 路由失败：不插入，渲染层回退到 EdgeRender::Normal。
    }

    routes
}

/// 当 A* 路径与其他边 smoothstep 路径交叉时，通过偏移 dst 消除交叉。
///
/// 多条边连到同一 dst 端口时（如 Notify→Summarize 与 done 边都连到 Summarize
/// 左端口），A* 路径从下方绕进 dst，与 done 边水平段在 dst 附近重叠交叉。
/// 此时对 dst 沿垂直于 dst_side 方向偏移（如 dst_side=Left 时沿 y 方向偏移），
/// 重新 A* 路由，使两条边在 dst 附近有间距，避免重叠。
///
/// 偏移策略：尝试 +OFFSET、-OFFSET 两个方向，选择不交叉且最短的路径。
/// 若都不行，回退原始 A* 路径。
///
/// 相比 via point（绕行锚点）方案，dst 偏移方案更简单且无贴边/锯齿问题：
/// 路径仍走 A* 避障，只是 dst 端点偏移，箭头在端口附近偏移点（视觉上可接受）。
fn resolve_edge_crossing(
    src: PointF,
    dst: PointF,
    src_side: PortSide,
    dst_side: PortSide,
    obstacles: &[RectF],
    routed: Option<Vec<PointF>>,
    edge_smooth_paths: &HashMap<EdgeId, Vec<PointF>>,
    edge_id: EdgeId,
) -> Option<Vec<PointF>> {
    let routed_crosses = routed.as_ref().map_or(false, |wp| {
        edge_smooth_paths
            .iter()
            .filter(|(eid, _)| **eid != edge_id)
            .any(|(_, other)| polylines_intersect(wp, other))
    });
    if !routed_crosses {
        return routed;
    }

    #[cfg(debug_assertions)]
    {
        eprintln!("[route]   → A* crosses other edge, trying dst offset");
        log_to_file(&format!(
            "  A* crosses other edge → trying dst offset (dst={:?} dst_side={:?})\n",
            dst, dst_side
        ));
    }

    // dst 偏移方向：垂直于 dst_side。
    // dst_side=Left/Right → 偏移沿 y 方向；dst_side=Top/Bottom → 偏移沿 x 方向。
    let (dx, dy) = match dst_side {
        PortSide::Left | PortSide::Right => (0.0, 1.0),
        PortSide::Top | PortSide::Bottom => (1.0, 0.0),
        PortSide::Auto => (0.0, 1.0),
    };

    const DST_OFFSET: f32 = 12.0;
    const MAX_ATTEMPTS: i32 = 3;

    let mut best: Option<(Vec<PointF>, f32)> = None;
    for sign in [1.0, -1.0] {
        for multiplier in 1..=MAX_ATTEMPTS {
            let offset = sign * DST_OFFSET * multiplier as f32;
            let shifted_dst = PointF::new(dst.x + dx * offset, dst.y + dy * offset);
            let shifted_path =
                route_edge(src, shifted_dst, src_side, dst_side, obstacles, GRID_CELL_SIZE);
            let path = match shifted_path {
                Some(p) => p,
                None => continue,
            };
            let crosses = edge_smooth_paths
                .iter()
                .filter(|(eid, _)| **eid != edge_id)
                .any(|(_, other)| polylines_intersect(&path, other));
            if crosses {
                continue;
            }
            let len: f32 = path.windows(2).map(|w| w[0].distance_to(w[1])).sum();
            if best.as_ref().map_or(true, |(_, b_len)| len < *b_len) {
                best = Some((path, len));
            }
            break;
        }
    }

    if let Some((path, _)) = &best {
        #[cfg(debug_assertions)]
        {
            eprintln!("[route]   → dst offset resolved crossing");
            log_to_file(&format!("  dst offset resolved crossing → path: {:?}\n", path));
        }
    } else {
        #[cfg(debug_assertions)]
        {
            eprintln!("[route]   → dst offset failed, fallback to original A*");
            log_to_file("  dst offset failed → fallback to original A*\n");
        }
    }
    best.map(|(p, _)| p).or(routed)
}

/// 计算 Loop 节点与其每个 body 节点之间的缝隙矩形。
///
/// 缝隙矩形是 Loop 和 body 之间的"连接走廊"，按相对位置选主轴：
/// - 水平为主（body 在 Loop 左/右）：x 取 Loop.right 到 body.left，y 覆盖两者并集
/// - 垂直为主（body 在 Loop 上/下）：y 取 Loop.bottom 到 body.top，x 覆盖两者并集
///
/// 相比 union bounds，缝隙矩形只堵缝不扩展成大矩形，不误包紧邻的非 loop 节点。
/// 但只堵缝不够——A* 仍能从 Loop 上方/body 上方的空隙穿过。因此缝隙矩形仅作为
/// union bounds 与 src/dst 相交时的回退方案。
fn compute_loop_gaps(
    graph: &FlowGraph,
    loop_node: NodeId,
    body_nodes: &HashSet<NodeId>,
) -> Vec<RectF> {
    let lb = match graph.node(loop_node) {
        Some(n) => n.bounds(),
        None => return Vec::new(),
    };
    let mut gaps = Vec::new();
    for &bid in body_nodes {
        if let Some(bn) = graph.node(bid) {
            let bb = bn.bounds();
            let dx = bb.center().x - lb.center().x;
            let dy = bb.center().y - lb.center().y;
            let gap = if dx.abs() >= dy.abs() {
                // 水平为主：缝隙在 x 方向（body 在 Loop 左或右）
                let (left, right) = if dx >= 0.0 {
                    (lb.right(), bb.left())
                } else {
                    (bb.right(), lb.left())
                };
                if right > left {
                    let top = lb.top().min(bb.top());
                    let bottom = lb.bottom().max(bb.bottom());
                    Some(RectF::new(
                        PointF::new(left, top),
                        SizeF::new(right - left, bottom - top),
                    ))
                } else {
                    None
                }
            } else {
                // 垂直为主：缝隙在 y 方向（body 在 Loop 上或下）
                let (top, bottom) = if dy >= 0.0 {
                    (lb.bottom(), bb.top())
                } else {
                    (bb.bottom(), lb.top())
                };
                if bottom > top {
                    let left = lb.left().min(bb.left());
                    let right = lb.right().max(bb.right());
                    Some(RectF::new(
                        PointF::new(left, top),
                        SizeF::new(right - left, bottom - top),
                    ))
                } else {
                    None
                }
            };
            if let Some(g) = gap {
                gaps.push(g);
            }
        }
    }
    gaps
}

/// 诊断日志：追加到文件（仅 debug 构建）。
#[cfg(debug_assertions)]
fn log_to_file(msg: &str) {
    use std::io::Write;
    if let Ok(mut f) = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open("route_debug.log")
    {
        let _ = writeln!(f, "{}", msg);
    }
}

/// 检测两条折线是否相交（任一段对任一段相交即返回 true）。
///
/// **跳过策略**：只跳过"首尾连接"的段对（一条边的终点连到另一条边的起点，
/// 即 p1[last]≈p2[0] 或 p1[0]≈p2[last]），这是正常的边连接，不算交叉。
///
/// **不跳过"尾尾连接"**（p1[last]≈p2[last]，多条边连到同一 dst 端口）：
/// 这种情况下两条边的最后段可能在 dst 附近重叠交叉，需要被检测到。
/// 例如 Notify→Summarize 与 done 边都连到 Summarize 左端口，A* 路径从下方
/// 绕进 dst，与 done 边水平段在 dst 附近重叠，必须检测到才能触发 dst 偏移。
fn polylines_intersect(p1: &[PointF], p2: &[PointF]) -> bool {
    if p1.len() < 2 || p2.len() < 2 {
        return false;
    }
    let p1_head = p1[0];
    let p1_tail = p1[p1.len() - 1];
    let p2_head = p2[0];
    let p2_tail = p2[p2.len() - 1];

    // 首尾连接：p1 连到 p2（p1_tail ≈ p2_head）或 p2 连到 p1（p1_head ≈ p2_tail）
    let p1_to_p2 = points_close(p1_tail, p2_head);
    let p2_to_p1 = points_close(p1_head, p2_tail);

    for w1 in p1.windows(2) {
        for w2 in p2.windows(2) {
            // 跳过首尾连接的段对（正常边连接，不算交叉）
            if p1_to_p2 && points_close(w1[1], p1_tail) && points_close(w2[0], p2_head) {
                continue;
            }
            if p2_to_p1 && points_close(w1[0], p1_head) && points_close(w2[1], p2_tail) {
                continue;
            }
            if segment_intersects_segment(w1[0], w1[1], w2[0], w2[1]) {
                return true;
            }
        }
    }
    false
}

/// 判断两点是否足够接近（视为重合端点）。
///
/// 阈值 0.1px：只跳过真正重合的端点（如两条边连到同一 dst 端口时端点完全重合）。
/// 不跳过 A* 网格量化误差导致的 0.5px 差距（如 (2490, 143.5) 与 (2490, 143)），
/// 这种差距是 A* 路径与 smoothstep 边在 dst 附近交叉的信号，需要被检测到。
fn points_close(a: PointF, b: PointF) -> bool {
    (a.x - b.x).abs() < 0.1 && (a.y - b.y).abs() < 0.1
}

/// 标准叉积线段相交检测。
///
/// 判断线段 (a1→a2) 是否与线段 (b1→b2) 相交，包括端点重合和共线情况。
fn segment_intersects_segment(a1: PointF, a2: PointF, b1: PointF, b2: PointF) -> bool {
    let cross = |ox: f32, oy: f32, px: f32, py: f32, qx: f32, qy: f32| -> f32 {
        (px - ox) * (qy - oy) - (py - oy) * (qx - ox)
    };
    let d1 = cross(a1.x, a1.y, b1.x, b1.y, a2.x, a2.y);
    let d2 = cross(a1.x, a1.y, b2.x, b2.y, a2.x, a2.y);
    let d3 = cross(b1.x, b1.y, a1.x, a1.y, b2.x, b2.y);
    let d4 = cross(b1.x, b1.y, a2.x, a2.y, b2.x, b2.y);

    if ((d1 > 0.0 && d2 < 0.0) || (d1 < 0.0 && d2 > 0.0))
        && ((d3 > 0.0 && d4 < 0.0) || (d3 < 0.0 && d4 > 0.0))
    {
        return true;
    }
    // 共线/端点重合情况
    d1.abs() < 1e-6 && point_on_segment(b1, a1, a2)
        || d2.abs() < 1e-6 && point_on_segment(b2, a1, a2)
        || d3.abs() < 1e-6 && point_on_segment(a1, b1, b2)
        || d4.abs() < 1e-6 && point_on_segment(a2, b1, b2)
}

/// 判断点 p 是否在线段 (a, b) 上（假定已共线）。
fn point_on_segment(p: PointF, a: PointF, b: PointF) -> bool {
    p.x >= a.x.min(b.x) - 1e-6
        && p.x <= a.x.max(b.x) + 1e-6
        && p.y >= a.y.min(b.y) - 1e-6
        && p.y <= a.y.max(b.y) + 1e-6
}

/// 检测折线是否与任何障碍矩形相交。
///
/// 对折线每一段调用 [`segment_intersects_rect`]；任一段命中即返回 `true`。
fn path_intersects_obstacles(path: &[PointF], obstacles: &[RectF]) -> bool {
    if path.len() < 2 {
        return false;
    }
    for window in path.windows(2) {
        let a = window[0];
        let b = window[1];
        for &rect in obstacles {
            if segment_intersects_rect(a, b, rect) {
                return true;
            }
        }
    }
    false
}

/// Liang-Barsky 线段-矩形相交检测。
///
/// 判断线段 (a→b) 是否与轴对齐矩形 `rect` 相交。端点落在矩形内也算相交
/// （调用方已排除 src/dst 自身障碍，此处命中即真穿其他节点）。
fn segment_intersects_rect(a: PointF, b: PointF, rect: RectF) -> bool {
    if rect.contains(a) || rect.contains(b) {
        return true;
    }

    let dx = b.x - a.x;
    let dy = b.y - a.y;

    let mut t0: f32 = 0.0;
    let mut t1: f32 = 1.0;

    // 四条边界：left, right, top, bottom。
    // p[i] 为线段方向在该边界法向上的分量，q[i] 为起点到边界的距离。
    let p = [-dx, dx, -dy, dy];
    let q = [
        a.x - rect.left(),
        rect.right() - a.x,
        a.y - rect.top(),
        rect.bottom() - a.y,
    ];

    for i in 0..4 {
        if p[i].abs() < 1e-6 {
            // 线段平行于此边界：若起点在此边界外（q[i] < 0），整段在外，无相交。
            if q[i] < 0.0 {
                return false;
            }
            continue;
        }
        let r = q[i] / p[i];
        if p[i] < 0.0 {
            if r > t1 {
                return false;
            }
            if r > t0 {
                t0 = r;
            }
        } else {
            if r < t0 {
                return false;
            }
            if r < t1 {
                t1 = r;
            }
        }
    }

    // t0 <= t1 表示裁剪后线段非空，即与矩形相交。
    t0 <= t1
}

#[cfg(test)]
mod tests {
    use super::*;
    use rust_agent_flow::SizeF;

    fn rect(x: f32, y: f32, w: f32, h: f32) -> RectF {
        RectF::new(PointF::new(x, y), SizeF::new(w, h))
    }

    #[test]
    fn segment_outside_rect_no_intersection() {
        // 线段在矩形右侧外，不相交。
        let a = PointF::new(20.0, 0.0);
        let b = PointF::new(20.0, 10.0);
        assert!(!segment_intersects_rect(a, b, rect(0.0, 0.0, 10.0, 10.0)));
    }

    #[test]
    fn segment_crossing_rect_intersects() {
        // 水平线段穿过矩形中央。
        let a = PointF::new(-5.0, 5.0);
        let b = PointF::new(15.0, 5.0);
        assert!(segment_intersects_rect(a, b, rect(0.0, 0.0, 10.0, 10.0)));
    }

    #[test]
    fn segment_endpoint_inside_rect_intersects() {
        // 起点在矩形内。
        let a = PointF::new(5.0, 5.0);
        let b = PointF::new(20.0, 20.0);
        assert!(segment_intersects_rect(a, b, rect(0.0, 0.0, 10.0, 10.0)));
    }

    #[test]
    fn segment_parallel_above_rect_no_intersection() {
        // 水平线段在矩形上方外，不相交。
        let a = PointF::new(0.0, -5.0);
        let b = PointF::new(10.0, -5.0);
        assert!(!segment_intersects_rect(a, b, rect(0.0, 0.0, 10.0, 10.0)));
    }

    #[test]
    fn path_with_crossing_segment_detected() {
        // 折线第二段穿过矩形。
        let path = vec![
            PointF::new(-5.0, 0.0),
            PointF::new(-5.0, 5.0),
            PointF::new(15.0, 5.0),
        ];
        assert!(path_intersects_obstacles(
            &path,
            &[rect(0.0, 0.0, 10.0, 10.0)]
        ));
    }

    #[test]
    fn path_clear_of_obstacles_not_detected() {
        // 路径在矩形上方绕过，所有点与矩形有间距，不相交。
        let path = vec![
            PointF::new(-5.0, -5.0),
            PointF::new(-5.0, -10.0),
            PointF::new(20.0, -10.0),
            PointF::new(20.0, -5.0),
        ];
        assert!(!path_intersects_obstacles(
            &path,
            &[rect(0.0, 0.0, 10.0, 10.0)]
        ));
    }

    /// 诊断测试：模拟生产环境横向布局下 Notify→Summarize 边的路由情况。
    ///
    /// 生产环境几何（从 route_debug.log 捕获）：
    /// - Notify 右端口: (1580, 263.5)
    /// - Summarize 左端口: (2500, 143)
    /// - Loop done port: (2160, 143)（Loop 在 (1940, 103, 220, 80)，done = right, mid_y）
    /// - loop_obstacles union: (1940, 103, 480, 144)
    /// - A* 路径: [1580,263.5]→[1920,263.5]→[1920,273.5]→[2490,273.5]→[2490,143.5]→[2500,143]
    /// - done 边 smoothstep: (2160, 143) → (2500, 143) 水平
    ///
    /// 问题：A* 路径垂直段 (2490, 273.5→143.5) 与 done 边水平段 (2490, 143) 在
    /// (2490, 143) 附近相交，但 polylines_intersect 未检测到（points_close
    /// 阈值 1.0px 把 (2490, 143.5) 和 (2490, 143) 视为重合端点跳过）。
    #[test]
    fn diag_horizontal_notify_to_summarize_routing() {
        // 生产环境几何
        let notify_right = PointF::new(1580.0, 263.5);
        let summarize_left = PointF::new(2500.0, 143.0);
        let loop_done_port = PointF::new(2160.0, 143.0);

        // loop_obstacles union bounds（生产环境捕获）
        let union = RectF::new(PointF::new(1940.0, 103.0), SizeF::new(480.0, 144.0));
        println!("union bounds: {:?}", union);

        // 障碍（外扩 margin）— 用 union 作为单一障碍
        let obstacles: Vec<RectF> = vec![union.expand(OBSTACLE_MARGIN)];

        // done 边 smoothstep 路径
        let done_smooth = smoothstep_path(
            loop_done_port,
            summarize_left,
            PortSide::Right,
            PortSide::Left,
            SMOOTHSTEP_BORDER_RADIUS,
        );
        println!("done edge smoothstep: {:?}", done_smooth);

        // Notify→Summarize A* 路由（复现生产环境路径）
        let routed = route_edge(
            notify_right,
            summarize_left,
            PortSide::Right,
            PortSide::Left,
            &obstacles,
            GRID_CELL_SIZE,
        );
        println!("Notify→Summarize A* routed: {:?}", routed);

        // 检测 A* 路径是否与 done 边 smoothstep 交叉
        if let Some(wp) = &routed {
            let crosses = polylines_intersect(wp, &done_smooth);
            println!("A* crosses done smooth (points_close 0.1): {}", crosses);
        }

        // 测试 dst 偏移方案：dst 偏移 +12px (y 方向)，重新 A* 路由
        let dst_offset = 12.0;
        let shifted_dst = PointF::new(summarize_left.x, summarize_left.y + dst_offset);
        let shifted_routed = route_edge(
            notify_right,
            shifted_dst,
            PortSide::Right,
            PortSide::Left,
            &obstacles,
            GRID_CELL_SIZE,
        );
        println!(
            "dst offset +{:.0} → shifted_dst={:?} routed: {:?}",
            dst_offset, shifted_dst, shifted_routed
        );
        if let Some(wp) = &shifted_routed {
            let crosses = polylines_intersect(wp, &done_smooth);
            println!("shifted A* crosses done smooth: {}", crosses);
        }

        // 测试 dst 偏移 -12px (y 方向)
        let shifted_dst_neg = PointF::new(summarize_left.x, summarize_left.y - dst_offset);
        let shifted_routed_neg = route_edge(
            notify_right,
            shifted_dst_neg,
            PortSide::Right,
            PortSide::Left,
            &obstacles,
            GRID_CELL_SIZE,
        );
        println!(
            "dst offset -{:.0} → shifted_dst={:?} routed: {:?}",
            dst_offset, shifted_dst_neg, shifted_routed_neg
        );
        if let Some(wp) = &shifted_routed_neg {
            let crosses = polylines_intersect(wp, &done_smooth);
            println!("shifted A* crosses done smooth: {}", crosses);
        }
    }
}
