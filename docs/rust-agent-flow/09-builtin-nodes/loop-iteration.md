# Loop 循环迭代

## 4 端口拓扑

Loop 是最复杂的内置节点——它有 4 个端口，分两组：主线（In→Done）和循环体支线（LoopBody→LoopIn）。这种拓扑让它能表达「执行循环体直到完成」的语义。

```
纵向布局（主要标准）：
              In
               ↓
        ┌──────────────────┐
        │     ⟳ Loop       │  标题栏 h=TITLE_H=36
        ├──────────────────┤
        │   For each item  │  循环条件区 h=BODY_H=44
        └─┬──────────────┬─┘
   ←LoopIn │              │ LoopBody──→ (body 节点，上进下出)
          ↑              ↓
          │           Done
          │
          └──回环边从 body 底部出，向下绕过，左进 loop_in
```

| 端口 | direction | 横向位置 | 纵向位置 | 语义 |
|------|-----------|----------|----------|------|
| `in` | In | 左中心 | 顶中心 | 主线入口 |
| `done` | Out | 右中心 | 底中心 | 主线出口（循环完成） |
| `loop_body` | Out | 右中心（条件区 Y） | 右中心（条件区 Y） | 循环体出口 |
| `loop_in` | In | 左中心（条件区 Y） | 左中心（条件区 Y） | 循环体回连入口 |

**关键差异**：主线 In→Done 随布局方向变化（横向左右、纵向上/下），循环体支线 LoopBody/LoopIn **两种布局一致**——LoopBody 始终右出，LoopIn 始终左进。

## 为何支线方向固定

循环体节点始终纵向编排（上进下出），无论主图布局方向。回环边从 body 节点底部出，向下绕过 body 组合边界，左进 loop_in。如果支线方向也随主布局变化，横向布局下回环边会变得复杂且易与主线冲突。

固定支线方向让回环边路由算法简化为「向下绕圈」一种模式——这是 Loop 节点设计的核心权衡。

## Schema 与字段

| 属性 | 值 |
|------|-----|
| kind | `"loop"` |
| 端口 | in, done, loop_body, loop_in（全 Auto） |
| 默认尺寸 | 220 × (TITLE_H + BODY_H) = 220 × 80 |
| BODY_H | 44 |
| Schema 字段 | `label`(Text), `loop_mode`(Dropdown), `loop_expr`(CodeBlock) |

`loop_mode` 的 4 个选项：

```rust
FieldType::Dropdown(vec![
    DropdownOption::new("for_each", "For each item"),
    DropdownOption::new("while", "while cond"),
    DropdownOption::new("for_loop", "for i in 0..n"),
    DropdownOption::new("batch_parallel", "parallel each"),
])
```

主体文案优先用 `node.data["desc"]`，否则按 `loop_mode` 显示模式标签（i18n）。

## 覆写的方法

| 方法 | 覆写 | 原因 |
|------|------|------|
| `ports_for_node` | 否 | 4 端口静态固定 |
| `port_position` | 是 | 4 端口需精确对齐（主线 vs 支线 Y 坐标不同） |
| `content_size` | 是（固定值） | Loop 自身不收起，高度固定 `TITLE_H + BODY_H` |
| `plus_button_at_target` | 是 | done 出口的「+」按钮放目标侧避让 loop_body 按钮 |

### port_position：主线 vs 支线 Y 坐标

```rust
let node_mid_y = node.position.y + node.size.h * 0.5;       // 主线 Y
let body_mid_y = node.position.y + TITLE_H + BODY_H * 0.5;  // 支线 Y（条件区中心）

match port_id.as_str() {
    "in"  => /* 横向: (left, node_mid_y)   纵向: (mid_x, top) */,
    "done" => /* 横向: (right, node_mid_y)  纵向: (mid_x, bottom) */,
    "loop_body" => Some(PointF::new(right, body_mid_y)),     // 始终右
    "loop_in"   => Some(PointF::new(left, body_mid_y)),      // 始终左
    _ => None,
}
```

主线用 `node_mid_y`（节点几何中心），支线用 `body_mid_y`（条件区中心）——两组端口在 Y 方向错开 18px，视觉上清晰区分主线与支线。

### plus_button_at_target：避让按钮聚集

```rust
fn plus_button_at_target(&self, source_port: Option<&str>) -> bool {
    match source_port {
        Some("done") => true,   // done 的按钮放目标侧
        _ => false,             // loop_body 的按钮放源侧
    }
}
```

