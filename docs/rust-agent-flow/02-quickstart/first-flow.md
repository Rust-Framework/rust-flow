# 第一个流程图

## 两种构建方式

构建流程图有两种方式：

| 方式 | 适用场景 | 特点 |
|------|----------|------|
| `FlowGraph` API | 代码动态构建 | 直接操作图，需手动管理 NodeId |
| `FlowDocument` 数据驱动 | 从 JSON/配置加载 | 声明式，节点用索引引用 |

推荐使用 **FlowDocument 数据驱动**——它可序列化、可从文件加载，且与持久化协议一致。

## 方式一：FlowGraph API 构建

适合需要运行时动态增删节点的场景：

```rust
use rust_agent_flow::{Edge, FlowGraph};

let mut graph = FlowGraph::new();

// 添加节点（kind + data）
let start = graph.add_node("start", serde_json::json!({"label": "开始"}));
let action = graph.add_node("action", serde_json::json!({"label": "执行"}));
let end = graph.add_node("end", serde_json::json!({"label": "结束"}));

// 连线
graph.add_edge(Edge::new(start, action));
graph.add_edge(Edge::new(action, end));
```

- `add_node(kind, data)` 返回 `NodeId`（slotmap 稳定键）
- `Edge::new(source, target)` 创建边
- 节点尺寸默认 180×64，由 `sync_node_sizes` 在布局时按 schema 修正

## 方式二：FlowDocument 数据驱动（推荐）

声明式定义，可从 JSON 加载：

```rust
use rust_agent_flow::{EdgeDef, FlowDocument, NodeDef};

let mut doc = FlowDocument::new("我的流程");

let start = doc.add_node(NodeDef::new("start", serde_json::json!({"label": "开始"})));
let action = doc.add_node(NodeDef::new("action", serde_json::json!({"label": "执行"})));
let end = doc.add_node(NodeDef::new("end", serde_json::json!({"label": "结束"})));

doc.add_edge(EdgeDef::new(start, action));
doc.add_edge(EdgeDef::new(action, end));

// 转为 FlowGraph
let graph = FlowGraph::from_document(&doc);
```

`doc.add_node` 返回**节点索引**（usize），边用索引引用——这保证了序列化稳定性（不依赖 slotmap 内部 key）。

## 从 JSON 加载

FlowDocument 可直接从 JSON 反序列化：

```rust
let json = r#"{
  "version": "1.0",
  "metadata": { "name": "简单流程", "description": null },
  "nodes": [
    { "kind": "start", "data": {"label":"开始"}, "size": null, "position": null },
    { "kind": "end",   "data": {"label":"结束"}, "size": null, "position": null }
  ],
  "edges": [
    { "source": 0, "target": 1, "source_port": null, "target_port": null, "edge_type": null }
  ]
}"#;

let doc: FlowDocument = serde_json::from_str(json).unwrap();
let graph = FlowGraph::from_document(&doc);
```

## 带端口的连线

结构化节点（Condition/Loop）需要指定端口：

```rust
use rust_agent_flow::{EdgeDef, FlowDocument, NodeDef};

let mut doc = FlowDocument::new("条件流程");
let cond = doc.add_node(NodeDef::new("condition", serde_json::json!({
    "label": "检查",
    "conditions": [{"id":"if_0","label":"x > 0"}]
})));
let yes = doc.add_node(NodeDef::new("action", serde_json::json!({"label":"是"})));
let no  = doc.add_node(NodeDef::new("action", serde_json::json!({"label":"否"})));

// condition 的 if_0 出口 → yes
doc.add_edge(EdgeDef::new(cond, yes).with_source_port("if_0"));
// condition 的 else 出口 → no
doc.add_edge(EdgeDef::new(cond, no).with_source_port("else"));
```

端口 ID 与 `NodeSchema.ports` 中的 `PortSpec.id` 对应。

## 完整示例：显示流程图

```rust
use rust_agent_flow::{EdgeDef, FlowDocument, FlowGraph, NodeDef};
use rust_agent_flow_gpui::{CombinedAssets, FlowEditorView};

fn build_graph() -> FlowGraph {
    let mut doc = FlowDocument::new("第一个流程");
    let s = doc.add_node(NodeDef::new("start", serde_json::json!({"label":"开始"})));
    let a = doc.add_node(NodeDef::new("action", serde_json::json!({"label":"处理"})));
    let e = doc.add_node(NodeDef::new("end",   serde_json::json!({"label":"结束"})));
    doc.add_edge(EdgeDef::new(s, a));
    doc.add_edge(EdgeDef::new(a, e));
    FlowGraph::from_document(&doc)
}

fn main() {
    gpui_platform::application()
        .with_assets(CombinedAssets)
        .run(move |cx: &mut gpui::App| {
            rust_agent_flow_gpui::init(cx);
            cx.spawn(async move |cx| {
                cx.open_window(gpui::WindowOptions::default(), |window, cx| {
                    let graph = build_graph();
                    let view = cx.new(|cx| {
                        let mut editor = FlowEditorView::new(graph, cx);
                        editor.auto_layout(cx);
                        editor
                    });
                    cx.new(|cx| gpui_component::Root::new(view, window, cx))
                }).expect("Failed to open window");
            }).detach();
        });
}
```

运行后你会看到三个节点自动排成一行：`开始 → 处理 → 结束`，带 SmoothStep 圆角连线。

## 节点 data 的字段

每种节点的 `data` 字段由其 `NodeSchema.fields` 定义。常用字段：

| kind | 关键字段 |
|------|----------|
| `start` | `label`、`params`（数组）、`variables`（数组） |
| `end` | `label`、`returns`（数组） |
| `action` | `label`、`code` |
| `condition` | `label`、`conditions`（数组：id + label） |
| `loop` | `label`、`loop_mode`、`loop_expr` |

`label` 是所有节点通用字段，显示在标题栏。

## 小结

推荐用 `FlowDocument` 数据驱动构建流程图——声明式、可序列化、与持久化协议一致。端口通过 `with_source_port`/`with_target_port` 指定，与 Schema 端口 ID 对应。

下一节：[运行、调试与验证](run-and-debug.md)
