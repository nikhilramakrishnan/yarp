//! Yarp Home
//!
//! This is the landing page for new tabs if session creation isn't supported (e.g. on the web).
//! It's barebones at the moment, but may grow into a more full-featured admin experience.

use yarpui::ViewContext;

use super::view::Workspace;
use crate::pane_group::{AnyPaneContent, FilePane};

const YARP_HOME_TITLE: &str = "Sandford station, web detachment";
const YARP_HOME_CONTENT: &str = r#"
Welcome to the Sandford web detachment — your browser-based outpost.
Sign on here to:
* Hop onto open channels
* Open, read, and amend Yarp Drive case files
* Amend your station's Standing Orders

The web detachment also lets fellow officers without the desktop kit observe shared channels, casebooks, and duty rosters."#;

/// Create a static "home page" pane.
pub fn create_home_pane(ctx: &mut ViewContext<Workspace>) -> Box<dyn AnyPaneContent> {
    let pane = FilePane::new(
        None,
        None,
        #[cfg(feature = "local_fs")]
        None,
        ctx,
    );
    pane.file_view(ctx).update(ctx, |pane, ctx| {
        pane.open_static(YARP_HOME_TITLE, YARP_HOME_CONTENT, ctx);
    });
    Box::new(pane)
}
