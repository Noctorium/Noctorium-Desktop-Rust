//! Noctorium's icon in the tray, for while the window is closed and the music is not: the Kotlin desktop's
//! `DesktopTray`. A click brings the window back; its menu plays, pauses, skips and quits.

/// What the tray asked for.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Command {
    Open,
    PlayPause,
    Next,
    Previous,
    Quit,
}

#[cfg(any(windows, target_os = "macos"))]
pub struct Tray {
    icon: tray_icon::TrayIcon,
    play: tray_icon::menu::MenuItem,
}

#[cfg(any(windows, target_os = "macos"))]
impl Tray {
    /// Puts the icon in the tray. None where the desktop has no tray to put it in.
    pub fn start(rgba: Vec<u8>, side: u32, on_command: impl Fn(Command) + Send + Sync + Clone + 'static) -> Option<Tray> {
        use tray_icon::menu::{Menu, MenuEvent, MenuItem, PredefinedMenuItem};
        use tray_icon::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent};
        let open = MenuItem::new("Open Noctorium", true, None);
        let play = MenuItem::new("Play", true, None);
        let next = MenuItem::new("Next", true, None);
        let previous = MenuItem::new("Previous", true, None);
        let quit = MenuItem::new("Quit Noctorium", true, None);
        let menu = Menu::with_items(&[
            &open,
            &PredefinedMenuItem::separator(),
            &play,
            &next,
            &previous,
            &PredefinedMenuItem::separator(),
            &quit,
        ])
        .ok()?;
        let ids = [
            (open.id().clone(), Command::Open),
            (play.id().clone(), Command::PlayPause),
            (next.id().clone(), Command::Next),
            (previous.id().clone(), Command::Previous),
            (quit.id().clone(), Command::Quit),
        ];
        let menu_command = on_command.clone();
        MenuEvent::set_event_handler(Some(move |event: MenuEvent| {
            if let Some((_, command)) = ids.iter().find(|(id, _)| *id == event.id) {
                menu_command(*command);
            }
        }));
        // A single click, as Windows' own tray icons answer, rather than only a double.
        TrayIconEvent::set_event_handler(Some(move |event: TrayIconEvent| {
            if let TrayIconEvent::Click { button: MouseButton::Left, button_state: MouseButtonState::Up, .. } = event {
                on_command(Command::Open);
            }
        }));
        let image = tray_icon::Icon::from_rgba(rgba, side, side).ok()?;
        let icon = TrayIconBuilder::new()
            .with_menu(Box::new(menu))
            .with_tooltip("Noctorium")
            .with_icon(image)
            .build()
            .ok()?;
        Some(Tray { icon, play })
    }

    /// The song playing, in the tooltip, and the menu's first word matching what a click would do.
    pub fn show(&self, song: Option<&str>, playing: bool) {
        // Windows cuts a tray tooltip off at 127 characters; it is shortened here, where the cut is seen.
        let text = match song {
            Some(song) => format!("Noctorium — {song}").chars().take(120).collect::<String>(),
            None => "Noctorium".to_string(),
        };
        let _ = self.icon.set_tooltip(Some(text));
        self.play.set_text(if playing { "Pause" } else { "Play" });
    }
}

#[cfg(not(any(windows, target_os = "macos")))]
pub struct Tray;

#[cfg(not(any(windows, target_os = "macos")))]
impl Tray {
    pub fn start(_rgba: Vec<u8>, _side: u32, _on_command: impl Fn(Command) + Send + Sync + Clone + 'static) -> Option<Tray> {
        None
    }

    pub fn show(&self, _song: Option<&str>, _playing: bool) {}
}
