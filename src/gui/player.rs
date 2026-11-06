//! The player along the bottom, and the lyrics beside the page.

use super::widgets::{self, icon_button, pause_bars, play_triangle};
use super::{Action, App, Inputs, Loadable, Page};
use crate::domain::format_duration;
use super::icons::{self, Icon};
use crate::looks::{IconSet, LyricsAlignment, PlayerBarPosition, PlayerBarStyle, PlayerButton, ProgressBarStyle, SurfaceStyle};
use crate::lyrics::Lyrics;
use crate::mpv::Status;
use crate::queue::RepeatMode;
use crate::theme;
use eframe::egui::{
    self, pos2, vec2, Align, Align2, CornerRadius, Layout, Rect, RichText, Sense, Stroke, UiBuilder, Vec2,
};

pub fn player_bar(app: &App, ctx: &egui::Context, inputs: &mut Inputs, actions: &mut Vec<Action>) {
    let palette = app.palette;
    let looks = &app.settings.looks;
    let style = looks.player_bar;
    let glass = looks.surface == SurfaceStyle::Glass;
    let height = match style {
        PlayerBarStyle::Inline => 80.0,
        PlayerBarStyle::Stacked => 112.0,
        PlayerBarStyle::Centered => 96.0,
        PlayerBarStyle::Slim | PlayerBarStyle::SlimLeft => 56.0,
        PlayerBarStyle::Spotlight => 136.0,
        PlayerBarStyle::Floating => 112.0,
        PlayerBarStyle::Island => 92.0,
        PlayerBarStyle::Display => 100.0,
        PlayerBarStyle::Taskbar => 44.0,
        PlayerBarStyle::Toolbar | PlayerBarStyle::Toolbar2 => 48.0,
    };
    let top = looks.player_bar_position == PlayerBarPosition::Top;
    let fill = if glass { palette.panel.gamma_multiply(0.42) } else { palette.panel.gamma_multiply(app.panel_opacity()) };
    let edge = Stroke::new(1.0_f32, palette.line.gamma_multiply(0.6));
    let frame = match style {
        PlayerBarStyle::Floating => egui::Frame::new()
            .fill(if glass { palette.card.gamma_multiply(0.45) } else { palette.card.gamma_multiply(0.94) })
            .stroke(Stroke::new(1.0_f32, palette.line))
            .corner_radius(theme::corner(18))
            .inner_margin(egui::Margin::symmetric(16, 10))
            .outer_margin(if top {
                egui::Margin { left: 14, right: 14, top: 12, bottom: 4 }
            } else {
                egui::Margin { left: 14, right: 14, top: 4, bottom: 12 }
            }),
        PlayerBarStyle::Island => egui::Frame::new().inner_margin(egui::Margin::symmetric(16, 10)),
        PlayerBarStyle::Slim | PlayerBarStyle::SlimLeft => egui::Frame::new().fill(fill).stroke(edge).inner_margin(egui::Margin::symmetric(16, 6)),
        PlayerBarStyle::Taskbar | PlayerBarStyle::Toolbar | PlayerBarStyle::Toolbar2 => {
            egui::Frame::new().fill(fill).stroke(edge).inner_margin(egui::Margin::symmetric(8, 6))
        }
        _ => egui::Frame::new().fill(fill).stroke(edge).inner_margin(egui::Margin::symmetric(16, 10)),
    };
    let panel = if top { egui::TopBottomPanel::top("player") } else { egui::TopBottomPanel::bottom("player") };
    panel
        .exact_height(height)
        .resizable(false)
        .show_separator_line(false)
        .frame(frame)
        .show(ctx, |ui| {
            let full = ui.max_rect();
            if glass && style != PlayerBarStyle::Island {
                // The card itself when floating; otherwise the whole panel, whatever its margins.
                let outer = if style == PlayerBarStyle::Floating { full.expand2(vec2(16.0, 10.0)) } else { ui.clip_rect() };
                glass_sheen(ui.painter(), outer, if style == PlayerBarStyle::Floating { theme::corner(18) } else { 0 });
            }
            let bar = Bar::of(app);
            match style {
                PlayerBarStyle::Inline => {
                    let controls = Rect::from_min_size(full.left_top(), vec2(150.0, full.height()));
                    transport(ui, app, controls, false, 44.0, actions);
                    let track_width = (full.width() * 0.24).clamp(200.0, 320.0);
                    let track = Rect::from_min_size(pos2(controls.right() + 12.0, full.top()), vec2(track_width, full.height()));
                    track_block(ui, app, track, 52.0, 15.0, true, actions);
                    let tools_rect = Rect::from_min_max(pos2(full.right() - tools_width(app, true), full.top()), full.max);
                    tools(ui, app, tools_rect, true, actions);
                    let seek = Rect::from_min_max(pos2(track.right() + 48.0, full.center().y - 8.0), pos2(tools_rect.left() - 48.0, full.center().y + 8.0));
                    if seek.width() > 60.0 {
                        seek_bar(ui, app, seek, bar.position, bar.duration, inputs, actions);
                    }
                }
                PlayerBarStyle::Stacked => {
                    let seek = Rect::from_min_max(pos2(full.left() + 48.0, full.top() + 2.0), pos2(full.right() - 48.0, full.top() + 18.0));
                    seek_bar(ui, app, seek, bar.position, bar.duration, inputs, actions);
                    let row = Rect::from_min_max(pos2(full.left(), full.top() + 26.0), full.max);
                    let (left, centre, right) = thirds(row, tools_width(app, false));
                    track_block(ui, app, left, 56.0, 15.0, true, actions);
                    transport(ui, app, centre, true, 46.0, actions);
                    tools(ui, app, right, false, actions);
                }
                PlayerBarStyle::Centered | PlayerBarStyle::Floating => centred(ui, app, full, 64.0, 15.0, &bar, inputs, actions),
                PlayerBarStyle::Spotlight => {
                    // Tinted with the cover's colour, from the left where the cover sits.
                    let tint = app.artwork_colour().unwrap_or(palette.accent);
                    let wash = full.expand2(vec2(16.0, 10.0));
                    theme::gradient(ui.painter(), wash, tint.gamma_multiply(0.38), tint.gamma_multiply(0.0), true);
                    centred(ui, app, full, 108.0, 19.0, &bar, inputs, actions);
                }
                PlayerBarStyle::Slim | PlayerBarStyle::SlimLeft => {
                    hairline(ui, app, Rect::from_min_max(pos2(full.left() - 16.0, full.top() - 6.0), pos2(full.right() + 16.0, full.top() - 3.0)), &bar, actions);
                    let controls_width = 230.0;
                    let tools_rect = Rect::from_min_max(pos2(full.right() - tools_width(app, true), full.top()), full.max);
                    tools(ui, app, tools_rect, true, actions);
                    if style == PlayerBarStyle::SlimLeft {
                        let controls = Rect::from_min_size(full.left_top(), vec2(controls_width, full.height()));
                        transport(ui, app, controls, true, 36.0, actions);
                        let track = Rect::from_min_max(pos2(controls.right() + 12.0, full.top()), pos2(tools_rect.left() - 12.0, full.bottom()));
                        track_block(ui, app, track, 40.0, 14.0, true, actions);
                    } else {
                        let track = Rect::from_min_size(full.left_top(), vec2((full.width() * 0.3).clamp(200.0, 340.0), full.height()));
                        track_block(ui, app, track, 40.0, 14.0, true, actions);
                        let controls = Rect::from_min_max(pos2(track.right(), full.top()), pos2(tools_rect.left(), full.bottom()));
                        transport(ui, app, controls, true, 36.0, actions);
                    }
                    let time = format!("{} / {}", format_duration(bar.position), if bar.duration > 0 { format_duration(bar.duration) } else { "–:––".into() });
                    ui.painter().text(pos2(tools_rect.left() - 6.0, full.center().y), Align2::RIGHT_CENTER, time, theme::regular(11.5), palette.faint);
                }
                PlayerBarStyle::Island => island(ui, app, full, &bar, actions),
                PlayerBarStyle::Display => {
                    let width = (full.width() * 0.4).clamp(300.0, 460.0);
                    let display = Rect::from_center_size(full.center(), vec2(width, full.height()));
                    stereo_display(ui, app, display, &bar, actions);
                    let controls = Rect::from_min_max(full.min, pos2(display.left() - 12.0, full.bottom()));
                    transport(ui, app, controls, true, 44.0, actions);
                    let right = Rect::from_min_max(pos2(display.right() + 12.0, full.top()), full.max);
                    tools(ui, app, right, true, actions);
                }
                PlayerBarStyle::Taskbar => taskbar(ui, app, full, &bar, actions),
                PlayerBarStyle::Toolbar => toolbar(ui, app, full, &bar, false, inputs, actions),
                PlayerBarStyle::Toolbar2 => toolbar(ui, app, full, &bar, true, inputs, actions),
            }
        });
}

