//! Mind-map / flowchart read-only demo ? Mermaid `graph TB` orchestrator workflow.

use rust_agent_flow_gpui::MindMapView;
use gpui::*;

fn main() {
    gpui_platform::application()
        .with_assets(gpui_component_assets::Assets)
        .run(|app: &mut App| {
            gpui_component::init(app);
            app.open_window(
                WindowOptions {
                    window_bounds: Some(WindowBounds::Windowed(Bounds::centered(
                        None,
                        size(px(1280.), px(800.)),
                        app,
                    ))),
                    titlebar: Some(TitlebarOptions {
                        title: Some(SharedString::from("rust-agent-flow ? ????")),
                        appears_transparent: false,
                        ..Default::default()
                    }),
                    window_min_size: Some(size(px(640.), px(480.))),
                    ..Default::default()
                },
                |_window, app| {
                    app.new(|_cx| MindMapView::orchestrator_demo())
                },
            )
            .expect("failed to open window");
        });
}
