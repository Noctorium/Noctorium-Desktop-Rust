//! What [`App`](super::App) asks of the window it lives in, whichever toolkit draws that window.
//!
//! The app's state and everything it does -- the queue, mpv, yt-dlp, the services -- is the same under egui and
//! under Slint. What differs is the window, and the app needs only a handful of things from it: to be woken when a
//! thread has news, to be shown, minimised, focused or closed, to be given a title, to copy text, and to say whether
//! it has the focus. Each toolkit answers those its own way; the threads only ever hold a [`Host`].

use std::sync::Arc;
use std::time::Duration;

/// Something the app wants done to its window.
#[derive(Clone, Debug, PartialEq)]
pub enum WindowCommand {
    /// Out of the tray, or wherever it went, and on the screen.
    Show,
    Minimise(bool),
    Focus,
    Close,
    /// The window asked to close, and the app keeps it open: the music goes on in the tray.
    CancelClose,
    Title(String),
}

/// The window, as the app sees it. Safe to hold on any thread: threads wake the window when they post an event.
pub trait WindowHost: Send + Sync {
    /// Have the app look at its events and the window drawn again, soon.
    fn wake(&self);
    /// The same, after a while: for a clock ticking, or a sleep timer counting down.
    fn wake_after(&self, after: Duration);
    fn command(&self, command: WindowCommand);
    fn copy_text(&self, text: String);
    /// Whether the window has the keyboard: a song starting is announced only when the listener is elsewhere.
    fn focused(&self) -> bool;
}

pub type Host = Arc<dyn WindowHost>;

/// The egui window.
pub struct EguiHost(pub eframe::egui::Context);

impl WindowHost for EguiHost {
    fn wake(&self) {
        self.0.request_repaint();
    }

    fn wake_after(&self, after: Duration) {
        self.0.request_repaint_after(after);
    }

    fn command(&self, command: WindowCommand) {
        use eframe::egui::ViewportCommand;
        let command = match command {
            WindowCommand::Show => ViewportCommand::Visible(true),
            WindowCommand::Minimise(minimised) => ViewportCommand::Minimized(minimised),
            WindowCommand::Focus => ViewportCommand::Focus,
            WindowCommand::Close => ViewportCommand::Close,
            WindowCommand::CancelClose => ViewportCommand::CancelClose,
            WindowCommand::Title(title) => ViewportCommand::Title(title),
        };
        self.0.send_viewport_cmd(command);
    }

    fn copy_text(&self, text: String) {
        self.0.copy_text(text);
    }

    fn focused(&self) -> bool {
        self.0.input(|i| i.focused)
    }
}
