//! Mind-map demo — loads `mindmap-1.0` JSON with bidirectional tree layout.

use rust_agent_flow::mindmap_document_json;
use rust_agent_flow_gpui::MindMapView;
use gpui::*;

/// 默认加载内置的思维导图示例。
/// 可通过命令行参数指定加载其他 JSON 文件：
///   cargo run --example mindmap_demo --features demo -- schemas/orchestrator.mindmap.json
fn main() {
    // 读取命令行参数，默认使用内置示例
    let args: Vec<String> = std::env::args().collect();
    let json_text: String = if args.len() > 1 {
        // 从文件加载
        std::fs::read_to_string(&args[1]).unwrap_or_else(|e| {
            eprintln!("无法加载文件 {}: {}", args[1], e);
            mindmap_document_json().to_string()
        })
    } else {
        // 使用内置示例
        mindmap_document_json().to_string()
    };

    gpui_platform::application()
        .with_assets(gpui_component_assets::Assets)
        .run(move |app: &mut App| {
            gpui_component::init(app);
            app.open_window(
                WindowOptions {
                    window_bounds: Some(WindowBounds::Windowed(Bounds::centered(
                        None,
                        size(px(1280.), px(800.)),
                        app,
                    ))),
                    titlebar: Some(TitlebarOptions {
                        title: Some(SharedString::from("rust-agent-flow · 思维导图")),
                        appears_transparent: false,
                        ..Default::default()
                    }),
                    window_min_size: Some(size(px(640.), px(480.))),
                    ..Default::default()
                },
                |_window, app| {
                    app.new(|_cx| MindMapView::from_text(&json_text))
                },
            )
            .expect("failed to open window");
        });
}
