# 创建项目与依赖

## 最小 Cargo.toml

rust-agent-flow-gpui 依赖 GPUI（来自 Zed 仓库），需要在 workspace 根 `Cargo.toml` 统一声明 git 依赖：

```toml
[workspace]
members = ["crates/my-app"]
resolver = "2"

[workspace.dependencies]
gpui = { git = "https://github.com/zed-industries/zed", rev = "1d217ee39d381ac101b7cf49d3d22451ac1093fe" }
gpui_platform = { git = "https://github.com/zed-industries/zed", rev = "1d217ee39d381ac101b7cf49d3d22451ac1093fe" }
gpui-component = { git = "https://github.com/longbridge/gpui-component", rev = "e416af7f" }
rust-agent-flow = { path = "../rust-agent-flow/crates/core", version = "0.1.0" }
rust-agent-flow-gpui = { path = "../rust-agent-flow/crates/gpui", version = "0.1.0" }
```

> **注意**：GPUI 锁定特定 Zed commit（`rev`），不同版本 API 可能不兼容。请使用与框架相同的 rev。

## 应用 crate 的 Cargo.toml

```toml
[package]
name = "my-app"
version = "0.1.0"
edition = "2021"

[dependencies]
gpui = { workspace = true }
gpui_platform = { workspace = true }
gpui-component = { workspace = true }
rust-agent-flow = { workspace = true }
rust-agent-flow-gpui = { workspace = true }
serde = { version = "1", features = ["derive"] }
serde_json = "1"

[[bin]]
name = "my-app"
path = "src/main.rs"
```

## 推荐目录结构

```
my-app/
├── Cargo.toml
└── src/
    ├── main.rs              # 入口
    ├── flow_data.rs         # 流程图数据加载
    └── toolbar.rs           # 工具栏扩展（可选）
```

## 依赖说明

| 依赖 | 是否必需 | 说明 |
|------|----------|------|
| `rust-agent-flow-gpui` | 必需 | 主框架（自动依赖 core） |
| `gpui` | 必需 | GPUI 框架 |
| `gpui_platform` | 必需 | GPUI 平台层 |
| `gpui-component` | 必需 | 组件库（Button/Input 等） |
| `serde` / `serde_json` | 推荐 | FlowDocument 序列化 |
| `rust-agent-flow`（core） | 可选 | 直接使用图模型/算法时 |

## 编译注意事项

首次编译可能需要数分钟（GPUI 从 git 编译）。后续增量编译会快很多。

如遇 GPUI 相关链接错误，确保系统已安装 GPU 驱动与所需系统库（Windows 需 DirectX，macOS 需 Metal）。

## 小结

依赖配置完成后，下一步创建第一个窗口并显示 `FlowEditorView`。

下一节：[Hello World 详解](hello-world.md)
