//! The pieces drawn the same way everywhere: covers, song rows and their menus, cards, the light behind the
//! page, and the little messages in the corner.

use super::{Action, App};
use crate::domain::{format_duration, ProviderType, Track};
use crate::theme::{self, Glow, GlowPlacement, Palette};
use eframe::egui::{
    self, pos2, vec2, Align, Align2, Color32, CornerRadius, Id, Layout, Mesh, Rect, RichText, Sense, Stroke,
    StrokeKind, Ui, UiBuilder, Vec2,
};

/// Noctorium's mark, the same picture the Kotlin desktop and the phone carry.
pub const MARK_PNG: &[u8] = include_bytes!("../../assets/noctorium-mark.png");

/// The mark at `side` pixels with its rounded corners cut out, as the Kotlin desktop's `AppIcon` draws it:
/// halved step by step on the way down so the fine artwork averages rather than speckles, then clipped to a
/// rounded square whose corners are truly transparent.
pub fn mark_rgba(side: u32) -> Vec<u8> {
    let Ok(decoded) = image::load_from_memory(MARK_PNG) else { return vec![0; (side * side * 4) as usize] };
    let mut current = decoded.to_rgba8();
    while current.width() / 2 > side {
        let half = current.width() / 2;
        current = image::imageops::resize(&current, half, half, image::imageops::FilterType::Triangle);
    }
    let mut out = image::imageops::resize(&current, side, side, image::imageops::FilterType::Triangle);
    // The same corner as the Kotlin mark: twelve thirty-sixths of the side.
    let radius = side as f32 * 12.0 / 36.0;
    let size = side as f32;
    for (x, y, pixel) in out.enumerate_pixels_mut() {
        let (px, py) = (x as f32 + 0.5, y as f32 + 0.5);
        let cx = px.clamp(radius, size - radius);
        let cy = py.clamp(radius, size - radius);
        let distance = ((px - cx).powi(2) + (py - cy).powi(2)).sqrt();
        // A pixel's worth of soft edge, so the corner is smooth rather than stepped.
        let coverage = (radius - distance + 0.5).clamp(0.0, 1.0);
        pixel.0[3] = (pixel.0[3] as f32 * coverage) as u8;
    }
    out.into_raw()
}

/// The window's icon, for the title bar, the taskbar, Alt+Tab and the tray.
pub fn app_icon() -> egui::IconData {
    static ICON: std::sync::OnceLock<Vec<u8>> = std::sync::OnceLock::new();
    let side = 256u32;
    egui::IconData { rgba: ICON.get_or_init(|| mark_rgba(side)).clone(), width: side, height: side }
}

/// The mark drawn in the window, beside the name.
pub fn mark(ui: &mut Ui, side: f32) -> egui::Response {
    ui.add(
        egui::Image::from_bytes("bytes://noctorium-mark.png", MARK_PNG)
            .fit_to_exact_size(Vec2::splat(side))
            .corner_radius((side * 12.0 / 36.0) as u8),
    )
}

/// The page, and the light across it for the Glow themes. Painted before anything else, under the panels,
/// which are see-through enough to let it show.
pub fn paint_background(ctx: &egui::Context, palette: &Palette, glow: Option<(Color32, Glow)>) {
    let painter = ctx.layer_painter(egui::LayerId::background());
    let screen = ctx.screen_rect();
    painter.rect_filled(screen, 0.0, palette.page);
    let Some((colour, glow)) = glow else { return };
    // The light sits over the page, and the rail and the player let it through, a little dimmed.
    match glow.placement {
        GlowPlacement::Artwork => {
            // Rising from the cover in the player's corner, and a wider wash along the bottom of the page.
            let strength = (glow.strength * 1.6).min(0.7);
            radial(&painter, pos2(screen.left() + 180.0, screen.bottom() - 60.0), vec2(760.0, 640.0), colour, strength);
            radial(&painter, pos2(screen.center().x + 120.0, screen.bottom() + 60.0), vec2(screen.width() * 0.75, 380.0), colour, strength * 0.55);
        }
        GlowPlacement::Top => {
            let strength = (glow.strength * 1.8).min(0.6);
            radial(&painter, pos2(screen.center().x, screen.top() - 60.0), vec2(screen.width() * 0.8, screen.height() * 0.6), colour, strength);
        }
    }
}

