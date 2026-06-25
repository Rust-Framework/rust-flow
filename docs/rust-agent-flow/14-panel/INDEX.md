# 第十四章 Schema 驱动属性面板

前几章关注「画布与节点」如何呈现，本章转向用户编辑节点数据的另一半战场——右侧属性面板。rust-agent-flow 的属性面板并非为每种节点写一份独立代码，而是采用 **schema 驱动**：节点声明 `NodeSchema.fields`，面板据此自动生成编辑界面。这一设计消除了 per-kind 的面板分发，让新增节点类型几乎「零 UI 代码」。

## 本章小节

| 小节 | 内容 |
|------|------|
| [PanelView 面板实体](panelview.md) | PanelView 结构、PanelEntity 分发、schema 驱动渲染 |
| [FieldState 与同步机制](fieldstate-sync.md) | 字段状态类型、事件回环、sync_from_node 双路径 |
| [Start 节点专属面板](start-panel.md) | StartPanelView 树形编辑、参数/变量、数据类型注册表 |

## 学习目标

读完本章，你应能回答：

- 为什么新增一种节点类型不需要写任何面板代码？
- 用户在面板输入一个字符，框架内部走了哪条事件链？
- 为什么「节点数据被外部改写」后，面板输入框的光标不会跳动？
- Start 节点为何要脱离通用面板，单独实现一套树形编辑器？

## 下一步

从 [PanelView 面板实体](panelview.md) 开始。
