# 智能避障连线问题修复迭代计划

## 概述

基于对截图连线问题的逐条分析和代码深度审查，发现 `route_with_channels` 通道分配路由算法存在 **4 个层级的缺陷**，从严重到轻微依次为：倒置间隙导致穿框、缺少通道安全校验、目标端口方向不匹配、边界常量语义不严谨。

## 现状分析

### 已确认的正确逻辑（无需修改）

| 逻辑 | 位置 | 结论 |
|------|------|------|
| Rank 相邻判断 `dst_rank <= src_rank + 1` | `rendering.rs:90` | ✅ 正确，相邻层正确回退到 smoothstep |
| smoothstep 对 Bottom→Top（src 在 dst 右上方）| `edge_path.rs:57-213` | ✅ 产生干净 L 型路径，无 U 型绕行 |
| dagre rank 计算（NetworkSimplex + minlen 引导）| `dagre.rs:61-132` | ✅ 正确，rank 从 0 开始递增 |
| Loop 回环边 U 型路由 | `edge_path.rs:436-458` | ✅ 设计如此，行为正确 |

### 已确认的缺陷

#### 缺陷 1（严重）：重叠障碍物 → 倒置间隙 → 通道穿框

**位置**：`edge_path.rs:544-585`，`find_channels` 函数

**根因**：`find_channels` 按 `.0`（交叉轴起点）排序障碍物后直接生成间隙，**不合并重叠区间**。当同层两个障碍物在交叉轴上重叠或间距 < `2 * CHANNEL_MARGIN`（60px）时，中间间隙倒置（`lo > hi`），中点 `(lo + hi) / 2` 落入障碍物内部。

**触发条件**：
- 同层节点在交叉轴投影重叠（横向布局中同 rank 节点 Y 范围重叠——dagre 将同 rank 节点放在同一 Y，必然触发）
- 同层节点间距 < 60px

**影响**：`min_by` 选中距离 `desired` 最近的候选值，倒置间隙的中点距离通常最小，被优先选中 → 通道坐标落在节点矩形内 → **路径穿过节点**。

**推演示例**（横向布局，同 rank 两节点 Y 范围 [80,160] 和 [100,200] 重叠）：
```
间隙 1 = (160+30, 100-30) = (190, 70)  ← 倒置！
中点 = 130  ← 落在两节点重叠区 [100,160] 内
desired=130 时距离=0，min_by 必选 → 通道 Y=130 穿过节点
```

#### 缺陷 2（重要）：缺少通道安全后置校验

**位置**：`edge_path.rs:565-585`

**根因**：`min_by` 仅比较 `|candidate - desired|`，不校验候选值是否真的避开所有障碍物（含 margin 缓冲带）。一旦候选集合混入非法值（如倒置间隙中点），`min_by` 毫无防备地选中。

#### 缺陷 3（重要）：目标端口方向不匹配

**位置**：`edge_path.rs:655-679`（`build_orthogonal_horizontal`）、`687-711`（`build_orthogonal_vertical`）

**根因**：`route_with_channels` 接收 `src_side` 和 `dst_side` 参数，但 `build_orthogonal_path` 完全忽略它们。路径的最终段方向由布局方向硬编码决定：
- 横向布局：最终段始终垂直（`(dst.x, prev_y) → dst`）→ 对 Left/Right 端口，箭头方向错误
- 纵向布局：最终段始终水平（`(prev_x, dst.y) → dst`）→ 对 Top/Bottom 端口，箭头方向错误

**影响**：箭头方向与端口不匹配，视觉上连线"从错误方向进入"节点。

#### 缺陷 4（轻微）：f32::MIN/MAX 语义不严谨

**位置**：`edge_path.rs:558, 562`

**根因**：`f32::MIN`（≈ -3.4e38）和 `f32::MAX`（≈ +3.4e38）是有限值，不是无穷大。语义上应使用 `f32::NEG_INFINITY` / `f32::INFINITY`。现实坐标不会触发问题，但对退化输入（NaN/inf 污染）不够鲁棒。

## 修改方案

### 修改 1：合并重叠/过近障碍物（修复缺陷 1）

**文件**：`crates/core/src/geometry/edge_path.rs`
**函数**：`find_channels`（第 513-625 行）