/// What every arrangement needs to know about the song: what is shown, and where in it the music is.
struct Bar {
    position: i64,
    duration: i64,
}

impl Bar {
    fn of(app: &App) -> Bar {
        let state = app.engine.state();
        let shown = app.loaded.as_ref().or(app.resolving.as_ref()).or(app.queue.current());
        let duration = if state.duration_ms > 0 { state.duration_ms } else { shown.and_then(|t| t.duration_ms).unwrap_or(0) };
        Bar { position: if app.loaded.is_some() { state.position_ms } else { 0 }, duration }
    }
}

/// The track on the left, the controls in the middle, the tools on the right with room for every one shown.
fn thirds(row: Rect, tools: f32) -> (Rect, Rect, Rect) {
    let side = (row.width() * 0.28).clamp(220.0, 360.0);
    let left = Rect::from_min_max(row.min, pos2(row.left() + side, row.bottom()));
    let right = Rect::from_min_max(pos2(row.right() - side.max(tools), row.top()), row.max);
    let centre = Rect::from_min_max(pos2(left.right() + 16.0, row.top()), pos2(right.left() - 16.0, row.bottom()));
    (left, centre, right)
}

/// The track on the left, the controls in the middle with the seek bar under them, the tools on the right.
#[allow(clippy::too_many_arguments)]
fn centred(ui: &mut egui::Ui, app: &App, full: Rect, cover: f32, title: f32, bar: &Bar, inputs: &mut Inputs, actions: &mut Vec<Action>) {
    let (left, centre, right) = thirds(full, tools_width(app, false));
    track_block(ui, app, left, cover, title, true, actions);
    let controls = Rect::from_min_max(pos2(centre.left(), centre.top()), pos2(centre.right(), centre.top() + 50.0));
    transport(ui, app, controls, true, 46.0, actions);
    let seek = Rect::from_min_max(pos2(centre.left() + 48.0, centre.bottom() - 26.0), pos2(centre.right() - 48.0, centre.bottom() - 10.0));
    seek_bar(ui, app, seek, bar.position, bar.duration, inputs, actions);
    tools(ui, app, right, false, actions);
}

/// Glass: a pale rim along the top and a sheen fading down it, over a see-through panel.
fn glass_sheen(painter: &egui::Painter, rect: Rect, radius: u8) {
    let top_half = Rect::from_min_max(rect.min, pos2(rect.right(), rect.center().y));
    theme::gradient(painter, top_half, egui::Color32::from_white_alpha(22), egui::Color32::from_white_alpha(0), false);
    painter.rect_stroke(rect.shrink(0.5), radius, Stroke::new(1.0_f32, egui::Color32::from_white_alpha(40)), egui::StrokeKind::Inside);
}

/// The cover, the title and the artist, and the heart, within `rect`.
fn track_block(ui: &mut egui::Ui, app: &App, rect: Rect, cover: f32, title: f32, heart: bool, actions: &mut Vec<Action>) {
    let palette = app.palette;
    let shown = app.loaded.as_ref().or(app.resolving.as_ref()).or(app.queue.current());
    let mut row = ui.new_child(UiBuilder::new().max_rect(rect).layout(Layout::left_to_right(Align::Center)));
    let Some(track) = shown else {
        row.label(RichText::new("Nothing playing").color(palette.faint));
        return;
    };
    if widgets::artwork(&mut row, &palette, track.artwork_url.as_deref(), cover, 8).on_hover_text("Now playing").clicked() {
        actions.push(Action::Open(Page::NowPlaying));
    }
    row.add_space(12.0);
    let text_width = (rect.width() - cover - 12.0 - if heart { 40.0 } else { 0.0 }).max(60.0);
    row.allocate_ui_with_layout(vec2(text_width, cover.min(rect.height())), Layout::top_down(Align::Min).with_main_align(Align::Center), |ui| {
        ui.add(egui::Label::new(RichText::new(&track.title).font(theme::bold(title)).color(palette.text)).truncate());
        ui.add(egui::Label::new(RichText::new(track.artist_line()).font(theme::regular(title - 2.0)).color(palette.subtext)).truncate());
    });
    if heart && app.settings.looks.shows(PlayerButton::Like) {
        let liked = app.library.is_liked(track);
        let icon = if liked { Icon::HeartFilled } else { Icon::Heart };
        if icons::icon_button(&mut row, &palette, icon, 32.0, liked, if liked { "Remove from Liked songs" } else { "Add to Liked songs" }).clicked() {
            actions.push(Action::ToggleLike(track.clone()));
        }
    }
}

/// The big round play button: a spinner while the song is being found.
fn play_button(ui: &mut egui::Ui, app: &App, size: f32, actions: &mut Vec<Action>) {
    let palette = app.palette;
    let state = app.engine.state();
    let (rect, play) = ui.allocate_exact_size(Vec2::splat(size), Sense::click());
    let fill = if play.hovered() { theme::mix(palette.text, palette.accent, 0.25) } else { palette.text };
    ui.painter().circle_filled(rect.center(), size / 2.0, fill);
    let busy = app.resolving.is_some() || state.status == Status::Loading;
    let glyph = Rect::from_center_size(rect.center(), Vec2::splat(size * 0.38));
    if busy {
        egui::Spinner::new().color(palette.page).paint_at(ui, Rect::from_center_size(rect.center(), Vec2::splat(size * 0.48)));
    } else if icons::icon_set() != IconSet::Standard {
        let icon = if state.status == Status::Playing { Icon::Pause } else { Icon::Play };
        icons::paint(ui.painter(), glyph, icon, palette.page);
    } else if state.status == Status::Playing {
        pause_bars(ui.painter(), rect.center(), size * 0.163, palette.page);
    } else {
        play_triangle(ui.painter(), rect.center() + vec2(size * 0.033, 0.0), size * 0.185, palette.page);
    }
    if play.on_hover_text("Play or pause (Space)").clicked() {
        actions.push(Action::TogglePlay);
    }
}

