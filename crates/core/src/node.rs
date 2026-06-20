use std::collections::HashMap;

use serde::{Deserialize, Serialize};
use slotmap::SlotMap;

use crate::auto_layout::LayoutDirection;
use crate::geometry::handle_position;
use crate::id::{NodeId, PortId};
use crate::layout::VISUAL_HEIGHT;
use crate::math::{Point, Size};
use crate::node_layout::apply_structured_port_local;
use crate::node_type;
use crate::port::{FlowPort, PortSide};
use crate::viewport::Viewport;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FlowNode {
    pub id: NodeId,
    #[serde(default)]
    pub schema_id: String,
    pub position: Point,
    pub size: Size,
    pub node_type: String,
    pub label: String,
    #[serde(default)]
    pub data: serde_json::Value,
    pub inputs: Vec<(String, PortId)>,
    pub outputs: Vec<(String, PortId)>,
    #[serde(default)]
    pub selected: bool,
    #[serde(default)]
    pub z_order: u32,
}

#[derive(Debug, Clone)]
pub struct ResolvedNode {
    pub id: NodeId,
    pub node_type: String,
    pub label: String,
    pub data: serde_json::Value,
    pub selected: bool,
    pub zoom: f32,
    pub screen_pos: Point,
    pub screen_size: Size,
    pub inputs: Vec<(String, PortId)>,
    pub outputs: Vec<(String, PortId)>,
    pub port_anchors: HashMap<PortId, Point>,
    /// Handle center in node-local coordinates (on the border edge).
    pub port_local: HashMap<PortId, Point>,
    pub port_sides: HashMap<PortId, PortSide>,
}

impl FlowNode {
    pub fn new(id: NodeId, label: impl Into<String>, position: Point) -> Self {
        Self::typed(id, node_type::COMMON, label, position)
    }

    pub fn typed(
        id: NodeId,
        node_type: impl Into<String>,
        label: impl Into<String>,
        position: Point,
    ) -> Self {
        Self {
            id,
            schema_id: String::new(),
            position,
            size: Size::new(200.0, VISUAL_HEIGHT),
            node_type: node_type.into(),
            label: label.into(),
            data: serde_json::Value::Object(Default::default()),
            inputs: Vec::new(),
            outputs: Vec::new(),
            selected: false,
            z_order: 0,
        }
    }

    pub fn rendered_size(&self, zoom: f32) -> Size {
        Size::new(self.size.width * zoom, self.size.height * zoom)
    }

    fn side_count(ports: &SlotMap<PortId, FlowPort>, node_id: NodeId, side: PortSide) -> usize {
        ports
            .iter()
            .filter(|(_, p)| p.node == node_id && p.side == side)
            .count()
    }

    fn side_index(
        &self,
        ports: &SlotMap<PortId, FlowPort>,
        port_id: PortId,
        side: PortSide,
    ) -> usize {
        self.inputs
            .iter()
            .chain(self.outputs.iter())
            .map(|(_, id)| *id)
            .filter(|id| ports.get(*id).is_some_and(|p| p.side == side))
            .position(|id| id == port_id)
            .unwrap_or(0)
    }

    pub fn port_world_center(
        &self,
        port_id: PortId,
        side: PortSide,
        ports: &SlotMap<PortId, FlowPort>,
    ) -> Point {
        let total = Self::side_count(ports, self.id, side);
        let index = self.side_index(ports, port_id, side);
        handle_position(self.position, self.size, side, index, total)
    }

    /// Resolve screen-space node + port anchors for this frame.
    /// `port_local` centers sit on the node border; `port_anchors` = `screen_pos` + `port_local`.
    pub fn resolve(
        &self,
        viewport: &Viewport,
        ports: &SlotMap<PortId, FlowPort>,
        direction: LayoutDirection,
    ) -> ResolvedNode {
        let zoom = viewport.zoom;
        let screen_pos = viewport.world_to_screen(self.position);
        let screen_size = self.rendered_size(zoom);

        let mut port_anchors = HashMap::new();
        let mut port_local = HashMap::new();
        let mut port_sides = HashMap::new();
        for (_, pid) in self.inputs.iter().chain(self.outputs.iter()) {
            if let Some(port) = ports.get(*pid) {
                let total = Self::side_count(ports, self.id, port.side);
                let index = self.side_index(ports, *pid, port.side);
                let local =
                    handle_position(Point::default(), screen_size, port.side, index, total);
                port_local.insert(*pid, local);
                port_anchors.insert(
                    *pid,
                    Point::new(screen_pos.x + local.x, screen_pos.y + local.y),
                );
                port_sides.insert(*pid, port.side);
            }
        }

        apply_structured_port_local(
            &self.node_type,
            &self.data,
            screen_size,
            direction,
            ports,
            &self.inputs,
            &self.outputs,
            &mut port_local,
        );

        for (_, pid) in self.inputs.iter().chain(self.outputs.iter()) {
            if let Some(local) = port_local.get(pid) {
                port_anchors.insert(
                    *pid,
                    Point::new(screen_pos.x + local.x, screen_pos.y + local.y),
                );
            }
        }

        ResolvedNode {
            id: self.id,
            node_type: self.node_type.clone(),
            label: self.label.clone(),
            data: self.data.clone(),
            selected: self.selected,
            zoom,
            screen_pos,
            screen_size,
            inputs: self.inputs.clone(),
            outputs: self.outputs.clone(),
            port_anchors,
            port_local,
            port_sides,
        }
    }
}
