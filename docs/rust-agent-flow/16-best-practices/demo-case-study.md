# Demo 案例研究

前面三节讲了通用的最佳实践。本节把镜头对准 `demo/` 目录——它是 rust-agent-flow 官方维护的「最小完整集成」范本，把数据驱动、扩展点注入、工具栏定制、流程控制结构（条件分支 + 循环回环）全部串了起来。读懂 Demo，你就掌握了把框架用起来的全部要素。

## Demo 的叙事流程

Demo 默认加载的 `AgentFlow` 数据源展示了一个图灵完备的控制流：

```
Start ──→ Planner(规划)
              │
              ▼
         Condition(条件判断)
          ├── if_0 → Search(检索)   ┐
          ├── if_1 → Notify(通知)   ├── 三路汇合
          └── else → ToolCall(工具) ┘
                        │
                        ▼
                      Loop(循环)
                   ┌────┴────┐
              loop_body    done
                   │         │
                   ▼         ▼
              Process     Summarize ──→ End
              (循环体)        │
                   │          │
                   └──loop_in──┘  (回环边)
```

这条流程覆盖了框架的所有关键结构：顺序、条件分支（含 else 兜底）、多路汇合、循环体（含 loop_body 出边与 loop_in 回环边）、循环出口（done）。能正确渲染与编辑它，就证明集成完整。

## 数据驱动：流程即 JSON

三个预置流程以 JSON 形式存在 `demo/data/`，编译期 `include_str!` 嵌入：

```rust
const AGENT_FLOW_JSON: &str    = include_str!("../data/agent_flow.json");
const DATA_PIPELINE_JSON: &str = include_str!("../data/data_pipeline.json");
const SIMPLE_FLOW_JSON: &str   = include_str!("../data/simple_flow.json");
```

`DemoDataSource` 枚举统一管理：

```rust
#[derive(Clone, Copy, PartialEq, Eq, Default)]
pub enum DemoDataSource {
    #[default]
    AgentFlow,      // 条件分支 + 循环回环（默认）
    DataPipeline,   // 数据清洗 → 分流 → 处理 → 汇合
    SimpleFlow,     // Start → Action → End 线性流程
}

impl DemoDataSource {
    pub fn to_graph(&self) -> FlowGraph {
        FlowGraph::from_document(&self.to_document())
    }
    pub fn to_document(&self) -> FlowDocument {
        let json = match self { ... };
        serde_json::from_str(json).expect("内置 JSON 解析失败")
    }
    pub fn label(&self, lang: Language) -> &'static str { ... }
    pub fn all() -> &'static [DemoDataSource] { ... }
}
```

数据驱动的收益：新增流程只需加一个 JSON 文件 + 一个枚举变体，**零渲染代码**。节点/边定义与渲染逻辑完全解耦——这是 rust-agent-flow 设计哲学在 Demo 层的体现。

## main.rs：装配四步

Demo 的 `main.rs` 是集成骨架的范本，正好对应上一节讲的四步：

```rust
fn main() {
    gpui_platform::application()
        .with_assets(CombinedAssets)
        .run(move |cx: &mut gpui::App| {
            // 第1步：框架初始化
            rust_agent_flow_gpui::init(cx);

            cx.spawn(async move |cx| {
                cx.open_window(gpui::WindowOptions::default(), |window, cx| {
                    // 第2步：数据驱动建图
                    let initial_ds = DemoDataSource::AgentFlow;
                    let graph = initial_ds.to_graph();

                    let view = cx.new(|cx| {
                        let mut editor = FlowEditorView::new(graph, cx);
                        // 第3步：自动布局
                        editor.auto_layout(cx);
                        // 第4步：注入扩展（累积型，两个 provider）
                        editor.add_toolbar_provider(
                            Arc::new(DataSourceToolbar::new(initial_ds)), cx,
                        );
                        editor.add_toolbar_provider(
                            Arc::new(AppControlsToolbar::new()), cx,
                        );
                        editor
                    });
                    cx.new(|cx| gpui_component::Root::new(view, window, cx))
                }).expect("Failed to open window");
            }).detach();
        });
}
```