/// Previous, play and next -- and shuffle and repeat round them when `full` -- centred in `rect`.
pub(super) fn transport(ui: &mut egui::Ui, app: &App, rect: Rect, full: bool, play: f32, actions: &mut Vec<Action>) {
    let palette = app.palette;
    let small = (play * 0.74).max(28.0);
    let gap = 10.0;
    let looks = &app.settings.looks;
    let (shuffle, repeat) = (full && looks.shows(PlayerButton::Shuffle), full && looks.shows(PlayerButton::Repeat));
    let extra = shuffle as u8 as f32 + repeat as u8 as f32;
    let width = small * (2.0 + extra) + play + gap * (2.0 + extra);
    let mut row = ui.new_child(UiBuilder::new().max_rect(rect).layout(Layout::left_to_right(Align::Center)));
    row.spacing_mut().item_spacing.x = gap;
    row.add_space(((rect.width() - width) / 2.0).max(0.0));
    if shuffle && icons::icon_button(&mut row, &palette, Icon::Shuffle, small, app.queue.shuffle, "Shuffle (Ctrl+S)").clicked() {
        actions.push(Action::ToggleShuffle);
    }
    if icons::icon_button(&mut row, &palette, Icon::Previous, small, false, "Previous (Ctrl+←)").clicked() {
        actions.push(Action::Previous);
    }
    play_button(&mut row, app, play, actions);
    if icons::icon_button(&mut row, &palette, Icon::Next, small, false, "Next (Ctrl+→)").clicked() {
        actions.push(Action::Next);
    }
    if repeat {
        let (icon, hint) = repeat_icon(app);
        if icons::icon_button(&mut row, &palette, icon, small, app.queue.repeat != RepeatMode::Off, hint).clicked() {
            actions.push(Action::CycleRepeat);
        }
    }
}

/// How wide the tools are with the buttons chosen, so what sits beside them is never drawn over.
fn tools_width(app: &App, compact: bool) -> f32 {
    let looks = &app.settings.looks;
    let button = 38.0;
    let mut width = 8.0;
    if looks.shows(PlayerButton::Volume) {
        width += if compact { 90.0 } else { 110.0 } + 6.0 + button;
    }
    for shown in [PlayerButton::Queue, PlayerButton::Lyrics, PlayerButton::Devices, PlayerButton::Stop] {
        if looks.shows(shown) {
            width += button;
        }
    }
    if looks.shows(PlayerButton::SleepTimer) {
        width += button + if app.sleep.is_some() { 40.0 } else { 0.0 };
    }
    if looks.shows(PlayerButton::Speed) {
        width += 48.0;
    }
    if crate::audio::boost::is_on(app.settings.boost_percent) {
        width += 52.0;
    }
    if !compact && app.loaded.is_some() && app.engine.state().format.is_some() {
        width += 96.0;
    }
    width
}

/// The speed, as a button that steps through the usual ones: 1×, 1.25×, 1.5×, 2×, 0.75×.
fn speed_button(ui: &mut egui::Ui, app: &App, actions: &mut Vec<Action>) {
    let speeds = [0.75_f32, 1.0, 1.25, 1.5, 2.0];
    let now = app.settings.speed;
    let label = format!("{now:.2}×").replace(".00×", "×").replace("0×", "×");
    let colour = if (now - 1.0).abs() > 0.01 { app.palette.accent } else { app.palette.subtext };
    let button = egui::Button::new(RichText::new(label).font(theme::bold(12.0)).color(colour)).frame(false);
    if ui.add_sized(vec2(42.0, 28.0), button).on_hover_text("Speed: click for the next").clicked() {
        let next = speeds.iter().copied().find(|s| *s > now + 0.01).unwrap_or(speeds[0]);
        actions.push(Action::SetSpeed(next));
    }
}

fn repeat_icon(app: &App) -> (Icon, &'static str) {
    match app.queue.repeat {
        RepeatMode::Off => (Icon::Repeat, "Repeat: off (Ctrl+R)"),
        RepeatMode::All => (Icon::Repeat, "Repeat: the queue (Ctrl+R)"),
        RepeatMode::One => (Icon::RepeatOne, "Repeat: this song (Ctrl+R)"),
    }
}

fn speaker_icon(app: &App) -> Icon {
    if app.settings.muted || app.settings.volume == 0.0 {
        Icon::Mute
    } else if app.settings.volume < 0.5 {
        Icon::VolumeLow
    } else {
        Icon::Volume
    }
}

fn volume_slider(ui: &mut egui::Ui, app: &App, width: f32, actions: &mut Vec<Action>) {
    let mut volume = app.settings.volume;
    let slider = ui.add_sized(vec2(width, 20.0), egui::Slider::new(&mut volume, 0.0..=1.0).show_value(false).trailing_fill(true));
    if slider.changed() {
        actions.push(Action::SetVolume(volume));
    }
}

/// Volume, the queue, the lyrics, the sleep timer, from the right edge of `rect` leftwards.
fn tools(ui: &mut egui::Ui, app: &App, rect: Rect, compact: bool, actions: &mut Vec<Action>) {
    let palette = app.palette;
    let state = app.engine.state();
    let looks = &app.settings.looks;
    let mut row = ui.new_child(UiBuilder::new().max_rect(rect).layout(Layout::right_to_left(Align::Center)));
    row.spacing_mut().item_spacing.x = 6.0;
    if looks.shows(PlayerButton::Volume) {
        volume_slider(&mut row, app, if compact { 90.0 } else { 110.0 }, actions);
        if icons::icon_button(&mut row, &palette, speaker_icon(app), 32.0, false, &format!("Volume {:.0}% (M mutes)", app.settings.volume * 100.0)).clicked() {
            actions.push(Action::ToggleMute);
        }
    }
    if looks.shows(PlayerButton::Queue) && icons::icon_button(&mut row, &palette, Icon::Queue, 32.0, app.page == Page::Queue, "Queue").clicked() {
        actions.push(if app.page == Page::Queue { Action::Back } else { Action::Open(Page::Queue) });
    }
    if looks.shows(PlayerButton::Lyrics) && icons::icon_button(&mut row, &palette, Icon::Lyrics, 32.0, app.settings.lyrics_open, "Lyrics (Ctrl+L)").clicked() {
        actions.push(Action::ToggleLyrics);
    }
    if looks.shows(PlayerButton::SleepTimer) {
        sleep_button(&mut row, app, actions);
    }
    if looks.shows(PlayerButton::Devices) && icons::icon_button(&mut row, &palette, Icon::Connect, 32.0, app.page == Page::Connect, "Devices and Connect").clicked() {
        actions.push(Action::Open(Page::Connect));
    }
    if looks.shows(PlayerButton::Speed) {
        speed_button(&mut row, app, actions);
    }
    if looks.shows(PlayerButton::Stop) && icons::icon_button(&mut row, &palette, Icon::Stop, 32.0, false, "Stop").clicked() {
        actions.push(Action::Stop);
    }
    if crate::audio::boost::is_on(app.settings.boost_percent) {
        row.label(RichText::new(crate::audio::boost::times_label(app.settings.boost_percent)).font(theme::bold(11.5)).color(palette.accent))
            .on_hover_text("The volume booster is on. Settings › Sound");
    }
    if !compact {
        if let (Some(format), true) = (&state.format, app.loaded.is_some()) {
            row.label(RichText::new(format).font(theme::regular(11.5)).color(palette.faint));
        }
    }
}

