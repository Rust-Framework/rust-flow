//! Automatic graph layout options (Dagre / React Flow–style defaults).

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum LayoutDirection {
    /// Primary flow left → right (default for control-flow editors).
    #[default]
    LeftRight,
    /// Primary flow top → bottom.
    TopBottom,
}

#[derive(Debug, Clone)]
pub struct LayoutOptions {
    pub direction: LayoutDirection,
    /// Horizontal/vertical gap between nodes in the same layer (`nodesep` in Dagre).
    pub node_spacing: f32,
    /// Gap between layers (`ranksep` in Dagre).
    pub rank_spacing: f32,
    /// Outer margin around the laid-out graph.
    pub margin: f32,
    /// Crossing-reduction passes (Dagre default 24).
    pub ordering_iterations: u32,
}

impl Default for LayoutOptions {
    fn default() -> Self {
        Self::comfortable()
    }
}

impl LayoutOptions {
    /// Spacing tuned for mixed control nodes (branch ~140px, loop ~176px tall).
    pub fn comfortable() -> Self {
        Self {
            direction: LayoutDirection::LeftRight,
            node_spacing: 80.0,
            rank_spacing: 160.0,
            margin: 60.0,
            ordering_iterations: 24,
        }
    }

    pub fn compact() -> Self {
        Self {
            direction: LayoutDirection::LeftRight,
            node_spacing: 48.0,
            rank_spacing: 80.0,
            margin: 32.0,
            ordering_iterations: 24,
        }
    }

    pub fn left_right() -> Self {
        Self::default()
    }

    pub fn mindmap_tb() -> Self {
        Self::mermaid_flowchart_tb()
    }

    /// Mind map bidirectional LR layout — root centered, children split left/right.
    /// Tuned between jsmind (hspace=30, vspace=20) and mind-elixir (HGap=65, VGap=25).
    pub fn mindmap_lr() -> Self {
        Self {
            direction: LayoutDirection::LeftRight,
            node_spacing: 24.0,
            rank_spacing: 80.0,
            margin: 40.0,
            ordering_iterations: 1,
        }
    }

    /// Mind map top-down tree layout (compact vertical tree).
    pub fn mindmap_tree_tb() -> Self {
        Self {
            direction: LayoutDirection::TopBottom,
            node_spacing: 20.0,
            rank_spacing: 60.0,
            margin: 20.0,
            ordering_iterations: 1,
        }
    }

    /// Mermaid flowchart defaults (`nodeSpacing` / `rankSpacing` 50, margin 8).
    pub fn mermaid_flowchart_tb() -> Self {
        Self {
            direction: LayoutDirection::TopBottom,
            node_spacing: 50.0,
            rank_spacing: 50.0,
            margin: 8.0,
            ordering_iterations: 32,
        }
    }

    pub fn top_bottom() -> Self {
        Self {
            direction: LayoutDirection::TopBottom,
            ..Self::default()
        }
    }
}
