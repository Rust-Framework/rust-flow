//! Data-driven node rendering from [`FlowTypeRegistry`] definitions.

use rust_agent_flow::{
    apply_template, FlowFieldType, FlowNodeTypeDef, FlowTypeRegistry, ResolvedNode, Size,
};
use gpui::*;

use crate::nodes::card::{node_card, panel_input_row, panel_section};
use crate::nodes::handles::render_port_handles;
use crate::provider::{FlowPanelContext, IFlowNodeProvider};
use crate::theme::FlowTheme;
use crate::zoom::Z;

/// Renders a node type purely from schema definition.
pub struct SchemaNodeProvider {
    def: FlowNodeTypeDef,
}

impl SchemaNodeProvider {
    pub fn new(def: FlowNodeTypeDef) -> Self {
        Self { def }
    }
}

impl IFlowNodeProvider for SchemaNodeProvider {
    fn node_type(&self) -> &'static str {
        // Schema providers are keyed by type_id in the registry map;
        // this is only used when registering individual instances.
        ""
    }

    fn default_size(&self) -> Size {
        self.def.default_size.to_size()
    }

    fn render_node(&self, node: &ResolvedNode, theme: &FlowTheme) -> Div {
        let z = Z::new(node.zoom);
        let render = &self.def.render;

        let title_text = render
            .title
            .as_deref()
            .map(|t| apply_template(t, &node.label, &node.data))
            .unwrap_or_else(|| node.label.clone());

        let mut body_children: Vec<Div> = vec![
            div()
                .font_weight(FontWeight::SEMIBOLD)
                .text_color(theme.node_title_text)
                .overflow_hidden()
                .text_ellipsis()
                .child(title_text),
        ];

        if let Some(sub) = &render.subtitle {
            let text = apply_template(sub, &node.label, &node.data);
            if !text.is_empty() {
                body_children.push(
                    div()
                        .text_size(z.text_xs())
                        .text_color(theme.body_muted_color)
                        .overflow_hidden()
                        .text_ellipsis()
                        .child(text),
                );
            }
        }

        for line in &render.body {
            let text = apply_template(line, &node.label, &node.data);
            body_children.push(
                div()
                    .text_size(z.text_xs())
                    .text_color(theme.port_label_color)
                    .child(text),
            );
        }

        if let Some(footer) = &render.footer {
            body_children.push(
                div()
                    .text_size(z.text_xs())
                    .text_color(theme.port_label_color)
                    .child(footer.clone()),
            );
        }

        let body = div().flex().flex_col().gap(z.px(2.0)).children(body_children);
        let card = node_card(node, theme, body);
        let handles = render_port_handles(node, theme);

        div().size_full().child(card).children(handles)
    }

    fn render_panel(&self, ctx: &mut FlowPanelContext<'_>) -> Div {
        let node = ctx.graph.nodes.get(ctx.node_id).cloned();
        let Some(node) = node else {
            return div();
        };

        let mut panel = div()
            .flex()
            .flex_col()
            .child(panel_section(&self.def.label, ctx.theme));

        for field in &self.def.fields {
            match field.field_type {
                FlowFieldType::Section => {
                    panel = panel.child(panel_section(&field.label, ctx.theme));
                }
                FlowFieldType::Text if field.key == "label" => {
                    panel = panel.child(panel_input_row(&field.label, node.label.clone(), ctx.theme));
                }
                FlowFieldType::Text | FlowFieldType::Expression | FlowFieldType::Number => {
                    let value = if field.key == "label" {
                        node.label.clone()
                    } else {
                        node.data
                            .get(&field.key)
                            .map(json_display)
                            .unwrap_or_default()
                    };
                    panel = panel.child(panel_input_row(&field.label, value, ctx.theme));
                }
            }
        }

        if let Some(desc) = &self.def.description {
            panel = panel.child(
                div()
                    .text_xs()
                    .text_color(ctx.theme.body_muted_color)
                    .child(desc.clone()),
            );
        }

        panel
    }
}

/// Lookup provider by type id from a schema-backed registry entry.
pub struct SchemaDrivenProvider {
    type_id: String,
    registry: FlowTypeRegistry,
}

impl SchemaDrivenProvider {
    pub fn new(type_id: impl Into<String>, registry: FlowTypeRegistry) -> Self {
        Self {
            type_id: type_id.into(),
            registry,
        }
    }
}

impl IFlowNodeProvider for SchemaDrivenProvider {
    fn node_type(&self) -> &'static str {
        ""
    }

    fn default_size(&self) -> Size {
        self.registry
            .get(&self.type_id)
            .map(|d| d.default_size.to_size())
            .unwrap_or_else(|| Size::new(200.0, 41.0))
    }

    fn render_node(&self, node: &ResolvedNode, theme: &FlowTheme) -> Div {
        if let Some(def) = self.registry.get(&self.type_id).cloned() {
            SchemaNodeProvider::new(def).render_node(node, theme)
        } else {
            div().child(node.label.clone())
        }
    }

    fn render_panel(&self, ctx: &mut FlowPanelContext<'_>) -> Div {
        if let Some(def) = self.registry.get(&self.type_id).cloned() {
            SchemaNodeProvider::new(def).render_panel(ctx)
        } else {
            div()
        }
    }
}

pub fn parse_accent(hex: &Option<String>, fallback: Rgba) -> Rgba {
    hex.as_deref()
        .and_then(parse_hex_rgba)
        .unwrap_or(fallback)
}

fn parse_hex_rgba(hex: &str) -> Option<Rgba> {
    let hex = hex.trim().trim_start_matches('#');
    match hex.len() {
        6 => {
            let r = u8::from_str_radix(&hex[0..2], 16).ok()?;
            let g = u8::from_str_radix(&hex[2..4], 16).ok()?;
            let b = u8::from_str_radix(&hex[4..6], 16).ok()?;
            Some(Rgba {
                r: f32::from(r) / 255.0,
                g: f32::from(g) / 255.0,
                b: f32::from(b) / 255.0,
                a: 1.0,
            })
        }
        8 => {
            let r = u8::from_str_radix(&hex[0..2], 16).ok()?;
            let g = u8::from_str_radix(&hex[2..4], 16).ok()?;
            let b = u8::from_str_radix(&hex[4..6], 16).ok()?;
            let a = u8::from_str_radix(&hex[6..8], 16).ok()?;
            Some(Rgba {
                r: f32::from(r) / 255.0,
                g: f32::from(g) / 255.0,
                b: f32::from(b) / 255.0,
                a: f32::from(a) / 255.0,
            })
        }
        _ => None,
    }
}

fn json_display(value: &serde_json::Value) -> String {
    match value {
        serde_json::Value::String(s) => s.clone(),
        serde_json::Value::Number(n) => n.to_string(),
        serde_json::Value::Bool(b) => b.to_string(),
        serde_json::Value::Null => String::new(),
        other => other.to_string(),
    }
}
