//! Node extension: [`IFlowNodeProvider`] supplies per-type node chrome and property panels.

use std::collections::HashMap;
use std::sync::Arc;

use rust_agent_flow::{builtin_type_registry, FlowGraph, FlowTypeRegistry, NodeId, ResolvedNode, Size};
use gpui::*;

use crate::schema::SchemaDrivenProvider;
use crate::theme::FlowTheme;

/// Context passed to property-panel builders so providers can mutate graph state.
pub struct FlowPanelContext<'a> {
    pub graph: &'a mut FlowGraph,
    pub node_id: NodeId,
    pub theme: &'a FlowTheme,
    pub notify: Arc<dyn Fn()>,
}

/// Extension interface: node rendering + property panel per `node_type`.
pub trait IFlowNodeProvider: Send + Sync {
    fn node_type(&self) -> &'static str;

    /// Visual content inside the framework positioning shell (includes styling).
    fn render_node(&self, node: &ResolvedNode, theme: &FlowTheme) -> Div;

    /// Side-panel editor for the selected node instance.
    fn render_panel(&self, ctx: &mut FlowPanelContext<'_>) -> Div;

    fn default_size(&self) -> Size;
}

/// Type-keyed provider that resolves by `node_type` string at render time.
struct TypedProvider {
    inner: HashMap<String, Arc<dyn IFlowNodeProvider>>,
    fallback: Arc<dyn IFlowNodeProvider>,
}

impl TypedProvider {
    fn get_for_type(&self, node_type: &str) -> &dyn IFlowNodeProvider {
        self.inner
            .get(node_type)
            .map(|p| p.as_ref())
            .unwrap_or(self.fallback.as_ref())
    }
}

/// Registry that maps `node_type` strings to providers.
pub struct FlowNodeRegistry {
    typed: TypedProvider,
    type_registry: FlowTypeRegistry,
}

impl FlowNodeRegistry {
    pub fn new(type_registry: FlowTypeRegistry) -> Self {
        let fallback = Arc::new(SchemaDrivenProvider::new("common", type_registry.clone()));
        let mut inner: HashMap<String, Arc<dyn IFlowNodeProvider>> = HashMap::new();

        for type_id in type_registry.types.keys() {
            let provider = Arc::new(SchemaDrivenProvider::new(
                type_id.clone(),
                type_registry.clone(),
            ));
            inner.insert(type_id.clone(), provider);
        }

        Self {
            typed: TypedProvider {
                inner,
                fallback,
            },
            type_registry,
        }
    }

    pub fn register(&mut self, provider: Arc<dyn IFlowNodeProvider>) {
        let type_id = provider.node_type();
        if !type_id.is_empty() {
            self.typed.inner.insert(type_id.to_string(), provider);
        }
    }

    pub fn get(&self, node_type: &str) -> &dyn IFlowNodeProvider {
        self.typed.get_for_type(node_type)
    }

    pub fn type_registry(&self) -> &FlowTypeRegistry {
        &self.type_registry
    }

    pub fn mindmap() -> Self {
        use crate::nodes::MindMapNodeProvider;

        let types = builtin_type_registry();
        let mut registry = Self::new(types);
        registry.register(Arc::new(MindMapNodeProvider));
        registry
    }

    pub fn builtin() -> Self {
        use crate::nodes::{
            RealBranchProvider, RealCommonProvider, RealHttpProvider, RealLoopProvider,
            RealTriggerProvider,
        };

        let types = builtin_type_registry();
        let mut registry = Self::new(types);
        registry.register(Arc::new(RealCommonProvider));
        registry.register(Arc::new(RealBranchProvider));
        registry.register(Arc::new(RealLoopProvider));
        registry.register(Arc::new(RealTriggerProvider));
        registry.register(Arc::new(RealHttpProvider));
        registry
    }

    pub fn from_type_registry(registry: FlowTypeRegistry) -> Self {
        Self::new(registry)
    }
}

impl Clone for FlowNodeRegistry {
    fn clone(&self) -> Self {
        Self::new(self.type_registry.clone())
    }
}