/// An elliptical light: `colour` at `strength` in the middle, nothing at the edge, falling off smoothly.
pub fn radial(painter: &egui::Painter, centre: egui::Pos2, radius: Vec2, colour: Color32, strength: f32) {
    const RINGS: usize = 14;
    const SEGMENTS: usize = 72;
    let mut mesh = Mesh::default();
    let tint = |t: f32| {
        let alpha = strength * (1.0 - t).powf(2.2);
        Color32::from_rgba_unmultiplied(colour.r(), colour.g(), colour.b(), (alpha * 255.0) as u8)
    };
    mesh.colored_vertex(centre, tint(0.0));
    for ring in 1..=RINGS {
        let t = ring as f32 / RINGS as f32;
        for segment in 0..SEGMENTS {
            let angle = segment as f32 / SEGMENTS as f32 * std::f32::consts::TAU;
            let point = centre + vec2(angle.cos() * radius.x * t, angle.sin() * radius.y * t);
            mesh.colored_vertex(point, tint(t));
        }
    }
    let at = |ring: usize, segment: usize| (1 + (ring - 1) * SEGMENTS + segment % SEGMENTS) as u32;
    for segment in 0..SEGMENTS {
        mesh.add_triangle(0, at(1, segment), at(1, segment + 1));
    }
    for ring in 1..RINGS {
        for segment in 0..SEGMENTS {
            let (a, b) = (at(ring, segment), at(ring, segment + 1));
            let (c, d) = (at(ring + 1, segment), at(ring + 1, segment + 1));
            mesh.add_triangle(a, c, b);
            mesh.add_triangle(b, c, d);
        }
    }
    painter.add(mesh);
}

/// The part of a cover to show square. YouTube's thumbnails are letterboxed: a 4:3 frame around a 16:9 one
/// around, for a song, a square cover; the middle square is the cover.
pub fn square_uv(url: &str) -> Rect {
    let lower = url.to_lowercase();
    if lower.contains("ytimg.com/vi") {
        if lower.contains("hqdefault") || lower.contains("sddefault") || lower.ends_with("/default.jpg") {
            return Rect::from_min_max(pos2(0.21875, 0.125), pos2(0.78125, 0.875));
        }
        if lower.contains("mqdefault") || lower.contains("maxresdefault") || lower.contains("hq720") {
            return Rect::from_min_max(pos2(0.21875, 0.0), pos2(0.78125, 1.0));
        }
    }
    Rect::from_min_max(pos2(0.0, 0.0), pos2(1.0, 1.0))
}

/// A cover at `size`, or a note on a card where there is none.
pub fn artwork(ui: &mut Ui, palette: &Palette, url: Option<&str>, size: f32, radius: u8) -> egui::Response {
    let (rect, response) = ui.allocate_exact_size(Vec2::splat(size), Sense::click());
    paint_artwork(ui, palette, url, rect, radius);
    response
}

pub fn paint_artwork(ui: &Ui, palette: &Palette, url: Option<&str>, rect: Rect, radius: u8) {
    paint_artwork_exact(ui, palette, url, rect, theme::corner(radius));
}

/// A cover with exactly these corners, whatever the corner style: a round cover stays round.
pub fn paint_artwork_exact(ui: &Ui, palette: &Palette, url: Option<&str>, rect: Rect, radius: u8) {
    ui.painter().rect_filled(rect, CornerRadius::same(radius), palette.card);
    match url {
        Some(url) => {
            egui::Image::new(url.to_string())
                .uv(square_uv(url))
                .corner_radius(radius)
                .show_loading_spinner(false)
                .paint_at(ui, rect);
        }
        None => {
            ui.painter().text(
                rect.center(),
                Align2::CENTER_CENTER,
                "♪",
                theme::regular(rect.height() * 0.42),
                palette.faint,
            );
        }
    }
}

/// Whether badges are the services' marks rather than their letters; set each frame from the settings.
pub static BADGE_MARKS: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(true);

/// A service's mark as an SVG, made once for each colour it is drawn in.
fn mark_svg(provider: ProviderType, fill: &str) -> (String, &'static [u8]) {
    use std::collections::HashMap;
    use std::sync::{Mutex, OnceLock};
    static MADE: OnceLock<Mutex<HashMap<String, &'static [u8]>>> = OnceLock::new();
    let key = format!("bytes://mark-{}-{}.svg", provider.key_name(), fill.trim_start_matches('#'));
    let mut made = MADE.get_or_init(Default::default).lock().unwrap_or_else(|e| e.into_inner());
    let bytes = *made.entry(key.clone()).or_insert_with(|| {
        let svg = format!(
            r#"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 {v} {v}" width="48" height="48"><path fill="{fill}" d="{d}"/></svg>"#,
            v = crate::marks::VIEWPORT,
            d = crate::marks::path(provider),
        );
        Box::leak(svg.into_bytes().into_boxed_slice())
    });
    (key, bytes)
}

