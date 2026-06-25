# 常见陷阱与排查

rust-agent-flow 的设计目标是「默认正确」，但有几类问题反复出现在新用户的集成里。它们大多源于对框架某条不变量的忽视：布局必须显式触发、初始化必须先于视图、尺寸必须自洽、端口必须对齐、面板状态必须新鲜、事件必须放行。本节把这六类陷阱连同现象、根因、解法一并梳理。

## 陷阱一：未调用 auto_layout

**现象**：所有节点堆在画布左上角原点 (0,0)，互相重叠，看不见结构。

**根因**：`FlowGraph` 里的 `Node.position` 初始是 `PointF::zero()`。框架不会自动布局——dagre 排版是显式调用 `auto_layout(cx)` 才发生的。

**解法**：在 `FlowEditorView::new` 之后立即调用：

```rust
let mut editor = FlowEditorView::new(graph, cx);
editor.auto_layout(cx);   // 必须显式调用
```

**排查**：在 `auto_layout` 前后打印节点 position，若全是 (0,0) 即未布局；若调用了仍是 (0,0)，检查图里是否有环（dagre 要求有向无环，回环边需标记 `EdgeKind::LoopBack` 才会被正确处理）。

## 陷阱二：未调用 init(cx)

**现象**：画布上节点能显示，但 `gpui_component::ListItem` 的选中边框样式异常，或某些组件主题不生效。

**根因**：`rust_agent_flow_gpui::init(cx)` 注册内置节点类型、初始化框架所需的全局状态（含 gpui-component 相关）。漏调会导致组件缺少必要的全局上下文。

**解法**：在 `application().run` 闭包最开头调用：

```rust
.run(move |cx: &mut gpui::App| {
    rust_agent_flow_gpui::init(cx);   // 第一行
    cx.spawn(...)
});
```

**排查**：若节点画布空白（找不到 IFlowNode），是 init 没调或调用晚于视图创建；若仅组件样式异常，通常是 init 缺失导致的全局主题/资源未注册。

## 陷阱三：content_size 未覆写

**现象**：自定义结构化节点（Condition/Loop 或自己的多端口节点）尺寸不对——要么过大留白，要么过小内容被裁；更隐蔽的是 dagre 布局结果错乱、命中测试区域偏移、回环边边界算错。

**根因**：结构化节点的尺寸不能靠「内容自然撑开」——它的端口位置、子区域布局都依赖一个确定的 `content_size`。若 `IFlowNode` 实现没覆写 `content_size`，框架用默认值，与实际渲染不匹配。

**解法**：在自定义 `IFlowNode` 实现里精确覆写 `content_size`，返回主体内容区的尺寸：

```rust
impl IFlowNode for MyNode {
    fn content_size(&self) -> SizeF {
        SizeF::new(px(220.0), px(120.0))   // 与渲染实际尺寸一致
    }
}
```

**排查**：连线端点与可见端口错位、点击命中区域与可视区域不符，几乎都是 `content_size` 与渲染尺寸不一致。量一下渲染出的节点宽高，对照 `content_size` 返回值。

## 陷阱四：port_position 与渲染端口不一致

**现象**：连线端点漂在节点外、悬在节点中部、或与可见端口圆点错位。

**根因**：`IFlowNode::port_position` 返回的坐标必须与 `get_view` 渲染端口圆点的实际位置一致。框架用 `port_position` 计算连线起止点，用 `get_view` 画端口圆点——二者分属不同代码路径，一旦不一致就错位。

**解法**：让 `port_position` 的返回值严格等于渲染时端口圆点的中心坐标（相对于节点左上角）。建议在渲染代码里把端口圆点的布局参数提取为常量，`port_position` 引用同一组常量：

```rust
const PORT_INSET: f32 = 12.0;
const PORT_GAP: f32 = 24.0;

// 渲染：port 圆点 x = PORT_INSET + i * PORT_GAP
// port_position 返回同样坐标
fn port_position(&self, port_id: &str, side: PortSide) -> PointF {
    let i = self.port_index(port_id);
    PointF::new(PORT_INSET + i as f32 * PORT_GAP, 0.0)
}
```