在排序后、生成间隙前，增加一步**区间合并**：将交叉轴投影重叠或间距 < `2 * CHANNEL_MARGIN` 的障碍物区间合并为一个，从源头消除倒置间隙。

```rust
// 在 sorted 排序后（第 554 行之后），增加合并逻辑：
let merged = merge_overlapping_intervals(&sorted, CHANNEL_MARGIN);
```

新增辅助函数：
```rust
/// 合并重叠或间距 < 2*margin 的区间。
fn merge_overlapping_intervals(
    intervals: &[(f32, f32)],
    margin: f32,
) -> Vec<(f32, f32)> {
    if intervals.is_empty() {
        return Vec::new();
    }
    let mut merged = vec![intervals[0]];
    for &(lo, hi) in &intervals[1..] {
        let last = merged.last_mut().unwrap();
        // 间距 < 2*margin 即合并（两区间间需要 2*margin 才能形成有效通道）
        if lo <= last.1 + 2.0 * margin {
            last.1 = last.1.max(hi);
        } else {
            merged.push((lo, hi));
        }
    }
    merged
}
```

然后后续间隙生成使用 `merged` 而非 `sorted`。

### 修改 2：增加通道安全后置校验（修复缺陷 2）

**文件**：`crates/core/src/geometry/edge_path.rs`
**函数**：`find_channels`（第 565-585 行）

在 `min_by` 选定通道后，增加校验：若通道坐标落在任一障碍物的膨胀区（`[min - margin, max + margin]`）内，回退到最近的安全无界间隙边界。

```rust
// 在 min_by 结果（第 585 行）之后增加：
let channel = channel_candidate; // min_by 的结果
let safe_channel = if is_channel_safe(channel, &merged, CHANNEL_MARGIN) {
    channel
} else {
    // 回退：选最近的无界间隙边界
    let above = merged.first().map(|(lo, _)| lo - CHANNEL_MARGIN).unwrap_or(desired);
    let below = merged.last().map(|(_, hi)| hi + CHANNEL_MARGIN).unwrap_or(desired);
    if (above - desired).abs() <= (below - desired).abs() {
        above
    } else {
        below
    }
};
```

新增辅助函数：
```rust
/// 检查通道坐标是否安全（不在任何障碍物的 margin 膨胀区内）。
fn is_channel_safe(channel: f32, intervals: &[(f32, f32)], margin: f32) -> bool {
    intervals.iter().all(|(lo, hi)| {
        channel < *lo - margin || channel > *hi + margin
    })
}
```

### 修改 3：尊重目标端口方向（修复缺陷 3）

**文件**：`crates/core/src/geometry/edge_path.rs`
**函数**：`build_orthogonal_horizontal`（第 655-679 行）、`build_orthogonal_vertical`（第 687-711 行）

修改路径构建的**最终段**，根据 `dst_side` 调整进入方向：

**横向布局**（`build_orthogonal_horizontal`）：
- 当前：最终段始终垂直 `(dst.x, prev_y) → dst`
- 修改：若 `dst_side` 是 Left/Right（水平端口），最终段应水平进入 → 先垂直到 `dst.y`，再水平到 `dst.x`
- 若 `dst_side` 是 Top/Bottom（垂直端口），保持当前行为（垂直进入）

```rust
fn build_orthogonal_horizontal(
    src: PointF,
    dst: PointF,
    channels: &[(f32, f32)],
    dst_side: PortSide,  // 新增参数
) -> Vec<PointF> {
    // ... 前面不变 ...

    // 根据目标端口方向调整最终段
    match dst_side {
        PortSide::Left | PortSide::Right => {
            // 水平端口：先垂直到 dst.y，再水平到 dst
            points.push(PointF::new(prev_x_after_channels, dst.y));
            // 但 prev_x_after_channels 可能 != dst.x，需要调整
            // 实际上：最后一个通道的 tx 已经是过渡 X
            // 最终段：从 (last_tx, last_ch) → (last_tx, dst.y) → dst
            // 但这需要重构路径尾部
        }
        PortSide::Top | PortSide::Bottom => {
            // 垂直端口：保持当前行为
            points.push(PointF::new(dst.x, prev_y));
            points.push(dst);
        }
        PortSide::Auto => {
            points.push(PointF::new(dst.x, prev_y));
            points.push(dst);
        }
    }
}
```