/// Which service a song is from: its mark, or its two or three letters, in the service's colour.
pub fn badge(ui: &mut Ui, provider: ProviderType) {
    if BADGE_MARKS.load(std::sync::atomic::Ordering::Relaxed) {
        // Tidal's mark is black, which would vanish on a dark theme: it takes the writing's colour instead.
        let text = ui.visuals().text_color();
        let fill = if provider == ProviderType::Tidal {
            format!("#{:02X}{:02X}{:02X}", text.r(), text.g(), text.b())
        } else {
            crate::marks::colour(provider).to_string()
        };
        let (uri, bytes) = mark_svg(provider, &fill);
        ui.add(egui::Image::from_bytes(uri, bytes).fit_to_exact_size(vec2(14.0, 14.0)))
            .on_hover_text(provider.display_name());
        return;
    }
    let colour = theme::rgb(u32::from_be_bytes([0, provider.colour()[0], provider.colour()[1], provider.colour()[2]]));
    egui::Frame::new()
        .fill(colour.gamma_multiply(0.18))
        .corner_radius(CornerRadius::same(theme::corner(4)))
        .inner_margin(egui::Margin::symmetric(5, 1))
        .show(ui, |ui| {
            ui.label(RichText::new(provider.badge()).font(theme::bold(10.0)).color(colour));
        })
        .response
        .on_hover_text(provider.display_name());
}

pub fn heading(ui: &mut Ui, palette: &Palette, title: &str, subtitle: Option<&str>) {
    ui.add_space(6.0);
    ui.label(RichText::new(title).font(theme::bold(20.0)).color(palette.text));
    if let Some(subtitle) = subtitle {
        ui.label(RichText::new(subtitle).font(theme::regular(13.0)).color(palette.subtext));
    }
    ui.add_space(6.0);
}

/// A filled button in the accent; under the 98 skin a grey slab with a black rim, as a default button was.
pub fn primary(ui: &mut Ui, palette: &Palette, text: &str) -> egui::Response {
    if let Some(w) = theme::skin().windows98() {
        let response = ui.add(
            egui::Button::new(RichText::new(text).font(theme::bold(14.0)).color(theme::rgb(w.text)))
                .fill(theme::rgb(w.face))
                .stroke(Stroke::NONE)
                .corner_radius(0)
                .min_size(vec2(0.0, 30.0)),
        );
        let pressed = response.is_pointer_button_down_on();
        ui.painter().rect_stroke(response.rect, 0, Stroke::new(1.0_f32, theme::rgb(w.dark_shadow)), StrokeKind::Inside);
        theme::bevel(ui.painter(), response.rect.shrink(1.0), !pressed, w);
        return response;
    }
    ui.add(
        egui::Button::new(RichText::new(text).font(theme::bold(14.0)).color(palette.on_accent))
            .fill(palette.accent)
            .corner_radius(CornerRadius::same(theme::corner(18)))
            .min_size(vec2(0.0, 34.0)),
    )
}

/// The 98 skin's raised edge round a button just drawn, pressed in while it is held.
fn bevelled(ui: &Ui, response: &egui::Response) {
    if let Some(w) = theme::skin().windows98() {
        theme::bevel(ui.painter(), response.rect, !response.is_pointer_button_down_on(), w);
    }
}

/// A quieter button beside it.
pub fn secondary(ui: &mut Ui, palette: &Palette, text: &str) -> egui::Response {
    let skinned = theme::skin().windows98();
    let response = ui.add(
        egui::Button::new(RichText::new(text).font(theme::regular(14.0)).color(skinned.map(|w| theme::rgb(w.text)).unwrap_or(palette.text)))
            .fill(skinned.map(|w| theme::rgb(w.face)).unwrap_or(palette.card))
            .stroke(if skinned.is_some() { Stroke::NONE } else { Stroke::new(1.0_f32, palette.line) })
            .corner_radius(CornerRadius::same(theme::corner(18)))
            .min_size(vec2(0.0, if skinned.is_some() { 30.0 } else { 34.0 })),
    );
    bevelled(ui, &response);
    response
}

/// A round button with a symbol in it, for the player's controls.
pub fn icon_button(ui: &mut Ui, palette: &Palette, symbol: &str, size: f32, active: bool, hint: &str) -> egui::Response {
    let (rect, response) = ui.allocate_exact_size(Vec2::splat(size), Sense::click());
    let colour = if active { palette.accent } else if response.hovered() { palette.text } else { palette.subtext };
    if response.hovered() {
        ui.painter().circle_filled(rect.center(), size / 2.0, palette.raised.gamma_multiply(0.8));
    }
    ui.painter().text(rect.center(), Align2::CENTER_CENTER, symbol, theme::regular(size * 0.52), colour);
    if active {
        ui.painter().circle_filled(rect.center_bottom() + vec2(0.0, -2.0), 2.0, palette.accent);
    }
    response.on_hover_text(hint)
}

/// Where a song row is, for its menu to offer what makes sense there.
#[derive(Clone, Debug)]
pub enum RowPlace {
    List,
    /// Search results: a song picked there plays as Settings › Playback says, a radio or the rest of the list.
    Picked,
    Playlist(String),
    Queue,
    /// One of the listener's own YouTube playlists: its id and link.
    YouTube { id: String, url: String },
}