/// A line of progress along an edge, which seeks where it is clicked.
fn hairline(ui: &mut egui::Ui, app: &App, rect: Rect, bar: &Bar, actions: &mut Vec<Action>) {
    let palette = app.palette;
    let hit = rect.expand2(vec2(0.0, 4.0));
    let response = ui.interact(hit, ui.id().with("hairline"), Sense::click());
    let fraction = if bar.duration > 0 { (bar.position as f32 / bar.duration as f32).clamp(0.0, 1.0) } else { 0.0 };
    let thick = if response.hovered() { rect.expand2(vec2(0.0, 1.5)) } else { rect };
    ui.painter().rect_filled(thick, 0, palette.line);
    ui.painter().rect_filled(Rect::from_min_max(thick.min, pos2(thick.left() + thick.width() * fraction, thick.bottom())), 0, palette.accent);
    if bar.duration > 0 {
        if let (true, Some(pointer)) = (response.clicked(), response.interact_pointer_pos()) {
            let at = ((pointer.x - rect.left()) / rect.width()).clamp(0.0, 1.0);
            actions.push(Action::Seek((at as f64 * bar.duration as f64) as i64));
        }
        if let Some(pointer) = response.hover_pos() {
            let at = ((pointer.x - rect.left()) / rect.width()).clamp(0.0, 1.0);
            response.on_hover_text_at_pointer(format_duration((at as f64 * bar.duration as f64) as i64));
        }
    }
}

/// Island: a small pill in the middle with the cover, the track, previous, play and next, the progress
/// along its foot.
fn island(ui: &mut egui::Ui, app: &App, full: Rect, bar: &Bar, actions: &mut Vec<Action>) {
    let palette = app.palette;
    let width = full.width().min(540.0);
    let pill = Rect::from_center_size(full.center(), vec2(width, 64.0));
    let glass = app.settings.looks.surface == SurfaceStyle::Glass;
    ui.painter().rect_filled(pill, 32, if glass { palette.card.gamma_multiply(0.5) } else { palette.card });
    ui.painter().rect_stroke(pill, 32, Stroke::new(1.0_f32, palette.line), egui::StrokeKind::Inside);
    if glass {
        glass_sheen(ui.painter(), pill, 32);
    }
    let inner = pill.shrink2(vec2(12.0, 8.0));
    let controls = Rect::from_min_max(pos2(inner.right() - 130.0, inner.top()), inner.max);
    let track = Rect::from_min_max(inner.min, pos2(controls.left() - 8.0, inner.bottom()));
    track_block(ui, app, track, 44.0, 14.0, false, actions);
    transport(ui, app, controls, false, 40.0, actions);
    let foot = Rect::from_min_max(pos2(pill.left() + 30.0, pill.bottom() - 4.0), pos2(pill.right() - 30.0, pill.bottom() - 2.0));
    hairline(ui, app, foot, bar, actions);
}

/// Display: the track in a stereo's display, glowing letters on a dark glass, with its own small progress.
fn stereo_display(ui: &mut egui::Ui, app: &App, rect: Rect, bar: &Bar, actions: &mut Vec<Action>) {
    let palette = app.palette;
    let painter = ui.painter();
    let glass = egui::Color32::from_rgb(0x0A, 0x10, 0x0C);
    let glow = theme::mix(palette.accent, egui::Color32::WHITE, 0.25);
    painter.rect_filled(rect, theme::corner(6), glass);
    painter.rect_stroke(rect, theme::corner(6), Stroke::new(1.0_f32, palette.line), egui::StrokeKind::Inside);
    let shown = app.loaded.as_ref().or(app.resolving.as_ref()).or(app.queue.current());
    let mono = |size: f32| egui::FontId::monospace(size);
    let clip = painter.with_clip_rect(rect.shrink(6.0));
    let (title, artist) = match shown {
        Some(t) => (t.title.to_uppercase(), t.artist_line().to_uppercase()),
        None => ("NO DISC".to_string(), String::new()),
    };
    // A long title scrolls across the display, as a stereo's does.
    let title_width = painter.layout_no_wrap(title.clone(), mono(16.0), glow).rect.width();
    let room = rect.width() - 24.0;
    let offset = if title_width > room && app.settings.looks.animations {
        let travel = title_width - room + 40.0;
        let t = (ui.input(|i| i.time) as f32 * 30.0) % (travel * 2.0);
        ui.ctx().request_repaint();
        -(if t > travel { travel * 2.0 - t } else { t })
    } else {
        0.0
    };
    clip.text(pos2(rect.left() + 12.0 + offset, rect.top() + 10.0), Align2::LEFT_TOP, &title, mono(16.0), glow);
    clip.text(pos2(rect.left() + 12.0, rect.top() + 32.0), Align2::LEFT_TOP, &artist, mono(12.0), glow.gamma_multiply(0.7));
    let time = format!("{} / {}", format_duration(bar.position), if bar.duration > 0 { format_duration(bar.duration) } else { "-:--".into() });
    clip.text(pos2(rect.right() - 12.0, rect.top() + 32.0), Align2::RIGHT_TOP, time, mono(12.0), glow.gamma_multiply(0.7));
    let foot = Rect::from_min_max(pos2(rect.left() + 12.0, rect.bottom() - 12.0), pos2(rect.right() - 12.0, rect.bottom() - 9.0));
    hairline(ui, app, foot, bar, actions);
}

