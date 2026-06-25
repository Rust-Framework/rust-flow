# 第十五章 扩展点体系

rust-agent-flow 的核心库刻意保持「克制」：不绑定特定语法高亮引擎、不内置业务数据类型、不决定工具栏长什么样。这些「应该由调用方决定」的能力，统一通过 **扩展点**（extension point）暴露。扩展点的本质是一组 trait + `Arc<dyn Trait>` + setter 注入——同一套模式贯穿四个领域：工具栏、数据类型、语法高亮、主题与国际化。

## 本章小节

| 小节 | 内容 |
|------|------|
| [ToolbarProvider 工具栏扩展](toolbar-provider.md) | 自定义工具项注入、ToolbarCtx、Demo 实现 |
| [IDataTypeProvider 数据类型扩展](data-type-provider.md) | 自定义复杂类型、DataTypeRegistry 合并 |
| [SyntaxService 语法高亮扩展](syntax-service.md) | 逻辑语言→高亮语言映射、注入重建 |
| [主题与国际化](theme-i18n.md) | Theme 集中配色、Language/i18n、注入销毁语义 |

## 学习目标

读完本章，你应能：

- 说出扩展点四件套（trait + Arc 别名 + setter + 注入语义）的统一形态
- 区分「累积注入」（toolbar）与「替换注入」（syntax/data-type/language）的差别
- 理解为什么替换型注入要销毁 panel_view
- 写出一个自定义 ToolbarProvider / IDataTypeProvider / SyntaxService

## 下一步

从 [ToolbarProvider 工具栏扩展](toolbar-provider.md) 开始。
