# 动态端口与 ports_for_node

## 何时需要动态端口

大多数节点端口静态固定——schema 声明什么就是什么。但当节点的出口数量**随数据变化**时，就需要覆写 `ports_for_node`。典型场景：

| 场景 | 例子 |
|------|------|
| 多条件分支 | Condition 的 if_0, if_1, ... 随 conditions 数组 |
| 多输出通道 | 自定义「Split」节点按配置输出 N 路 |
| 状态机出口 | 每个状态一个出口，状态数动态 |
| Switch 分支 | 类似 Condition 但无 else 兜底 |

## ports_for_node vs schema().ports

| 方法 | 用途 | 调用时机 |
|------|------|----------|
| `schema().ports` | 静态声明，用于面板/校验/默认值 | 注册时确定 |
| `ports_for_node(node)` | 动态查询，用于实际连线/布局推导 | 渲染/布局时确定 |

`schema().ports` 是「这个 kind 的节点最多有哪些端口」，`ports_for_node` 是「这个具体节点实例当前有哪些端口」。Condition 的 schema 声明 `in/else/if_0/if_1`（默认 2 条件），但 `ports_for_node` 根据 `node.data["conditions"]` 返回实际端口列表。

## 实现：以 Switch 节点为例

下面实现一个 Switch 节点——按 `cases` 数组动态生成出口端口，无 else 兜底。

### schema 声明（静态默认）

```rust
impl SwitchNode {
    pub fn new() -> Self {
        Self {
            schema: NodeSchema::new("switch", "Switch")
                .with_size(SizeF::new(220.0, TITLE_H + ITEM_H * 3.0))
                // schema 声明默认 2 case 端口（静态，用于面板默认值）
                .with_port(PortSpec::new("in", PortDirection::In, PortSide::Auto))
                .with_port(PortSpec::new("case_0", PortDirection::Out, PortSide::Auto))
                .with_port(PortSpec::new("case_1", PortDirection::Out, PortSide::Auto))
                .with_field(
                    FieldSpec::new("label", "Label", FieldType::Text)
                        .with_default(serde_json::json!("")),
                )
                .with_field(
                    FieldSpec::new("cases", "Cases",
                        FieldType::List(ListSpec::new(vec![
                            FieldSpec::new("id", "ID", FieldType::Text)...,
                            FieldSpec::new("label", "Match Expression", FieldType::CodeEditor)...,
                        ]))
                        .with_default(serde_json::json!([
                            { "id": "case_0", "label": "" },
                            { "id": "case_1", "label": "" }
                        ]))),
                ),
        }
    }
}
```

### ports_for_node 覆写（动态生成）

```rust
fn ports_for_node(&self, node: &Node) -> Vec<PortSpec> {
    let cases = get_cases(node);  // 解析 node.data["cases"]
    let mut ports = vec![
        PortSpec::new("in", PortDirection::In, PortSide::Auto),
    ];
    for (id, _) in &cases {
        ports.push(PortSpec::new(id.as_str(), PortDirection::Out, PortSide::Auto));
    }
    ports
}

fn get_cases(node: &Node) -> Vec<(String, String)> {
    node.data.get("cases")
        .and_then(|v| v.as_array())
        .map(|arr| {
            arr.iter().filter_map(|item| {
                let id = item.get("id")?.as_str()?.to_string();
                let label = item.get("label")?.as_str()?.to_string();
                Some((id, label))
            }).collect()
        })
        .unwrap_or_else(|| vec![
            ("case_0".to_string(), String::new()),
            ("case_1".to_string(), String::new()),
        ])
}
```

### 数据变化时端口同步

用户在面板增删 case 后，触发 `NodeAction::SetData("cases", new_array)`，编辑器更新 `node.data` 后调用 `update_node_size_if_changed`。如果 `content_size` 也覆写了（高度随 case 数变化），尺寸变化触发 `relayout`，新的端口列表通过 `ports_for_node` 被布局引擎读取——**端口拓扑自动同步**。

## 端口 ID 的命名约定

端口 ID 是字符串，但建议遵循约定：

| 命名 | 含义 | 例子 |
|------|------|------|
| `in` / `out` | 单一主端口 | Action/Variable |
| `if_<i>` | 索引分支 | Condition |
| `case_<i>` | 索引匹配 | Switch |
| `<语义名>` | 语义端口 | Loop 的 `done`/`loop_body`/`loop_in` |