/// Taskbar: a start button that opens Now playing, the song as a pressed button, and a tray with the
/// volume and the clock.
fn taskbar(ui: &mut egui::Ui, app: &App, full: Rect, bar: &Bar, actions: &mut Vec<Action>) {
    let palette = app.palette;
    let skin = theme::skin();
    let painter = ui.painter().clone();
    let xp = skin == theme::Skin::WindowsXp;
    if xp {
        theme::gradient(&painter, full.expand2(vec2(8.0, 6.0)), theme::rgb(0x3168D5), theme::rgb(0x1941A5), false);
    }
    hairline(ui, app, Rect::from_min_max(pos2(full.left() - 8.0, full.top() - 6.0), pos2(full.right() + 8.0, full.top() - 4.0)), bar, actions);
    // The start button.
    let start = Rect::from_min_size(full.left_top(), vec2(104.0, full.height()));
    let response = ui.interact(start, ui.id().with("start"), Sense::click());
    if let Some(w) = skin.windows98() {
        painter.rect_filled(start, 0, theme::rgb(w.face));
        theme::bevel(&painter, start, !response.is_pointer_button_down_on(), w);
        painter.text(start.center() + vec2(10.0, 0.0), Align2::CENTER_CENTER, "Start", theme::bold(14.0), theme::rgb(w.text));
        start_mark(ui, start);
    } else {
        let green = if xp { theme::rgb(0x3C9A3C) } else { palette.accent };
        let green = if response.hovered() { theme::mix(green, egui::Color32::WHITE, 0.15) } else { green };
        painter.rect_filled(start, theme::corner(if xp { 10 } else { 8 }), green);
        painter.text(start.center() + vec2(10.0, 0.0), Align2::CENTER_CENTER, "start", theme::bold(15.0), if xp { egui::Color32::WHITE } else { palette.on_accent });
        start_mark(ui, start);
    }
    if response.on_hover_text("Now playing").clicked() {
        actions.push(Action::Open(Page::NowPlaying));
    }
    // The tray, at the right.
    let tray = Rect::from_min_max(pos2(full.right() - 230.0, full.top()), full.max);
    if let Some(w) = skin.windows98() {
        painter.rect_filled(tray, 0, theme::rgb(w.face));
        theme::bevel(&painter, tray, false, w);
    } else {
        painter.rect_filled(tray, theme::corner(4), if xp { theme::rgb(0x0F8BF2) } else { palette.card });
    }
    let text_colour = if xp { egui::Color32::WHITE } else if let Some(w) = skin.windows98() { theme::rgb(w.text) } else { palette.text };
    if app.settings.looks.taskbar_clock {
        let clock = chrono::Local::now().format("%H:%M").to_string();
        painter.text(pos2(tray.right() - 10.0, tray.center().y), Align2::RIGHT_CENTER, clock, theme::regular(13.0), text_colour);
        ui.ctx().request_repaint_after(std::time::Duration::from_secs(20));
    }
    let mut tray_ui = ui.new_child(UiBuilder::new().max_rect(tray.shrink2(vec2(56.0, 0.0)).translate(vec2(-46.0, 0.0))).layout(Layout::right_to_left(Align::Center)));
    volume_slider(&mut tray_ui, app, 80.0, actions);
    if icons::icon_button(&mut tray_ui, &palette, speaker_icon(app), 26.0, false, "Mute (M)").clicked() {
        actions.push(Action::ToggleMute);
    }
    // The song, as the window in use: a pressed button, with previous, play and next beside it.
    let task = Rect::from_min_max(pos2(start.right() + 10.0, full.top()), pos2((start.right() + 330.0).min(tray.left() - 130.0), full.bottom()));
    let held = ui.interact(task, ui.id().with("task"), Sense::click());
    if let Some(w) = skin.windows98() {
        painter.rect_filled(task, 0, theme::rgb(w.light));
        theme::bevel(&painter, task, false, w);
    } else {
        painter.rect_filled(task, theme::corner(4), if xp { theme::rgb(0x1E52B7) } else { palette.raised });
    }
    let shown = app.loaded.as_ref().or(app.resolving.as_ref()).or(app.queue.current());
    if let Some(track) = shown {
        let cover = Rect::from_min_size(task.left_top() + vec2(6.0, (task.height() - 24.0) / 2.0), Vec2::splat(24.0));
        widgets::paint_artwork(ui, &palette, track.artwork_url.as_deref(), cover, 3);
        let clip = painter.with_clip_rect(task.shrink(2.0));
        clip.text(pos2(cover.right() + 8.0, task.center().y), Align2::LEFT_CENTER, format!("{} - {}", track.title, track.artist_line()), theme::regular(13.0), text_colour);
    }
    if held.on_hover_text("Play or pause").clicked() {
        actions.push(Action::TogglePlay);
    }
    let controls = Rect::from_min_max(pos2(task.right() + 6.0, full.top()), pos2(tray.left() - 6.0, full.bottom()));
    let mut row = ui.new_child(UiBuilder::new().max_rect(controls).layout(Layout::left_to_right(Align::Center)));
    row.spacing_mut().item_spacing.x = 2.0;
    let playing = app.engine.state().status == Status::Playing;
    for (icon, action, hint) in [
        (Icon::Previous, Action::Previous, "Previous"),
        (if playing { Icon::Pause } else { Icon::Play }, Action::TogglePlay, "Play or pause"),
        (Icon::Next, Action::Next, "Next"),
    ] {
        if icons::flat_button(&mut row, &palette, icon, false, hint).clicked() {
            actions.push(action);
        }
    }
}

/// Noctorium's mark on the start button, where Windows had its flag.
fn start_mark(ui: &egui::Ui, start: Rect) {
    let side = 20.0;
    let at = Rect::from_min_size(pos2(start.left() + 10.0, start.center().y - side / 2.0), Vec2::splat(side));
    egui::Image::from_bytes("bytes://noctorium-mark.png", widgets::MARK_PNG).corner_radius(5).paint_at(ui, at);
}

/// Toolbar: a row of flat buttons as a desktop music player has them; the second has more of them, and play
/// and pause as one.
#[allow(clippy::too_many_arguments)]
fn toolbar(ui: &mut egui::Ui, app: &App, full: Rect, bar: &Bar, more: bool, inputs: &mut Inputs, actions: &mut Vec<Action>) {
    let palette = app.palette;
    let shown = app.loaded.as_ref().or(app.resolving.as_ref()).or(app.queue.current()).cloned();
    let playing = app.engine.state().status == Status::Playing;
    let left_count = if more { 4.0 } else { 6.0 };
    let left = Rect::from_min_size(full.left_top(), vec2(left_count * 32.0 + 4.0, full.height()));
    let mut row = ui.new_child(UiBuilder::new().max_rect(left).layout(Layout::left_to_right(Align::Center)));
    row.spacing_mut().item_spacing.x = 2.0;
    if more {
        let liked = shown.as_ref().is_some_and(|t| app.library.is_liked(t));
        if icons::flat_button(&mut row, &palette, if liked { Icon::HeartFilled } else { Icon::Heart }, liked, "Like").clicked() {
            if let Some(track) = shown.clone() {
                actions.push(Action::ToggleLike(track));
            }
        }
        if icons::flat_button(&mut row, &palette, Icon::Previous, false, "Previous").clicked() {
            actions.push(Action::Previous);
        }
        if icons::flat_button(&mut row, &palette, if playing { Icon::Pause } else { Icon::Play }, false, "Play or pause (Space)").clicked() {
            actions.push(Action::TogglePlay);
        }
        if icons::flat_button(&mut row, &palette, Icon::Next, false, "Next").clicked() {
            actions.push(Action::Next);
        }
    } else {
        let add = icons::flat_button(&mut row, &palette, Icon::Plus, false, "Add to a playlist");
        egui::Popup::menu(&add).show(|ui| {
            ui.set_min_width(180.0);
            let Some(track) = shown.clone() else {
                ui.label("Nothing playing");
                return;
            };
            if ui.button("New playlist").clicked() {
                actions.push(Action::AddToNewPlaylist(track.clone()));
                ui.close();
            }
            for playlist in &app.library.playlists {
                if ui.button(&playlist.title).clicked() {
                    actions.push(Action::AddToPlaylist { id: playlist.id.clone(), track: track.clone() });
                    ui.close();
                }
            }
        });
        for (icon, action, hint, on) in [
            (Icon::Previous, Action::Previous, "Previous", false),
            (Icon::Play, Action::TogglePlay, "Play", playing),
            (Icon::Pause, Action::TogglePlay, "Pause", !playing && app.loaded.is_some()),
            (Icon::Stop, Action::Stop, "Stop", false),
            (Icon::Next, Action::Next, "Next", false),
        ] {
            let clicked = icons::flat_button(&mut row, &palette, icon, on, hint).clicked();
            // Play only plays and Pause only pauses, as on the players these buttons come from.
            let wanted = match icon {
                Icon::Play => !playing,
                Icon::Pause => playing,
                _ => true,
            };
            if clicked && wanted {
                actions.push(action);
            }
        }
    }
    let right_width = if more { 9.0 * 32.0 + 100.0 } else { 3.0 * 32.0 + 100.0 };
    let right = Rect::from_min_max(pos2(full.right() - right_width, full.top()), full.max);
    let mut tools_row = ui.new_child(UiBuilder::new().max_rect(right).layout(Layout::right_to_left(Align::Center)));
    tools_row.spacing_mut().item_spacing.x = 2.0;
    volume_slider(&mut tools_row, app, 90.0, actions);
    if icons::flat_button(&mut tools_row, &palette, speaker_icon(app), app.settings.muted, "Mute (M)").clicked() {
        actions.push(Action::ToggleMute);
    }
    if more {
        let speeds = [0.75_f32, 1.0, 1.25, 1.5, 2.0];
        let now = app.settings.speed;
        let label = format!("{now:.2}×").replace(".00×", "×");
        if tools_row.add_sized(vec2(50.0, 26.0), egui::Button::new(RichText::new(label).font(theme::regular(12.0))).frame(false)).on_hover_text("Speed").clicked() {
            let next = speeds.iter().copied().find(|s| *s > now + 0.01).unwrap_or(speeds[0]);
            actions.push(Action::SetSpeed(next));
        }
        if icons::flat_button(&mut tools_row, &palette, Icon::Connect, app.page == Page::Connect, "Connect").clicked() {
            actions.push(Action::Open(Page::Connect));
        }
        sleep_button(&mut tools_row, app, actions);
    }
    if icons::flat_button(&mut tools_row, &palette, Icon::Shuffle, app.queue.shuffle, "Shuffle (Ctrl+S)").clicked() {
        actions.push(Action::ToggleShuffle);
    }
    let (repeat, hint) = repeat_icon(app);
    if icons::flat_button(&mut tools_row, &palette, repeat, app.queue.repeat != RepeatMode::Off, hint).clicked() {
        actions.push(Action::CycleRepeat);
    }
    if more {
        if icons::flat_button(&mut tools_row, &palette, Icon::Queue, app.page == Page::Queue, "Queue").clicked() {
            actions.push(if app.page == Page::Queue { Action::Back } else { Action::Open(Page::Queue) });
        }
        if icons::flat_button(&mut tools_row, &palette, Icon::Lyrics, app.settings.lyrics_open, "Lyrics (Ctrl+L)").clicked() {
            actions.push(Action::ToggleLyrics);
        }
    }
    // The song's name, then the slider with the times either side of it.
    let middle = Rect::from_min_max(pos2(left.right() + 8.0, full.top()), pos2(right.left() - 8.0, full.bottom()));
    let name_width = (middle.width() * 0.32).clamp(80.0, 260.0);
    if let Some(track) = &shown {
        let clip = ui.painter().with_clip_rect(Rect::from_min_size(middle.min, vec2(name_width, middle.height())));
        clip.text(pos2(middle.left(), middle.center().y - 7.0), Align2::LEFT_CENTER, &track.title, theme::bold(12.5), palette.text);
        clip.text(pos2(middle.left(), middle.center().y + 8.0), Align2::LEFT_CENTER, track.artist_line(), theme::regular(11.5), palette.subtext);
    }
    let seek = Rect::from_min_max(pos2(middle.left() + name_width + 48.0, middle.center().y - 8.0), pos2(middle.right() - 44.0, middle.center().y + 8.0));
    if seek.width() > 40.0 {
        seek_bar(ui, app, seek, bar.position, bar.duration, inputs, actions);
    }
}