/// Asks for `track` to be looked up once the pointer has rested on it a quarter of a second: long enough that
/// passing over a list does not look up every song in it, short enough to be done before most clicks land.
fn look_ahead_on_rest(ui: &Ui, response: &egui::Response, track: &Track, playing: bool, actions: &mut Vec<Action>) {
    // When it rested there first, and whether it has been asked for since. Moving off forgets both, so
    // coming back counts again.
    let id = response.id.with("rested");
    let looked_up_slowly = matches!(track.provider, ProviderType::YoutubeMusic | ProviderType::YoutubeVideo);
    if playing || !looked_up_slowly || !response.hovered() {
        ui.data_mut(|d| d.remove::<(f64, bool)>(id));
        return;
    }
    let now = ui.input(|i| i.time);
    let (since, asked) = ui.data(|d| d.get_temp::<(f64, bool)>(id)).unwrap_or((now, false));
    if asked {
        return;
    }
    if now - since >= 0.25 {
        ui.data_mut(|d| d.insert_temp(id, (since, true)));
        actions.push(Action::LookAhead(track.clone()));
    } else {
        ui.data_mut(|d| d.insert_temp(id, (since, false)));
        ui.ctx().request_repaint_after(std::time::Duration::from_millis(260));
    }
}

/// One song: its cover, title and artist, service and length. A click plays the list from it; the right
/// button, or the ⋯ at the end, opens what else can be done with it.
pub fn track_row(
    ui: &mut Ui,
    app: &App,
    track: &Track,
    index: usize,
    list: &[Track],
    place: &RowPlace,
    actions: &mut Vec<Action>,
) {
    let palette = &app.palette;
    // The song as the listener calls it, where they edited its details.
    let shown = app.edited(track);
    let track = &shown;
    let height = 54.0;
    let width = ui.available_width();
    let (rect, response) = ui.allocate_exact_size(vec2(width, height), Sense::click());
    let playing = app.queue.current().is_some_and(|c| c.queue_key() == track.queue_key())
        && (app.loaded.is_some() || app.resolving.is_some());
    let is_current_row = matches!(place, RowPlace::Queue) && index as isize == app.queue.current_index;
    if response.hovered() || is_current_row {
        ui.painter().rect_filled(rect, CornerRadius::same(theme::corner(10)), palette.card.gamma_multiply(0.9));
    }
    look_ahead_on_rest(ui, &response, track, playing, actions);
    let art = Rect::from_min_size(rect.left_top() + vec2(8.0, 7.0), Vec2::splat(40.0));
    paint_artwork(ui, palette, track.artwork_url.as_deref(), art, 6);
    if playing {
        ui.painter().rect_filled(art, CornerRadius::same(theme::corner(6)), Color32::from_black_alpha(140));
        ui.painter().text(art.center(), Align2::CENTER_CENTER, "♫", theme::regular(18.0), palette.accent);
    }
    let menu_rect = Rect::from_center_size(pos2(rect.right() - 22.0, rect.center().y), Vec2::splat(30.0));
    let duration_width = 54.0;
    let text_rect = Rect::from_min_max(
        pos2(art.right() + 12.0, rect.top() + 6.0),
        pos2(rect.right() - 44.0 - duration_width - 40.0, rect.bottom() - 6.0),
    );
    let mut text_ui = ui.new_child(UiBuilder::new().max_rect(text_rect).layout(Layout::top_down(Align::Min)));
    let title_colour = if playing { palette.accent } else { palette.text };
    text_ui.add(egui::Label::new(RichText::new(&track.title).font(theme::bold(14.5)).color(title_colour)).truncate().selectable(false));
    text_ui.add(egui::Label::new(RichText::new(track.artist_line()).font(theme::regular(13.0)).color(palette.subtext)).truncate().selectable(false));

    let badge_rect = Rect::from_min_size(pos2(text_rect.right() + 6.0, rect.center().y - 9.0), vec2(40.0, 18.0));
    if badged(services_in(list)) {
        let mut badge_ui = ui.new_child(UiBuilder::new().max_rect(badge_rect).layout(Layout::left_to_right(Align::Center)));
        badge(&mut badge_ui, track.provider);
    }
    if let Some(ms) = track.duration_ms {
        ui.painter().text(
            pos2(rect.right() - 44.0, rect.center().y),
            Align2::RIGHT_CENTER,
            format_duration(ms),
            theme::regular(13.0),
            palette.faint,
        );
    }
    let menu = ui.interact(menu_rect, Id::new(("row-menu", place_key(place), index, track.queue_key())), Sense::click());
    if response.hovered() || menu.hovered() || buttons_always() {
        ui.painter().text(menu_rect.center(), Align2::CENTER_CENTER, "⋯", theme::bold(18.0), palette.text);
    }
    if response.clicked() {
        actions.push(match place {
            RowPlace::Queue => Action::QueueJump(index),
            RowPlace::Picked => Action::PlayPicked { tracks: list.to_vec(), index },
            _ => Action::PlayList { tracks: list.to_vec(), index },
        });
    }
    let liked = app.library.is_liked(track);
    egui::Popup::menu(&menu).show(|ui| track_menu(ui, app, track, index, list.len(), place, liked, actions));
    response.context_menu(|ui| track_menu(ui, app, track, index, list.len(), place, liked, actions));
}

