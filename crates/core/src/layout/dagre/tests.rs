#[cfg(test)]
mod tests {
    use super::super::*;
    use crate::graph::{Edge, FlowGraph};
    use crate::SizeF;

    #[test]
    fn dagre_layouts_simple_chain() {
        let mut g = FlowGraph::new();
        let a = g.add_node_with_size("action", serde_json::json!({}), SizeF::new(100.0, 40.0));
        let b = g.add_node_with_size("action", serde_json::json!({}), SizeF::new(100.0, 40.0));
        let c = g.add_node_with_size("action", serde_json::json!({}), SizeF::new(100.0, 40.0));
        g.add_edge(Edge::new(a, b));
        g.add_edge(Edge::new(b, c));

        let result = DagreLayout::new().layout(&g, LayoutDirection::Horizontal);
        // All three nodes should have positions.
        assert_eq!(result.positions.len(), 3);
        // In horizontal layout, A should be left of B, B left of C.
        let pa = result.positions[&a];
        let pb = result.positions[&b];
        let pc = result.positions[&c];
        assert!(pa.x < pb.x, "A ({}) should be left of B ({})", pa.x, pb.x);
        assert!(pb.x < pc.x, "B ({}) should be left of C ({})", pb.x, pc.x);
    }

    #[test]
    fn dagre_handles_cycle_for_loop() {
        // Loop: A → B → A (back-edge). dagre should handle the cycle
        // without panicking and still produce positions for all nodes.
        let mut g = FlowGraph::new();
        let a = g.add_node_with_size("action", serde_json::json!({}), SizeF::new(100.0, 40.0));
        let b = g.add_node_with_size("action", serde_json::json!({}), SizeF::new(100.0, 40.0));
        g.add_edge(Edge::new(a, b));
        g.add_edge(Edge::new(b, a)); // back-edge

        let result = DagreLayout::new().layout(&g, LayoutDirection::Vertical);
        assert_eq!(result.positions.len(), 2);
    }

    #[test]
    fn branch_targets_reordered_to_match_port_order() {
        // Condition node with 3 branches: if_0, if_1, else → targets T1, T2, T0.
        // After layout, targets must be ordered top-to-bottom (horizontal layout)
        // matching port order: if_0 → if_1 → else (else is last / fallback).
        let mut g = FlowGraph::new();
        let cond = g.add_node_with_size("condition", serde_json::json!({}), SizeF::new(200.0, 100.0));
        let t0 = g.add_node_with_size("action", serde_json::json!({}), SizeF::new(100.0, 40.0));
        let t1 = g.add_node_with_size("action", serde_json::json!({}), SizeF::new(100.0, 40.0));
        let t2 = g.add_node_with_size("action", serde_json::json!({}), SizeF::new(100.0, 40.0));
        let sink = g.add_node_with_size("end", serde_json::json!({}), SizeF::new(100.0, 40.0));

        let mut e_else = Edge::new(cond, t0);
        e_else.source_port = Some("else".to_string());
        let mut e_if0 = Edge::new(cond, t1);
        e_if0.source_port = Some("if_0".to_string());
        let mut e_if1 = Edge::new(cond, t2);
        e_if1.source_port = Some("if_1".to_string());
        g.add_edge(e_else);
        g.add_edge(e_if0);
        g.add_edge(e_if1);
        // All targets converge to sink — forces them into the same rank.
        g.add_edge(Edge::new(t0, sink));
        g.add_edge(Edge::new(t1, sink));
        g.add_edge(Edge::new(t2, sink));

        let result = DagreLayout::new().layout(&g, LayoutDirection::Horizontal);

        // In horizontal layout, targets should be ordered by Y:
        //   if_0 (t1) top-most, if_1 (t2) middle, else (t0) bottom (fallback).
        let y_else = result.positions[&t0].y;
        let y_if0 = result.positions[&t1].y;
        let y_if1 = result.positions[&t2].y;
        assert!(
            y_if0 <= y_if1 && y_if1 <= y_else,
            "if_0 ({}) should be above if_1 ({}) above else ({})",
            y_if0, y_if1, y_else
        );
    }

    #[test]
    fn linear_chain_aligned_along_cross_axis() {
        // Main flow: Start → A → B → Cond (branch) → ...
        // Start, A, B are linear (1 in, 1 out). Cond is a branch source.
        // After layout, Start/A/B should have aligned port-Y (center Y)
        // so the connecting edges are straight horizontal lines.
        let mut g = FlowGraph::new();
        let start = g.add_node_with_size("start", serde_json::json!({}), SizeF::new(160.0, 56.0));
        let a = g.add_node_with_size("action", serde_json::json!({}), SizeF::new(200.0, 64.0));
        let b = g.add_node_with_size("action", serde_json::json!({}), SizeF::new(180.0, 35.0));
        let cond = g.add_node_with_size("condition", serde_json::json!({}), SizeF::new(220.0, 144.0));
        // Branch targets + sink to give Cond something to branch to.
        let t0 = g.add_node_with_size("action", serde_json::json!({}), SizeF::new(100.0, 40.0));
        let t1 = g.add_node_with_size("action", serde_json::json!({}), SizeF::new(100.0, 40.0));
        let sink = g.add_node_with_size("end", serde_json::json!({}), SizeF::new(100.0, 40.0));

        g.add_edge(Edge::new(start, a));
        g.add_edge(Edge::new(a, b));
        g.add_edge(Edge::new(b, cond));
        let mut e_if0 = Edge::new(cond, t0);
        e_if0.source_port = Some("if_0".to_string());
        let mut e_else = Edge::new(cond, t1);
        e_else.source_port = Some("else".to_string());
        g.add_edge(e_if0);
        g.add_edge(e_else);
        g.add_edge(Edge::new(t0, sink));
        g.add_edge(Edge::new(t1, sink));

        let result = DagreLayout::new().layout(&g, LayoutDirection::Horizontal);

        // For horizontal layout, port Y = node.y + node.h / 2.
        // Linear chain nodes (start, a, b) should have matching port Y.
        let port_y = |id| {
            let p = &result.positions[&id];
            let node = g.node(id).unwrap();
            p.y + node.size.h * 0.5
        };
        let py_start = port_y(start);
        let py_a = port_y(a);
        let py_b = port_y(b);
        assert!(
            (py_start - py_a).abs() < 1.0 && (py_a - py_b).abs() < 1.0,
            "Linear chain port Y should be aligned: start={}, a={}, b={}",
            py_start, py_a, py_b
        );
    }
}