pub(super) fn seek_bar(
    ui: &mut egui::Ui,
    app: &App,
    bar: Rect,
    position: i64,
    duration: i64,
    inputs: &mut Inputs,
    actions: &mut Vec<Action>,
) {
    let palette = app.palette;
    let track = Rect::from_min_max(pos2(bar.left(), bar.center().y - 2.0), pos2(bar.right(), bar.center().y + 2.0));
    let response = ui.interact(bar, ui.id().with("seek"), Sense::click_and_drag());
    let fraction_at = |x: f32| ((x - track.left()) / track.width()).clamp(0.0, 1.0);
    if duration > 0 && (response.dragged() || response.drag_started()) {
        if let Some(pointer) = response.interact_pointer_pos() {
            inputs.scrubbing = Some(fraction_at(pointer.x));
        }
    }
    if duration > 0 && (response.drag_stopped() || response.clicked()) {
        let fraction = response
            .interact_pointer_pos()
            .map(|p| fraction_at(p.x))
            .or(inputs.scrubbing)
            .unwrap_or(0.0);
        actions.push(Action::Seek((fraction as f64 * duration as f64) as i64));
        inputs.scrubbing = None;
    }
    let shown = match inputs.scrubbing {
        Some(fraction) => (fraction as f64 * duration as f64) as i64,
        None => position,
    };
    let fraction = if duration > 0 { (shown as f32 / duration as f32).clamp(0.0, 1.0) } else { 0.0 };
    let hot = response.hovered() || inputs.scrubbing.is_some();
    let moving = app.engine.state().status == Status::Playing && app.settings.looks.animations;
    paint_progress(ui, app, track, fraction, duration, hot && duration > 0, moving);
    let painter = ui.painter();
    painter.text(pos2(bar.left() - 10.0, bar.center().y), Align2::RIGHT_CENTER, format_duration(shown), theme::regular(12.0), palette.subtext);
    painter.text(
        pos2(bar.right() + 10.0, bar.center().y),
        Align2::LEFT_CENTER,
        match (duration > 0, app.settings.looks.time_display) {
            (false, _) => "–:––".into(),
            (true, crate::looks::TimeDisplay::Total) => format_duration(duration),
            (true, crate::looks::TimeDisplay::Remaining) => format!("-{}", format_duration((duration - shown).max(0))),
        },
        theme::regular(12.0),
        palette.subtext,
    );
    if let (Some(pointer), true) = (response.hover_pos(), duration > 0) {
        let at = (fraction_at(pointer.x) as f64 * duration as f64) as i64;
        response.on_hover_text_at_pointer(format_duration(at));
    }
}