fn place_key(place: &RowPlace) -> String {
    match place {
        RowPlace::List => "list".into(),
        RowPlace::Picked => "picked".into(),
        RowPlace::Playlist(id) => format!("playlist:{id}"),
        RowPlace::Queue => "queue".into(),
        RowPlace::YouTube { id, .. } => format!("youtube:{id}"),
    }
}

#[allow(clippy::too_many_arguments)]
fn track_menu(
    ui: &mut Ui,
    app: &App,
    track: &Track,
    index: usize,
    count: usize,
    place: &RowPlace,
    liked: bool,
    actions: &mut Vec<Action>,
) {
    ui.set_min_width(210.0);
    let mut item = |ui: &mut Ui, text: &str, action: Action| {
        if ui.button(text).clicked() {
            actions.push(action);
            ui.close();
        }
    };
    match place {
        RowPlace::Queue => {
            item(ui, "▶  Play", Action::QueueJump(index));
            if index > 0 {
                item(ui, "↑  Move up", Action::QueueMove { from: index, to: index - 1 });
            }
            if index + 1 < count {
                item(ui, "↓  Move down", Action::QueueMove { from: index, to: index + 1 });
            }
            item(ui, "✕  Remove from queue", Action::QueueRemove(index));
        }
        _ => {
            item(ui, "⏭  Play next", Action::PlayNext(track.clone()));
            item(ui, "☰  Add to queue", Action::AddToQueue(track.clone()));
        }
    }
    item(ui, "📻  Start radio", Action::StartRadio(track.clone()));
    item(ui, if liked { "♥  Remove from Liked songs" } else { "♡  Add to Liked songs" }, Action::ToggleLike(track.clone()));
    ui.menu_button("＋  Add to playlist", |ui| {
        ui.set_min_width(180.0);
        if ui.button("New playlist").clicked() {
            actions.push(Action::AddToNewPlaylist(track.clone()));
            ui.close();
        }
        if !app.library.playlists.is_empty() {
            ui.separator();
        }
        for playlist in &app.library.playlists {
            if ui.button(&playlist.title).clicked() {
                actions.push(Action::AddToPlaylist { id: playlist.id.clone(), track: track.clone() });
                ui.close();
            }
        }
        let youtube_song = matches!(track.provider, crate::domain::ProviderType::YoutubeMusic | crate::domain::ProviderType::YoutubeVideo);
        if youtube_song && app.youtube.is_some() {
            ui.separator();
            ui.label(egui::RichText::new("On YouTube Music").small());
            if ui.button("New YouTube Music playlist").clicked() {
                actions.push(Action::YouTubeCreate { title: track.title.clone(), tracks: vec![track.clone()] });
                ui.close();
            }
            for playlist in &app.service_library.youtube_playlists {
                if ui.button(&playlist.title).clicked() {
                    actions.push(Action::AddToServicePlaylist {
                        playlist_id: playlist.id.clone(),
                        title: playlist.title.clone(),
                        track: track.clone(),
                    });
                    ui.close();
                }
            }
        }
    });
    if let RowPlace::Playlist(id) = place {
        ui.separator();
        if index > 0 && ui.button("↑  Move up").clicked() {
            actions.push(Action::MoveInPlaylist { id: id.clone(), from: index, to: index - 1 });
            ui.close();
        }
        if index + 1 < count && ui.button("↓  Move down").clicked() {
            actions.push(Action::MoveInPlaylist { id: id.clone(), from: index, to: index + 1 });
            ui.close();
        }
        if ui.button("✕  Remove from this playlist").clicked() {
            actions.push(Action::RemoveFromPlaylist { id: id.clone(), index });
            ui.close();
        }
    }
    if let RowPlace::YouTube { id, url } = place {
        ui.separator();
        if ui.button("✕  Remove from this playlist").clicked() {
            actions.push(Action::YouTubeRemove { id: id.clone(), url: url.clone(), track: track.clone() });
            ui.close();
        }
    }
    ui.separator();
    if track.provider.downloadable() {
        let key = track.queue_key();
        if let Some((_, progress)) = app.jobs.get(&key) {
            ui.label(format!("⬇  Downloading… {:.0}%", progress * 100.0));
        } else if app.downloads.file_for(track).is_some() {
            if ui.button("✕  Remove download").clicked() {
                actions.push(Action::RemoveDownload(track.clone()));
                ui.close();
            }
        } else if ui.button("⬇  Download to play offline").clicked() {
            actions.push(Action::Download(track.clone()));
            ui.close();
        }
        ui.menu_button("💾  Save as a file", |ui| {
            for format in [crate::downloads::ExportFormat::Mp3, crate::downloads::ExportFormat::Original] {
                if ui.button(format.name()).clicked() {
                    actions.push(Action::Export(track.clone(), format));
                    ui.close();
                }
            }
        });
        ui.separator();
    }
    let playing_now = app.loaded.as_ref().is_some_and(|l| l.queue_key() == track.queue_key());
    if playing_now {
        let set = app.sleep == Some(super::Sleep::EndOfSong);
        if ui.button(if set { "✓  Stop after this song" } else { "⏹  Stop after this song" }).clicked() {
            actions.push(Action::ToggleStopAfter);
            ui.close();
        }
    }
    if ui.button("✎  Edit details…").on_hover_text("What this song is called here: in the queue, in lyrics searches and in what is scrobbled").clicked() {
        actions.push(Action::EditDetails(track.clone(), track.title.clone(), track.artist_line()));
        ui.close();
    }
    if ui.button("🔗  Copy link").clicked() {
        actions.push(Action::Copy(track.source_url.clone()));
        ui.close();
    }
    if ui.button("🌐  Open in browser").clicked() {
        actions.push(Action::OpenUrl(track.source_url.clone()));
        ui.close();
    }
}

