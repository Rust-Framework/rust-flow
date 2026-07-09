# ReactFlow 的启发

## 为什么对标 ReactFlow

ReactFlow（`@xyflow/xyflow`）是 Web 端流程编辑器的事实标准。rust-agent-flow 在边路径几何上直接移植其算法，确保桌面端获得同样的视觉体验与连线质量。

## 移植的算法

### 1. 边路径生成

| 算法 | ReactFlow 源 | rust-agent-flow 对应 |
|------|--------------|----------------------|
| 直线 | `straight` | `straight_path` |
| 三次贝塞尔 | `bezier` | `bezier_path` |
| 直角折线 | `step` | `step_path` |
| 圆角折线 | `smoothstep` | `smoothstep_path` |

移植的核心函数：

- `getPoints()`（正交路由）→ `rf_get_points()`
- `getBend()`（二次贝塞尔圆角）→ `rf_get_bend()`
- `calculateControlOffset()`（贝塞尔控制点偏移）→ `control_offset()`

### 2. 贝塞尔控制点

ReactFlow 的 `calculateControlOffset` 处理了「反向连接」（目标在源后方）时控制点塌缩的问题：

```rust
fn control_offset(distance: f32, curvature: f32) -> f32 {
    if distance >= 0.0 {
        0.5 * distance                    // 正常：半距离
    } else {
        curvature * 25.0 * (-distance).sqrt()  // 反向：防塌缩
    }
}
```

这保证从右到左的回环连线不会退化为直线。

### 3. 圆角折线

ReactFlow 用 SVG `Q`（二次贝塞尔）命令做圆角。rust-agent-flow 将其**采样为多段折线点**，供 GPUI 的 `PathBuilder` 消费（GPUI 不支持 SVG Q 命令）：

```rust
fn rf_get_bend(a: PointF, b: PointF, c: PointF, size: f32) -> Vec<PointF> {
    // 控制点始终在角点 b，从 bend_size 处开始到 bend_size 处结束
    // 采样 SAMPLES=8 个点近似二次贝塞尔曲线
}
```

`round_corners` 是提炼出的通用圆角函数，对任意折线的每个内部角点应用 `rf_get_bend`。

## 概念对齐

| ReactFlow 概念 | rust-agent-flow 对应 |
|----------------|----------------------|
| Node | `Node`（slotmap 键） |
| Edge | `Edge`（含 source_port/target_port） |
| Handle（连接点） | `Port`（PortId + PortDirection + PortSide） |
| handle Position | `PortSide`（Top/Right/Bottom/Left/Auto） |
| Connection Mode | `DrawingEdge` 交互状态 |
| dagre 布局示例 | `DagreLayout`（包装 dagre crate） |

## 有意不对齐之处

### Auto 端口方向

ReactFlow 的 handle 需显式声明 Position。rust-agent-flow 的 `PortSide::Auto` 让框架**根据节点相对位置自动推导**方向：

```rust
fn compute_side_from_position(self_center, other_center) -> PortSide {
    // 比较 dx/dy 绝对值，选择面向对方的一侧
}
```

这降低了声明负担——大多数节点只需声明 `PortSide::Auto`，框架自动算出正确的连线方向。

### 同侧端口分布

当同一节点的同一侧既有 In 又有 Out 端口时，框架自动将 In 放下半区、Out 放上半区，避免重叠：

```rust
let (start, end) = if has_opposite {
    match dir {
        PortDirection::In => (0.5, 1.0),   // In 占下半
        PortDirection::Out => (0.0, 0.5),  // Out 占上半
    }
} else {
    (0.0, 1.0)
};
```

> 说明：此分区算法由 core 层纯几何工具 `distribute_on_side` 提供。历史上的批量入口 `resolve_endpoints` **已废弃**（`#[deprecated]`）——它不识别 `PortSpec.fixed` 强约束，也无法调用节点实现的 `port_position` 回调。渲染层现在改走 gpui 层的 `resolve_port` + `compute_edge_endpoints` 路径，二者在内部复用上述分区逻辑。

ReactFlow 需手动处理这种布局，rust-agent-flow 内置解决。

### 循环回环边

ReactFlow 无内置循环节点。rust-agent-flow 的 `loop_back_path` 专门处理 Loop 节点的回环连线——U 形路由从循环体底部向下绕过，左进 `loop_in` 端口：

```rust
pub fn loop_back_path(src, dst, _horizontal, node_bounds) -> Vec<PointF> {
    // DOWN → LEFT → UP → RIGHT 五点 U 形
    vec![src, (src.x, bottom_y), (approach_x, bottom_y), (approach_x, dst.y), dst]
}
```

## 算法可独立复用

由于 core 层框架无关，这些算法可脱离 GPUI 独立使用：

```rust
use rust_agent_flow::{smoothstep_path, PortSide, PointF};

// 在任意非 GPUI 项目中计算边路径
let pts = smoothstep_path(
    PointF::new(0.0, 0.0),
    PointF::new(200.0, 100.0),
    PortSide::Right,
    PortSide::Left,
    8.0,
);
```

## 小结

rust-agent-flow 移植 ReactFlow 的边路径算法以保证连线质量，同时针对桌面场景做了增强：Auto 方向推导、同侧端口分布、循环回环 U 形路由。算法位于框架无关的 core 层，可独立复用。

下一节：[GPUI 惯用法与所有权](gpui-idioms.md)
