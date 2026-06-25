# NodeViewCtx 与 NodeAction

`IFlowNode` 的方法签名里反复出现两个类型：`NodeViewCtx`（渲染上下文）和 `NodeAction`（动作回调）。前者把渲染所需的「环境」打包传给节点实现，后者把节点视图/面板的「操作意图」回传给编辑器。两者构成视图与编辑器之间的双向桥梁。

## NodeViewCtx：渲染上下文

```rust
pub struct NodeViewCtx<'a> {
    pub window: &'a mut Window,   // GPUI 窗口句柄
    pub cx: &'a mut App,          // GPUI 应用上下文
    pub selected: bool,           // 当前节点是否选中
    pub hovered: bool,            // 是否被鼠标悬停
    pub scale: f32,               // 视口缩放比例
    pub layout: LayoutDirection,  // 横向/纵向布局
    pub theme: Theme,             // 当前主题颜色
    pub language: Language,       // 中英文
    pub on_action: Option<ActionCallback>,  // 动作回调
}
```

**设计要点**：

| 字段 | 用途 |
|------|------|
| `window` / `cx` | 调用 GPUI API（创建元素、查询字体、触发重绘） |
| `selected` | 选中态边框色（`node_border_selected` vs `node_border`） |
| `hovered` | 控制删除按钮等 hover 元素的显隐 |
| `scale` | 节点内部元素按此缩放（端口圆圈、字号、按钮尺寸都乘 scale） |
| `layout` | 端口位置随方向切换（横向：左右；纵向：上下） |
| `theme` | 所有颜色从 theme 取，支持主题切换 |
| `language` | 文案本地化（`t(lang, TKey::...)`） |
| `on_action` | 视图层动作回传通道 |

`scale` 是关键：节点内部所有尺寸都是**逻辑坐标**，渲染时乘 `scale`。例如端口圆点 `port_size = 6.0 * scale`，外环 `port_outer = (6.0 + 4.0) * scale`。这保证了缩放时视觉一致性。

## NodeAction：动作枚举

```rust
#[derive(Clone, Debug)]
pub enum NodeAction {
    Delete,                                  // 删除此节点
    ToggleCollapse,                          // 切换展开/收起
    SetData(String, serde_json::Value),      // 更新 node.data[key] = value
}

pub type ActionCallback = Arc<dyn Fn(NodeAction, &mut App) + Send + Sync>;
```

只有三种动作——**刻意保持极简**。任何节点视图/面板的需求都能映射到这三种：

| 视图操作 | 对应 NodeAction |
|----------|-----------------|
| 点击删除按钮 | `Delete` |
| 点击展开/收起按钮 | `ToggleCollapse` |
| 编辑 label、修改 conditions 数组、切换 loop_mode | `SetData(key, value)` |

`SetData` 是万能键——属性面板的所有字段编辑都走它：`SetData("label", json!("新名字"))`、`SetData("collapsed", json!(true))`、`SetData("conditions", json!([...]))`。

## 闭包捕获 node_id

`ActionCallback` 是 `Arc<dyn Fn(NodeAction, &mut App) + Send + Sync>`——闭包内部已捕获 `node_id`，调用方无需传入：

```rust
// FlowEditorView 渲染节点时构造回调
let on_action: ActionCallback = {
    let entity = entity.clone();
    Arc::new(move |action: NodeAction, cx: &mut App| {
        cx.update_entity(&entity, |view: &mut FlowEditorView, cx| {
            view.handle_node_action(node_id, action, cx);
        });
    })
};
// 传给 NodeViewCtx.on_action，节点视图内部调用：
// (ctx.on_action.as_ref().unwrap())(NodeAction::Delete, cx);
```

**为何不直接传 `node_id` 给回调？** 因为节点视图是 `RenderOnce` 的无状态组件，闭包捕获 `node_id` 后，视图代码只需调 `on_action(action)`——无需知道自己属于哪个节点。这降低了节点实现的认知负担。

## 编辑器侧的处理路径

