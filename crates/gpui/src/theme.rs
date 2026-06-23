//! 主题模块：集中管理所有颜色，支持亮色/暗色主题切换。
//!
//! [`Theme`] 结构体持有编辑器所有视觉元素的颜色。通过 [`Theme::light`] /
//! [`Theme::dark`] 创建预设，在 [`FlowEditorView`](crate::FlowEditorView) 中
//! 持有当前主题实例，并通过 [`NodeViewCtx`](crate::NodeViewCtx) 传递给节点
//! 渲染代码。
//!
//! 切换主题时只需替换 `FlowEditorView.theme` 字段并调用 `cx.notify()` 即可，
//! 所有渲染代码自动从新主题取色。

use gpui::Rgba;

/// 主题配置：集中管理编辑器所有颜色。
///
/// 使用 `Theme::light()` 或 `Theme::dark()` 创建预设实例。
/// `is_dark` 标记用于工具栏切换按钮状态判断。
#[derive(Clone, Copy, Debug)]
pub struct Theme {
    /// 是否为暗色主题（用于切换按钮状态）。
    pub is_dark: bool,

    // ====== 画布 ======
    /// 画布主背景色。
    pub canvas_bg: Rgba,
    /// 点阵背景的网格点颜色。
    pub grid_dot: Rgba,

    // ====== 边 ======
    /// 默认边描边色。
    pub edge_default: Rgba,
    /// Loop 回环边描边色。
    pub edge_loop_back: Rgba,

    // ====== 端口通用 ======
    /// 端口圆圈背景色（dot 外的底色圆环背景）。
    pub port_bg: Rgba,

    // ====== 默认节点（Action / fallback） ======
    pub node_bg: Rgba,
    pub node_border: Rgba,
    pub node_border_selected: Rgba,
    pub node_text: Rgba,
    pub node_subtext: Rgba,
    pub node_in_dot: Rgba,
    pub node_in_ring: Rgba,
    pub node_out_dot: Rgba,
    pub node_out_ring: Rgba,

    // ====== Start 节点 ======
    pub start_bg: Rgba,
    pub start_border: Rgba,
    pub start_border_selected: Rgba,
    pub start_text: Rgba,
    pub start_subtext: Rgba,
    pub start_out_dot: Rgba,

    // ====== End 节点 ======
    pub end_bg: Rgba,
    pub end_border: Rgba,
    pub end_border_selected: Rgba,
    pub end_text: Rgba,
    pub end_subtext: Rgba,
    pub end_in_dot: Rgba,

    // ====== Condition 节点 ======
    pub cond_title_bg: Rgba,
    pub cond_title_text: Rgba,
    pub cond_item_bg: Rgba,
    pub cond_else_bg: Rgba,
    pub cond_item_text: Rgba,
    pub cond_border: Rgba,
    pub cond_border_selected: Rgba,
    pub cond_item_border: Rgba,
    pub cond_in_ring: Rgba,
    pub cond_in_dot: Rgba,
    pub cond_if_ring: Rgba,
    pub cond_if_dot: Rgba,
    pub cond_else_ring: Rgba,
    pub cond_else_dot: Rgba,

    // ====== Loop 节点 ======
    pub loop_title_bg: Rgba,
    pub loop_title_text: Rgba,
    pub loop_body_bg: Rgba,
    pub loop_body_text: Rgba,
    pub loop_body_border: Rgba,
    pub loop_border: Rgba,
    pub loop_border_selected: Rgba,
    pub loop_in_ring: Rgba,
    pub loop_in_dot: Rgba,
    pub loop_done_ring: Rgba,
    pub loop_done_dot: Rgba,

    // ====== 属性面板 ======
    pub panel_bg: Rgba,
    pub panel_border: Rgba,
    pub panel_title_text: Rgba,
    pub panel_label_text: Rgba,
    pub panel_subtext: Rgba,

