//! 主题模块：集中管理所有颜色，支持亮色/暗色主题切换。
//!
//! 所有节点使用统一的背景和边框颜色，不按节点类型区分特殊颜色。
//! 通过 [`Theme::light`] / [`Theme::dark`] 创建预设，在
//! [`FlowEditorView`](crate::FlowEditorView) 中持有当前主题实例，
//! 并通过 [`NodeViewCtx`](crate::NodeViewCtx) 传递给节点渲染代码。

use gpui::Rgba;

/// 主题配置：集中管理编辑器所有颜色。
///
/// 所有节点统一使用 `node_*` 系列颜色，不再为 Start/End/Condition/Loop
/// 等节点类型定义特殊颜色。标题栏使用 `node_title_bg` / `node_title_text`
/// 与主体 `node_bg` 形成微妙层次感。
#[derive(Clone, Copy, Debug)]
pub struct Theme {
    /// 是否为暗色主题（用于切换按钮状态）。
    pub is_dark: bool,

    // ====== 画布 ======
    pub canvas_bg: Rgba,
    pub grid_dot: Rgba,

    // ====== 边 ======
    pub edge_default: Rgba,
    pub edge_loop_back: Rgba,

    // ====== 端口通用 ======
    pub port_bg: Rgba,

    // ====== 节点（所有节点统一） ======
    /// 节点主体背景色。
    pub node_bg: Rgba,
    /// 节点标题栏背景色（与主体微妙区分）。
    pub node_title_bg: Rgba,
    /// 节点边框色。
    pub node_border: Rgba,
    /// 节点选中边框色。
    pub node_border_selected: Rgba,
    /// 节点主体文字色。
    pub node_text: Rgba,
    /// 节点标题栏文字色。
    pub node_title_text: Rgba,
    /// 节点副文字色（描述等）。
    pub node_subtext: Rgba,
    /// 输入端口圆点色。
    pub node_in_dot: Rgba,
    /// 输入端口圆环色。
    pub node_in_ring: Rgba,
    /// 输出端口圆点色。
    pub node_out_dot: Rgba,
    /// 输出端口圆环色。
    pub node_out_ring: Rgba,

    // ====== 属性面板 ======
    pub panel_bg: Rgba,
    pub panel_border: Rgba,
    pub panel_title_text: Rgba,
    pub panel_label_text: Rgba,
    pub panel_subtext: Rgba,

    // ====== 节点按钮 ======
    pub delete_btn_bg: Rgba,
    pub delete_btn_text: Rgba,
    pub toggle_btn_bg: Rgba,
    pub toggle_btn_text: Rgba,
    pub collapse_pill_bg: Rgba,
    pub collapse_pill_text: Rgba,

    // ====== 工具栏 ======
    pub toolbar_bg: Rgba,
    pub toolbar_border: Rgba,
    pub toolbar_hover_bg: Rgba,
    pub toolbar_active_bg: Rgba,
    pub toolbar_text: Rgba,
    pub toolbar_subtext: Rgba,
    pub toolbar_accent: Rgba,
    pub toolbar_accent_text: Rgba,
    pub toolbar_toggle_bg: Rgba,
    pub toolbar_toggle_text: Rgba,
    pub toolbar_toggle_hover_bg: Rgba,
    pub toolbar_divider: Rgba,
}

impl Default for Theme {
    fn default() -> Self {
        Self::light()
    }
}