/// How far through the song, drawn in the style chosen in Settings › Personalisation.
fn paint_progress(ui: &egui::Ui, app: &App, track: Rect, fraction: f32, duration: i64, hot: bool, moving: bool) {
    let palette = app.palette;
    let painter = ui.painter();
    let played_x = track.left() + track.width() * fraction;
    let cy = track.center().y;
    let thin = |height: f32| Rect::from_min_max(pos2(track.left(), cy - height / 2.0), pos2(track.right(), cy + height / 2.0));
    let played = |rect: Rect| Rect::from_min_max(rect.min, pos2(played_x, rect.bottom()));
    // What is still to play, plain enough to see on a pale theme too.
    let unlit = theme::mix(palette.line, palette.subtext, 0.3);
    match app.settings.looks.progress {
        ProgressBarStyle::Minimal => {
            let rail = thin(if hot { 6.0 } else { 4.0 });
            painter.rect_filled(rail, CornerRadius::same(theme::corner(2)), palette.line);
            painter.rect_filled(played(rail), CornerRadius::same(theme::corner(2)), if hot { palette.accent } else { palette.text });
            if hot {
                painter.circle_filled(pos2(played_x, cy), 6.0, palette.text);
            }
        }
        ProgressBarStyle::Material => {
            let rail = thin(6.0);
            painter.rect_filled(rail, CornerRadius::same(theme::corner(3)), palette.wash(0.25));
            painter.rect_filled(played(rail), CornerRadius::same(theme::corner(3)), palette.accent);
            painter.circle_filled(pos2(played_x, cy), if hot { 8.0 } else { 7.0 }, palette.accent);
        }
        ProgressBarStyle::Wave => {
            painter.line_segment([pos2(played_x, cy), pos2(track.right(), cy)], Stroke::new(2.0_f32, palette.line));
            let phase = if moving { ui.input(|i| i.time) as f32 * 5.0 } else { 0.0 };
            let mut points = Vec::new();
            let mut x = track.left();
            while x < played_x {
                points.push(pos2(x, cy + ((x - track.left()) * 0.22 - phase).sin() * 3.5));
                x += 2.0;
            }
            points.push(pos2(played_x, cy));
            if points.len() > 1 {
                painter.add(egui::Shape::line(points, Stroke::new(2.5_f32, palette.accent)));
            }
            painter.circle_filled(pos2(played_x, cy), if hot { 7.0 } else { 5.0 }, palette.accent);
            if moving {
                ui.ctx().request_repaint();
            }
        }
        ProgressBarStyle::Segments => {
            let mut x = track.left();
            while x < track.right() {
                let end = (x + 7.0).min(track.right());
                let lit = (x + end) / 2.0 <= played_x;
                let dash = Rect::from_min_max(pos2(x, cy - 2.5), pos2(end, cy + 2.5));
                painter.rect_filled(dash, CornerRadius::same(theme::corner(1)), if lit { palette.accent } else { unlit });
                x += 10.0;
            }
        }
        ProgressBarStyle::Capsule => {
            let rail = thin(12.0);
            painter.rect_filled(rail, CornerRadius::same(6), palette.line);
            if played_x > rail.left() + 1.0 {
                painter.rect_filled(played(rail), CornerRadius::same(6), palette.accent);
            }
            if hot {
                painter.rect_stroke(rail, CornerRadius::same(6), Stroke::new(1.0_f32, palette.subtext), egui::StrokeKind::Outside);
            }
        }
        ProgressBarStyle::Bars => {
            let mut x = track.left();
            let mut i = 0u32;
            while x < track.right() {
                // The same heights each time, like a waveform drawn once for the song.
                let noise = ((i as f32 * 12.9898).sin() * 43758.547).fract().abs();
                let height = 4.0 + noise * 12.0;
                let bar = Rect::from_min_max(pos2(x, cy - height / 2.0), pos2(x + 2.5, cy + height / 2.0));
                painter.rect_filled(bar, CornerRadius::same(1), if x <= played_x { palette.accent } else { unlit });
                x += 4.5;
                i += 1;
            }
        }
        ProgressBarStyle::Neon => {
            painter.line_segment([pos2(track.left(), cy), pos2(track.right(), cy)], Stroke::new(1.0_f32, palette.line));
            for (width, alpha) in [(12.0_f32, 0.08_f32), (7.0, 0.16), (4.0, 0.35)] {
                painter.line_segment([pos2(track.left(), cy), pos2(played_x, cy)], Stroke::new(width, palette.accent.gamma_multiply(alpha)));
            }
            painter.line_segment([pos2(track.left(), cy), pos2(played_x, cy)], Stroke::new(2.0_f32, theme::mix(palette.accent, egui::Color32::WHITE, 0.6)));
            painter.circle_filled(pos2(played_x, cy), 4.0, egui::Color32::WHITE);
        }
        ProgressBarStyle::Beads => {
            let step = 11.0;
            let count = ((track.width() / step).floor() as usize).max(2);
            let gap = track.width() / (count - 1) as f32;
            let reached = (fraction * (count - 1) as f32).round() as usize;
            for i in 0..count {
                let centre = pos2(track.left() + i as f32 * gap, cy);
                if i == reached && duration > 0 {
                    painter.circle_filled(centre, 5.5, palette.accent);
                } else if i < reached {
                    painter.circle_filled(centre, 3.0, palette.accent);
                } else {
                    painter.circle_stroke(centre, 2.6, Stroke::new(1.2_f32, unlit));
                }
            }
        }
        ProgressBarStyle::Ruler => {
            painter.line_segment([pos2(track.left(), cy + 4.0), pos2(track.right(), cy + 4.0)], Stroke::new(1.0_f32, unlit));
            if duration > 0 {
                // A tick every ten seconds, a longer one each minute; fewer when the song is long.
                let seconds = duration as f32 / 1000.0;
                let every = if seconds > 900.0 { 60.0 } else if seconds > 420.0 { 30.0 } else { 10.0 };
                let mut t = 0.0;
                while t <= seconds {
                    let x = track.left() + track.width() * (t / seconds);
                    let minute = (t % 60.0) < 0.5;
                    let height = if minute { 9.0 } else { 4.0 };
                    let colour = if x <= played_x { palette.accent } else { unlit };
                    painter.line_segment([pos2(x, cy + 4.0), pos2(x, cy + 4.0 - height)], Stroke::new(1.0_f32, colour));
                    t += every;
                }
            }
            painter.add(egui::Shape::convex_polygon(
                vec![pos2(played_x - 5.0, cy - 9.0), pos2(played_x + 5.0, cy - 9.0), pos2(played_x, cy - 3.0)],
                if hot { palette.accent } else { palette.text },
                Stroke::NONE,
            ));
        }
        ProgressBarStyle::Luna => {
            let well = thin(10.0);
            painter.rect_filled(well, 3, egui::Color32::WHITE);
            painter.rect_stroke(well, 3, Stroke::new(1.0_f32, theme::rgb(0x7F9DB9)), egui::StrokeKind::Inside);
            let inside = well.shrink(2.0);
            let mut x = inside.left();
            while x + 6.0 <= inside.left() + inside.width() * fraction {
                let block = Rect::from_min_max(pos2(x, inside.top()), pos2(x + 6.0, inside.bottom()));
                theme::gradient(painter, Rect::from_min_max(block.min, pos2(block.right(), block.center().y)), theme::rgb(0xACEDAD), theme::rgb(0x2ED330), false);
                theme::gradient(painter, Rect::from_min_max(pos2(block.left(), block.center().y), block.max), theme::rgb(0x2ED330), theme::rgb(0xACEDAD), false);
                x += 8.0;
            }
            // Its pointed handle.
            let hx = played_x.clamp(track.left() + 5.0, track.right() - 5.0);
            let handle = vec![pos2(hx - 5.0, cy - 9.0), pos2(hx + 5.0, cy - 9.0), pos2(hx + 5.0, cy + 4.0), pos2(hx, cy + 9.0), pos2(hx - 5.0, cy + 4.0)];
            painter.add(egui::Shape::convex_polygon(handle, theme::rgb(0xF4F3EE), Stroke::new(1.0_f32, theme::rgb(if hot { 0xF8B636 } else { 0x7F9DB9 }))));
        }
        ProgressBarStyle::Slider => {
            let groove = thin(3.0);
            painter.rect_filled(groove, 1, unlit);
            painter.rect_filled(played(groove), 1, palette.accent);
            painter.circle_filled(pos2(played_x, cy), if hot { 7.0 } else { 6.0 }, palette.text);
            painter.circle_stroke(pos2(played_x, cy), if hot { 7.0 } else { 6.0 }, Stroke::new(1.0_f32, palette.line));
        }
        ProgressBarStyle::Classic => {
            let colours = theme::skin().windows98().copied().unwrap_or(theme::WINDOWS_98);
            let groove = thin(6.0);
            painter.rect_filled(groove, 0, palette.page);
            theme::bevel(painter, groove.expand(1.0), false, &colours);
            let thumb = Rect::from_center_size(pos2(played_x.clamp(track.left() + 5.0, track.right() - 5.0), cy), vec2(10.0, 18.0));
            painter.rect_filled(thumb, 0, theme::rgb(colours.face));
            theme::bevel(painter, thumb, true, &colours);
        }
    }
}