    // ====== 节点按钮 ======
    /// 删除按钮背景色。
    pub delete_btn_bg: Rgba,
    /// 删除按钮文字色。
    pub delete_btn_text: Rgba,
    /// 切换按钮背景色。
    pub toggle_btn_bg: Rgba,
    /// 切换按钮文字色。
    pub toggle_btn_text: Rgba,
    /// 收起状态"..."胶囊背景色。
    pub collapse_pill_bg: Rgba,
    /// 收起状态"..."胶囊文字色。
    pub collapse_pill_text: Rgba,

    // ====== 工具栏 ======
    pub toolbar_bg: Rgba,
    pub toolbar_border: Rgba,
    pub toolbar_hover_bg: Rgba,
    pub toolbar_active_bg: Rgba,
    pub toolbar_text: Rgba,
    pub toolbar_subtext: Rgba,
    /// 激活态强调背景色（如选中的方向按钮）。
    pub toolbar_accent: Rgba,
    /// 激活态强调上的文字色。
    pub toolbar_accent_text: Rgba,
    /// toggle 激活背景色（如网格开关激活时的浅色背景）。
    pub toolbar_toggle_bg: Rgba,
    /// toggle 激活文字色。
    pub toolbar_toggle_text: Rgba,
    /// toggle hover 背景色。
    pub toolbar_toggle_hover_bg: Rgba,
    /// 工具栏分隔线颜色。
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
            grid_dot: gpui::rgb(0x94a3b8),

            // 边
            edge_default: gpui::Rgba {
                r: 0xb1 as f32 / 255.0,
                g: 0xb1 as f32 / 255.0,
                b: 0xb7 as f32 / 255.0,
                a: 1.0,
            },
            edge_loop_back: gpui::rgb(0x3b82f6),

            // 端口
            port_bg: gpui::rgb(0xffffff),

            // 默认节点
            node_bg: gpui::rgb(0xffffff),
            node_border: gpui::rgb(0xe2e8f0),
            node_border_selected: gpui::rgb(0x6366f1),
            node_text: gpui::rgb(0x1e293b),
            node_subtext: gpui::rgb(0x64748b),
            node_in_dot: gpui::rgb(0x6366f1),
            node_in_ring: gpui::rgb(0xc7d2fe),
            node_out_dot: gpui::rgb(0x22c55e),
            node_out_ring: gpui::rgb(0xbbf7d0),

            // Start
            start_bg: gpui::rgb(0x22c55e),
            start_border: gpui::rgb(0x16a34a),
            start_border_selected: gpui::rgb(0x15803d),
            start_text: gpui::rgb(0xffffff),
            start_subtext: gpui::rgb(0xdcfce7),
            start_out_dot: gpui::rgb(0xffffff),

            // End
            end_bg: gpui::rgb(0xef4444),
            end_border: gpui::rgb(0xdc2626),
            end_border_selected: gpui::rgb(0xb91c1c),
            end_text: gpui::rgb(0xffffff),
            end_subtext: gpui::rgb(0xfee2e2),
            end_in_dot: gpui::rgb(0xffffff),

            // Condition
            cond_title_bg: gpui::rgb(0xf97316),
            cond_title_text: gpui::rgb(0xffffff),
            cond_item_bg: gpui::rgb(0xfff7ed),
            cond_else_bg: gpui::rgb(0xffedd5),
            cond_item_text: gpui::rgb(0x9a3412),
            cond_border: gpui::rgb(0xfdba74),
            cond_border_selected: gpui::rgb(0xf97316),
            cond_item_border: gpui::rgb(0xfed7aa),
            cond_in_ring: gpui::rgb(0xc7d2fe),
            cond_in_dot: gpui::rgb(0x6366f1),
            cond_if_ring: gpui::rgb(0xfde68a),
            cond_if_dot: gpui::rgb(0xf97316),
            cond_else_ring: gpui::rgb(0xe2e8f0),
            cond_else_dot: gpui::rgb(0x64748b),

