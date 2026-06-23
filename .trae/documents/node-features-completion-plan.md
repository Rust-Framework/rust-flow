# 节点功能完善实施计划

## 概述

本计划聚焦于完成节点功能的剩余工作。经探索发现，节点删除、Condition/Loop 展开/收起、属性面板编辑、dagre 布局等核心功能**已完成**。剩余工作集中在：

1. **rhai 语法高亮扩展接口**（用户要求：不直接引入 rhai 语言包，提供扩展接口由独立 crate 提供）
2. **CodeEditor 集成**（Condition 条件项 + Loop 表达式改用 CodeEditor）
3. **节点尺寸生产级优化**（Action/Start/End 尺寸偏小）
4. **Loop 循环模式标签**（节点视觉显示模式文案）
5. **清理小问题**（死代码、未使用导入）

## 当前状态分析

### 已完成功能（无需改动）

| 功能 | 文件 | 状态 |
|------|------|------|
| 节点删除（线性桥接 + 级联删边） | [flow_editor.rs](file:///d:/GitCode/RF/rust-agent-flow/crates/gpui/src/editor/flow_editor.rs#L242-L280) | ✓ |
| start/end 不可删除 | [hit_test.rs](file:///d:/GitCode/RF/rust-agent-flow/crates/gpui/src/editor/hit_test.rs#L73) | ✓ |
| 删除按钮 hover 显示 | [common.rs](file:///d:/GitCode/RF/rust-agent-flow/crates/gpui/src/builtin/common.rs#L129) | ✓ |
| Condition 展开/收起 + 端口合并 | [condition.rs](file:///d:/GitCode/RF/rust-agent-flow/crates/gpui/src/builtin/condition.rs#L195-L293) | ✓ |
| Condition 条件列表编辑（普通 Input） | [panel/mod.rs](file:///d:/GitCode/RF/rust-agent-flow/crates/gpui/src/panel/mod.rs#L361-L479) | ✓ |
| Loop 展开/收起 | [loop_node.rs](file:///d:/GitCode/RF/rust-agent-flow/crates/gpui/src/builtin/loop_node.rs#L139-L236) | ✓ |
| Loop 循环模式选择器 | [panel/mod.rs](file:///d:/GitCode/RF/rust-agent-flow/crates/gpui/src/panel/mod.rs#L528-L576) | ✓ |
| PanelView 有状态实体 + Input 集成 | [panel/mod.rs](file:///d:/GitCode/RF/rust-agent-flow/crates/gpui/src/panel/mod.rs#L29-L52) | ✓ |
| IFlowNode trait + 动态端口 | [flow_node.rs](file:///d:/GitCode/RF/rust-agent-flow/crates/gpui/src/node/flow_node.rs#L78-L81) | ✓ |
| handle_node_action | [flow_editor.rs](file:///d:/GitCode/RF/rust-agent-flow/crates/gpui/src/editor/flow_editor.rs#L206-L235) | ✓ |
| dagre 布局 + 5 个后处理步骤 | [dagre.rs](file:///d:/GitCode/RF/rust-agent-flow/crates/core/src/layout/dagre.rs) | ✓ |

### 待完成工作

1. **rhai 语法高亮**：当前 Condition 条件项和 Loop 表达式使用普通 `Input`（非 CodeEditor），无语法高亮、行号、自动缩进
2. **节点尺寸**：Action 180×35、Start/End 120×35 偏小，不符合生产级水平
3. **Loop 模式语义**：4 种模式仅存储为字符串，节点视觉不体现差异
4. **小问题**：condition.rs:340-342 死代码、panel/mod.rs 未使用导入

## 设计决策

### 决策 1：rhai 语法高亮扩展接口（用户确认）

**问题**：gpui-component 的 `Language` enum 不原生支持 Rhai，`code_editor(language: impl Into<SharedString>)` 接受任意字符串但未知语言无高亮。

**方案**：核心库提供 `SyntaxService` trait 接口，不引入 rhai 依赖。默认实现将 `"rhai"` 映射到 `"rust"`（近似高亮，rhai 语法与 Rust 高度相似）。未来独立 crate（如 `rust-agent-flow-syntax-rhai`）可实现该 trait 提供精确高亮。

```rust
/// 语法高亮服务接口（扩展点）。
///
/// 核心库提供默认实现 `DefaultSyntaxService`，将 "rhai" 映射到 "rust" 近似高亮。
/// 外部 crate 可实现此接口提供精确的语法高亮支持。
pub trait SyntaxService: Send + Sync {
    /// 返回 code_editor 应使用的语言字符串。
    ///
    /// 返回 None 表示不支持该语言，回退到普通 Input。
    fn language_for(&self, kind: &str) -> Option<&str>;
}

/// 默认语法服务：rhai → rust 近似高亮。
pub struct DefaultSyntaxService;

impl SyntaxService for DefaultSyntaxService {
    fn language_for(&self, kind: &str) -> Option<&str> {
        match kind {
            "rhai" => Some("rust"),  // rhai 语法与 Rust 高度相似，近似高亮
            _ => None,
        }
    }
}
```

### 决策 2：节点尺寸优化（用户确认：调整 Action/Start/End）

| 节点 | 当前尺寸 | 新尺寸 | 原因 |
|------|----------|--------|------|
| Action | 180×35 | 200×56 | 增加 desc 副标题行，更稳重 |
| Start | 120×35 | 140×44 | 药丸形更稳重，文字不挤 |
| End | 120×35 | 140×44 | 同 Start |
| Condition | 220×144 | 保持 | 已合理（TITLE_H + ITEM_H×3） |
| Loop | 220×80 | 保持 | 已合理（TITLE_H + BODY_H） |

`render_node_card` 已支持 desc 副标题渲染（[view.rs:183-190](file:///d:/GitCode/RF/rust-agent-flow/crates/gpui/src/node/view.rs#L183-L190)），只需调整 schema 默认尺寸。

### 决策 3：Loop 模式标签（用户确认：节点显示模式标签）

根据 `loop_mode` 在节点循环条件区显示不同文案：

| loop_mode | 显示文案 |
|-----------|----------|
| `for_each` | `For each item` |
| `for_loop` | `for i in 0..n` |
| `while` | `while cond` |
| `batch_parallel` | `parallel each` |

仅修改 `loop_node.rs` 的 `get_view` 渲染逻辑，不影响端口/布局。

## 实施步骤

### Phase 1: 语法服务接口（核心库扩展点）

**文件**：`crates/gpui/src/node/mod.rs`（或新建 `crates/gpui/src/node/syntax.rs`）

**操作**：
1. 新建 `crates/gpui/src/node/syntax.rs`，定义 `SyntaxService` trait 和 `DefaultSyntaxService`
2. 在 `crates/gpui/src/node/mod.rs` 中 `pub mod syntax;` 并 `pub use syntax::*;`
3. `FlowEditorView` 新增字段 `syntax_service: Arc<dyn SyntaxService>`，默认 `DefaultSyntaxService`
4. 提供 `set_syntax_service(&mut self, service: Arc<dyn SyntaxService>)` 方法供外部注入
5. `PanelView::new` 接收 `syntax_service` 参数，存为字段

**关键代码**：
```rust
// crates/gpui/src/node/syntax.rs
use std::sync::Arc;

/// 语法高亮服务接口（扩展点）。
pub trait SyntaxService: Send + Sync {
    fn language_for(&self, kind: &str) -> Option<&str>;
}

/// 默认语法服务：rhai → rust 近似高亮。
#[derive(Default, Clone)]
pub struct DefaultSyntaxService;

impl SyntaxService for DefaultSyntaxService {
    fn language_for(&self, kind: &str) -> Option<&str> {
        match kind {
            "rhai" => Some("rust"),
            _ => None,
        }
    }
}

pub type SharedSyntaxService = Arc<dyn SyntaxService>;
```

### Phase 2: CodeEditor 集成

**文件**：`crates/gpui/src/panel/mod.rs`

**操作**：
1. `PanelView` 新增字段 `syntax_service: SharedSyntaxService`
2. 修改 `PanelView::new` 签名，接收 `syntax_service` 参数
3. `build` 方法中创建 Condition 条件项 InputState 时，若 `syntax_service.language_for("rhai")` 返回 Some，使用 `.code_editor(language)` 替代普通 Input
4. Loop 表达式 InputState 同理，用 `.code_editor(language)` 替代 `.multi_line(true)`
5. `sync_from_node` 重建 InputState 时保持 code_editor 模式
6. `add_branch` 创建新 InputState 时保持 code_editor 模式

**关键改动点**：
- [panel/mod.rs:95-99](file:///d:/GitCode/RF/rust-agent-flow/crates/gpui/src/panel/mod.rs#L95-L99) Condition 条件项创建
- [panel/mod.rs:113-118](file:///d:/GitCode/RF/rust-agent-flow/crates/gpui/src/panel/mod.rs#L113-L118) Loop 表达式创建
- [panel/mod.rs:185-189](file:///d:/GitCode/RF/rust-agent-flow/crates/gpui/src/panel/mod.rs#L185-L189) sync_from_node 重建
- [panel/mod.rs:304-308](file:///d:/GitCode/RF/rust-agent-flow/crates/gpui/src/panel/mod.rs#L304-L308) add_branch 创建

**辅助方法**：
```rust
impl PanelView {
    /// 创建 rhai CodeEditor InputState（若语法服务支持）。
    fn new_rhai_input(
        &self,
        default_value: &str,
        placeholder: &str,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Entity<InputState> {
        let language = self.syntax_service.language_for("rhai");
        cx.new(|cx| {
            let mut state = InputState::new(window, cx)
                .default_value(default_value)
                .placeholder(placeholder);
            if let Some(lang) = language {
                state = state.code_editor(lang).line_number(false).rows(2);
            } else {
                state = state.multi_line(true).rows(2);
            }
            state
        })
    }
}
```

**渲染层**：`rendering.rs::ensure_panel_view` 创建 PanelView 时传入 `self.syntax_service.clone()`

### Phase 3: 节点尺寸优化

**文件**：
- `crates/gpui/src/builtin/action.rs`
- `crates/gpui/src/builtin/start.rs`
- `crates/gpui/src/builtin/end.rs`

**操作**：
1. `action.rs:25` `SizeF::new(180.0, 35.0)` → `SizeF::new(200.0, 56.0)`
2. `start.rs:25` `SizeF::new(120.0, 35.0)` → `SizeF::new(140.0, 44.0)`
3. `end.rs:25` `SizeF::new(120.0, 35.0)` → `SizeF::new(140.0, 44.0)`

**注意**：`render_node_card` 已支持 desc 副标题（[view.rs:183-190](file:///d:/GitCode/RF/rust-agent-flow/crates/gpui/src/node/view.rs#L183-L190)），Action 节点已传入 `desc: desc_of(node)`（[action.rs:41](file:///d:/GitCode/RF/rust-agent-flow/crates/gpui/src/builtin/action.rs#L41)），尺寸增大后副标题将正常显示。

### Phase 4: Loop 模式标签

**文件**：`crates/gpui/src/builtin/loop_node.rs`

**操作**：
1. 新增辅助函数 `loop_mode_label(node: &Node) -> &'static str`，根据 `node.data["loop_mode"]` 返回显示文案
2. 修改 `get_view` 展开态渲染（[loop_node.rs:238-398](file:///d:/GitCode/RF/rust-agent-flow/crates/gpui/src/builtin/loop_node.rs#L238-L398)），将循环条件区的固定文案替换为 `loop_mode_label(node)`

**关键代码**：
```rust
/// 根据 loop_mode 返回节点显示文案。
fn loop_mode_label(node: &Node) -> &'static str {
    match node.data.get("loop_mode").and_then(|v| v.as_str()) {
        Some("for_each") => "For each item",
        Some("for_loop") => "for i in 0..n",
        Some("while") => "while cond",
        Some("batch_parallel") => "parallel each",
        _ => "For each item",  // 默认
    }
}
```

在 `get_view` 展开态的循环条件区渲染处，将 `"For each item"` 等硬编码文案替换为 `loop_mode_label(node)`。

### Phase 5: 清理小问题

**文件**：
- `crates/gpui/src/builtin/condition.rs`
- `crates/gpui/src/panel/mod.rs`

**操作**：
1. 删除 [condition.rs:340-342](file:///d:/GitCode/RF/rust-agent-flow/crates/gpui/src/builtin/condition.rs#L340-L342) 永真 false 的死代码分支：
   ```rust
   // 删除这段（n_br == n_cond 永远为 false，因为 n_br = n_cond + 1）
   if n_cond == n_br - 1 && i == n_cond - 1 && n_br == n_cond {
       row = row.rounded_b_lg();
   }
   ```
2. 检查 `panel/mod.rs` 的 `EventEmitter` 导入是否实际使用（`subscribe_in` 的 trait bound 隐式使用），若编译器警告则保留，否则移除

### Phase 6: 验证

**操作**：
1. `cargo build` — 验证编译通过
2. `cargo clippy` — 检查代码质量
3. 手动验证（若可行）：
   - Condition 条件项编辑有语法高亮
   - Loop 表达式编辑有语法高亮
   - Action 节点尺寸增大，显示副标题
   - Start/End 节点尺寸增大
   - Loop 节点根据模式显示不同文案

## 假设与约束

1. **不破坏现有排版**：所有改动仅影响节点自身渲染，不修改 dagre 布局参数和后处理步骤
2. **向后兼容**：`SyntaxService` 有默认实现，不注入也能正常工作（回退到普通 multi_line Input）
3. **rhai 依赖保留**：`Cargo.toml` 中的 `rhai` 依赖保留（未来执行引擎使用），本计划不实现 rhai 表达式求值
4. **CodeEditor 行号**：Condition 条件项单行，关闭行号（`.line_number(false)`）；Loop 表达式多行，开启行号

## 风险与缓解

| 风险 | 缓解措施 |
|------|----------|
| CodeEditor 在面板内布局异常 | 先用 `.rows(2)` 限制高度，`.line_number(false)` 减少宽度占用 |
| 节点尺寸变化导致排版错乱 | 仅改 schema 默认尺寸，dagre 自动适配；`sync_node_sizes` 已有机制 |
| SyntaxService 注入路径复杂 | 默认实现内置，FlowEditorView 构造时自动初始化 |
| code_editor 模式下 InputEvent 事件流变化 | 事件机制不变，仅渲染模式变化，现有 subscribe_in 逻辑复用 |

## 文件改动清单

| 文件 | 改动类型 | 说明 |
|------|----------|------|
| `crates/gpui/src/node/syntax.rs` | 新建 | SyntaxService trait + DefaultSyntaxService |
| `crates/gpui/src/node/mod.rs` | 修改 | 导出 syntax 模块 |
| `crates/gpui/src/editor/flow_editor.rs` | 修改 | 新增 syntax_service 字段 + setter |
| `crates/gpui/src/editor/rendering.rs` | 修改 | ensure_panel_view 传入 syntax_service |
| `crates/gpui/src/panel/mod.rs` | 修改 | CodeEditor 集成 + syntax_service 字段 |
| `crates/gpui/src/builtin/action.rs` | 修改 | 尺寸 180×35 → 200×56 |
| `crates/gpui/src/builtin/start.rs` | 修改 | 尺寸 120×35 → 140×44 |
| `crates/gpui/src/builtin/end.rs` | 修改 | 尺寸 120×35 → 140×44 |
| `crates/gpui/src/builtin/loop_node.rs` | 修改 | 新增 loop_mode_label + 渲染使用 |
| `crates/gpui/src/builtin/condition.rs` | 修改 | 删除死代码 |

## 验证步骤

1. **编译验证**：`cargo build` 通过，无错误
2. **Clippy 检查**：`cargo clippy` 无警告
3. **功能验证**：
   - 选中 Condition 节点 → 面板条件项编辑器有 Rust 语法高亮（近似 rhai）
   - 选中 Loop 节点 → 面板表达式编辑器有语法高亮 + 行号
   - Action 节点尺寸变大，显示 label + desc 两行
   - Start/End 节点尺寸变大
   - Loop 节点循环条件区显示对应模式文案
   - 删除节点、展开/收起等现有功能不受影响
