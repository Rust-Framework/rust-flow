//! Mermaid flowchart text → [`FlowDocument`] (mind-map / read-only view).

use std::collections::HashMap;

use crate::auto_layout::LayoutDirection;
use crate::node_layout::mindmap_node_size;
use crate::schema::document::{
    FlowDocument, FlowDocumentEdge, FlowDocumentNode, FlowDocumentPosition,
};

#[derive(Debug, Clone)]
struct MermaidEdge {
    source: String,
    target: String,
    label: Option<String>,
}

/// Parse `graph TB` / `flowchart LR` Mermaid into a flow document.
pub fn mermaid_to_flow_document(text: &str) -> Result<FlowDocument, String> {
    let _direction = parse_header_direction(text).unwrap_or(LayoutDirection::TopBottom);
    let edges = parse_edges(text)?;
    if edges.is_empty() {
        return Err("no edges found in mermaid graph".into());
    }

    let mut labels: HashMap<String, String> = HashMap::new();
    let mut layout_order: HashMap<String, u64> = HashMap::new();
    let mut order = 0u64;
    for edge in &edges {
        for id in [&edge.source, &edge.target] {
            labels.entry(id.clone()).or_insert_with(|| id.clone());
            if !layout_order.contains_key(id) {
                layout_order.insert(id.clone(), order);
                order += 1;
            }
        }
    }

    for line in text.lines() {
        let line = strip_comment(line).trim();
        if line.is_empty() || is_header_line(line) {
            continue;
        }
        collect_inline_labels(line, &mut labels);
    }

    let nodes: Vec<FlowDocumentNode> = labels
        .iter()
        .map(|(id, label)| mindmap_flow_node(id, label, layout_order.get(id).copied()))
        .collect();

    let flow_edges: Vec<FlowDocumentEdge> = edges
        .iter()
        .map(|e| FlowDocumentEdge {
            id: Some(format!("e_{}_{}", e.source, e.target)),
            source: e.source.clone(),
            target: e.target.clone(),
            source_handle: Some("out".into()),
            target_handle: Some("in".into()),
            label: e.label.clone(),
            data: None,
            shape: None,
            stroke: None,
        })
        .collect();

    let title = labels
        .values()
        .next()
        .cloned()
        .unwrap_or_else(|| "Mind Map".into());

    Ok(FlowDocument {
        version: "mindmap-1.0".into(),
        name: title,
        nodes,
        edges: flow_edges,
        viewport: None,
    })
}

pub fn mermaid_layout_direction(text: &str) -> LayoutDirection {
    parse_header_direction(text).unwrap_or(LayoutDirection::TopBottom)
}

fn parse_header_direction(text: &str) -> Option<LayoutDirection> {
    for line in text.lines() {
        let line = strip_comment(line).trim();
        let lower = line.to_ascii_lowercase();
        if lower.starts_with("graph ") || lower.starts_with("flowchart ") {
            let parts: Vec<&str> = lower.split_whitespace().collect();
            if parts.len() >= 2 {
                return match parts[1] {
                    "tb" | "td" | "bt" => Some(LayoutDirection::TopBottom),
                    "lr" | "rl" => Some(LayoutDirection::LeftRight),
                    _ => Some(LayoutDirection::TopBottom),
                };
            }
        }
    }
    None
}

fn parse_edges(text: &str) -> Result<Vec<MermaidEdge>, String> {
    let mut edges = Vec::new();
    for line in text.lines() {
        let line = strip_comment(line).trim();
        if line.is_empty() || is_header_line(line) {
            continue;
        }
        if !line.contains("-->") {
            continue;
        }
        parse_edge_line(line, &mut edges)?;
    }
    Ok(edges)
}

fn parse_edge_line(line: &str, edges: &mut Vec<MermaidEdge>) -> Result<(), String> {
    let arrow = line.find("-->").ok_or_else(|| format!("invalid edge: {line}"))?;
    let left = line[..arrow].trim();
    let right = line[arrow + 3..].trim();

    let (source, _) = parse_node_ref(left)?;
    let (label, targets_part) = split_edge_label(right);

    for target_raw in targets_part.split('&') {
        let (target, _) = parse_node_ref(target_raw.trim())?;
        edges.push(MermaidEdge {
            source: source.clone(),
            target,
            label: label.clone(),
        });
    }
    Ok(())
}

fn split_edge_label(s: &str) -> (Option<String>, &str) {
    let trimmed = s.trim();
    if trimmed.starts_with('|') {
        if let Some(end) = trimmed[1..].find('|') {
            let label = trimmed[1..1 + end].trim().to_string();
            let rest = trimmed[1 + end + 1..].trim();
            return (Some(label), rest);
        }
    }
    (None, trimmed)
}

fn parse_node_ref(s: &str) -> Result<(String, Option<String>), String> {
    let s = s.trim();
    if s.is_empty() {
        return Err("empty node id".into());
    }
    if let Some(bracket) = s.find('[') {
        let id = s[..bracket].trim();
        let rest = &s[bracket + 1..];
        let end = rest.find(']').ok_or_else(|| format!("unclosed label in {s}"))?;
        let label = rest[..end].trim();
        if id.is_empty() {
            return Err(format!("empty node id in {s}"));
        }
        return Ok((id.to_string(), Some(label.to_string())));
    }
    Ok((s.to_string(), None))
}

fn collect_inline_labels(line: &str, labels: &mut HashMap<String, String>) {
    let mut rest = line;
    while let Some(bracket) = rest.find('[') {
        let before = rest[..bracket].trim();
        let after_bracket = &rest[bracket + 1..];
        if let Some(end) = after_bracket.find(']') {
            let label = after_bracket[..end].trim();
            let id = before
                .split_whitespace()
                .last()
                .unwrap_or(before)
                .trim_end_matches("-->")
                .trim();
            if !id.is_empty() {
                labels.insert(id.to_string(), label.to_string());
            }
            rest = &after_bracket[end + 1..];
        } else {
            break;
        }
    }
}

fn mindmap_flow_node(id: &str, label: &str, layout_order: Option<u64>) -> FlowDocumentNode {
    let size = mindmap_node_size(label);
    let mut data = serde_json::json!({ "label": label, "mindmap": true });
    if let Some(order) = layout_order {
        data["layout_order"] = serde_json::json!(order);
    }
    FlowDocumentNode {
        id: id.to_string(),
        node_type: "common".into(),
        position: FlowDocumentPosition { x: 0.0, y: 0.0 },
        data,
        width: Some(size.width),
        height: Some(size.height),
        selected: None,
        z_index: None,
    }
}

fn strip_comment(line: &str) -> &str {
    line.split("%%").next().unwrap_or(line)
}

fn is_header_line(line: &str) -> bool {
    let lower = line.to_ascii_lowercase();
    lower.starts_with("graph ") || lower.starts_with("flowchart ")
}

#[cfg(test)]
mod tests {
    use super::*;

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

    #[test]
    fn mermaid_orchestrator_graph() {
        let doc = mermaid_to_flow_document(ORCHESTRATOR).unwrap();
        assert_eq!(doc.nodes.len(), 9);
        assert_eq!(doc.edges.len(), 11);
        let labels: Vec<_> = doc
            .edges
            .iter()
            .filter_map(|e| e.label.as_deref())
            .collect();
        assert!(labels.contains(&"FAIL"));
        assert!(labels.contains(&"PASS"));
    }
}