            // Loop
            loop_title_bg: gpui::rgb(0x3b82f6),
            loop_title_text: gpui::rgb(0xffffff),
            loop_body_bg: gpui::rgb(0xeff6ff),
            loop_body_text: gpui::rgb(0x1e3a8a),
            loop_body_border: gpui::rgb(0xbfdbfe),
            loop_border: gpui::rgb(0x93c5fd),
            loop_border_selected: gpui::rgb(0x3b82f6),
            loop_in_ring: gpui::rgb(0xbfdbfe),
            loop_in_dot: gpui::rgb(0x3b82f6),
            loop_done_ring: gpui::rgb(0xe2e8f0),
            loop_done_dot: gpui::rgb(0x64748b),

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
            grid_dot: gpui::rgb(0x475569),

            // 边
            edge_default: gpui::Rgba {
                r: 0x71 as f32 / 255.0,
                g: 0x77 as f32 / 255.0,
                b: 0x86 as f32 / 255.0,
                a: 1.0,
            },
            edge_loop_back: gpui::rgb(0x60a5fa),

            // 端口
            port_bg: gpui::rgb(0x1e293b),

            // 默认节点
            node_bg: gpui::rgb(0x334155),
            node_border: gpui::rgb(0x475569),
            node_border_selected: gpui::rgb(0x818cf8),
            node_text: gpui::rgb(0xf1f5f9),
            node_subtext: gpui::rgb(0x94a3b8),
            node_in_dot: gpui::rgb(0x818cf8),
            node_in_ring: gpui::rgb(0x3730a3),
            node_out_dot: gpui::rgb(0x4ade80),
            node_out_ring: gpui::rgb(0x166534),

            // Start
            start_bg: gpui::rgb(0x16a34a),
            start_border: gpui::rgb(0x15803d),
            start_border_selected: gpui::rgb(0x22c55e),
            start_text: gpui::rgb(0xffffff),
            start_subtext: gpui::rgb(0xbbf7d0),
            start_out_dot: gpui::rgb(0xffffff),

            // End
            end_bg: gpui::rgb(0xdc2626),
            end_border: gpui::rgb(0xb91c1c),
            end_border_selected: gpui::rgb(0xef4444),
            end_text: gpui::rgb(0xffffff),
            end_subtext: gpui::rgb(0xfecaca),
            end_in_dot: gpui::rgb(0xffffff),

            // Condition
            cond_title_bg: gpui::rgb(0xc2410c),
            cond_title_text: gpui::rgb(0xffffff),
            cond_item_bg: gpui::rgb(0x431407),
            cond_else_bg: gpui::rgb(0x7c2d12),
            cond_item_text: gpui::rgb(0xfed7aa),
            cond_border: gpui::rgb(0x9a3412),
            cond_border_selected: gpui::rgb(0xfb923c),
            cond_item_border: gpui::rgb(0x7c2d12),
            cond_in_ring: gpui::rgb(0x3730a3),
            cond_in_dot: gpui::rgb(0x818cf8),
            cond_if_ring: gpui::rgb(0x78350f),
            cond_if_dot: gpui::rgb(0xfb923c),
            cond_else_ring: gpui::rgb(0x475569),
            cond_else_dot: gpui::rgb(0x94a3b8),

            // Loop
            loop_title_bg: gpui::rgb(0x1d4ed8),
            loop_title_text: gpui::rgb(0xffffff),
            loop_body_bg: gpui::rgb(0x1e3a8a),
            loop_body_text: gpui::rgb(0xbfdbfe),
            loop_body_border: gpui::rgb(0x1e40af),
            loop_border: gpui::rgb(0x1e40af),
            loop_border_selected: gpui::rgb(0x60a5fa),
            loop_in_ring: gpui::rgb(0x1e40af),
            loop_in_dot: gpui::rgb(0x60a5fa),
            loop_done_ring: gpui::rgb(0x475569),
            loop_done_dot: gpui::rgb(0x94a3b8),

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
