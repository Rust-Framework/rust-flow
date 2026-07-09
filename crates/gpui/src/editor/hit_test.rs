//! 命中测试：根据逻辑坐标判断点击的节点/端口/按钮/空白区域。
//!
//! **多端口支持**：遍历节点 schema 中的所有端口，用 `resolve_port` 计算每个端口的
//! 精确位置，检查点击是否落在端口的命中区域内（以端口位置为中心的正方形）。
//!
//! **命中优先级**：端口 > 按钮（删除/切换） > 节点主体 > 空白。
//! 端口命中区域：以端口位置为中心，边长 `PORT_HIT_WIDTH * 2` 的正方形。
//! 按钮命中必须在节点主体命中之前检查，否则按钮区域会被节点主体"吞掉"。

use rust_agent_flow::{point_in_rect, EdgeId, NodeId, PortDirection, PortId, PointF, PortSide, RectF, SizeF};

use super::flow_editor::{FlowEditorView, LayoutDirection};
use super::ports::{port_position_by_side, resolve_port, PORT_HIT_WIDTH};
use crate::builtin::common::{BTN_MARGIN, DELETE_BTN_SIZE, TITLE_H, TOGGLE_BTN_SIZE};

/// 命中测试结果。
pub(crate) enum HitResult {
    /// 空白区域。
    Empty,
    /// 节点主体（非端口、非按钮区域）。
    Node(NodeId),
    /// 出端口。
    OutPort(NodeId, PortId),
    /// 入端口。
    InPort(NodeId, PortId),
    /// 删除按钮（仅可删除节点：非 start/end）。
    DeleteButton(NodeId),
    /// 展开/收起切换按钮（仅条件/循环节点）。
    ToggleButton(NodeId),
    /// 边中点「+」按钮（点击弹出节点选择面板，拆边插入新节点）。
    EdgePlusButton(EdgeId),
}

impl FlowEditorView {
    /// 命中测试：返回点击位置的节点和端口（如果有）。
    ///
    /// 遍历所有节点，按优先级检查：端口 > 按钮 > 节点主体。
    /// 边中点「+」按钮优先级最高（在实际中不与端口冲突，因 plus button 在边中点）。
    pub(crate) fn hit_test(&self, logical: PointF) -> HitResult {
        // 0. 检查边中点「+」按钮命中
        if let Some(edge_id) = self.hit_test_edge_plus(logical) {
            return HitResult::EdgePlusButton(edge_id);
        }

        let layout = self.layout_direction;

        for node in self.graph.nodes() {
            let bounds = node.bounds();

            // 1. 检查端口命中（遍历节点实例的端口列表，支持动态端口）
            if let Some(flow_node) = self.registry.get(&node.kind) {
                let node_layout = if self.cached_all_body_nodes.contains(&node.id) {
                    LayoutDirection::Vertical
                } else {
                    layout
                };
                for port_spec in &flow_node.ports_for_node(node) {
                    let (port_pos, _) = resolve_port(node, &port_spec.id, &self.registry, node_layout);
                    // 端口命中区域：以端口位置为中心的正方形
                    let hit_rect = RectF::new(
                        PointF::new(
                            port_pos.x - PORT_HIT_WIDTH,
                            port_pos.y - PORT_HIT_WIDTH,
                        ),
                        SizeF::new(PORT_HIT_WIDTH * 2.0, PORT_HIT_WIDTH * 2.0),
                    );
                    if point_in_rect(logical, hit_rect) {
                        return match port_spec.direction {
                            PortDirection::In => {
                                HitResult::InPort(node.id, port_spec.id.clone())
                            }
                            PortDirection::Out => {
                                HitResult::OutPort(node.id, port_spec.id.clone())
                            }
                        };
                    }
                }
            } else {
                // 未注册节点：回退到 side-based 端口检测（"in"/"out"）
                if let Some(hit) = self.hit_test_fallback_ports(node, logical) {
                    return hit;
                }
            }

            // 2. 检查删除按钮命中（仅可删除节点：非 start/end）
            if node.kind != "start" && node.kind != "end" {
                let btn_rect = RectF::new(
                    PointF::new(
                        node.position.x + node.size.w - DELETE_BTN_SIZE - BTN_MARGIN,
                        node.position.y + BTN_MARGIN,
                    ),
                    SizeF::new(DELETE_BTN_SIZE, DELETE_BTN_SIZE),
                );
                if point_in_rect(logical, btn_rect) {
                    return HitResult::DeleteButton(node.id);
                }
            }

            // 3. 检查切换按钮命中（仅条件/循环节点）
            if node.kind == "condition" || node.kind == "loop" {
                let btn_left = node.position.x + node.size.w
                    - TOGGLE_BTN_SIZE
                    - BTN_MARGIN
                    - TOGGLE_BTN_SIZE
                    - BTN_MARGIN;
                let btn_top = node.position.y + (TITLE_H - TOGGLE_BTN_SIZE) * 0.5;
                let btn_rect = RectF::new(
                    PointF::new(btn_left, btn_top),
                    SizeF::new(TOGGLE_BTN_SIZE, TOGGLE_BTN_SIZE),
                );
                if point_in_rect(logical, btn_rect) {
                    return HitResult::ToggleButton(node.id);
                }
            }

            // 4. 检查节点主体命中
            if point_in_rect(logical, bounds) {
                return HitResult::Node(node.id);
            }
        }
        HitResult::Empty
    }

