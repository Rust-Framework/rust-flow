# FieldState 与同步机制

属性面板的核心难题不是「渲染控件」，而是「双向同步」：用户在面板编辑要写回节点；节点被外部改写要同步回面板——且两条路径都不能让光标乱跳、不能让事件无限回环。`FieldState` 与 `sync_from_node` 是这套机制的答案。

## FieldState：四种字段形态

`FieldState` 是个枚举，与 `FieldType` 一一对应：

```rust
enum FieldState {
    Input(Entity<InputState>),        // Text/Number/TextArea/CodeEditor/CodeBlock
    Switch(bool),                     // Switch
    Dropdown(String),                 // Dropdown（存当前值）
    List(Vec<Vec<Entity<InputState>>>), // List（每行一组 InputState）
}
```

对照关系：

| FieldType | FieldState | 说明 |
|-----------|-----------|------|
| Text / Number | `Input` | 单行输入框 |
| TextArea | `Input` | 多行，`multi_line(true).rows(4)` |
| CodeEditor | `Input` | 单行代码，`code_editor(lang).multi_line(false)` |
| CodeBlock | `Input` | 多行代码，`code_editor(lang).line_number(true).rows(4)` |
| Switch | `Switch(bool)` | 内联布尔，无需 InputState |
| Dropdown | `Dropdown(String)` | 内联字符串，菜单渲染时读 schema 选项 |
| List | `List(Vec<Vec<...>>)` | 二维：行 × 列，列对应 `item_fields` |

代码类字段共用 `Input` 形态，区别仅在 `InputState` 的构造参数（是否 `code_editor`、是否多行、是否显示行号）。

## List 的二维结构

List 字段最复杂，是「行 × 列」的二维数组：

```
schema.fields[i].field_type = List(ListSpec {
    item_fields: [FieldSpec{key:"name",...}, FieldSpec{key:"expr",...}],
})

field_states[i] = List(vec![
    vec![ InputState(name1), InputState(expr1) ],   // 第1行
    vec![ InputState(name2), InputState(expr2) ],   // 第2行
])
```

三个操作配套：

- `add_list_item`：按 `item_fields` 创建一行新 `InputState`，订阅变化，push 进 `rows`，再 `sync_list_to_node`。
- `delete_list_item`：`rows.remove(row_idx)`，再 `sync_list_to_node`。
- `sync_list_to_node`：遍历所有行，收集每个 `item_field.key` 的值，组装成 `Vec<Value>` 数组，`dispatch_set_data` 写回节点。

注意 `add_list_item` 创建的每个行内 Input 都被订阅到同一个闭包，触发 `sync_list_to_node(field_idx, cx)`——任何一格改动都会整体回写。

## 事件链：从输入到回写

用户敲一个字符，事件链如下：

```
用户编辑 Input
   │
   ▼
InputState 发出 InputEvent::Change
   │
   ▼
subscribe_in 回调（on_input_field_change / on_label_change）
   │  检查 syncing 标记：若 syncing=true 直接 return
   ▼
dispatch_set_data(key, json!(value))
   │
   ▼
on_action(NodeAction::SetData(key, value))
   │
   ▼
FlowEditorView::handle_node_action
   │  更新 node.data[key] = value
   ▼
relayout（如有必要）+ cx.notify
   │
   ▼
FlowEditorView::render 检测节点数据变化
   │
   ▼
panel_view.sync_from_node(new_node)
   │  快速路径：node.data 一致 → 直接返回（不碰 InputState）
   ▼
逐字段比较，仅变化时 set_value
```

关键点：`dispatch_set_data` 不直接修改 `self.node`，而是绕一圈由编辑器回灌。这保证 `self.node` 始终是「编辑器认可的真相」，避免面板与编辑器状态分裂。

## syncing：防止回环

`sync_from_node` 在更新 InputState 前置 `syncing = true`，结束后置 false。所有 `on_*_change` 回调首行检查：

```rust
fn on_input_field_change(&mut self, field_idx, event, cx) {
    if self.syncing || !matches!(event, InputEvent::Change) {
        return;
    }
    ...
}
```

为什么需要它？`set_value` 会触发 `InputEvent::Change`。若不加保护：

```
sync_from_node 调 set_value → 触发 Change → on_change 调 dispatch_set_data
→ 编辑器更新 node.data → 又触发 sync_from_node → 又调 set_value ...
```

`syncing` 在同步窗口内屏蔽回调，斩断回环。

## sync_from_node：双路径优化

`ensure_panel_view` 每帧都会调用 `sync_from_node`，因此它必须极快。两个优化路径：

### 快速路径：数据一致直接返回

```rust
pub fn sync_from_node(&mut self, node: Node, window, cx) {
    if self.node.id != node.id { return; }
    // 快速路径：数据完全一致时跳过所有更新
    if self.node.data == node.data {
        return;
    }
    ...
}
```

`serde_json::Value` 的 `==` 是结构化比较。绝大多数帧里用户没编辑，节点也没被外部改，这条路径一比较即返回，零成本。

### 慢路径：逐字段比较，仅变化时 set_value

数据不一致时进入慢路径，但仍避免无脑 `set_value`：

```rust
match &mut self.field_states[i] {
    FieldState::Input(entity) => {
        let text = value_to_string(&value);
        let current = entity.read(cx).value().to_string();
        if current != text {                          // 仅变化时才更新
            entity.update(cx, |s, cx| s.set_value(text.as_str(), window, cx));
        }
    }
    FieldState::Switch(b) => { *b = value.as_bool().unwrap_or(false); }
    FieldState::Dropdown(s) => { if let Some(v) = value.as_str() { *s = v.to_string(); } }
    FieldState::List(rows) => { sync_list_rows(rows, &value, field, ...); }
}
```

**为什么不能直接 set_value？** `set_value` 会重置光标位置。如果用户正在某个输入框打字，外部（如 undo/redo、另一处面板）触发同步，无脑 `set_value` 会把光标甩到末尾。逐字段比较确保「用户正在编辑的字段」因为值已是最新的而跳过更新，光标纹丝不动。

## List 同步的两个分支

`sync_list_rows` 同样有快慢两条路径，但判断依据是**行数**：

```rust
if arr.len() == rows.len() {
    // 行数一致：逐格比较更新值
    for (i, item) in arr.iter().enumerate() {
        for (col, item_field) in list_spec.item_fields.iter().enumerate() {
            let text = value_to_string(&val);
            if current != text { rows[i][col].update(...set_value...); }
        }
    }
} else {
    // 行数变化：整体重建（丢失订阅，但 syncing=true 不触发回调）
    rows.clear();
    for item in &arr { /* 重建每行 */ }
}
```

行数变化时只能重建——但此时 `syncing=true`，新建的 InputState 即使触发 Change 也被屏蔽，不会回写。

## 小结

`FieldState` 四种形态覆盖 8 种 `FieldType`；事件链「Input→Change→on_action→SetData→relayout→sync_from_node」把面板与节点数据绑成一体；`syncing` 标记斩断回环；`sync_from_node` 用「数据一致快速返回 + 逐字段比较按需 set_value」双路径，既快又不打扰用户光标。

下一节：[Start 节点专属面板](start-panel.md)