注意 `gpui_component::Root::new(view, window, cx)` 这层包裹——它为 Button/DropdownMenu 等组件提供主题与焦点上下文，漏了会导致组件样式异常、菜单不响应。这是 gpui-component 的集成要求，非 rust-agent-flow 特有。

## DataSourceToolbar：数据源切换

`DataSourceToolbar` 是 `ToolbarProvider` 的典型实现，持有当前数据源状态，切换时 `set_graph` 重建图：

```rust
pub struct DataSourceToolbar {
    current: Arc<Mutex<DemoDataSource>>,
}

impl ToolbarProvider for DataSourceToolbar {
    fn render_items(&self, ctx: &ToolbarCtx) -> Vec<AnyElement> {
        let current = *self.current.lock().unwrap();
        let entity = ctx.entity.clone();
        let current_state = self.current.clone();

        Button::new("demo-data-source")
            .icon(IconName::ALargeSmall).small().ghost()
            .tooltip(t(ctx.language, TKey::TbDataSource))
            .dropdown_menu(move |menu, _, _| {
                let mut menu = menu;
                for &ds in DemoDataSource::all() {
                    let label = ds.label(lang);
                    let entity = entity.clone();
                    let st = current_state.clone();
                    menu = menu.item(
                        PopupMenuItem::new(label)
                            .checked(ds == current)
                            .on_click(move |_, _, cx| {
                                *st.lock().unwrap() = ds;        // 更新自身状态
                                let graph = ds.to_graph();
                                entity.update(cx, |this, cx| this.set_graph(graph, cx));
                            }),
                    );
                }
                menu
            })
            .into();
        vec![btn]
    }
}
```

要点：

- `Arc<Mutex<...>>` 让多份闭包共享可变状态（DropdownMenu 的每个菜单项闭包都要读写 current）
- 切换时先更新自身 `current`，再 `set_graph`——保证下次渲染时 `checked` 标记正确
- `entity.update(cx, |this, cx| this.set_graph(graph, cx))` 是 provider 操作编辑器的标准姿势：通过 `ToolbarCtx.entity` 句柄调编辑器方法

## AppControlsToolbar：能力与呈现分离

`AppControlsToolbar` 展示「框架提供能力、调用方决定呈现」的分工典范。三个按钮分别调三个框架能力方法：

| 按钮 | 调用 | 框架能力 |
|------|------|----------|
| 拖拽开关 | `toggle_drag(cx)` | 切换是否允许拖拽节点 |
| 主题切换 | `toggle_theme(cx)` | 切换亮/暗主题 + 三重同步 |
| 语言切换 | `toggle_language(cx)` | 切换中/英文 + 重建面板 |

```rust
// 拖拽开关：selected 态随 ctx.drag_enabled
Button::new("app-toggle-drag")
    .icon(FlowIcon::Drag).small().ghost()
    .selected(ctx.drag_enabled)
    .tooltip(t(lang, TKey::TbToggleDrag))
    .on_click(move |_, _, cx| {
        drag_entity.update(cx, |this, cx| this.toggle_drag(cx));
    });

// 主题切换：图标随当前主题变（暗色显太阳=切到亮色）
Button::new("app-toggle-theme")
    .icon(if ctx.theme.is_dark { IconName::Sun } else { IconName::Moon })
    .on_click(move |_, _, cx| {
        theme_entity.update(cx, |this, cx| this.toggle_theme(cx));
    });

// 语言切换
Button::new("app-toggle-language")
    .icon(IconName::Globe)
    .on_click(move |_, _, cx| {
        lang_entity.update(cx, |this, cx| this.toggle_language(cx));
    });
```

框架从不规定「拖拽开关按钮放哪」「主题按钮用什么图标」——它只暴露 `toggle_*` 能力方法。Demo 把它们做成三个 ghost Button 放在工具栏末尾，你的应用完全可以换成菜单项、快捷键、侧边栏开关。这是扩展点精神的延伸：**能力在框架、呈现归调用方**。

## 扩展点空位：DemoDataTypeProvider

Demo 的 `DemoDataTypeProvider` 当前返回空 vec：

