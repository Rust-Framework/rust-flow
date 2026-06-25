# port_position 与 content_size

## 两个方法的协作关系

`port_position` 和 `content_size` 是 `IFlowNode` 中最容易被忽视、却最影响视觉正确性的两个方法。前者决定连线端点位置，后者决定节点占多大空间——两者必须与 `get_view` 的实际渲染严格一致，否则会出现：

- 连线端点悬在节点外（`port_position` 与渲染端口圆圈不对齐）
- 节点重叠（`content_size` 小于实际渲染高度，dagre 给的空间不够）
- 命中测试偏移（点击节点没反应，因为命中区域用 `content_size` 计算）

## port_position 的返回值语义

```rust
fn port_position(&self, node: &Node, port_id: &PortId, layout: LayoutDirection) -> Option<PointF>
```

返回**逻辑坐标**（节点 position 为左上角原点的绝对坐标）下的端口圆心位置：

```rust
// 端口在节点右边缘垂直中心
let right = node.position.x + node.size.w;
let mid_y = node.position.y + node.size.h * 0.5;
Some(PointF::new(right, mid_y))
```

注意是**绝对坐标**（`node.position.x + ...`），不是相对节点左上角的偏移。这与 `get_view` 内部的相对坐标（`make_port(left, top, ...)` 用的相对父容器坐标）不同——`get_view` 在节点容器内渲染，`port_position` 在画布坐标系计算。

## 返回 None 的含义

```rust
fn port_position(&self, _node: &Node, _port_id: &PortId, _layout: LayoutDirection) -> Option<PointF> {
    None  // 默认实现
}
```

返回 `None` 表示「用框架统一算法」——按 `PortSpec.side` 计算节点边缘中点：

| side | 横向布局位置 | 纵向布局位置 |
|------|--------------|--------------|
| Left | 左中心 | 左中心 |
| Right | 右中心 | 右中心 |
| Top | 顶中心 | 顶中心 |
| Bottom | 底中心 | 底中心 |
| Auto | 按方向推导（In 左/顶，Out 右/底） | 按方向推导 |

`Auto` 让框架根据端口方向（In/Out）和布局方向自动推导——大多数简单节点用 Auto + 不覆写 `port_position` 即可工作。但覆写后位置更精确（与 `get_view` 内 `make_port` 的位置严格对齐）。

## 何时必须覆写 port_position

| 场景 | 原因 |
|------|------|
| 多端口在同一侧 | 默认算法让所有右侧端口都返回右中心，重叠 |
| 端口需对齐视觉行 | Condition 的 if_i 必须对齐条件行 Y |
| 主线/支线 Y 错开 | Loop 的主线用节点中心，支线用条件区中心 |
| 端口在节点内部 | 非边缘端口（罕见，不推荐） |

简单节点（单 In + 单 Out）可不覆写——默认算法的边缘中心与 `make_port` 的位置一致。但内置 8 种节点都覆写了，是为了**显式声明位置，避免默认算法的隐式正确性**。

## 横向 vs 纵向布局的处理

`port_position` 接收 `layout: LayoutDirection` 参数，必须按方向返回不同位置：

```rust
match layout {
    LayoutDirection::Horizontal => {
        // 横向：In 左中心，Out 右中心
        match port_id.as_str() {
            "in" => Some(PointF::new(left, node_mid_y)),
            "out" => Some(PointF::new(right, node_mid_y)),
            _ => None,
        }
    }
    LayoutDirection::Vertical => {
        // 纵向：In 顶中心，Out 底中心
        match port_id.as_str() {
            "in" => Some(PointF::new(mid_x, top)),
            "out" => Some(PointF::new(mid_x, bottom)),
            _ => None,
        }
    }
}
```

**例外**：Loop 的 `loop_body`/`loop_in` 两种布局都返回相同位置（始终右出/左进）——这是循环体支线方向固定的特殊设计，不推荐普通节点效仿。

## content_size 的返回值语义

```rust
fn content_size(&self, node: &Node) -> SizeF
```

返回节点**应该**的渲染尺寸。框架用此值同步 `node.size`：

```rust
// update_node_size_if_changed 的核心逻辑
let new_size = flow_node.content_size(node);
if new_size != old_size {
    node.size = new_size;  // 同步到 node.size
    return true;           // 触发 relayout
}
```

`content_size` 的返回值会写入 `node.size`，影响 dagre 布局、命中测试、回环边边界计算。**返回值必须与 `get_view` 的实际渲染尺寸一致**。

## content_size 的两种模式

### 模式一：固定高度（简单节点）

```rust
fn content_size(&self, node: &Node) -> SizeF {
    SizeF::new(node.size.w, TITLE_H + BODY_H)  // 固定常量
}
```

