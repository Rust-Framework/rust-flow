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
use super::rendering::compute_edge_endpoints;

/// smoothstep_path 检测用的圆角半径，与渲染层 `paint_edge_scaled` 保持一致，
/// 保证碰撞检测路径与实际渲染路径吻合。
const SMOOTHSTEP_BORDER_RADIUS: f32 = 12.0;

/// 为所有非 LoopBack 边计算路由路径（混合策略）。
///
/// - LoopBack 边（`target_port == "loop_in"`）跳过，使用专用 `loop_back_path`
/// - 连接到隐藏节点（收起的循环体）的边跳过
/// - smoothstep 几何路径不穿节点的边跳过（渲染层回退 Normal，保持整齐）
/// - 穿节点的边用 A* 避障路由；路由失败不写入缓存，渲染层回退 Normal
///
/// **loop 缝隙障碍**：每个 Loop 节点与其每个 body 节点之间的"连接走廊"作为
/// 额外障碍，避免 A* 从 Loop 与 body 节点之间的缝隙穿过（视觉上"穿 loop body"）。
/// 相比 union bounds，缝隙矩形只堵缝不扩展成大矩形，不会误包紧邻的非 loop 节点。
/// 排除 src/dst 所在的 loop，保证 loop 内部边（loop_body/done）不被自身缝隙阻挡。
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

    // 预计算 loop 缝隙矩形：每个 Loop 节点与其每个 body 节点之间的"连接走廊"。
    // 避免 A* 从 Loop 与 body 节点之间的缝隙穿过 loop 循环体范围。
    // 相比 union bounds（凸包围盒），缝隙矩形只堵 Loop 和 body 之间的缝，
    // 不扩展成大矩形，不会误包紧邻的非 loop 节点（如 Summarize 紧邻 Process）。
    let loop_gaps: Vec<(NodeId, RectF)> = body_groups
        .iter()
        .flat_map(|(loop_id, body)| {
            let loop_node = match graph.node(*loop_id) {
                Some(n) => n,
                None => return Vec::new(),
            };
            let lb = loop_node.bounds();
            let mut gaps = Vec::new();
            for &bid in body {
                if let Some(bn) = graph.node(bid) {
                    let bb = bn.bounds();
                    let dx = bb.center().x - lb.center().x;
                    let dy = bb.center().y - lb.center().y;
                    // 按 Loop 与 body 的相对位置选主轴，计算两者之间的缝隙矩形。
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
                        gaps.push((*loop_id, g));
                    }
                }
            }
            gaps
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

        // loop 缝隙障碍：排除 src/dst 所在的 loop（即 src 或 dst 是该 loop 的
        // Loop 节点或 body 节点时，不把该 loop 的缝隙作为障碍）。
        // 这样 loop 内部边（loop_body/done/in）不被自身缝隙阻挡，
        // 而外部边（如分支→Summarize）会被缝隙挡住，A* 绕行而非穿缝。
        let loop_obstacles: Vec<RectF> = loop_gaps
            .iter()
            .filter(|(loop_id, _)| {
                let body = &body_groups[loop_id];
                *loop_id != edge.source
                    && *loop_id != edge.target
                    && !body.contains(&edge.source)
                    && !body.contains(&edge.target)
            })
            .map(|(_, r)| *r)
            .collect();

        // 合并障碍：单个节点 bounds + loop 缝隙矩形。
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
        eprintln!(
            "[route]   → A* result: {} waypoints",
            routed.as_ref().map(|w| w.len()).unwrap_or(0)
        );
        if let Some(waypoints) = routed {
            routes.insert(edge.id, waypoints);
        }
        // 路由失败：不插入，渲染层回退到 EdgeRender::Normal。
    }

    routes
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
}
