//! 自定义图标资产：使用 `rust_embed` 在编译时将 `assets/` 目录下的 SVG 文件
//! 嵌入二进制，并通过组合 [`CombinedAssets`] 同时支持自定义图标与
//! `gpui-component` 默认图标。
//!
//! ## 使用方式
//!
//! 1. 在应用启动时使用 [`CombinedAssets`] 作为全局 `AssetSource`：
//!
//! ```rust,ignore
//! gpui_platform::application().with_assets(CombinedAssets);
//! ```
//!
//! 2. 在需要自定义图标的地方使用 [`FlowIcon`]：
//!
//! ```rust,ignore
//! use rust_agent_flow_gpui::FlowIcon;
//! Button::new("tb-drag").icon(FlowIcon::Drag);
//! ```

use std::borrow::Cow;

use gpui::{AssetSource, Result, SharedString};
use gpui_component::IconNamed;
use rust_embed::RustEmbed;

/// 嵌入工作区根目录 `assets/` 下的所有 SVG 图标文件。
///
/// 路径使用 `$CARGO_MANIFEST_DIR` 插值，确保从 `crates/gpui` 定位到
/// `../../assets/`。
#[derive(RustEmbed)]
#[folder = "$CARGO_MANIFEST_DIR/../../assets"]
struct FlowAssets;

/// 组合型 `AssetSource`：优先查找自定义嵌入图标，未命中时回退到
/// `gpui-component-assets` 默认图标包。
///
/// `with_assets` 仅接受单个 `AssetSource`，因此需要用此组合类型把两套资产
/// 合并到一个入口。
pub struct CombinedAssets;

impl AssetSource for CombinedAssets {
    fn load(&self, path: &str) -> Result<Option<Cow<'static, [u8]>>> {
        if path.is_empty() {
            return Ok(None);
        }
        // 优先从自定义图标中查找
        if let Some(file) = FlowAssets::get(path) {
            return Ok(Some(file.data));
        }
        // 回退到 gpui-component 默认资产
        gpui_component_assets::Assets.load(path)
    }

    fn list(&self, path: &str) -> Result<Vec<SharedString>> {
        let mut result: Vec<SharedString> = FlowAssets::iter()
            .filter(|p| p.starts_with(path))
            .map(Into::into)
            .collect();
        let mut defaults = gpui_component_assets::Assets.list(path)?;
        result.append(&mut defaults);
        Ok(result)
    }
}

/// 自定义流程编辑器图标枚举。
///
/// 每个变体对应 `assets/` 目录下的一个 SVG 文件，通过实现 [`IconNamed`]
/// 可直接用于 `gpui-component` 的 `Button::icon`、`Icon::new` 等接口。
///
/// 路径需与 [`FlowAssets`] 中嵌入的文件名一致。
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FlowIcon {
    /// 拖拽工具（square-mouse-pointer），用于拖拽开关按钮。
    Drag,
    /// 抓手（grip），用于拖拽手柄。
    Grip,
    /// 水平双向箭头，用于水平布局方向按钮。
    Horizontal,
    /// 全屏（maximize），用于适应视图按钮。
    Screen,
    /// 垂直双向箭头，用于垂直布局方向按钮。
    Vertical,
}

impl IconNamed for FlowIcon {
    fn path(self) -> SharedString {
        match self {
            FlowIcon::Drag => "drag.svg".into(),
            FlowIcon::Grip => "grip.svg".into(),
            FlowIcon::Horizontal => "horizontal.svg".into(),
            FlowIcon::Screen => "screen.svg".into(),
            FlowIcon::Vertical => "vertical.svg".into(),
        }
    }
}