Start/End/Action/Variable/Adapter/Agent/Loop 都用此模式——高度不随数据变化。覆写是为了显式声明（比依赖 `node.size.h` 的初值更稳健）。

### 模式二：动态高度（结构化节点）

```rust
fn content_size(&self, node: &Node) -> SizeF {
    let h = if is_collapsed(node) {
        TITLE_H + ITEM_H  // 收起态
    } else {
        TITLE_H + ITEM_H * n_branches(node) as f32  // 展开态，随条件数变化
    };
    SizeF::new(node.size.w, h)
}
```

Condition 用此模式——高度随 conditions 数组长度变化。`update_node_size_if_changed` 检测到尺寸变化后触发 `relayout`，dagre 重新排版周围节点。

## 宽度的处理约定

`content_size` 的宽度通常返回 `node.size.w`，**不主动改变宽度**：

```rust
SizeF::new(node.size.w, computed_height)
```

宽度由 schema `default_size` 或创建时指定，运行时保持不变。原因：

- 宽度变化会触发 dagre 全量重排（高度变化也会，但宽度影响 nodesep 更显著）
- 用户预期节点宽度稳定，文字超长时由 GPUI 裁剪
- 高度变化是「内容驱动」，宽度变化是「配置驱动」——后者应由用户显式操作

如果节点确实需要动态宽度（如根据 label 长度自适应），需在 `content_size` 返回推导宽度，但要权衡重排性能。

## get_view 与 content_size 的一致性

**这是最容易出错的点**：`get_view` 渲染的高度必须等于 `content_size` 返回的高度。

```rust
// 错误：get_view 用 BODY_H=28，content_size 用 BODY_H=30
fn get_view(&self, node: &Node, ctx: &mut NodeViewCtx) -> AnyElement {
    let h = (TITLE_H + 28.0) * s;  // 渲染高度 64
    // ...
}
fn content_size(&self, node: &Node) -> SizeF {
    SizeF::new(node.size.w, TITLE_H + 30.0)  // 声明高度 66
}
```

不一致会导致：
- 节点底部出现 2px 空白（content_size > 渲染高度）
- 端口圆圈被裁剪（content_size < 渲染高度）
- 命中测试区域与视觉节点不匹配

**最佳实践**：用常量统一管理高度，`get_view` 和 `content_size` 引用同一常量：

```rust
const BODY_H: f32 = 28.0;

fn get_view(...) -> AnyElement {
    let h = (TITLE_H + BODY_H) * s;  // 引用常量
}
fn content_size(...) -> SizeF {
    SizeF::new(node.size.w, TITLE_H + BODY_H)  // 引用同一常量
}
```

## port_position 与 get_view 内 make_port 的一致性

`port_position` 返回的坐标必须与 `get_view` 内 `make_port` 渲染的位置一致——否则连线端点与视觉端口圆圈错位。

```rust
// get_view 内（相对坐标，乘 scale）
let mid_y_node = h * 0.5;  // h = node.size.h * s
container.child(make_port(
    w - port_outer_half,           // 右边缘
    mid_y_node - port_outer_half,  // 垂直中心
    ...
));

// port_position 内（绝对坐标，不乘 scale）
let right = node.position.x + node.size.w;
let node_mid_y = node.position.y + node.size.h * 0.5;
Some(PointF::new(right, node_mid_y))
```

两者描述同一位置——`get_view` 用相对坐标 + scale 渲染，`port_position` 用绝对坐标计算端点。坐标系不同但位置一致：节点右边缘垂直中心。

## 调试技巧

端口位置错位时的排查步骤：

1. 检查 `content_size` 返回高度是否与 `get_view` 渲染高度一致
2. 检查 `port_position` 的 `node.position + offset` 是否对应 `get_view` 内的相对 offset
3. 检查 `port_position` 是否处理了 `layout` 参数（横向/纵向）
4. 用 `println!` 打印 `port_position` 返回值，与 `get_view` 内 `make_port` 的参数对比

## 小结

`port_position` 返回逻辑绝对坐标，`None` 时用框架默认算法（边缘中心）。多端口同侧、行对齐、主线/支线错开时必须覆写。`content_size` 返回节点应占尺寸，框架据此同步 `node.size` 并触发 relayout——返回值必须与 `get_view` 渲染尺寸严格一致。最佳实践是用常量统一管理高度，`get_view`/`content_size`/`port_position` 引用同一常量，避免不一致导致的视觉错位。这三者的协调是自定义节点开发中最需要细致打磨的部分。

下一节：[Start / End / Action](../09-builtin-nodes/start-end-action.md)
