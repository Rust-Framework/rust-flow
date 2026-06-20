//! Per-node-type layout metrics and port placement.

use std::collections::HashMap;

use crate::auto_layout::LayoutDirection;
use crate::id::PortId;
use crate::math::{Point, Size};
use crate::node_type::{BRANCH, LOOP};
use crate::port::FlowPort;
use slotmap::SlotMap;

pub const BRANCH_HEADER: f32 = 36.0;
pub const BRANCH_ROW: f32 = 32.0;
pub const BRANCH_PAD: f32 = 8.0;
pub const BRANCH_COLLAPSED_H: f32 = 40.0;
pub const BRANCH_WIDTH: f32 = 260.0;

pub const LOOP_HEADER: f32 = 40.0;
pub const LOOP_BODY_ZONE: f32 = 88.0;
pub const LOOP_FOOTER: f32 = 28.0;
pub const LOOP_WIDTH: f32 = 248.0;
pub const LOOP_HEIGHT: f32 = LOOP_HEADER + LOOP_BODY_ZONE + LOOP_FOOTER;

pub const COMMON_WIDTH: f32 = 200.0;
pub const COMMON_HEIGHT: f32 = 45.0;
pub const COMMON_PAD_V: f32 = 8.0;
pub const COMMON_LINE_HEIGHT: f32 = 18.0;
pub const COMMON_CHARS_PER_LINE: usize = 22;
pub const TRIGGER_HEIGHT: f32 = 44.0;
pub const HTTP_HEIGHT: f32 = 56.0;

pub const MINDMAP_PAD_H: f32 = 12.0;
pub const MINDMAP_MIN_WIDTH: f32 = 40.0;
pub const MINDMAP_MAX_WIDTH: f32 = 360.0;
/// Approximate glyph width at mind-map `text-sm` (14px).
pub const MINDMAP_CHAR_WIDTH: f32 = 7.5;

/// Compact mind-map / flowchart node height from label (width fixed at 200).
pub fn common_node_size(label: &str) -> Size {
    let lines = estimate_label_lines(label);
    let h = COMMON_PAD_V * 2.0 + lines as f32 * COMMON_LINE_HEIGHT;
    Size::new(COMMON_WIDTH, h.max(COMMON_HEIGHT))
}

/// Mind-map node box: width and height from label text (Mermaid-style, no fixed width).
pub fn mindmap_node_size(label: &str) -> Size {
    let segments: Vec<&str> = label.split('\n').collect();
    let line_count = segments.len().max(1);
    let max_chars = segments
        .iter()
        .map(|s| s.chars().count())
        .max()
        .unwrap_or(1)
        .max(1);
    let w = (max_chars as f32 * MINDMAP_CHAR_WIDTH + MINDMAP_PAD_H * 2.0)
        .clamp(MINDMAP_MIN_WIDTH, MINDMAP_MAX_WIDTH);
    let h = COMMON_PAD_V * 2.0 + line_count as f32 * COMMON_LINE_HEIGHT;
    Size::new(w, h.max(COMMON_HEIGHT))
}

fn estimate_label_lines(label: &str) -> usize {
    let mut total = 0;
    for segment in label.split('\n') {
        let chars = segment.chars().count();
        total += chars.div_ceil(COMMON_CHARS_PER_LINE).max(1);
    }
    total.max(1)
}

#[derive(Debug, Clone)]
pub struct BranchItem {
    pub id: String,
    pub label: String,
    pub condition: String,
}

pub fn parse_branch_items(data: &serde_json::Value) -> Vec<BranchItem> {
    let mut items = Vec::new();
    if let Some(arr) = data.get("branches").and_then(|v| v.as_array()) {
        for entry in arr {
            let id = entry
                .get("id")
                .and_then(|v| v.as_str())
                .unwrap_or("branch")
                .to_string();
            let label = entry
                .get("label")
                .and_then(|v| v.as_str())
                .unwrap_or(&id)
                .to_string();
            let condition = entry
                .get("condition")
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string();
            items.push(BranchItem {
                id,
                label,
                condition,
            });
        }
    }
    if items.is_empty() {
        items.push(BranchItem {
            id: "true".into(),
            label: "满足".into(),
            condition: "true".into(),
        });
        items.push(BranchItem {
            id: "false".into(),
            label: "否则".into(),
            condition: "else".into(),
        });
    }
    items
}