/// A list of songs, each a [`track_row`].
pub fn track_list(ui: &mut Ui, app: &App, tracks: &[Track], place: RowPlace, actions: &mut Vec<Action>) {
    for (index, track) in tracks.iter().enumerate() {
        ui.push_id(("track", index), |ui| track_row(ui, app, track, index, tracks, &place, actions));
    }
}

/// A row of cards that scrolls sideways: a song or a playlist each.
pub fn card_strip(ui: &mut Ui, id: impl std::hash::Hash, add: impl FnOnce(&mut Ui)) {
    egui::ScrollArea::horizontal()
        .id_salt(id)
        .auto_shrink([false, true])
        .scroll_bar_visibility(egui::scroll_area::ScrollBarVisibility::VisibleWhenNeeded)
        .show(ui, |ui| {
            ui.horizontal_top(|ui| {
                ui.spacing_mut().item_spacing.x = 14.0;
                add(ui);
            });
            ui.add_space(8.0);
        });
}

pub const CARD: f32 = 156.0;

/// The browsing looks in force, set each frame from Settings › Browsing: the card size, whether a card's
/// buttons stay put, and when a service is named on a cover or a row.
static CARD_SIZE: std::sync::atomic::AtomicU32 = std::sync::atomic::AtomicU32::new(0);
static BUTTONS_ALWAYS: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);
static BADGE_POLICY: std::sync::atomic::AtomicU8 = std::sync::atomic::AtomicU8::new(0);

pub fn set_browsing(looks: &crate::looks::Looks) {
    use std::sync::atomic::Ordering::Relaxed;
    CARD_SIZE.store(looks.card_size.width().to_bits(), Relaxed);
    BUTTONS_ALWAYS.store(looks.hover_controls == crate::looks::HoverControls::Always, Relaxed);
    BADGE_POLICY.store(
        match looks.badge_policy {
            crate::looks::BadgePolicy::Auto => 0,
            crate::looks::BadgePolicy::Always => 1,
            crate::looks::BadgePolicy::Never => 2,
        },
        Relaxed,
    );
}

pub fn card_size() -> f32 {
    let bits = CARD_SIZE.load(std::sync::atomic::Ordering::Relaxed);
    if bits == 0 { CARD } else { f32::from_bits(bits) }
}

pub fn buttons_always() -> bool {
    BUTTONS_ALWAYS.load(std::sync::atomic::Ordering::Relaxed)
}

/// Whether a song from `provider` is badged, among songs from `services` different services.
pub fn badged(services: usize) -> bool {
    match BADGE_POLICY.load(std::sync::atomic::Ordering::Relaxed) {
        1 => true,
        2 => false,
        _ => services > 1,
    }
}

/// How many different services a list of songs is from.
pub fn services_in(tracks: &[Track]) -> usize {
    let mut seen: Vec<ProviderType> = Vec::new();
    for track in tracks {
        let provider = match track.provider {
            ProviderType::YoutubeVideo => ProviderType::YoutubeMusic,
            other => other,
        };
        if !seen.contains(&provider) {
            seen.push(provider);
        }
    }
    seen.len()
}