索引命名（`if_0`、`case_1`）便于 `port_position` 用 `pid[3..].parse::<usize>()` 解析索引，按行/列定位。语义命名（`done`）便于在 `port_position` 用 `match port_id.as_str()` 直接分支。

## port_position 必须配合覆写

覆写 `ports_for_node` 后，**必须同时覆写 `port_position`**——否则新端口的位置由框架默认算法计算（节点边缘中点），多个动态端口会重叠在边缘中心。

```rust
fn port_position(&self, node: &Node, port_id: &PortId, layout: LayoutDirection) -> Option<PointF> {
    let cases = get_cases(node);
    let n_cases = cases.len();
    let right = node.position.x + node.size.w;
    let left = node.position.x;
    let mid_x = node.position.x + node.size.w * 0.5;
    let top = node.position.y;

    match port_id.as_str() {
        "in" => match layout {
            LayoutDirection::Horizontal => Some(PointF::new(left, node.position.y + node.size.h * 0.5)),
            LayoutDirection::Vertical => Some(PointF::new(mid_x, top)),
        },
        pid if pid.starts_with("case_") => {
            let idx: usize = pid[5..].parse().ok()?;
            match layout {
                LayoutDirection::Horizontal => {
                    let y = node.position.y + TITLE_H + ITEM_H * (idx as f32 + 0.5);
                    Some(PointF::new(right, y))
                }
                LayoutDirection::Vertical => {
                    let t = (idx as f32 + 0.5) / n_cases as f32;
                    Some(PointF::new(left + node.size.w * t, node.position.y + node.size.h))
                }
            }
        }
        _ => None,
    }
}
```

纵向布局下 `(idx + 0.5) / n_cases` 让端口沿底部均匀分布——与 Condition 的 if_i 算法一致。

## content_size 也必须配合

动态端口通常意味着动态高度：

```rust
fn content_size(&self, node: &Node) -> SizeF {
    let n_cases = get_cases(node).len();
    let h = TITLE_H + ITEM_H * n_cases as f32;
    SizeF::new(node.size.w, h)
}
```

**三件套联动**：`ports_for_node`（端口数）→ `content_size`（高度）→ `port_position`（每端口 Y 坐标）。三者必须基于同一份 `node.data` 计算，保持一致。

## 端口方向的一致性

`PortSpec::new(id, direction, side)` 中：

- `direction`：In/Out，决定端口是输入还是输出
- `side`：Top/Right/Bottom/Left/Auto，决定端口在节点哪一边

动态端口通常 `side = Auto`——让 `port_position` 完全控制位置，框架的 Auto 推导只用于无 `port_position` 覆写时的回退。Condition 的所有端口都是 Auto，实际位置由 `port_position` 精确指定。

## 容错与回退

`get_cases` 必须有回退默认值：

```rust
.unwrap_or_else(|| vec![
    ("case_0".to_string(), String::new()),
    ("case_1".to_string(), String::new()),
])
```

`node.data` 在节点刚创建、JSON 损坏、字段缺失等情况下可能不完整。回退默认值保证节点始终可渲染——这是内置节点的通用范式。

## 验证端口与边的一致性

动态端口增删后，已存在的边可能引用失效端口：

```rust
// 用户删除 case_1，但仍有边 source_port = "case_1"
// 渲染时 port_position 返回 None，边端点用节点边缘中心
```

框架对失效端口有容错（`port_position` 返回 `None` 时用默认算法），但建议在面板编辑 cases 时同步清理失效边——这属于编辑器职责，不在 IFlowNode 范围内。

## 小结

动态端口通过覆写 `ports_for_node` 实现——根据 `node.data` 实时生成端口列表。典型场景是多分支节点（Condition/Switch）。覆写时必须三件套联动：`ports_for_node`（端口数）+ `content_size`（高度）+ `port_position`（每端口坐标），三者基于同一份数据计算保持一致。端口 ID 命名遵循约定（`if_<i>`/`case_<i>`/语义名），`get_cases` 类函数必须有回退默认值保证节点始终可渲染。

下一节：[port_position 与 content_size](port-position-size.md)
