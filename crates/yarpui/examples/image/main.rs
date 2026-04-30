use anyhow::Result;
use root_view::RootView;
pub mod root_view;

extern crate yarpui;
use yarpui::platform;

fn main() -> Result<()> {
    let app_builder =
        platform::AppBuilder::new(platform::AppCallbacks::default(), Box::new(()), None);
    let _ = app_builder.run(move |ctx| {
        ctx.add_window(yarpui::AddWindowOptions::default(), |_| RootView::new());
    });

    Ok(())
}