`FlowEditorView::handle_node_action` 是回调的终点：

```rust
pub(crate) fn handle_node_action(&mut self, node_id: NodeId, action: NodeAction, cx) {
    match action {
        NodeAction::Delete => self.delete_node(node_id, cx),
        NodeAction::ToggleCollapse => {
            if let Some(node) = self.graph.node_mut(node_id) {
                // Loop 节点 toggle body_collapsed（收起循环体）
                // 其他节点 toggle collapsed（收起节点自身）
                let key = if node.kind == "loop" { "body_collapsed" } else { "collapsed" };
                let current = node.data.get(key).and_then(|v| v.as_bool()).unwrap_or(false);
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
            // 仅当渲染尺寸变化时才 relayout（避免每次按键都跑 dagre）
            if self.update_node_size_if_changed(node_id) {
                self.relayout();
            }
            cx.notify();
        }
    }
}
```

**关键优化**：`SetData` 路径调用 `update_node_size_if_changed`——只有结构化节点（如 Condition 的 conditions 数量变化）的尺寸真正改变时才触发 `relayout`。普通节点（如 Action 编辑 desc）不会触发 dagre，避免每次按键都全量重排。

## ToggleCollapse 的 kind 感知

`ToggleCollapse` 对不同 kind 行为不同：

| kind | toggle 的 data key | 含义 |
|------|-------------------|------|
| `loop` | `body_collapsed` | 隐藏/显示外部循环体节点（Loop 自身始终完整显示） |
| 其他（condition 等） | `collapsed` | 收起/展开节点自身的多分支内容 |

这是 `handle_node_action` 里硬编码的 `if node.kind == "loop"` 分支——少数几处编辑器需要感知 kind 的地方。之所以不放到 `IFlowNode`，是因为 toggle 逻辑与图结构（收起循环体需要隐藏一组节点）强耦合，属于编辑器职责。

## update_node_size_if_changed 的协作

```rust
pub(crate) fn update_node_size_if_changed(&mut self, node_id: NodeId) -> bool {
    let (kind, old_size) = /* 从 graph 取 */;
    let flow_node = self.registry.get(&kind)?;
    let new_size = flow_node.content_size(node);  // 调用 IFlowNode::content_size
    if new_size != old_size {
        self.graph.node_mut(node_id).size = new_size;
        true
    } else { false }
}
```

这是 `content_size` 方法被调用的**唯一入口**——`SetData` 后用它判断是否需要重排。Condition 覆写了 `content_size` 返回 `TITLE_H + ITEM_H * n_branches`，所以增删条件项时尺寸变化触发 relayout；Action 的 `content_size` 返回固定 `TITLE_H + BODY_H`，编辑 desc 不变尺寸，跳过 relayout。

## 完整调用链

```
用户点击删除按钮
  ↓
节点视图：on_action(NodeAction::Delete)
  ↓
ActionCallback 闭包：cx.update_entity(entity, |view, cx| view.handle_node_action(...))
  ↓
FlowEditorView::handle_node_action(node_id, Delete, cx)
  ↓
delete_node(node_id, cx) → 删节点+关联边+桥接前后节点 → relayout → cx.notify()
  ↓
下一帧 render() 重新渲染
```

这条链路贯穿视图层（节点）、实体层（FlowEditorView）、数据层（graph），用 `Arc` 闭包 + `cx.update_entity` 跨越 GPUI 的所有权边界——是理解整个框架事件流的关键。

## 小结

`NodeViewCtx` 把渲染所需的全部环境（window/cx/theme/scale/layout/language）打包传给节点实现，`on_action` 字段是回传通道。`NodeAction` 三种变体覆盖所有视图操作需求，闭包捕获 `node_id` 让视图无状态。`handle_node_action` 是动作终点，其中 `SetData` 路径通过 `update_node_size_if_changed` 智能跳过不必要的 relayout——这是 `content_size` 方法的实际价值所在。

下一节：[Start / End / Action](../09-builtin-nodes/start-end-action.md)
