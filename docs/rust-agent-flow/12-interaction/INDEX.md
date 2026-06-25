# 第12章 交互与命中测试

如果说第 11 章是「画」，第 12 章就是「响应」。节点编辑器的交互复杂度远高于普通表单：同一个鼠标按下，可能是开始拖拽节点、开始平移画布、开始画边、点开「+」浮层，也可能是删除节点。框架用一套有限状态机（FSM）把这些可能性收敛成清晰的转移图。

本章回答三个问题：

1. 鼠标事件如何被分派到不同的交互状态？
2. 一个屏幕点如何被判定为「命中了哪个元素」？
3. 「从边插入节点」的浮层是怎么定位与响应的？

## 本章小节

| 小节 | 内容 |
|------|------|
| [交互状态机](interaction-fsm.md) | InteractionState 五种状态、转移条件、坐标语义 |
| [命中测试](hit-test.md) | HitResult 优先级、点-矩形/点-端口/点-折线判定 |
| [鼠标事件与节点选择浮层](mouse-events-picker.md) | 事件分派、AddingNodeFromEdge 浮层、6 种插入节点 |

## 学习目标

读完本章后，你应当能够：

- 画出 `InteractionState` 的状态转移图，标注每个转移的触发条件。
- 说出 `HitResult` 的七种变体及其优先级顺序。
- 解释为什么 `Panning` 用屏幕坐标、`DraggingNode` 用逻辑坐标。
- 在自己的代码里正确调用 `hit_test` 并按 `HitResult` 分发。
- 描述 `render_node_picker` 的定位公式与防冒泡机制。

## 关键源码定位

| 模块 | 路径 |
|------|------|
| 交互状态 | `crates/gpui/src/editor/interaction.rs` |
| 命中测试 | `crates/gpui/src/editor/hit_test.rs` |
| 节点浮层 | `crates/gpui/src/editor/flow_editor.rs`（`render_node_picker`） |
| 图操作 | `crates/gpui/src/editor/graph_ops.rs` |

## 下一步

从 [交互状态机](interaction-fsm.md) 开始。
