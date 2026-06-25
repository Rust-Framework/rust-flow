# Condition 条件分支

## 结构化布局的首次登场

Condition 是第一个**结构化节点**——节点内部有按数据动态变化的行结构。它打破了「标题栏 + 单一主体」的二段式，引入「标题栏 + N 条件行 + 1 else 行」的多段式。

```
横向布局（左进右出）：
┌───────────────────────────────┐
[In]│ ◆ Condition                │  标题栏 h=TITLE_H=36
├───────────────────────────────┤
    │ If amount > 100     [if_0]→│  条件行 h=ITEM_H=36
    │ If user.is_admin    [if_1]→│  条件行 h=ITEM_H=36
    │ Else                [else]→│  兜底行 h=ITEM_H=36
└───────────────────────────────┘

纵向布局（上进下出）：
              In
               ↓
        ┌─Condition──┐
        │ if amount   │
        │ if user     │
        │ else        │
        └─┬──┬──┬────┘
          ↓  ↓  ↓     底部均布出口（if_0 最左，else 最右）
```

## 关键常量与数据格式

| 常量 | 值 | 用途 |
|------|-----|------|
| `TITLE_H` | 36 | 标题栏高度 |
| `ITEM_H` | 36 | 每个条件/else 行高度 |

数据格式：

```json
{
  "label": "Check",
  "conditions": [
    { "id": "if_0", "label": "amount > 100" },
    { "id": "if_1", "label": "user.is_admin" }
  ],
  "collapsed": false
}
```

`conditions` 数组长度决定 `if_i` 端口数量；`collapsed` 控制收起态。schema 默认 2 条件 + 1 else，默认高度 `TITLE_H + ITEM_H * 3 = 144`。

## 四个覆写方法

Condition 是覆写 `IFlowNode` 可选方法最多的内置节点：

| 方法 | 覆写 | 原因 |
|------|------|------|
| `ports_for_node` | 是 | if_i 端口随 conditions 数组动态生成 |
| `port_position` | 是 | if_i 端口需对齐条件行，else 对齐兜底行 |
| `content_size` | 是 | 高度 = `TITLE_H + ITEM_H * n_branches`，随条件数变化 |
| `plus_button_at_target` | 否 | 出口虽多但端口在节点右侧分散，按钮聚集不严重 |

### ports_for_node：动态端口

```rust
fn ports_for_node(&self, node: &Node) -> Vec<PortSpec> {
    let conditions = get_conditions(node);
    let mut ports = vec![
        PortSpec::new("in", PortDirection::In, PortSide::Auto),
        PortSpec::new("else", PortDirection::Out, PortSide::Auto),
    ];
    for (id, _) in &conditions {
        ports.push(PortSpec::new(id.as_str(), PortDirection::Out, PortSide::Auto));
    }
    ports
}
```

端口列表 = `in` + `else` + 每个条件一个 `if_i`。这是 Condition 区别于 Action 的核心——端口拓扑随数据变化。

### content_size：高度随条件数推导

```rust
fn content_size(&self, node: &Node) -> SizeF {
    let h = if is_collapsed(node) {
        TITLE_H + ITEM_H  // 收起：标题栏 + 主体（提示条件数）
    } else {
        TITLE_H + ITEM_H * n_branches(node) as f32  // 展开：标题栏 + N 条件行 + 1 else 行
    };
    SizeF::new(node.size.w, h)
}
```

`n_branches = conditions.len() + 1`（else 占一行）。收起态高度固定为 `TITLE_H + ITEM_H`（与 Action 同规格），展开态高度随条件数线性增长。

### port_position：精确对齐视觉行

横向布局下，每个 `if_i` 端口必须对齐第 i 行条件项的垂直中心：

```rust
"if_0" => {
    let idx = 0;
    let y = node.position.y + TITLE_H + ITEM_H * (idx as f32 + 0.5);  // 第 0 行中心
    Some(PointF::new(right, y))  // 节点右边缘
}
"else" => {
    let y = node.position.y + TITLE_H + ITEM_H * (n_cond as f32 + 0.5);  // 最后一行中心
    Some(PointF::new(right, y))
}
```