**为何只对 done 覆写**：Loop 的 done 和 loop_body 两个出口都在右侧，Y 坐标差仅 18px（`node_mid_y` 与 `body_mid_y` 的差）。如果两个按钮都放源侧（Loop 右侧），会聚集重叠。把 done（主线出口）的按钮移到目标节点侧，loop_body（支线出口）保持源侧（循环体节点就在 Loop 右侧，源侧按钮位置自然）。

这是 `plus_button_at_target` 方法的典型用例——**出口密集时的视觉冲突解决**。

## Loop 不收起自身

```rust
fn content_size(&self, node: &Node) -> SizeF {
    SizeF::new(node.size.w, TITLE_H + BODY_H)  // 固定，不随 body_collapsed 变化
}
```

Loop 节点本身始终完整显示（标题栏 + 循环条件区），收起的是**外部循环体节点**。`is_body_collapsed` 读 `node.data["body_collapsed"]`，控制的是循环体节点的显隐，不是 Loop 自身的高度。

这与 Condition 的 `collapsed` 语义不同——Condition 收起自身内容，Loop 收起外部节点。`handle_node_action` 里 `if node.kind == "loop" { "body_collapsed" } else { "collapsed" }` 的分支正是为此。

## 循环体分组与回环边

Loop 的 4 端口拓扑支持回环边：

```rust
// 回环边：从循环体最后节点 → loop_in
edge.source = body_end_node_id;
edge.target = loop_node_id;
edge.target_port = Some("loop_in".to_string());
edge.kind = EdgeKind::LoopBack;  // 标记为回环边，路由算法用 U 形
```

`FlowGraph::loop_body_groups()` 通过 BFS 沿 `loop_body` 出边展开循环体节点集合，排除 `loop_in` 回环边与 `done` 出口节点。这是渲染层隐藏循环体、回环边路由的唯一数据源。

## toggle 按钮的语义

```
body_collapsed=true  → ChevronRight（▷，循环体已收起，点击展开）
body_collapsed=false → ChevronDown（▽，循环体已展开，点击收起）
```

点击 toggle 按钮触发 `NodeAction::ToggleCollapse`，编辑器写入 `body_collapsed` 字段，然后 `sync_node_sizes` + `relayout`——因为收起循环体改变了图结构（隐藏一组节点），dagre 需要重新排版周围节点。

## 端口颜色语义

| 端口 | ring/dot 颜色 | 含义 |
|------|--------------|------|
| `in` | `node_in_ring`/`node_in_dot` | 靛蓝（主线输入） |
| `done` | `node_out_ring`/`node_out_dot` | 橙色（主线输出） |
| `loop_body` | `node_in_ring`/`node_in_dot` | 靛蓝（支线输出，与 in 同色） |
| `loop_in` | `node_in_ring`/`node_in_dot` | 靛蓝（支线输入） |

**注意**：loop_body 是 Out 方向，但颜色用 `node_in_*`（靛蓝）——因为它连向循环体节点（作为循环体的输入），视觉上与 in 同色更直观。这是颜色语义对方向语义的覆盖，体现了 Loop 拓扑的特殊性。

## 完整布局示意

```
横向布局：
In ─▶│ ┌──────────────────────┐ │◀── Done
      │ │     ⟳ Loop          │ │
      │ ├──────────────────────┤ │
      │ │   For each item      │ │
      │ └─┬──────────────────┬─┘ │
      │   │ LoopIn   LoopBody│──→ (body 节点)
      │   ↑                  │
      │   │      回环边向下绕回
      └───┘
```

横向布局下，主线 In→Done 走节点左右中心，支线 LoopBody/LoopIn 走条件区中心 Y。回环边从 body 底部出，向下绕过 body 组合边界，左进 loop_in——与纵向布局的路由模式一致。

## 小结

Loop 是最复杂的内置节点：4 端口拓扑分主线（In→Done）与支线（LoopBody/LoopIn）。支线方向固定（右出/左进），简化回环边路由。覆写 `port_position` 让主线用节点中心 Y、支线用条件区中心 Y，视觉区分两组端口。覆写 `plus_button_at_target` 让 done 按钮放目标侧，避让 loop_body 按钮聚集。Loop 自身不收起，收起的是外部循环体节点（`body_collapsed` 字段）。

下一节：[Variable / Adapter / Agent](variable-adapter-agent.md)
