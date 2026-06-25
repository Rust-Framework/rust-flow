# Start / End / Action

## 共性：标题栏 + 主体的二段式

这三种节点是顺序流的最小完备集，结构最简单，可作为理解其他节点的基线。它们共享 `common.rs` 的辅助函数与布局模式：

```
┌────────────────────────────┐
│ [图标] Label               │  标题栏 h=TITLE_H=36
├────────────────────────────┤
│  主体文案                  │  主体 h=BODY_H（按节点类型）
└────────────────────────────┘
   ○ In                    Out ○   （端口半外露在节点边缘）
```

`common.rs` 提供的共享常量与函数：

| 符号 | 值/签名 | 用途 |
|------|---------|------|
| `TITLE_H` | `36.0` | 标题栏高度（逻辑坐标） |
| `TITLE_ICON_SIZE` | `16.0` | 标题图标尺寸 |
| `DELETE_BTN_SIZE` | `20.0` | 删除按钮尺寸 |
| `TOGGLE_BTN_SIZE` | `20.0` | 展开/收起按钮尺寸 |
| `label_of_localized` | `(node, lang) -> String` | label 空时回退到本地化类型名 |
| `make_port` | `(left, top, ...) -> AnyElement` | 渲染端口圆圈（半外露） |
| `render_delete_button` | `(node_w, scale, theme)` | hover 时显示的删除按钮 |
| `render_toggle_button` | `(node_w, scale, collapsed, theme)` | 展开/收起按钮 |
| `render_simple_panel` | `(node, lang, theme)` | 简单属性面板（kind/label/desc） |

## StartNode：流程起点

| 属性 | 值 |
|------|-----|
| kind | `"start"` |
| 端口 | 仅 `out`（Auto） |
| 默认尺寸 | 160 × (TITLE_H + 20) = 160 × 56 |
| Schema 字段 | `label`(Text), `params`(List[name/type/value]), `variables`(List[name/type/value]) |
| 主体文案 | 有参数时显示「有参数」，否则「无参数」 |
| `get_panel` | 返回空 div（由 StartPanelView 接管） |

**端口位置**（覆写了 `port_position`）：

```
横向布局：                    纵向布局：
                  Out ○       Out
┌─────────────┐              ○
│ Start       │              ┌─Start─┐
│  有参数     │              │ 有参数 │
└─────────────┘              └───────┘
                              （底部中心）
```

横向：Out 在节点右侧垂直中心（非标题栏中心）；纵向：Out 在节点底部中心。**注意是节点几何中心，不是标题栏中心**——这是 `node_mid_y = node.position.y + node.size.h * 0.5` 的语义。

**为何 `get_panel` 返回空 div**：Start 节点的参数/变量编辑用树形列表，比通用 Schema 面板更复杂，故由独立的 `StartPanelView` 实体接管。`IFlowNode::get_panel` 在 PanelView 分发时被跳过——这是框架为复杂面板预留的逃生舱。

## EndNode：流程终点

| 属性 | 值 |
|------|-----|
| kind | `"end"` |
| 端口 | 仅 `in`（Auto） |
| 默认尺寸 | 160 × 56 |
| Schema 字段 | `label`(Text), `returns`(List[name/type]) |
| 主体文案 | 有返回显示「有返回」，否则「无返回」 |
| `get_panel` | 返回空 div（由 PanelView 接管） |

End 是 Start 的镜像：单端口、主体显示有/无返回摘要、面板由独立实体处理。`returns` 数组只有 `name`/`type` 两个字段（无 value）——返回值是运行时产生，编辑期不预设默认值。

## ActionNode：顺序步骤

| 属性 | 值 |
|------|-----|
| kind | `"action"` |
| 端口 | `in`(Auto) + `out`(Auto) |
| 默认尺寸 | 200 × (TITLE_H + 28) = 200 × 64 |
| Schema 字段 | `label`(Text), `desc`(Text) |
| 主体文案 | desc 为空时回退到 i18n 的「Action」 |
| `get_panel` | 调用 `render_simple_panel`（标准 Schema 面板） |

Action 是最常见的节点：执行一个步骤，传入流出。`desc` 是 Text 字段（单行），不是 CodeBlock——任务描述而非代码。主体高度 28px 比 Start/End 的 20px 略大，容纳更长文案。

**端口对称布局**：

```
横向：In ○──┐              ┌──○ Out
            │  [Code] Action │
            └──执行步骤──────┘
纵向：
            ○ In
            ┌──Action──┐
            │ 执行步骤  │
            └────┬─────┘
                 ○ Out
```

横向：In 左中心、Out 右中心；纵向：In 顶中心、Out 底中心。`port_position` 显式覆写，但值与框架默认算法一致——覆写是为了**与实际渲染位置严格对齐**，避免默认算法在边距计算上的微小偏差。

## 三者对比

| 维度 | Start | End | Action |
|------|-------|-----|--------|
| 端口数 | 1 (Out) | 1 (In) | 2 (In+Out) |
| 主体高度 | 20 | 20 | 28 |
| Schema 字段 | label/params/variables | label/returns | label/desc |
| 主体文案 | 有/无参数 | 有/无返回 | desc 或「Action」 |
| `get_panel` | 空 div | 空 div | `render_simple_panel` |
| `port_position` | 覆写 | 覆写 | 覆写 |
| `content_size` | 覆写固定值 | 覆写固定值 | 覆写固定值 |
| `ports_for_node` | 默认 | 默认 | 默认 |

三者都不覆写 `ports_for_node`——端口拓扑静态固定。`content_size` 虽然覆写，但返回的是与 `node.size` 等价的固定值——这更多是显式声明的可读性选择，而非动态尺寸需求。

## 节点尺寸的「逻辑坐标」约定

三种节点的 `content_size` 都返回 `SizeF::new(node.size.w, TITLE_H + BODY_H)`——宽度用 `node.size.w`（由 schema default_size 或创建时指定），高度用常量推导。这是内置节点的通用模式：

| 节点类型 | 宽度来源 | 高度来源 |
|----------|----------|----------|
| 固定高度节点（Start/End/Action/Loop） | `node.size.w` | `TITLE_H + BODY_H`（常量） |
| 结构化节点（Condition） | `node.size.w` | `TITLE_H + ITEM_H * n_branches`（动态） |

宽度永远来自 `node.size.w`——dagre 根据节点宽度计算 nodesep/ranksep，宽度变化会触发重排。高度可以是常量或动态推导，但**必须与实际渲染高度一致**，否则命中测试、回环边边界、dagre 排版都会出错。

## hover 与 selected 的视觉态

```rust
let border_color = if ctx.selected {
    t.node_border_selected
} else {
    t.node_border
};
// ...
if ctx.hovered {
    container = container.child(render_delete_button(node.size.w, s, t));
}
```

- **selected**：边框色切换为 `node_border_selected`（通常更亮/更醒目）
- **hovered**：叠加删除按钮（Start/End 无 toggle 按钮，因为无可收起内容）

Action 没有 toggle 按钮——它的内容是单行 desc，无需收起。Condition/Loop 才有 toggle，因为它们有可收起的多分支结构。

## 小结

Start/End/Action 是顺序流三件套，共享「标题栏 + 主体」二段式布局。Start/End 是单端口端点节点（面板由独立实体接管），Action 是双端口步骤节点（标准 Schema 面板）。三者都覆写 `port_position` 与 `content_size` 以显式对齐渲染，但不覆写 `ports_for_node`（端口拓扑静态）。理解这三种简单节点是掌握 Condition/Loop 结构化节点的基础。

下一节：[Condition 条件分支](condition-branch.md)