**具体实现策略**：在路径尾部增加一个"入口段"——根据 `dst_side` 决定最后两个点的排列：
- 水平端口（Left/Right）：倒数第二个点 Y = dst.y，最后水平进入
- 垂直端口（Top/Bottom）：倒数第二个点 X = dst.x，最后垂直进入（当前行为）

需要同步修改 `build_orthogonal_path` 和 `route_with_channels` 的调用链，将 `dst_side` 传递下去。`src_side` 的处理类似（路径起始段方向），但优先级较低，可后续迭代。

### 修改 4：修正边界常量（修复缺陷 4）

**文件**：`crates/core/src/geometry/edge_path.rs`
**行号**：558, 562

```rust
// 第 558 行：
- gaps.push((f32::MIN, sorted[0].0 - CHANNEL_MARGIN));
+ gaps.push((f32::NEG_INFINITY, merged[0].0 - CHANNEL_MARGIN));

// 第 562 行：
- gaps.push((sorted.last().unwrap().1 + CHANNEL_MARGIN, f32::MAX));
+ gaps.push((merged.last().unwrap().1 + CHANNEL_MARGIN, f32::INFINITY));
```

同步修改 `min_by` 中的边界判断（第 570-573 行）：
```rust
- if *lo == f32::MIN {
+ if lo.is_infinite() && *lo < 0.0 {
      *hi
- } else if *hi == f32::MAX {
+ } else if hi.is_infinite() && *hi > 0.0 {
      *lo
  }
```

### 修改 5：补充单元测试

**文件**：`crates/core/src/geometry/edge_path.rs`，`#[cfg(test)] mod tests` 部分

新增测试用例：

1. **`route_overlapping_obstacles_does_not_pass_through`**：两障碍物交叉轴重叠，验证路径不穿过障碍物
2. **`route_close_obstacles_merged_into_one`**：两障碍物间距 < 60px，验证合并后通道不落入间隙
3. **`route_channel_safety_fallback`**：构造 desired 落在障碍物内的场景，验证回退到安全边界
4. **`route_respects_dst_side_horizontal`**：验证横向布局下 dst_side=Left 时最终段水平进入
5. **`route_respects_dst_side_vertical`**：验证纵向布局下 dst_side=Top 时最终段垂直进入

## 假设与决策

| 决策点 | 选择 | 理由 |
|--------|------|------|
| 障碍物合并策略 | 间距 < 2*margin 即合并 | 两个节点间需要 2*margin（60px）才能形成有效通道，否则通道会贴边 |
| 通道回退策略 | 选最近的无界间隙边界 | 无界间隙（所有节点上方/下方）是 guaranteed 安全的 |
| 端口方向修复范围 | 仅 dst_side，暂不修 src_side | dst_side 影响箭头方向（视觉显著），src_side 影响起始段（影响较小），分步迭代 |
| 端口方向修复方式 | 在路径尾部增加入口段 | 不改变通道分配逻辑，仅在路径组装时调整最后两个点的排列 |
| f32::MIN/MAX 修改 | 改为 INFINITY 常量 | 语义准确，对退化输入更鲁棒 |

## 验证步骤

1. **编译验证**：`cargo build` 确保无编译错误
2. **单元测试**：`cargo test -p rust-agent-flow-core` 全部通过
3. **新增测试**：5 个新测试用例全部通过
4. **回归测试**：现有 17 个 edge_path 测试不破坏
5. **视觉验证**（需手动）：
   - 纵向布局：Check(else)→Notify 不再横向甩出过宽
   - 纵向布局：Notify→Adapter 路径合理（若为相邻 rank 则 smoothstep L 型，若跨层则通道避障不穿框）
   - 纵向布局：Notify→Summarize 通道不贴边极端
   - 横向布局：所有边箭头方向与端口匹配
   - Loop 回环边不受影响（仍为 U 型虚线）

## 实施顺序

1. 修改 4（f32 常量）— 最简单，无依赖
2. 修改 1（合并障碍物）— 核心修复，消除倒置间隙
3. 修改 2（后置校验）— 防御性修复，依赖修改 1 的 merged 区间
4. 修改 3（端口方向）— 独立修改，需调整函数签名
5. 修改 5（单元测试）— 最后补充，验证所有修改