impl Theme {
    /// 亮色主题（默认）。
    pub fn light() -> Self {
        Self {
            is_dark: false,

            // 画布
            canvas_bg: gpui::rgb(0xf8fafc),
            grid_dot: gpui::Rgba { r: 0x94 as f32 / 255.0, g: 0xa3 as f32 / 255.0, b: 0xb8 as f32 / 255.0, a: 0.35 },

            // 边
            edge_default: gpui::Rgba {
                r: 0xb1 as f32 / 255.0,
                g: 0xb1 as f32 / 255.0,
                b: 0xb7 as f32 / 255.0,
                a: 1.0,
            },
            edge_loop_back: gpui::rgb(0x6366f1),

            // 端口
            port_bg: gpui::rgb(0xffffff),

            // 节点（统一）
            node_bg: gpui::rgb(0xffffff),
            node_title_bg: gpui::rgb(0xf1f5f9),
            node_border: gpui::rgb(0xe2e8f0),
            node_border_selected: gpui::rgb(0x6366f1),
            node_text: gpui::rgb(0x1e293b),
            node_title_text: gpui::rgb(0x1e293b),
            node_subtext: gpui::rgb(0x64748b),
            node_in_dot: gpui::rgb(0x6366f1),
            node_in_ring: gpui::rgb(0xc7d2fe),
            node_out_dot: gpui::rgb(0x22c55e),
            node_out_ring: gpui::rgb(0xbbf7d0),

            // 面板
            panel_bg: gpui::rgb(0xf8fafc),
            panel_border: gpui::rgb(0xe2e8f0),
            panel_title_text: gpui::rgb(0x1e293b),
            panel_label_text: gpui::rgb(0x1e293b),
            panel_subtext: gpui::rgb(0x64748b),

            // 节点按钮
            delete_btn_bg: gpui::rgba(0xef4444dd),
            delete_btn_text: gpui::rgb(0xffffff),
            toggle_btn_bg: gpui::rgba(0xffffffaa),
            toggle_btn_text: gpui::rgb(0x475569),
            collapse_pill_bg: gpui::rgba(0xffffffcc),
            collapse_pill_text: gpui::rgb(0x64748b),

            // 工具栏
            toolbar_bg: gpui::rgba(0xffffffee),
            toolbar_border: gpui::rgb(0xe2e8f0),
            toolbar_hover_bg: gpui::rgb(0xf1f5f9),
            toolbar_active_bg: gpui::rgb(0xe2e8f0),
            toolbar_text: gpui::rgb(0x475569),
            toolbar_subtext: gpui::rgb(0x64748b),
            toolbar_accent: gpui::rgb(0x6366f1),
            toolbar_accent_text: gpui::rgb(0xffffff),
            toolbar_toggle_bg: gpui::rgb(0xede9fe),
            toolbar_toggle_text: gpui::rgb(0x6366f1),
            toolbar_toggle_hover_bg: gpui::rgb(0xf8fafc),
            toolbar_divider: gpui::rgb(0xe2e8f0),
        }
    }

    /// 暗色主题。
    pub fn dark() -> Self {
        Self {
            is_dark: true,

            // 画布
            canvas_bg: gpui::rgb(0x1e293b),
            grid_dot: gpui::Rgba { r: 0x47 as f32 / 255.0, g: 0x55 as f32 / 255.0, b: 0x69 as f32 / 255.0, a: 0.4 },

            // 边
            edge_default: gpui::Rgba {
                r: 0x71 as f32 / 255.0,
                g: 0x77 as f32 / 255.0,
                b: 0x86 as f32 / 255.0,
                a: 1.0,
            },
            edge_loop_back: gpui::rgb(0x818cf8),

            // 端口
            port_bg: gpui::rgb(0x1e293b),

            // 节点（统一）
            node_bg: gpui::rgb(0x334155),
            node_title_bg: gpui::rgb(0x475569),
            node_border: gpui::rgb(0x475569),
            node_border_selected: gpui::rgb(0x818cf8),
            node_text: gpui::rgb(0xf1f5f9),
            node_title_text: gpui::rgb(0xf1f5f9),
            node_subtext: gpui::rgb(0x94a3b8),
            node_in_dot: gpui::rgb(0x818cf8),
            node_in_ring: gpui::rgb(0x3730a3),
            node_out_dot: gpui::rgb(0x4ade80),
            node_out_ring: gpui::rgb(0x166534),

            // 面板
            panel_bg: gpui::rgb(0x1e293b),
            panel_border: gpui::rgb(0x475569),
            panel_title_text: gpui::rgb(0xf1f5f9),
            panel_label_text: gpui::rgb(0xe2e8f0),
            panel_subtext: gpui::rgb(0x94a3b8),

            // 节点按钮
            delete_btn_bg: gpui::rgba(0xdc2626dd),
            delete_btn_text: gpui::rgb(0xffffff),
            toggle_btn_bg: gpui::rgba(0x334155aa),
            toggle_btn_text: gpui::rgb(0xcbd5e1),
            collapse_pill_bg: gpui::rgba(0x334155cc),
            collapse_pill_text: gpui::rgb(0x94a3b8),

            // 工具栏
            toolbar_bg: gpui::rgba(0x1e293bee),
            toolbar_border: gpui::rgb(0x475569),
            toolbar_hover_bg: gpui::rgb(0x334155),
            toolbar_active_bg: gpui::rgb(0x475569),
            toolbar_text: gpui::rgb(0xcbd5e1),
            toolbar_subtext: gpui::rgb(0x94a3b8),
            toolbar_accent: gpui::rgb(0x6366f1),
            toolbar_accent_text: gpui::rgb(0xffffff),
            toolbar_toggle_bg: gpui::rgb(0x312e81),
            toolbar_toggle_text: gpui::rgb(0xa5b4fc),
            toolbar_toggle_hover_bg: gpui::rgb(0x334155),
            toolbar_divider: gpui::rgb(0x475569),
        }
    }

    /// 切换主题：暗色 → 亮色，亮色 → 暗色。
    pub fn toggle(self) -> Self {
        if self.is_dark {
            Self::light()
        } else {
            Self::dark()
        }
    }
}
