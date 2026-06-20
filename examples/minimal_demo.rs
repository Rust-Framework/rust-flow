//! Minimal demo ? loads flow from embedded Flow Schema JSON (data-driven).

use rust_agent_flow_gpui::FlowEditorView;
use rust_agent_flow::{builtin_type_registry, demo_document_json};
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
                    app.new(|_cx| {
                        FlowEditorView::from_document_json(demo_document_json(), builtin_type_registry())
                    })
                },
            )
            .expect("failed to open window");
        });
}
