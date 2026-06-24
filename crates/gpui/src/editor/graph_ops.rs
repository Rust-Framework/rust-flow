//! 图操作：替换图、边中插入节点、节点动作分发、删除节点。
//!
//! 稳定模块：图变更的核心业务逻辑，含桥接策略和动作分发。
//! 独立于渲染/交互层，便于单独测试和维护。

use gpui::Context;
use rust_agent_flow::{EdgeId, EdgeType, FlowGraph, NodeId, PortId, SizeF, Viewport};

use crate::node::NodeAction;

use super::flow_editor::FlowEditorView;

impl FlowEditorView {
    /// 替换当前流程图，重置选中/悬停/视口状态并自动重排。
    ///
    /// 供调用侧在切换数据源等场景使用。框架本身不持有"数据源"概念，
    /// 调用侧自行管理数据源状态，切换时调用此方法传入新图。
    pub fn set_graph(&mut self, graph: FlowGraph, cx: &mut Context<Self>) {
        self.graph = graph;
        self.selected = None;
        self.hovered = None;
        self.hovered_plus = None;
        self.panel_view = None;
        self.viewport = Viewport::default();
        self.relayout();
        cx.notify();
    }

    /// 在边中间插入新节点：拆边 → 插入节点 → 连两条新边。
    ///
    /// 用于连线「+」按钮添加节点：点击边中点 plus button → 弹出节点选择面板
    /// → 选择节点类型 → 调用此方法完成插入。
    pub(crate) fn insert_node_at_edge(
        &mut self,
        edge_id: EdgeId,
        kind: &str,
        cx: &mut Context<Self>,
    ) {
        // 1. 读取原边信息
        let (src, src_port, dst, dst_port, edge_type) = match self.graph.edge(edge_id) {
            Some(e) => (e.source, e.source_port.clone(), e.target, e.target_port.clone(), e.edge_type),
            None => return,
        };
        // 2. 删除原边
        self.graph.remove_edge(edge_id);
        // 3. 创建新节点（用 schema default_data + default_size）
        let flow_node = self.registry.get(kind);
        let data = flow_node
            .as_ref()
            .map(|f| f.schema().default_data())
            .unwrap_or_else(|| serde_json::json!({"label": kind}));
        let size = flow_node
            .as_ref()
            .map(|f| f.schema().default_size)
            .unwrap_or_else(|| SizeF::new(180.0, 64.0));
        let new_id = self.graph.add_node_with_size(kind, data, size);
        // 4. 连接 src → new → dst
        let mut e1 = rust_agent_flow::Edge::new(src, new_id);
        e1.source_port = src_port;
        e1.edge_type = edge_type;
        self.graph.add_edge(e1);
        let mut e2 = rust_agent_flow::Edge::new(new_id, dst);
        e2.target_port = dst_port;
        e2.edge_type = edge_type;
        self.graph.add_edge(e2);
        // 5. 选中新节点 + 重排
        self.selected = Some(new_id);
        self.relayout();
        cx.notify();
    }

    /// 处理节点动作（由 NodeView/PanelView 的回调调用）。
    pub(crate) fn handle_node_action(
        &mut self,
        node_id: NodeId,
        action: NodeAction,
        cx: &mut Context<Self>,
    ) {
        match action {
            NodeAction::Delete => self.delete_node(node_id, cx),
            NodeAction::ToggleCollapse => {
                if let Some(node) = self.graph.node_mut(node_id) {
                    // Loop 节点：toggle body_collapsed（收起/展开循环体）
                    // 其他节点（Condition）：toggle collapsed（收起/展开节点自身内容）
                    let key = if node.kind == "loop" {
                        "body_collapsed"
                    } else {
                        "collapsed"
                    };
                    let current = node
                        .data
                        .get(key)
                        .and_then(|v| v.as_bool())
                        .unwrap_or(false);
                    node.data[key] = serde_json::json!(!current);
                }
                self.sync_node_sizes();
                self.relayout();
                cx.notify();
            }
            NodeAction::SetData(key, value) => {
                if let Some(node) = self.graph.node_mut(node_id) {
                    node.data[key] = value;
                }
                // 仅当节点实际渲染尺寸变化时才触发 relayout，
                // 避免每次按键都运行 dagre 布局导致严重卡顿。
                if self.update_node_size_if_changed(node_id) {
                    self.relayout();
                }
                cx.notify();
            }
        }
    }

    /// 删除节点：线性桥接 + 级联删边 + 自动重排。
    ///
    /// 桥接策略（行业标准，参考 n8n/ReactFlow）：
    /// - 仅当节点恰好有 1 条入边和 1 条出边时，自动桥接前驱→后继
    /// - 多端口节点（条件/循环）删除时直接删除所有关联边，不做桥接
    pub(crate) fn delete_node(&mut self, node_id: NodeId, cx: &mut Context<Self>) {
        // 收集边信息（避免借用冲突）
        let in_edges: Vec<(NodeId, Option<PortId>, EdgeType)> = self
            .graph
            .in_edges(node_id)
            .map(|e| (e.source, e.source_port.clone(), e.edge_type))
            .collect();
        let out_edges: Vec<(NodeId, Option<PortId>)> = self
            .graph
            .out_edges(node_id)
            .map(|e| (e.target, e.target_port.clone()))
            .collect();

        // 线性桥接：1 入 1 出 → 创建桥接边
        if in_edges.len() == 1 && out_edges.len() == 1 {
            let (src, src_port, edge_type) = &in_edges[0];
            let (dst, dst_port) = &out_edges[0];
            let mut bridge = rust_agent_flow::Edge::new(*src, *dst);
            bridge.source_port = src_port.clone();
            bridge.target_port = dst_port.clone();
            bridge.edge_type = *edge_type;
            self.graph.add_edge(bridge);
        }

        // 删除节点（级联删除所有关联边）
        self.graph.remove_node(node_id);

        // 清理选中/悬停状态
        if self.selected == Some(node_id) {
            self.selected = None;
        }
        if self.hovered == Some(node_id) {
            self.hovered = None;
        }

        // 自动重排
        self.relayout();
        cx.notify();
    }
}
