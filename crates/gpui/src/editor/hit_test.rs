//! 命中测试：根据逻辑坐标判断点击的节点/端口/按钮/空白区域。
//!
//! **多端口支持**：遍历节点 schema 中的所有端口，用 `resolve_port` 计算每个端口的
//! 精确位置，检查点击是否落在端口的命中区域内（以端口位置为中心的正方形）。
//!
//! **命中优先级**：端口 > 按钮（删除/切换） > 节点主体 > 空白。
//! 端口命中区域：以端口位置为中心，边长 `PORT_HIT_WIDTH * 2` 的正方形。
//! 按钮命中必须在节点主体命中之前检查，否则按钮区域会被节点主体"吞掉"。

use rust_agent_flow::{point_in_rect, NodeId, PortDirection, PortId, PointF, RectF, SizeF};

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
}

impl FlowEditorView {
    /// 命中测试：返回点击位置的节点和端口（如果有）。
    ///
    /// 遍历所有节点，按优先级检查：端口 > 按钮 > 节点主体。
    pub(crate) fn hit_test(&self, logical: PointF) -> HitResult {
        let layout = self.layout_direction;

        for node in self.graph.nodes() {
            let bounds = node.bounds();

            // 1. 检查端口命中（遍历节点实例的端口列表，支持动态端口）
            if let Some(flow_node) = self.registry.get(&node.kind) {
                for port_spec in &flow_node.ports_for_node(node) {
                    let (port_pos, _) = resolve_port(node, &port_spec.id, &self.registry, layout);
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
}
