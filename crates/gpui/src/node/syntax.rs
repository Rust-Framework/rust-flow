//! 语法高亮服务接口（扩展点）。
//!
//! 核心库提供 [`SyntaxService`] trait 和默认实现 [`DefaultSyntaxService`]，
//! 用于将逻辑语言标识（如 `"rhai"`）映射到 gpui-component `code_editor`
//! 支持的语言字符串（如 `"rust"`）。
//!
//! **设计目的**：避免核心库直接引入 rhai tree-sitter grammar 等大依赖。
//! 外部 crate 可实现 [`SyntaxService`] 提供精确的语法高亮，通过
//! [`FlowEditorView::set_syntax_service`](crate::editor::FlowEditorView::set_syntax_service)
//! 注入。

use std::sync::Arc;

/// 语法高亮服务接口（扩展点）。
///
/// 核心库提供默认实现 [`DefaultSyntaxService`]，将 `"rhai"` 映射到 `"rust"`
/// 近似高亮（rhai 语法与 Rust 高度相似：let/fn/if/while/数组/Map）。
///
/// 外部 crate 可实现此接口提供精确的语法高亮支持，例如：
/// ```ignore
/// struct RhaiSyntaxService;
/// impl SyntaxService for RhaiSyntaxService {
///     fn language_for(&self, kind: &str) -> Option<&str> {
///         match kind {
///             "rhai" => Some("rhai"),  // 假设已注册 rhai grammar
///             _ => None,
///         }
///     }
/// }
/// ```
pub trait SyntaxService: Send + Sync {
    /// 返回 `code_editor` 应使用的语言字符串。
    ///
    /// 返回 `None` 表示不支持该语言，调用方回退到普通 `multi_line` Input。
    ///
    /// `kind` 为逻辑语言标识，如 `"rhai"`、`"javascript"` 等。
    fn language_for(&self, kind: &str) -> Option<&str>;
}

/// 默认语法服务：`rhai` → `rust` 近似高亮。
///
/// rhai 语法与 Rust 高度相似（let/fn/if/while/数组/Map），用 Rust 语法
/// 高亮近似可获得良好的视觉体验，无需引入额外依赖。
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

/// 共享语法服务类型（`Arc<dyn SyntaxService>` 的别名）。
pub type SharedSyntaxService = Arc<dyn SyntaxService>;

/// 返回默认语法服务实例。
pub fn default_syntax_service() -> SharedSyntaxService {
    Arc::new(DefaultSyntaxService)
}