pub fn lyrics_panel(app: &App, ctx: &egui::Context, actions: &mut Vec<Action>) {
    let palette = app.palette;
    egui::SidePanel::right("lyrics")
        .resizable(true)
        .default_width(360.0)
        .width_range(260.0..=560.0)
        .show_separator_line(false)
        .frame(
            egui::Frame::new()
                .fill(palette.panel.gamma_multiply(0.7))
                .inner_margin(egui::Margin::symmetric(18, 14)),
        )
        .show(ctx, |ui| {
            ui.horizontal(|ui| {
                ui.label(RichText::new("Lyrics").font(theme::bold(18.0)).color(palette.text));
                ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                    if icon_button(ui, &palette, "✕", 28.0, false, "Close lyrics (Ctrl+L)").clicked() {
                        actions.push(Action::ToggleLyrics);
                    }
                });
            });
            ui.add_space(8.0);
            let Some(track) = app.queue.current() else {
                ui.label(RichText::new("Play something to see its lyrics.").color(palette.faint));
                return;
            };
            match app.lyrics.get(&track.queue_key()) {
                None | Some(Loadable::Loading) => widgets::loading(ui, &palette, "Looking for the lyrics…"),
                Some(Loadable::Failed(why)) => widgets::notice(ui, &palette, why, true),
                Some(Loadable::Ready(None)) => {
                    ui.label(RichText::new("LRCLIB has no lyrics for this song.").color(palette.faint));
                }
                Some(Loadable::Ready(Some(lyrics))) => lyric_lines(ui, app, lyrics, false, actions),
            }
        });
}

/// The lyrics, the line being sung lit and followed. `centred_here` is how they sit when the listener left
/// the alignment as laid out: centred where they have the page to themselves.
pub(super) fn lyric_lines(ui: &mut egui::Ui, app: &App, lyrics: &Lyrics, centred_here: bool, actions: &mut Vec<Action>) {
    let palette = app.palette;
    let scale = app.settings.looks.lyrics_size.scale();
    let centred = match app.settings.looks.lyrics_alignment {
        LyricsAlignment::Auto => centred_here,
        LyricsAlignment::Start => false,
        LyricsAlignment::Centre => true,
    };
    let align = if centred { Align::Center } else { Align::Min };
    lyrics_sources(ui, app, lyrics, actions);
    if lyrics.lines.is_empty() {
        if let Some(link) = &lyrics.link {
            ui.label(RichText::new(format!("{} has these lyrics on its own page.", lyrics.provider.display_name())).color(palette.subtext));
            if ui.button(format!("Open on {}", lyrics.provider.display_name())).clicked() {
                actions.push(Action::OpenUrl(link.clone()));
            }
        }
        return;
    }
    let dim = app.settings.looks.dim_lyrics;
    let state = app.engine.state();
    let playing_this = app.loaded.is_some();
    let current = if playing_this { lyrics.current_line(state.position_ms + 250) } else { None };
    // Scrolled to only when the line changes, so the listener can still scroll the lyrics by hand.
    let followed = egui::Id::new("lyrics-followed-line");
    let moved = ui.data(|d| d.get_temp::<Option<usize>>(followed)) != Some(current);
    if moved {
        ui.data_mut(|d| d.insert_temp(followed, current));
    }
    egui::ScrollArea::vertical().auto_shrink([false, false]).show(ui, |ui| {
        ui.with_layout(Layout::top_down(align), |ui| {
        ui.add_space(ui.available_height() * 0.2);
        for (index, line) in lyrics.lines.iter().enumerate() {
            let is_current = current == Some(index);
            let past = current.is_some_and(|c| index < c);
            let colour = if is_current || !dim {
                palette.text
            } else if past {
                palette.faint
            } else if lyrics.synced {
                palette.subtext
            } else {
                palette.text
            };
            let size = if is_current { 21.0 } else { 18.0 } * scale;
            let response = ui.add(
                egui::Label::new(RichText::new(&line.text).font(theme::bold(size)).color(colour))
                    .wrap()
                    .halign(align)
                    .sense(if line.start_ms.is_some() { Sense::click() } else { Sense::hover() }),
            );
            if let Some(start) = line.start_ms {
                if response.clicked() {
                    actions.push(Action::Seek(start));
                }
                if response.hovered() {
                    ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
                }
            }
            if is_current && moved {
                response.scroll_to_me(Some(Align::Center));
            }
            ui.add_space(10.0);
        }
        ui.add_space(20.0);
        ui.label(RichText::new(&lyrics.source).font(theme::regular(11.5)).color(palette.faint));
        ui.add_space(ui.available_height() * 0.3);
        });
    });
}

/// Which source the lyrics are from, and the others that had them, to switch to.
fn lyrics_sources(ui: &mut egui::Ui, app: &App, lyrics: &Lyrics, actions: &mut Vec<Action>) {
    use crate::lyrics::Status;
    let palette = app.palette;
    let usable: Vec<_> = lyrics.outcomes.iter().filter(|o| matches!(o.status, Status::Found | Status::LinkOnly)).collect();
    if usable.len() < 2 {
        return;
    }
    ui.horizontal_wrapped(|ui| {
        ui.spacing_mut().item_spacing.x = 4.0;
        for outcome in &lyrics.outcomes {
            let name = outcome.provider.display_name();
            match outcome.status {
                Status::Found | Status::LinkOnly => {
                    let synced = outcome.lyrics.as_ref().is_some_and(|l| l.synced);
                    let label = if synced { format!("{name} ⏱") } else { name.to_string() };
                    let chosen = outcome.provider == lyrics.provider;
                    let hint = match outcome.status {
                        Status::LinkOnly => "A link to the lyrics on its own page",
                        _ if synced => "Synced lyrics",
                        _ => "Plain lyrics",
                    };
                    if ui.selectable_label(chosen, RichText::new(label).font(theme::regular(12.0))).on_hover_text(hint).clicked() && !chosen {
                        actions.push(Action::ChooseLyrics(outcome.provider));
                    }
                }
                _ => {
                    let why = match outcome.status {
                        Status::NeedsKey => format!("Needs a key in {}", outcome.detail),
                        Status::Error => format!("Did not answer: {}", outcome.detail),
                        _ => "Has no lyrics for this song".to_string(),
                    };
                    ui.label(RichText::new(name).font(theme::regular(12.0)).color(palette.faint)).on_hover_text(why);
                }
            }
        }
    });
    ui.add_space(4.0);
}

/// The moon in the player bar: the sleep timer, with what is left of it beside it while it runs.
fn sleep_button(ui: &mut egui::Ui, app: &App, actions: &mut Vec<Action>) {
    let palette = app.palette;
    let label = app.sleep_label();
    let hint = match &label {
        Some(l) => format!("Sleep timer: {l} left"),
        None => "Sleep timer".to_string(),
    };
    let button = icons::icon_button(ui, &palette, Icon::Moon, 32.0, label.is_some(), &hint);
    if let Some(label) = &label {
        ui.label(RichText::new(label).font(theme::bold(11.5)).color(palette.accent));
    }
    egui::Popup::menu(&button).show(|ui| {
        ui.set_min_width(190.0);
        ui.label(RichText::new("Pause the music in").color(palette.subtext));
        for minutes in super::SLEEP_PRESETS {
            if ui.button(format!("{minutes} minutes")).clicked() {
                actions.push(Action::SetSleep(Some(minutes)));
                ui.close();
            }
        }
        if ui.button("At the end of this song").clicked() {
            actions.push(Action::SleepAtEndOfSong);
            ui.close();
        }
        if app.sleep.is_some() {
            ui.separator();
            if ui.button("Turn the timer off").clicked() {
                actions.push(Action::SetSleep(None));
                ui.close();
            }
        }
    });
}