纵向布局下，所有出口（if_i + else）沿底部均匀分布，不重叠：

```rust
// if_i 在底部，按 (i + 0.5) / n_branches 比例定位
let t = (idx as f32 + 0.5) / n_br as f32;
let x = left + node.size.w * t;
Some(PointF::new(x, bottom))

// else 在底部最右，按 (n_cond + 0.5) / n_branches 比例定位
let t = (n_cond as f32 + 0.5) / n_br as f32;
let x = left + node.size.w * t;
Some(PointF::new(x, bottom))
```

`(i + 0.5) / n` 的 0.5 偏移确保端口在「格子的中心」而非边缘——n 个端口将节点宽度等分为 n 段，每段中心放一个端口。

## 收起态的特殊处理

收起时 Condition 退化为「标题栏 + 单一主体」结构，与 Action 同规格：

```
收起态：
┌───────────────────────────────┐
[In]│ ◆ Condition          [▷]  │  标题栏
├───────────────────────────────┤
    │       2 条件              │  主体显示条件数
    └────────────────[else]→────┘  单一出口（用 else 位置）
```

`port_position` 在收起态把所有 out 端口（if_0/if_1/.../else）合并到节点边缘垂直中心：

```rust
if collapsed {
    return match port_id.as_str() {
        "in" => /* 左中心 / 顶中心 */,
        _ => /* 右中心 / 底中心（所有 out 合并） */,
    };
}
```

这样收起时连到 if_0 的边和连到 else 的边端点重合——视觉上 Condition 退化为单出口节点，但数据上边的 `source_port` 仍保留，展开后恢复原端点。

## toggle 按钮的 kind 感知

`handle_node_action` 对 `ToggleCollapse` 的处理：

```rust
let key = if node.kind == "loop" { "body_collapsed" } else { "collapsed" };
```

Condition 用 `collapsed` 字段，Loop 用 `body_collapsed`——因为 Loop 收起的是外部循环体节点，不是自身内容。这是编辑器少数感知 kind 的地方。

`collapsed` 字段写入后，`sync_node_sizes` + `relayout` 重排——因为收起/展开改变了节点高度，dagre 需要重新计算周围节点位置。

## get_conditions 的容错

```rust
fn get_conditions(node: &Node) -> Vec<(String, String)> {
    node.data.get("conditions")
        .and_then(|v| v.as_array())
        .map(|arr| /* 解析 (id, label) */)
        .unwrap_or_else(|| vec![
            ("if_0".to_string(), String::new()),
            ("if_1".to_string(), String::new()),
        ])
}
```

数据缺失或格式错误时回退到默认 2 条件——保证节点始终可渲染。这是内置节点的防御性编程范式：**永远假设 `node.data` 可能不完整**。

## 端口颜色语义

| 端口 | ring/dot 颜色 | 含义 |
|------|--------------|------|
| `in` | `node_in_ring`/`node_in_dot` | 靛蓝（输入） |
| `if_i` | `node_out_ring`/`node_out_dot` | 橙色（输出，条件命中） |
| `else` | `node_out_ring`/`node_out_dot` | 橙色（输出，兜底） |

Condition 的 if_i 和 else 用相同颜色——它们语义上都是「出口」，只是触发条件不同。颜色区分方向（in/out），不区分分支类型。

## 小结

Condition 是首个结构化节点：标题栏 + N 条件行 + 1 else 行的多段式布局。它覆写了 `ports_for_node`（动态端口）、`port_position`（行对齐）、`content_size`（高度随条件数）三个方法。收起态退化为单出口节点，所有 out 端口合并到边缘中心。纵向布局下出口沿底部 `(i+0.5)/n` 均匀分布，避免重叠。`get_conditions` 的容错回退保证节点始终可渲染。

下一节：[Loop 循环迭代](loop-iteration.md)