**排查**：缩放到 200% 看端点与圆点的偏移方向与距离，反推 `port_position` 是偏了 inset、gap 还是 index 计算。

## 陷阱五：panel_view 持有过期引用

**现象**：注入新扩展（`set_data_type_provider` / `set_language` / `set_syntax_service`）后面板仍显示旧类型、旧文案、旧高亮。

**根因**：这其实是「框架已处理」的陷阱——理解它有助于避免自己引入类似问题。面板在构造时把扩展的产物（DataTypeRegistry、t(lang,...) 文案、code_editor 配置）固化进组件状态。若不销毁重建，新扩展无法生效。

**解法**：框架的三个 setter 已统一处理：注入即 `panel_view = None`，下帧 `ensure_panel_view` 用新扩展重建。你无需手动销毁，但要知道这条语义：

```rust
pub fn set_data_type_provider(&mut self, provider, cx) {
    self.data_type_provider = Some(provider);
    self.panel_view = None;   // 框架自动销毁
    cx.notify();
}
```

**排查**：如果你自定义了类似「持有扩展并构造视图」的机制，注入新扩展后视图不更新，多半是忘了销毁重建。对照框架的 setter 写法补上 `panel_view = None`。

## 陷阱六：Input 组件 h_full() 冲突

**现象**：单行 Input 在某些容器里被撑到全高，破坏行高；或单行模式下行高异常。

**根因**：gpui-component 的 `Input` 有一个固有的 `h_full()` 方法，在 flex 容器里会被拉伸。单行模式下你不希望它占满父高度。

**解法**：显式调用 `Styled::h_full()` 消歧，或用定高容器包裹。在面板渲染里，单行字段通常放在定高的 flex 行里，避免 Input 撑高：

```rust
// 单行 Input 放在定高行容器
div().h(px(32.0)).flex().items_center().child(Input::new(&state).appearance(true))
```

**排查**：若单行 Input 异常变高，检查其父容器是否是 `flex_col` 且未限定高度，导致 Input 的 `h_full()` 撑满。

## 陷阱七：Popover 包裹整行拦截事件

**现象**：用 `Popover` 包裹一整行（如 Tree 行）做悬浮提示，结果行内 Input/Select 无法点击、选中失效。

**根因**：`Popover` 的触发元素会拦截鼠标事件用于显示/隐藏浮层，整行被包裹后行内控件收不到事件。

**解法**：移除整行 Popover，改用 `on_mouse_down` 手动选中行；提示信息用 `tooltip` 挂在具体控件上而非整行。框架的 Tree 渲染就是这样处理的——行选中靠 `on_mouse_down`，不靠 Popover。

```rust
// 错误：整行 Popover 拦截事件
div().child(Popover::new(...).child(div().child(Input::new(...))))

// 正确：on_mouse_down 选中，tooltip 单独挂
div()
    .on_mouse_down(MouseButton::Left, move |...| { /* 选中行 */ })
    .child(Input::new(...).tooltip("提示"))
```

## 陷阱速查表

| 现象 | 首查项 |
|------|--------|
| 节点堆原点 | `auto_layout` 是否调用 |
| 画布空白 / 组件样式异常 | `init(cx)` 是否调用、是否早于视图 |
| 连线端点错位 | `port_position` 与渲染端口是否一致 |
| 节点尺寸错乱 | `content_size` 是否覆写、是否与渲染一致 |
| 注入扩展面板不更新 | 是否漏了 `panel_view = None`（框架内置已处理） |
| 单行 Input 变高 | 父容器是否未限高，Input 的 `h_full()` 撑满 |
| 行内控件点不动 | 是否用 Popover 包了整行，改 `on_mouse_down` |

## 小结

六类陷阱分别对应框架六条不变量：布局显式触发、初始化先行、尺寸自洽、端口对齐、面板状态新鲜、事件放行。理解每条不变量的成因，比死记解法更重要——它们是框架设计的边界条件，越界即出问题。

下一节：[性能优化技巧](performance-tips.md)
