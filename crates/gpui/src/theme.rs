use gpui::Rgba;

/// Visual theme tokens for the flow editor.
#[derive(Clone)]
pub struct FlowTheme {
    pub canvas_background: Rgba,
    pub grid_dot_color: Rgba,
    pub grid_dot_spacing: f32,
    pub grid_dot_radius: f32,
    pub node_background: Rgba,
    pub node_border: Rgba,
    pub node_border_selected: Rgba,
    pub node_title_text: Rgba,
    pub body_muted_color: Rgba,
    pub port_color_input: Rgba,
    pub port_color_output: Rgba,
    pub port_label_color: Rgba,
    pub edge_color: Rgba,
    pub connection_preview: Rgba,
    pub accent_default: Rgba,
}

impl FlowTheme {
    pub fn light() -> Self {
        Self {
            canvas_background: rgba(0.976, 0.976, 0.980, 1.0),
            grid_dot_color: rgba(0.788, 0.788, 0.800, 0.60),
            grid_dot_spacing: 20.0,
            grid_dot_radius: 1.2,
            node_background: rgba(1.0, 1.0, 1.0, 1.0),
            node_border: rgba(0.855, 0.855, 0.871, 1.0),
            node_border_selected: rgba(0.306, 0.486, 0.902, 1.0),
            node_title_text: rgba(0.118, 0.118, 0.125, 1.0),
            body_muted_color: rgba(0.620, 0.620, 0.640, 1.0),
            port_color_input: rgba(0.300, 0.400, 0.800, 1.0),
            port_color_output: rgba(0.800, 0.500, 0.200, 1.0),
            port_label_color: rgba(0.500, 0.500, 0.520, 1.0),
            edge_color: rgba(0.550, 0.550, 0.570, 1.0),
            connection_preview: rgba(0.800, 0.750, 0.200, 0.70),
            accent_default: rgba(0.550, 0.550, 0.570, 1.0),
        }
    }

    pub fn dark() -> Self {
        Self {
            canvas_background: rgba(0.120, 0.120, 0.130, 1.0),
            grid_dot_color: rgba(0.280, 0.280, 0.300, 0.40),
            grid_dot_spacing: 24.0,
            grid_dot_radius: 1.0,
            node_background: rgba(0.180, 0.180, 0.190, 1.0),
            node_border: rgba(0.280, 0.280, 0.300, 1.0),
            node_border_selected: rgba(0.400, 0.600, 0.900, 1.0),
            node_title_text: rgba(0.900, 0.900, 0.900, 1.0),
            body_muted_color: rgba(0.450, 0.450, 0.470, 1.0),
            port_color_input: rgba(0.500, 0.500, 0.900, 1.0),
            port_color_output: rgba(0.900, 0.600, 0.300, 1.0),
            port_label_color: rgba(0.600, 0.600, 0.620, 1.0),
            edge_color: rgba(0.550, 0.550, 0.570, 1.0),
            connection_preview: rgba(0.900, 0.850, 0.200, 0.65),
            accent_default: rgba(0.480, 0.480, 0.520, 1.0),
        }
    }
}

impl Default for FlowTheme {
    fn default() -> Self {
        Self::light()
    }
}

fn rgba(r: f32, g: f32, b: f32, a: f32) -> Rgba {
    Rgba { r, g, b, a }
}