/// A card: a cover, a title and a line under it. Answers whether it was clicked.
pub fn card(ui: &mut Ui, palette: &Palette, artwork_url: Option<&str>, title: &str, subtitle: &str) -> egui::Response {
    card_badged(ui, palette, artwork_url, title, subtitle, None)
}

/// A card, with the mark of the service it is from in its corner when `badge` names one.
pub fn card_badged(ui: &mut Ui, palette: &Palette, artwork_url: Option<&str>, title: &str, subtitle: &str, badge_of: Option<ProviderType>) -> egui::Response {
    let size = card_size();
    let (rect, response) = ui.allocate_exact_size(vec2(size, size + 48.0), Sense::click());
    let art = Rect::from_min_size(rect.min, Vec2::splat(size));
    paint_artwork(ui, palette, artwork_url, art, 12);
    if response.hovered() {
        ui.painter().rect_filled(art, CornerRadius::same(theme::corner(12)), Color32::from_black_alpha(70));
    }
    if response.hovered() || buttons_always() {
        let button = art.right_bottom() - vec2(26.0, 26.0);
        ui.painter().circle_filled(button, 18.0, palette.accent);
        play_triangle(ui.painter(), button, 7.5, palette.on_accent);
    }
    if let Some(provider) = badge_of {
        let corner = Rect::from_min_size(art.left_top() + vec2(8.0, 8.0), vec2(22.0, 22.0));
        ui.painter().circle_filled(corner.center(), 12.0, Color32::from_black_alpha(150));
        let mut badge_ui = ui.new_child(UiBuilder::new().max_rect(corner.shrink(3.0)).layout(Layout::left_to_right(Align::Center)));
        badge(&mut badge_ui, provider);
    }
    let text = Rect::from_min_max(pos2(rect.left(), art.bottom() + 6.0), rect.max);
    let mut text_ui = ui.new_child(UiBuilder::new().max_rect(text).layout(Layout::top_down(Align::Min)));
    text_ui.add(egui::Label::new(RichText::new(title).font(theme::bold(14.0)).color(palette.text)).truncate().selectable(false));
    text_ui.add(egui::Label::new(RichText::new(subtitle).font(theme::regular(12.5)).color(palette.subtext)).truncate().selectable(false));
    response
}

pub fn play_triangle(painter: &egui::Painter, centre: egui::Pos2, size: f32, colour: Color32) {
    let points = vec![
        centre + vec2(-size * 0.7, -size),
        centre + vec2(size * 1.0, 0.0),
        centre + vec2(-size * 0.7, size),
    ];
    painter.add(egui::Shape::convex_polygon(points, colour, Stroke::NONE));
}

pub fn pause_bars(painter: &egui::Painter, centre: egui::Pos2, size: f32, colour: Color32) {
    for dx in [-size * 0.45, size * 0.45] {
        let bar = Rect::from_center_size(centre + vec2(dx, 0.0), vec2(size * 0.42, size * 1.8));
        painter.rect_filled(bar, CornerRadius::same(theme::corner(2)), colour);
    }
}

/// A box with a message and, below it, what could be done about it.
pub fn notice(ui: &mut Ui, palette: &Palette, text: &str, error: bool) {
    egui::Frame::new()
        .fill(if error { palette.error.gamma_multiply(0.12) } else { palette.card })
        .stroke(Stroke::new(1.0_f32, if error { palette.error.gamma_multiply(0.5) } else { palette.line }))
        .corner_radius(CornerRadius::same(theme::corner(10)))
        .inner_margin(egui::Margin::same(12))
        .show(ui, |ui| {
            ui.set_width(ui.available_width());
            ui.label(RichText::new(text).color(if error { palette.error } else { palette.subtext }));
        });
}

pub fn loading(ui: &mut Ui, palette: &Palette, text: &str) {
    ui.horizontal(|ui| {
        ui.add(egui::Spinner::new().color(palette.accent));
        ui.label(RichText::new(text).color(palette.subtext));
    });
}

/// The little messages in the corner.
pub fn toasts(app: &App, ctx: &egui::Context) {
    if app.toasts.is_empty() {
        return;
    }
    let palette = &app.palette;
    egui::Area::new(Id::new("toasts"))
        .anchor(Align2::RIGHT_BOTTOM, vec2(-18.0, -112.0))
        .order(egui::Order::Foreground)
        .interactable(false)
        .show(ctx, |ui| {
            for toast in &app.toasts {
                egui::Frame::new()
                    .fill(palette.card)
                    .stroke(Stroke::new(1.0_f32, if toast.error { palette.error } else { palette.line }))
                    .corner_radius(CornerRadius::same(theme::corner(10)))
                    .inner_margin(egui::Margin::symmetric(14, 10))
                    .show(ui, |ui| {
                        ui.set_max_width(380.0);
                        ui.label(RichText::new(&toast.text).color(if toast.error { palette.error } else { palette.text }));
                    });
                ui.add_space(6.0);
            }
        });
}