```rust
pub struct DemoDataTypeProvider;
impl IDataTypeProvider for DemoDataTypeProvider {
    fn data_types(&self) -> Vec<Box<dyn IDataType>> { vec![] }
}
```

它甚至没被 `main.rs` 注入——因为三个预置流程的参数/变量都用内置类型（String/Integer/Float/Boolean/DateTime/Dynamic）。这个文件存在的意义是**扩展点参考**：告诉你要加自定义类型时该写什么。`main.rs` 也没调用 `set_syntax_service`——默认的 rhai→rust 近似高亮已够用。

这传达了一个重要信息：**扩展点是可选的**。不注入任何扩展，框架用内置默认值就能完整工作；需要时再按需注入。Demo 的「最小注入」姿态（只注入两个 toolbar provider）正是这一理念的示范。

## 控制流结构的验证

Demo 的 AgentFlow 流程是框架控制流能力的「冒烟测试」：

| 结构 | 验证点 |
|------|--------|
| 顺序边 | Start→Planner 直连渲染 |
| 条件分支 | Condition 的 if_0/if_1/else 三个出端口，三条边分别从不同端口出 |
| 多路汇合 | Search/Notify/ToolCall 三条边汇入 Loop 同一入端口 |
| 循环体 | Loop 的 loop_body 出边指向 Process，Process 被归入 Loop 的 body_group |
| 循环回环 | Process 的边以 `loop_in` 为 target_port、`EdgeKind::LoopBack` 标记，渲染为回环边 |
| 循环出口 | Loop 的 done 出边指向 Summarize，离开循环体 |
| 折叠 | 收起 Loop 时 body_group 节点隐藏、回环边消失、collapse_pill 显示「已收起」 |

能在 Demo 里看到这些结构正确渲染与交互，就证明你的集成无误。这也是为什么默认数据源是 AgentFlow 而非最简单的 SimpleFlow——它一次性覆盖最多结构。

## 交互能力一览

Demo 同时是交互能力的演示场：

- 中键拖拽：平移视口
- 滚轮：以鼠标位置为锚点缩放
- 左键拖拽节点：移动节点（受 `toggle_drag` 控制）
- 左键从出端口拖到入端口：创建连线
- 点击节点：显示右侧属性面板（schema 驱动或 Start 专属）
- 点击边中点「+」：弹出节点选择面板，选类型后插入到边中间
- 工具栏：缩放、适应视图、布局方向、边类型、点阵、拖拽开关、主题、语言、数据源切换

这些交互全部由框架内置，Demo 无需写一行交互代码——只需把 `FlowEditorView` 放进窗口。

## 从 Demo 到你的项目

把 Demo 改造成自己的项目，路径清晰：

1. **替换数据源**：删掉 `data/*.json` 和 `DemoDataSource`，换成你的 `FlowDocument` 来源（文件/网络/动态构建）
2. **替换工具栏**：把 `DataSourceToolbar`/`AppControlsToolbar` 换成你的业务工具栏，或保留应用控件
3. **按需注入扩展**：有领域类型就实现 `IDataTypeProvider` 并 `set_data_type_provider`；要精确高亮就实现 `SyntaxService` 并 `set_syntax_service`
4. **按需自定义节点**：内置 8 种不够时实现 `IFlowNode`，schema 能表达的优先用 schema 驱动面板
5. **保留装配骨架**：`init` → `new` → `auto_layout` → 注入扩展 → `Root::new` 这条主轴不变

Demo 的价值不在「能跑」，而在它用最少的代码展示了「正确集成的样子」——数据驱动、扩展按需、装配清晰、能力与呈现分离。照这套骨架走，你的项目就站在了框架设计的正轨上。

## 小结

Demo 用 `AgentFlow` 数据源一次性验证了顺序、条件分支、多路汇合、循环体、回环边、折叠等全部控制流结构。`main.rs` 的四步装配（init/new/auto_layout/注入）是集成骨架范本；`DataSourceToolbar` 展示 provider 操作编辑器的标准姿势；`AppControlsToolbar` 示范能力与呈现分离；空的 `DemoDataTypeProvider` 提示扩展点的可选性。把 Demo 当模板，替换数据与扩展，即得你自己的流程编辑器。

全书完。