    /// 未注册节点的端口命中回退（兼容 "in"/"out" 双端口）。
    fn hit_test_fallback_ports(
        &self,
        node: &rust_agent_flow::Node,
        logical: PointF,
    ) -> Option<HitResult> {
        let is_vertical = self.layout_direction == LayoutDirection::Vertical;
        // 假设有 In + Out 端口
        let in_side = if is_vertical {
            rust_agent_flow::PortSide::Top
        } else {
            rust_agent_flow::PortSide::Left
        };
        let out_side = if is_vertical {
            rust_agent_flow::PortSide::Bottom
        } else {
            rust_agent_flow::PortSide::Right
        };

        let in_pos = port_position_by_side(node, in_side);
        let out_pos = port_position_by_side(node, out_side);

        let hit_half = PORT_HIT_WIDTH;
        for (pos, result) in [
            (in_pos, HitResult::InPort(node.id, "in".to_string())),
            (out_pos, HitResult::OutPort(node.id, "out".to_string())),
        ] {
            let rect = RectF::new(
                PointF::new(pos.x - hit_half, pos.y - hit_half),
                SizeF::new(hit_half * 2.0, hit_half * 2.0),
            );
            if point_in_rect(logical, rect) {
                return Some(result);
            }
        }
        None
    }

    /// 边「+」按钮命中测试。
    ///
    /// 遍历所有可见边，计算每条边的按钮位置，检查点击是否在按钮半径内。
    ///
    /// 按钮位置计算与 `render_edge_plus_buttons` 保持完全一致：
    /// - 默认：源端口 + 沿 src_side 轴向偏移 25px
    /// - `plus_button_at_target()` 为 true 的源节点：目标端口 + 沿 dst_side 轴向偏移 25px
    ///
    /// 回环边（`target_port == "loop_in"`）也参与命中测试，其 + 按钮在
    /// Process 起始侧。
    fn hit_test_edge_plus(&self, logical: PointF) -> Option<EdgeId> {
        const RADIUS: f32 = 12.0;
        const OFFSET: f32 = 25.0;
        let radius_sq = RADIUS * RADIUS;

        let layout = self.layout_direction;
        let (src_side_default, dst_side_default) = self.port_sides();

        for edge in self.graph.edges() {
            let (src, src_side, dst, dst_side) = super::rendering::compute_edge_endpoints(
                edge,
                &self.graph,
                &self.registry,
                layout,
                src_side_default,
                dst_side_default,
                &self.cached_all_body_nodes,
            );

            // 检查源节点是否要求按钮放在目标端（与 render_edge_plus_buttons 一致）
            let at_target = self
                .graph
                .node(edge.source)
                .and_then(|n| self.registry.get(&n.kind))
                .map(|fn_| fn_.plus_button_at_target(edge.source_port.as_deref()))
                .unwrap_or(false);

            let (base, side) = if at_target {
                (dst, dst_side)
            } else {
                (src, src_side)
            };
            let (dx, dy) = match side {
                PortSide::Right => (OFFSET, 0.0),
                PortSide::Bottom => (0.0, OFFSET),
                PortSide::Left => (-OFFSET, 0.0),
                PortSide::Top => (0.0, -OFFSET),
                PortSide::Auto => {
                    debug_assert!(false, "PortSide::Auto must be resolved before button offset calculation");
                    (OFFSET, 0.0)
                }
            };
            let button_pos = PointF::new(base.x + dx, base.y + dy);

            let ddx = logical.x - button_pos.x;
            let ddy = logical.y - button_pos.y;
            if ddx * ddx + ddy * ddy <= radius_sq {
                return Some(edge.id);
            }
        }
        None
    }
}