/// A navigation entry in the rail.
pub fn nav(ui: &mut Ui, palette: &Palette, symbol: &str, text: &str, selected: bool) -> egui::Response {
    let (rect, response) = ui.allocate_exact_size(vec2(ui.available_width(), 38.0), Sense::click());
    if selected {
        ui.painter().rect_filled(rect, CornerRadius::same(theme::corner(10)), palette.wash(0.16));
    } else if response.hovered() {
        ui.painter().rect_filled(rect, CornerRadius::same(theme::corner(10)), palette.card.gamma_multiply(0.8));
    }
    let colour = if selected { palette.accent } else { palette.text };
    ui.painter().text(rect.left_center() + vec2(14.0, 0.0), Align2::LEFT_CENTER, symbol, theme::regular(16.0), colour);
    ui.painter().text(rect.left_center() + vec2(42.0, 0.0), Align2::LEFT_CENTER, text, theme::bold(14.5), colour);
    response
}

/// A smaller entry for a playlist, with its cover.
pub fn nav_playlist(ui: &mut Ui, palette: &Palette, artwork: Option<&str>, text: &str, selected: bool) -> egui::Response {
    let (rect, response) = ui.allocate_exact_size(vec2(ui.available_width(), 40.0), Sense::click());
    if selected {
        ui.painter().rect_filled(rect, CornerRadius::same(theme::corner(8)), palette.wash(0.16));
    } else if response.hovered() {
        ui.painter().rect_filled(rect, CornerRadius::same(theme::corner(8)), palette.card.gamma_multiply(0.8));
    }
    let art = Rect::from_min_size(rect.left_top() + vec2(8.0, 5.0), Vec2::splat(30.0));
    paint_artwork(ui, palette, artwork, art, 5);
    let text_rect = Rect::from_min_max(pos2(art.right() + 10.0, rect.top()), pos2(rect.right() - 4.0, rect.bottom()));
    let mut text_ui = ui.new_child(UiBuilder::new().max_rect(text_rect).layout(Layout::left_to_right(Align::Center)));
    text_ui.add(
        egui::Label::new(RichText::new(text).font(theme::regular(13.5)).color(if selected { palette.accent } else { palette.subtext }))
            .truncate()
            .selectable(false),
    );
    response
}

pub fn back_button(ui: &mut Ui, palette: &Palette, actions: &mut Vec<Action>) {
    if ui
        .add(egui::Button::new(RichText::new("←  Back").color(palette.subtext)).frame(false))
        .clicked()
    {
        actions.push(Action::Back);
    }
}

/// The swatch for a theme in Settings: its page, card and accent, and its name.
pub fn theme_swatch(ui: &mut Ui, palette: &Palette, preset: &theme::ThemePreset, selected: bool) -> egui::Response {
    let (rect, response) = ui.allocate_exact_size(vec2(132.0, 82.0), Sense::click());
    let colours = preset.colours;
    let page = theme::rgb(colours.background);
    ui.painter().rect_filled(rect, CornerRadius::same(theme::corner(10)), page);
    if let Some(glow) = preset.glow {
        let painter = ui.painter_at(rect);
        let centre = match glow.placement {
            GlowPlacement::Artwork => rect.left_bottom(),
            GlowPlacement::Top => rect.center_top(),
        };
        radial(&painter, centre, vec2(rect.width() * 0.8, rect.height() * 0.9), theme::rgb(glow.colour), (glow.strength * 2.0).min(0.8));
    }
    let card = Rect::from_min_size(rect.left_top() + vec2(10.0, 10.0), vec2(rect.width() - 20.0, 30.0));
    ui.painter().rect_filled(card, CornerRadius::same(theme::corner(6)), theme::rgb(colours.card));
    ui.painter().circle_filled(card.right_center() - vec2(14.0, 0.0), 7.0, theme::rgb(colours.accent));
    ui.painter().rect_filled(
        Rect::from_min_size(card.left_center() + vec2(8.0, -3.0), vec2(50.0, 6.0)),
        CornerRadius::same(theme::corner(3)),
        theme::rgb(colours.text).gamma_multiply(0.8),
    );
    ui.painter().text(
        pos2(rect.left() + 10.0, rect.bottom() - 14.0),
        Align2::LEFT_CENTER,
        preset.name,
        theme::bold(13.0),
        theme::rgb(colours.text),
    );
    let stroke = if selected {
        Stroke::new(2.5_f32, palette.accent)
    } else if response.hovered() {
        Stroke::new(1.5_f32, palette.subtext)
    } else {
        Stroke::new(1.0_f32, palette.line)
    };
    ui.painter().rect_stroke(rect, CornerRadius::same(theme::corner(10)), stroke, StrokeKind::Inside);
    response.on_hover_text(format!("{} · {}", preset.family, preset.name))
}