pub fn branch_collapsed(data: &serde_json::Value) -> bool {
    data.get("collapsed")
        .and_then(|v| v.as_bool())
        .unwrap_or(false)
}

pub fn branch_node_size(data: &serde_json::Value) -> Size {
    if branch_collapsed(data) {
        Size::new(BRANCH_WIDTH, BRANCH_COLLAPSED_H)
    } else {
        let rows = parse_branch_items(data).len();
        let h = BRANCH_HEADER + rows as f32 * BRANCH_ROW + BRANCH_PAD;
        Size::new(BRANCH_WIDTH, h)
    }
}

pub fn loop_node_size() -> Size {
    Size::new(LOOP_WIDTH, LOOP_HEIGHT)
}

/// Branch output port center Y in node-local coordinates (right edge).
pub fn branch_output_row_y(index: usize) -> f32 {
    BRANCH_HEADER + BRANCH_ROW * (index as f32 + 0.5)
}

pub fn branch_in_y(collapsed: bool) -> f32 {
    if collapsed {
        BRANCH_COLLAPSED_H * 0.5
    } else {
        BRANCH_HEADER * 0.5
    }
}

pub const LOOP_MAIN_PORT_Y: f32 = LOOP_HEADER * 0.38;

pub fn loop_in_y() -> f32 {
    LOOP_MAIN_PORT_Y
}

pub fn loop_out_y() -> f32 {
    LOOP_MAIN_PORT_Y
}

pub fn loop_continue_in_y(height: f32) -> f32 {
    height - LOOP_FOOTER * 0.5
}

pub fn branch_output_col_x(index: usize, count: usize, width: f32) -> f32 {
    if count <= 1 {
        return width * 0.5;
    }
    let margin = BRANCH_PAD;
    let usable = width - 2.0 * margin;
    margin + usable * (index as f32 + 0.5) / count as f32
}

/// Override automatic port layout for structured control nodes.
pub fn apply_structured_port_local(
    node_type: &str,
    data: &serde_json::Value,
    screen_size: Size,
    direction: LayoutDirection,
    ports: &SlotMap<PortId, FlowPort>,
    inputs: &[(String, PortId)],
    outputs: &[(String, PortId)],
    port_local: &mut HashMap<PortId, Point>,
) {
    match node_type {
        BRANCH => {
            let collapsed = branch_collapsed(data);
            let items = parse_branch_items(data);
            for (name, pid) in inputs {
                if name == "in" {
                    let pos = match direction {
                        LayoutDirection::LeftRight => Point::new(0.0, branch_in_y(collapsed)),
                        LayoutDirection::TopBottom => {
                            Point::new(screen_size.width * 0.5, branch_in_y(collapsed))
                        }
                    };
                    port_local.insert(*pid, pos);
                }
            }
            if !collapsed {
                for (name, pid) in outputs {
                    if let Some(idx) = items.iter().position(|b| b.id == *name) {
                        port_local.insert(
                            *pid,
                            Point::new(screen_size.width, branch_output_row_y(idx)),
                        );
                    }
                }
            }
        }
        LOOP => {
            for (name, pid) in inputs {
                let pos = match name.as_str() {
                    "in" => Point::new(0.0, loop_in_y()),
                    "continue" => Point::new(0.0, loop_continue_in_y(screen_size.height)),
                    _ => Point::new(0.0, screen_size.height * 0.5),
                };
                port_local.insert(*pid, pos);
            }
            for (name, pid) in outputs {
                let pos = match name.as_str() {
                    "out" => Point::new(screen_size.width, loop_out_y()),
                    "body" => Point::new(screen_size.width * 0.5, screen_size.height),
                    _ => Point::new(screen_size.width, screen_size.height * 0.5),
                };
                port_local.insert(*pid, pos);
            }
        }
        _ => {}
    }

    let _ = ports;
}
