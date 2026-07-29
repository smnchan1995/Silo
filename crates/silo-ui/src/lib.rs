use gpui::{
    div, prelude::*, px, rgb, size, App, Bounds, Context, Window, WindowBounds, WindowOptions,
};
use gpui_platform::application;

struct Silo;

impl Render for Silo {
    fn render(&mut self, _window: &mut Window, _cx: &mut Context<Self>) -> impl IntoElement {
        div()
            .flex()
            .size_full()
            .justify_center()
            .items_center()
            .bg(rgb(0xf3f2f2))
            .child(div().text_color(rgb(0x201e1d)).child("Silo"))
    }
}

pub fn run() -> anyhow::Result<()> {
    application().run(|cx: &mut App| {
        let bounds = Bounds::centered(None, size(px(1100.0), px(720.0)), cx);
        cx.open_window(
            WindowOptions {
                window_bounds: Some(WindowBounds::Windowed(bounds)),
                ..Default::default()
            },
            |_, cx| cx.new(|_| Silo),
        )
        .expect("failed to open window");
    });
    Ok(())
}
