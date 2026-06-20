# rust-agent-flow

Framework-agnostic workflow design component library for Rust, built with **GPUI** and **gpui-component**.

## Architecture

```
rust-agent-flow   — graph model, viewport math, edge geometry (no GPUI)
rust-agent-flow-gpui   — canvas renderer, FlowEditorView, interaction FSM
```

Edge rendering borrows algorithms from gpui-component charts (`Catmull-Rom → cubic Bézier`) and ReactFlow-style port-aware Bézier curves.

## Quick start

First build may take several minutes (GPUI compiles from git).

```bash
cargo run -p rust-agent-flow-gpui --example minimal_demo --features demo
```

## MVP features

- Pan (middle mouse) and zoom (scroll wheel)
- Drag nodes on a dot-grid canvas
- Connect output ports to input ports
- Smooth Bézier edges with arrow markers

## Crates

| Crate | Description |
|-------|-------------|
| `rust-agent-flow` | `FlowGraph`, `Viewport`, geometry (`bezier`, `catmull`, `smoothstep`, hit-test) |
| `rust-agent-flow-gpui` | `FlowEditorView` — GPUI `Render` view |

## License

MIT
