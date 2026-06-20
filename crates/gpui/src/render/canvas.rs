use gpui::*;

use crate::coords::viewport_to_paint;
use crate::theme::FlowTheme;

/// Paint a dot-grid background covering the canvas bounds.
pub fn paint_dot_grid(
    bounds: Bounds<Pixels>,
    theme: &FlowTheme,
    window: &mut Window,
    zoom: f32,
) {
    let spacing = theme.grid_dot_spacing * zoom;
    let dot_r = theme.grid_dot_radius * zoom;
    let hsla = Hsla {
        h: 0.0,
        s: 0.0,
        l: theme.grid_dot_color.r * 0.2126
            + theme.grid_dot_color.g * 0.7152
            + theme.grid_dot_color.b * 0.0722,
        a: theme.grid_dot_color.a,
    };
    let transparent = transparent_black();

    let ox: f32 = bounds.origin.x.into();
    let oy: f32 = bounds.origin.y.into();
    let bw: f32 = bounds.size.width.into();
    let bh: f32 = bounds.size.height.into();

    let start_x = ((ox / spacing).floor() * spacing) - ox;
    let start_y = ((oy / spacing).floor() * spacing) - oy;

    let mut y = start_y;
    while y <= bh + dot_r {
        let mut x = start_x;
        while x <= bw + dot_r {
            let (wx, wy) = viewport_to_paint(x, y, ox, oy);
            window.paint_quad(quad(
                Bounds {
                    origin: Point {
                        x: px(wx - dot_r),
                        y: px(wy - dot_r),
                    },
                    size: Size {
                        width: px(dot_r * 2.0),
                        height: px(dot_r * 2.0),
                    },
                },
                px(0.0),
                hsla,
                Edges::<Pixels>::default(),
                transparent,
                Default::default(),
            ));
            x += spacing;
        }
        y += spacing;
    }
}
