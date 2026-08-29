**English** | [简体中文](README.zh-CN.md)

# rust-agent-flow

[![License](https://img.shields.io/badge/license-MIT-blue.svg)](/LICENSE)

RustAgentFlow is a **framework-agnostic workflow design component library** built on top of **GPUI** (the UI framework behind the Zed editor) and **gpui-component**. It gives you a ReactFlow-style visual flow designer with a clean separation between a pure graph core and a GPUI rendering layer.

The core graph model, geometry math and layout are renderer-agnostic, so the same `FlowGraph` can drive any future rendering backend. The GPUI layer turns that model into an interactive canvas editor with pan / zoom, drag-and-drop port wiring, obstacle-aware edge routing, a schema-driven property panel, i18n and theming.

```
rust-agent-flow        → graph model, viewport math, geometry, layout (no GPUI dependency)
rust-agent-flow-gpui   → canvas renderer, FlowEditorView, interaction state machine
```

## Why

Most workflow UIs are built as one big tightly-coupled widget. rust-agent-flow instead splits the problem:

- **`core`** stays framework-agnostic — pure data structures (`FlowGraph`), geometry (`PointF` / `RectF`), edge path algorithms and a pluggable layout engine. It is trivial to unit-test and can be reused by CLI tools or other backends.
- **`gpui`** owns everything visual — the dot-grid canvas, interaction FSM, hit-testing, node/edge rendering and the property panel.

Edge rendering borrows algorithms from gpui-component charts (`Catmull-Rom → cubic Bézier`) and ReactFlow-style port-aware Bézier curves, plus obstacle-aware A* routing on a grid.

## Architecture

```
┌──────────────────────────────────────────────────────────┐
│ rust-agent-flow-gpui   (renderer, no logic in widgets)   │
│  FlowEditorView ─ FlowGraph + Viewport + Interaction FSM │
│  NodeRegistry / IFlowNode        (strategy pattern)      │
│  Schema-driven property panel    (FieldSpec rendering)   │
│  Toolbar extensions, Theme, i18n (en / zh)               │
├──────────────────────────────────────────────────────────┤
│ rust-agent-flow  (core — framework-agnostic, no GPUI)    │
│  FlowGraph      stable slotmap keys (NodeId / EdgeId)    │
│  Viewport       pan + zoom transform math                │
│  geometry       edge paths, hit-test, port_calc, routing │
│  layout         LayoutEngine trait + DagreLayout         │
│  schema         NodeSchema / PortSpec / FieldSpec        │
└──────────────────────────────────────────────────────────┘
```

### rust-agent-flow (core)

- **`FlowGraph`** — nodes + directed edges with stable `slotmap` keys, a monotonic version counter for cache invalidation, and full `FlowDocument` (de)serialization interop.
- **`schema`** — declarative node schema (`NodeSchema`, `PortSpec`, `FieldSpec`, `FlowDocument`). `NodeSchema.fields` drives the automatically generated property-panel editors (Text / TextArea / CodeEditor / CodeBlock / Number / Switch / Dropdown / List).
- **`geometry`** — f32-based `PointF` / `SizeF` / `RectF`, edge path algorithms (`bezier`, `straight`, `step`, `smoothstep`, `loop_back`, `round_corners`), hit-testing, and `port_calc` for port side distribution across multiple outputs.
- **`routing`** — obstacle-aware A* edge routing over a grid, with grid-cell size, obstacle margin and turn penalties.
- **`layout`** — `LayoutEngine` trait with `DagreLayout` (wraps the `dagre` crate; the same Sugiyama algorithm ReactFlow uses), supporting horizontal / vertical directions and loop-body grouping.
- **`viewport`** — pan offset + zoom scale with screen ↔ logical transforms and anchor-preserving `zoom_around`.

### rust-agent-flow-gpui (renderer)

- **`FlowEditorView`** — a single GPUI `Render` view owning the graph, viewport and interaction state.
- **Dot-grid canvas** with pan (middle mouse) and mouse-anchored zoom (scroll wheel).
- **Drag nodes** on the grid; **wire output ports to input ports** by dragging.
- **Interactive edge drawing** — multiple edge types, arrow markers, edge midpoint `+` button that opens a node picker and inserts a node into the middle of an edge.
- **Hit-tested interaction** — the canvas handles mouse events uniformly and uses geometric hit-testing to find the target node/port (instead of per-node event closures).
- **Built-in nodes** — `start`, `end`, `action`, `condition`, `loop`, `variable`, `adapter`, `agent` covering Turing-complete control flow (sequence, branch, loop-body with back edges).
- **`NodeRegistry` + `IFlowNode`** — register custom node types at runtime via the strategy pattern.
- **Schema-driven property panel** — the right-hand panel is generated from `NodeSchema.fields`.
- **Toolbar extensions** — call sites inject their own tools through `ToolbarProvider`.
- **Theme + i18n** — English / 简体中文 with data-type and node-kind label mapping.

## Quick start

The first build may take several minutes because GPUI compiles from its git source.

```bash
cargo run -p rust-agent-flow-demo
```

If the default agent flow does not suit you, swap in any JSON flow document (`demo/data/*.json`) via `DemoDataSource`:

```rust
use rust_agent_flow_gpui::{CombinedAssets, FlowEditorView, SharedToolbarProvider};

// load a flow, auto-layout with dagre (the same Sugiyama algorithm ReactFlow uses),
// then register call-site toolbar extensions.
let mut editor = FlowEditorView::new(graph, cx);
editor.auto_layout(cx);
editor.add_toolbar_provider(data_source_toolbar, cx);
editor.add_toolbar_provider(app_controls_toolbar, cx);
```

Build the docs for the full design narrative:

```bash
# in-repo book under docs/rust-agent-flow
```

## Documentation

The repository ships a full documentation book under [`docs/rust-agent-flow`](docs/rust-agent-flow) covering introduction, quick start, design philosophy, architecture, the graph model, the schema system, geometry & layout, custom nodes, editor view, interaction, edge rendering, panels, extensions and best practices.

## Crates

| Crate | Description |
|-------|-------------|
| [`rust-agent-flow`](crates/core) | Core: `FlowGraph`, `Viewport`, geometry (`bezier`, `catmull`, `smoothstep`, `loop_back`, hit-test), A* edge routing, `DagreLayout`, `NodeSchema` / `PortSpec` / `FieldSpec` / `FlowDocument`. No GPUI dependency. |
| [`rust-agent-flow-gpui`](crates/gpui) | GPUI renderer: `FlowEditorView`, node registry, interaction FSM, property panel, toolbar extensions, theme & i18n. |
| [`rust-agent-flow-demo`](demo) | Runnable demo application for the GPUI designer. |

> Not yet published to [crates.io](https://crates.io). Dependencies (`gpui`, `gpui-component`) are pulled from git in the workspace `[workspace.dependencies]`.

## License

Licensed under the [MIT License](LICENSE).