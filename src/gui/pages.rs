//! The rail down the left and the pages it leads to.

use super::widgets::{self, back_button, card, card_strip, heading, notice, primary, secondary, track_list, RowPlace};
use super::{Action, App, Inputs, Loadable, Page};
use crate::bangs::{self, SearchMode};
use crate::domain::{format_duration, Track};
use crate::theme;
use crate::tools::{self, Tool};
use eframe::egui::{self, vec2, Align, Layout, RichText, Stroke};

pub fn rail(app: &App, ctx: &egui::Context, actions: &mut Vec<Action>) {
    let palette = app.palette;
    let glass = app.settings.looks.surface == crate::looks::SurfaceStyle::Glass;
    egui::SidePanel::left("rail")
        .exact_width(232.0)
        .resizable(false)
        .show_separator_line(false)
        .frame(
            egui::Frame::new()
                .fill(if glass { palette.panel.gamma_multiply(0.42) } else { palette.panel.gamma_multiply(app.panel_opacity()) })
                .inner_margin(egui::Margin { left: 12, right: 12, top: 18, bottom: 12 }),
        )
        .show(ctx, |ui| {
            if glass {
                // A pale rim down its inner edge and a sheen from the top, as glass catches the light.
                let rect = ui.max_rect().expand2(vec2(12.0, 18.0));
                let sheen = egui::Rect::from_min_max(rect.min, egui::pos2(rect.right(), rect.top() + rect.height() * 0.4));
                theme::gradient(ui.painter(), sheen, egui::Color32::from_white_alpha(18), egui::Color32::from_white_alpha(0), false);
                ui.painter().line_segment([rect.right_top(), rect.right_bottom()], Stroke::new(1.0_f32, egui::Color32::from_white_alpha(40)));
            }
            ui.horizontal(|ui| {
                ui.add_space(8.0);
                widgets::mark(ui, 30.0);
                ui.add_space(2.0);
                ui.label(RichText::new("Noctorium").font(theme::bold(21.0)).color(palette.text));
            });
            ui.add_space(18.0);
            let page = &app.page;
            use crate::looks::SidebarPage as S;
            for (symbol, text, target, part) in [
                ("🏠", "Home", Page::Home, None),
                ("🎵", "Now playing", Page::NowPlaying, Some(S::NowPlaying)),
                ("🔍", "Search", Page::Search, Some(S::Search)),
                ("📚", "Library", Page::Library, Some(S::Library)),
                ("☰", "Queue", Page::Queue, Some(S::Queue)),
                ("🌐", "Services", Page::Services, Some(S::Services)),
                ("📡", "Connect", Page::Connect, Some(S::Connect)),
                ("⬇", "Downloads", Page::Downloads, Some(S::Downloads)),
                ("👤", "Profile", Page::Profile, Some(S::Profile)),
                ("⚙", "Settings", Page::Settings, None),
            ] {
                // Home and Settings always stay; the rest as Settings › Browsing has them.
                if part.is_some_and(|p| !app.settings.looks.shows_page(p)) {
                    continue;
                }
                let lit = *page == target || (target == Page::Settings && matches!(page, Page::SettingsAt(_)));
                if widgets::nav(ui, &palette, symbol, text, lit).clicked() {
                    actions.push(Action::Open(target));
                }
            }
            ui.add_space(14.0);
            ui.separator();
            ui.add_space(8.0);
            ui.label(RichText::new("YOUR MUSIC").font(theme::bold(11.0)).color(palette.faint));
            ui.add_space(4.0);
            egui::ScrollArea::vertical().auto_shrink([false, false]).show(ui, |ui| {
                let liked_art = app.library.liked.first().and_then(|t| t.artwork_url.as_deref());
                if widgets::nav_playlist(ui, &palette, liked_art, &format!("♥ Liked songs · {}", app.library.liked.len()), *page == Page::Liked).clicked() {
                    actions.push(Action::Open(Page::Liked));
                }
                for playlist in &app.library.playlists {
                    let art = playlist.tracks.first().and_then(|t| t.artwork_url.as_deref());
                    let selected = *page == Page::Playlist(playlist.id.clone());
                    if widgets::nav_playlist(ui, &palette, art, &playlist.title, selected).clicked() {
                        actions.push(Action::Open(Page::Playlist(playlist.id.clone())));
                    }
                }
                for playlist in &app.library.followed {
                    let Some(url) = &playlist.source_url else { continue };
                    let selected = *page == Page::Remote(url.clone());
                    if widgets::nav_playlist(ui, &palette, playlist.artwork_url.as_deref(), &playlist.title, selected).clicked() {
                        actions.push(Action::Open(Page::Remote(url.clone())));
                    }
                }
            });
        });
}

pub fn central(app: &App, ctx: &egui::Context, inputs: &mut Inputs, actions: &mut Vec<Action>) {
    egui::CentralPanel::default()
        .frame(egui::Frame::new().inner_margin(egui::Margin { left: 28, right: 24, top: 20, bottom: 8 }))
        .show(ctx, |ui| {
            let id = format!("{:?}", app.page);
            egui::ScrollArea::vertical()
                .id_salt(id)
                .auto_shrink([false, false])
                .show(ui, |ui| {
                    ui.set_max_width(ui.available_width() - 8.0);
                    if app.settings.looks.animations {
                        let shown = (app.page_opened.elapsed().as_secs_f32() / 0.18).min(1.0);
                        if shown < 1.0 {
                            ui.set_opacity(0.25 + 0.75 * shown);
                            ui.ctx().request_repaint();
                        }
                    }
                    match app.page.clone() {
                        Page::Profile => profile_page(ui, app, inputs, actions),
                        Page::Home => home(ui, app, actions),
                        Page::Search => search(ui, app, inputs, actions),
                        Page::Library => library(ui, app, inputs, actions),
                        Page::Liked => songs_page(ui, app, "Liked songs", "The songs you liked, newest first.", &app.library.liked, actions),
                        Page::Recent => songs_page(ui, app, "Recently played", "What played here last.", &app.library.recent, actions),
                        Page::Playlist(id) => local_playlist(ui, app, &id, inputs, actions),
                        Page::Remote(url) => remote_playlist(ui, app, &url, inputs, actions),
                        Page::Queue => queue(ui, app, actions),
                        Page::Downloads => downloads_page(ui, app, actions),
                        Page::Shortcuts => shortcuts_page(ui, app, actions),
                        Page::NowPlaying => now_playing(ui, app, actions),
                        Page::Connect => connect_page(ui, app, inputs, actions),
                        Page::Services => services_page(ui, app, inputs, actions),
                        Page::ServiceLikes(crate::domain::ProviderType::AppleMusic) => match &app.services.apple_library {
                            Some(Ok((_, songs))) => songs_page(ui, app, "Your Apple Music songs", "In your Apple Music library.", songs, actions),
                            Some(Err(why)) => notice(ui, &app.palette, why, true),
                            None => widgets::loading(ui, &app.palette, "Reading your library…"),
                        },
                        Page::ServiceLikes(crate::domain::ProviderType::Vk) => match &app.services.vk_music {
                            Some(Ok(songs)) => songs_page(ui, app, "Your VK music", "The songs in your VK music.", songs, actions),
                            Some(Err(why)) => notice(ui, &app.palette, why, true),
                            None => widgets::loading(ui, &app.palette, "Reading your music…"),
                        },
                        Page::ServiceLikes(crate::domain::ProviderType::Tidal) => match &app.services.tidal_collection {
                            Some(Ok((songs, _))) => songs_page(ui, app, "Your Tidal collection", "The songs in your Tidal collection.", songs, actions),
                            Some(Err(why)) => notice(ui, &app.palette, why, true),
                            None => widgets::loading(ui, &app.palette, "Reading your collection…"),
                        },
                        Page::ServiceLikes(provider) => {
                            let (title, likes) = match provider {
                                crate::domain::ProviderType::Soundcloud => ("Your SoundCloud likes", &app.service_library.soundcloud_likes),
                                crate::domain::ProviderType::Spotify => ("Your Spotify Liked Songs", &app.services.spotify_likes),
                                crate::domain::ProviderType::AppleMusic | crate::domain::ProviderType::Tidal => ("", &None),
                                _ => ("Your YouTube Music likes", &app.service_library.youtube_likes),
                            };
                            match likes {
                                None => widgets::loading(ui, &app.palette, "Reading your likes…"),
                                Some(Err(why)) => notice(ui, &app.palette, why, true),
                                Some(Ok(tracks)) => songs_page(ui, app, title, "Liked on the service itself.", tracks, actions),
                            }
                        }
                        Page::Settings => super::settings::home(ui, app, inputs, actions),
                        Page::SettingsAt(page) => super::settings::page(ui, app, page, inputs, actions),
                    }
                    ui.add_space(24.0);
                });
        });
}

fn greeting() -> &'static str {
    use chrono::Timelike;
    match chrono::Local::now().hour() {
        5..=11 => "Good morning",
        12..=17 => "Good afternoon",
        _ => "Good evening",
    }
}

fn home(ui: &mut egui::Ui, app: &App, actions: &mut Vec<Action>) {
    let palette = app.palette;
    let looks = &app.settings.looks;
    ui.horizontal(|ui| {
        let name = app.settings.profile_name.split_whitespace().next().unwrap_or("").to_string();
        let words = if !looks.shows_home(crate::looks::HomePart::Greeting) {
            "Home".to_string()
        } else if name.is_empty() || app.settings.profile_name == "Noctorium Listener" {
            greeting().to_string()
        } else {
            format!("{}, {name}", greeting())
        };
        ui.label(RichText::new(words).font(theme::bold(30.0)).color(palette.text));
        ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
            if secondary(ui, &palette, "⟳  Refresh").clicked() {
                actions.push(Action::RefreshHome);
            }
        });
    });
    ui.add_space(8.0);
    if app.yt_dlp.path.is_none() {
        notice(ui, &palette, "yt-dlp is not installed, so there is nothing to search or play yet. Settings › Playback tools installs it.", true);
        if primary(ui, &palette, "Open Playback tools").clicked() {
            actions.push(Action::Open(Page::SettingsAt(super::settings::SettingsPage::PlaybackTools)));
        }
        ui.add_space(12.0);
    }
    if !app.library.recent.is_empty() && looks.shows_home(crate::looks::HomePart::Recent) {
        strip(ui, app, "Jump back in", Some("What you played last"), &app.library.recent, "recent", actions);
    }
    if !app.library.liked.is_empty() && looks.shows_home(crate::looks::HomePart::Pinned) {
        strip(ui, app, "Liked songs", None, &app.library.liked, "liked", actions);
    }
    for (slot, row) in app.home.iter().enumerate() {
        match row {
            None => {
                if app.yt_dlp.path.is_some() {
                    ui.add_space(10.0);
                    widgets::loading(ui, &palette, "Finding music…");
                }
            }
            Some(row) => match &row.tracks {
                Ok(tracks) if !tracks.is_empty() => strip(ui, app, &row.title, Some(&row.subtitle), tracks, ("home", slot), actions),
                Ok(_) => {}
                Err(why) => {
                    heading(ui, &palette, &row.title, Some(&row.subtitle));
                    notice(ui, &palette, why, true);
                }
            },
        }
    }
}

pub(super) fn strip(
    ui: &mut egui::Ui,
    app: &App,
    title: &str,
    subtitle: Option<&str>,
    tracks: &[Track],
    id: impl std::hash::Hash,
    actions: &mut Vec<Action>,
) {
    let palette = app.palette;
    heading(ui, &palette, title, subtitle);
    let badged = widgets::badged(widgets::services_in(tracks));
    card_strip(ui, id, |ui| {
        for (index, track) in tracks.iter().take(20).enumerate() {
            let response = widgets::card_badged(ui, &palette, track.artwork_url.as_deref(), &track.title, &track.artist_line(), badged.then_some(track.provider));
            if response.clicked() {
                actions.push(Action::PlayPicked { tracks: tracks.to_vec(), index });
            }
            response.context_menu(|ui| {
                if ui.button("⏭  Play next").clicked() {
                    actions.push(Action::PlayNext(track.clone()));
                    ui.close();
                }
                if ui.button("☰  Add to queue").clicked() {
                    actions.push(Action::AddToQueue(track.clone()));
                    ui.close();
                }
                if ui.button("📻  Start radio").clicked() {
                    actions.push(Action::StartRadio(track.clone()));
                    ui.close();
                }
                let liked = app.library.is_liked(track);
                if ui.button(if liked { "♥  Remove from Liked songs" } else { "♡  Add to Liked songs" }).clicked() {
                    actions.push(Action::ToggleLike(track.clone()));
                    ui.close();
                }
            });
        }
    });
    ui.add_space(10.0);
}

fn search(ui: &mut egui::Ui, app: &App, inputs: &mut Inputs, actions: &mut Vec<Action>) {
    let palette = app.palette;
    // Music or Radio beside the heading, above the box: what the page searches, with the box left to be a box.
    ui.horizontal(|ui| {
        ui.label(RichText::new("Search").font(theme::bold(28.0)).color(palette.text));
        ui.add_space(10.0);
        super::radio::kind_picker(ui, app, false, actions);
    });
    ui.add_space(10.0);
    let radio = app.search_kind == super::SearchKind::Radio;
    let field = egui::Frame::new()
        .fill(palette.card)
        .stroke(Stroke::new(1.0_f32, palette.line))
        .corner_radius(egui::CornerRadius::same(theme::corner(22)))
        .inner_margin(egui::Margin::symmetric(16, 10))
        .show(ui, |ui| {
            ui.horizontal(|ui| {
                ui.label(RichText::new("🔍").color(palette.subtext));
                let hint = if radio {
                    "A station, a city or a kind of music: “jazz”, “BBC”, “Tokyo”"
                } else {
                    "Songs, artists, or a YouTube or SoundCloud link — !sc, !ytm, !yt, !all"
                };
                let response = ui.add(
                    egui::TextEdit::singleline(&mut inputs.search)
                        .hint_text(hint)
                        .frame(false)
                        .font(theme::regular(16.0))
                        .desired_width(f32::INFINITY),
                );
                if inputs.focus_search {
                    response.request_focus();
                    inputs.focus_search = false;
                }
                response
            })
            .inner
        })
        .inner;
    if field.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter)) {
        actions.push(Action::Search);
    }
    ui.add_space(8.0);
    if radio {
        // A country to narrow the stations to, where the service chips would be.
        super::radio::country_picker(ui, app, inputs, actions);
        ui.add_space(14.0);
        super::radio::results(ui, app, actions);
        return;
    }
    let typed = bangs::parse(&inputs.search);
    ui.horizontal_wrapped(|ui| {
        for mode in SearchMode::OFFERED {
            let chosen = typed.mode().unwrap_or(app.settings.search_mode) == mode;
            let text = RichText::new(format!("{}  {}", mode.display_name(), bangs::bang_for(mode)))
                .color(if chosen { palette.on_accent } else { palette.text });
            let button = egui::Button::new(text)
                .fill(if chosen { palette.accent } else { palette.card })
                .corner_radius(egui::CornerRadius::same(theme::corner(14)));
            if ui.add(button).clicked() {
                actions.push(Action::SetSearchMode(mode));
            }
        }
    });
    if let Some((word, mode)) = &typed.bang {
        if !mode.offered_here() {
            ui.label(RichText::new(format!("{word} is {} — not searchable in this version yet.", mode.display_name())).color(palette.error));
        }
    }
    ui.add_space(12.0);

    if app.search.results.is_empty() {
        if app.library.recent.is_empty() {
            ui.label(RichText::new("Type and press Enter. A bang at the start or the end sends the search to one service.").color(palette.faint));
        } else {
            heading(ui, &palette, "Recently played", None);
            let recent: Vec<Track> = app.library.recent.iter().take(8).cloned().collect();
            track_list(ui, app, &recent, RowPlace::List, actions);
        }
        return;
    }
    let several = app.search.results.len() > 1;
    for (provider, result) in &app.search.results {
        if several {
            heading(ui, &palette, provider.display_name(), None);
        } else {
            heading(ui, &palette, &format!("“{}”", app.search.query), Some(&format!("on {}", provider.display_name())));
        }
        match result {
            None => widgets::loading(ui, &palette, &format!("Searching {}…", provider.display_name())),
            Some(Err(why)) => notice(ui, &palette, why, true),
            Some(Ok(tracks)) if tracks.is_empty() => {
                ui.label(RichText::new("Nothing found.").color(palette.faint));
            }
            Some(Ok(tracks)) => {
                let shown: Vec<Track> = if several { tracks.iter().take(8).cloned().collect() } else { tracks.clone() };
                ui.push_id(provider.key_name(), |ui| track_list(ui, app, &shown, RowPlace::Picked, actions));
            }
        }
        ui.add_space(8.0);
    }
}

fn library(ui: &mut egui::Ui, app: &App, inputs: &mut Inputs, actions: &mut Vec<Action>) {
    let palette = app.palette;
    ui.label(RichText::new("Library").font(theme::bold(28.0)).color(palette.text));
    ui.add_space(12.0);
    card_strip(ui, "library-collections", |ui| {
        let liked_art = app.library.liked.first().and_then(|t| t.artwork_url.as_deref());
        if card(ui, &palette, liked_art, "Liked songs", &format!("{} songs", app.library.liked.len())).clicked() {
            actions.push(Action::Open(Page::Liked));
        }
        let recent_art = app.library.recent.first().and_then(|t| t.artwork_url.as_deref());
        if card(ui, &palette, recent_art, "Recently played", &format!("{} songs", app.library.recent.len())).clicked() {
            actions.push(Action::Open(Page::Recent));
        }
    });

    service_library(ui, app, actions);

    heading(ui, &palette, "Your playlists", Some("Made here, free to mix every service. ☁ Noctorium playlists are on every device you use Noctorium on."));
    ui.horizontal(|ui| {
        let field = ui.add(egui::TextEdit::singleline(&mut inputs.new_playlist).hint_text("New playlist name").desired_width(260.0));
        let enter = field.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter));
        let named = !inputs.new_playlist.trim().is_empty();
        // A Noctorium playlist first, as the one most people want; Enter makes one.
        let where_kept = match app.settings.playlist_cloud.kind {
            crate::noctplaylists::CloudKind::None => "On every device you use Noctorium on. Choose where they are kept in Settings › Noctorium playlists.".to_string(),
            crate::noctplaylists::CloudKind::Folder => format!("On every device you use Noctorium on, kept in {}.", app.settings.playlist_cloud.folder),
            crate::noctplaylists::CloudKind::GoogleDrive => "On every device you use Noctorium on, kept in your Google Drive.".to_string(),
        };
        if (primary(ui, &palette, "☁ Noctorium playlist").on_hover_text(where_kept).clicked() || enter) && named {
            actions.push(Action::CreatePlaylist { title: std::mem::take(&mut inputs.new_playlist), noctorium: true });
        }
        if secondary(ui, &palette, "＋ Local playlist").on_hover_text("Stays on this computer only").clicked() && named {
            actions.push(Action::CreatePlaylist { title: std::mem::take(&mut inputs.new_playlist), noctorium: false });
        }
    });
    ui.add_space(8.0);
    if app.library.playlists.is_empty() {
        ui.label(RichText::new("No playlists yet. Make one here, or add a song to one from its ⋯ menu.").color(palette.faint));
    } else {
        ui.horizontal_wrapped(|ui| {
            ui.spacing_mut().item_spacing = vec2(14.0, 14.0);
            for playlist in &app.library.playlists {
                let art = playlist.tracks.first().and_then(|t| t.artwork_url.as_deref());
                let subtitle = format!("{}{} songs", if playlist.synced { "☁ " } else { "" }, playlist.tracks.len());
                if card(ui, &palette, art, &playlist.title, &subtitle).clicked() {
                    actions.push(Action::Open(Page::Playlist(playlist.id.clone())));
                }
            }
        });
    }

    heading(ui, &palette, "Followed playlists", Some("Playlists on YouTube Music, YouTube and SoundCloud, by their links"));
    ui.horizontal(|ui| {
        let field = ui.add(
            egui::TextEdit::singleline(&mut inputs.follow_link)
                .hint_text("https://music.youtube.com/playlist?list=…  or  https://soundcloud.com/…/sets/…")
                .desired_width(460.0),
        );
        let enter = field.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter));
        if (primary(ui, &palette, "Open").clicked() || enter) && !inputs.follow_link.trim().is_empty() {
            actions.push(Action::OpenLink(std::mem::take(&mut inputs.follow_link).trim().to_string()));
        }
    });
    ui.add_space(8.0);
    if !app.library.followed.is_empty() {
        ui.horizontal_wrapped(|ui| {
            ui.spacing_mut().item_spacing = vec2(14.0, 14.0);
            for playlist in &app.library.followed {
                let Some(url) = &playlist.source_url else { continue };
                let subtitle = playlist.owner_name.clone().unwrap_or_else(|| playlist.provider.display_name().to_string());
                if card(ui, &palette, playlist.artwork_url.as_deref(), &playlist.title, &subtitle).clicked() {
                    actions.push(Action::Open(Page::Remote(url.clone())));
                }
            }
        });
    }
}

fn header(ui: &mut egui::Ui, app: &App, artwork: Option<&str>, kind: &str, title: &str, detail: &str) {
    let palette = app.palette;
    ui.horizontal(|ui| {
        widgets::artwork(ui, &palette, artwork, 150.0, 14);
        ui.add_space(14.0);
        ui.vertical(|ui| {
            ui.add_space(28.0);
            ui.label(RichText::new(kind).font(theme::bold(12.0)).color(palette.subtext));
            ui.add(egui::Label::new(RichText::new(title).font(theme::bold(32.0)).color(palette.text)).truncate());
            ui.label(RichText::new(detail).color(palette.subtext));
        });
    });
    ui.add_space(12.0);
}

fn summary(tracks: &[Track]) -> String {
    let total: i64 = tracks.iter().map(|t| t.duration_ms.unwrap_or(0).max(0)).sum();
    let songs = if tracks.len() == 1 { "1 song".to_string() } else { format!("{} songs", tracks.len()) };
    if total > 0 {
        format!("{songs} · {}", format_duration(total))
    } else {
        songs
    }
}

fn play_buttons(ui: &mut egui::Ui, app: &App, tracks: &[Track], actions: &mut Vec<Action>) {
    let palette = app.palette;
    if primary(ui, &palette, "▶  Play").clicked() && !tracks.is_empty() {
        actions.push(Action::PlayList { tracks: tracks.to_vec(), index: 0 });
    }
    if secondary(ui, &palette, "🔀  Shuffle").clicked() && !tracks.is_empty() {
        actions.push(Action::PlayShuffled(tracks.to_vec()));
    }
    if secondary(ui, &palette, "☰  Add to queue").clicked() && !tracks.is_empty() {
        actions.push(Action::AddAllToQueue(tracks.to_vec()));
    }
    let missing = tracks.iter().filter(|t| t.provider.downloadable() && app.downloads.file_for(t).is_none()).count();
    if missing > 0 && secondary(ui, &palette, "⬇  Download").on_hover_text(format!("{missing} songs to play offline")).clicked() {
        actions.push(Action::DownloadAll(tracks.to_vec()));
    }
}

fn songs_page(ui: &mut egui::Ui, app: &App, title: &str, detail: &str, tracks: &[Track], actions: &mut Vec<Action>) {
    back_button(ui, &app.palette, actions);
    let art = tracks.first().and_then(|t| t.artwork_url.as_deref());
    header(ui, app, art, "COLLECTION", title, &format!("{detail} {}", summary(tracks)));
    ui.horizontal(|ui| play_buttons(ui, app, tracks, actions));
    ui.add_space(10.0);
    if tracks.is_empty() {
        ui.label(RichText::new("Nothing here yet.").color(app.palette.faint));
    }
    track_list(ui, app, tracks, RowPlace::List, actions);
}

fn local_playlist(ui: &mut egui::Ui, app: &App, id: &str, inputs: &mut Inputs, actions: &mut Vec<Action>) {
    let palette = app.palette;
    back_button(ui, &palette, actions);
    let Some(playlist) = app.library.playlists.iter().find(|p| p.id == id) else {
        notice(ui, &palette, "This playlist is gone.", true);
        return;
    };
    let art = playlist.tracks.first().and_then(|t| t.artwork_url.as_deref());
    header(ui, app, art, "PLAYLIST", &playlist.title, &summary(&playlist.tracks));
    ui.horizontal(|ui| {
        play_buttons(ui, app, &playlist.tracks, actions);
        ui.add_space(12.0);
        match &mut inputs.renaming {
            Some((renaming, title)) if renaming == id => {
                let field = ui.add(egui::TextEdit::singleline(title).desired_width(220.0));
                field.request_focus();
                let done = ui.button("Save").clicked() || (field.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter)));
                if done {
                    actions.push(Action::RenamePlaylist { id: id.to_string(), title: title.clone() });
                    inputs.renaming = None;
                } else if ui.button("Cancel").clicked() || ui.input(|i| i.key_pressed(egui::Key::Escape)) {
                    inputs.renaming = None;
                }
            }
            _ => {
                if secondary(ui, &palette, "✎  Rename").clicked() {
                    inputs.renaming = Some((id.to_string(), playlist.title.clone()));
                }
            }
        }
        if inputs.confirm_delete.as_deref() == Some(id) {
            ui.label(RichText::new("Delete it?").color(palette.error));
            if ui.button(RichText::new("Delete").color(palette.error)).clicked() {
                actions.push(Action::DeletePlaylist(id.to_string()));
                inputs.confirm_delete = None;
            }
            if ui.button("Keep").clicked() {
                inputs.confirm_delete = None;
            }
        } else if secondary(ui, &palette, "🗑  Delete").clicked() {
            inputs.confirm_delete = Some(id.to_string());
        }
        // Which kind it is, and the way to make it the other.
        let (label, hint) = if playlist.synced {
            ("💻  Keep on this computer only", "Takes it off your other devices and leaves it here")
        } else {
            ("☁  Make it a Noctorium playlist", "Puts it on every device you use Noctorium on")
        };
        if secondary(ui, &palette, label).on_hover_text(hint).clicked() {
            actions.push(Action::SetPlaylistNoctorium { id: id.to_string(), noctorium: !playlist.synced });
        }
        if !playlist.tracks.is_empty() && secondary(ui, &palette, "🔗  Share").on_hover_text("Copy a link that opens this playlist in Noctorium").clicked() {
            actions.push(Action::Copy(crate::sharelink::encode(&playlist.title, &playlist.tracks)));
        }
        let youtube_songs = playlist.tracks.iter().any(|t| matches!(t.provider, crate::domain::ProviderType::YoutubeMusic | crate::domain::ProviderType::YoutubeVideo));
        if app.youtube.is_some() && youtube_songs && secondary(ui, &palette, "⇪  To YouTube Music").on_hover_text("Make a private copy on YouTube Music").clicked() {
            actions.push(Action::YouTubeCreate { title: playlist.title.clone(), tracks: playlist.tracks.clone() });
        }
    });
    ui.add_space(10.0);
    if playlist.tracks.is_empty() {
        ui.label(RichText::new("Empty. Add songs from anywhere with their ⋯ menu › Add to playlist.").color(palette.faint));
    }
    track_list(ui, app, &playlist.tracks, RowPlace::Playlist(id.to_string()), actions);
}

fn remote_playlist(ui: &mut egui::Ui, app: &App, url: &str, inputs: &mut Inputs, actions: &mut Vec<Action>) {
    let palette = app.palette;
    back_button(ui, &palette, actions);
    match app.remote.get(url) {
        None | Some(Loadable::Loading) => {
            let known = app.library.followed.iter().find(|p| p.source_url.as_deref() == Some(url));
            if let Some(known) = known {
                header(ui, app, known.artwork_url.as_deref(), "PLAYLIST", &known.title, "");
            }
            widgets::loading(ui, &palette, "Reading the playlist…");
        }
        Some(Loadable::Failed(why)) => {
            notice(ui, &palette, why, true);
            if ui.button("Unfollow").clicked() {
                actions.push(Action::Unfollow(url.to_string()));
            }
        }
        Some(Loadable::Ready(playlist)) => {
            let shared = crate::sharelink::is_link(url);
            let owner = playlist.owner_name.clone().unwrap_or_else(|| playlist.provider.display_name().to_string());
            let detail = format!("{owner} · {}", summary(&playlist.tracks));
            let kind = if shared { "SHARED PLAYLIST".to_string() } else { format!("PLAYLIST ON {}", playlist.provider.display_name().to_uppercase()) };
            header(ui, app, playlist.artwork_url.as_deref(), &kind, &playlist.title, &detail);
            ui.horizontal(|ui| {
                play_buttons(ui, app, &playlist.tracks, actions);
                if shared {
                    if primary(ui, &palette, "⧉  Save a copy").on_hover_text("Keep it as one of your own playlists").clicked() {
                        actions.push(Action::SaveAsLocal(playlist.clone()));
                    }
                    return;
                }
                let followed = app.library.followed.iter().any(|p| p.source_url.as_deref() == Some(url));
                if followed {
                    if secondary(ui, &palette, "✓  Following").on_hover_text("Unfollow").clicked() {
                        actions.push(Action::Unfollow(url.to_string()));
                    }
                } else if secondary(ui, &palette, "＋  Follow").clicked() {
                    actions.push(Action::Follow(playlist.clone()));
                }
                if secondary(ui, &palette, "⧉  Save a copy").on_hover_text("Keep it as one of your own playlists").clicked() {
                    actions.push(Action::SaveAsLocal(playlist.clone()));
                }
                if secondary(ui, &palette, "🌐").on_hover_text("Open on the service").clicked() {
                    actions.push(Action::OpenUrl(url.to_string()));
                }
                if !playlist.tracks.is_empty() && secondary(ui, &palette, "🔗").on_hover_text("Copy a Noctorium link with these songs").clicked() {
                    actions.push(Action::Copy(crate::sharelink::encode(&playlist.title, &playlist.tracks)));
                }
            });
            // The listener's own YouTube playlists can be changed from here, as on YouTube Music.
            let own = app.youtube.is_some()
                && playlist.provider == crate::domain::ProviderType::YoutubeMusic
                && app.service_library.youtube_playlists.iter().any(|p| p.source_url.as_deref() == Some(url));
            let place = if own {
                youtube_editor(ui, app, url, playlist, inputs, actions);
                RowPlace::YouTube { id: playlist.id.clone(), url: url.to_string() }
            } else {
                RowPlace::List
            };
            ui.add_space(10.0);
            track_list(ui, app, &playlist.tracks, place, actions);
        }
    }
}

/// Edit and Delete for one of the listener's own YouTube playlists.
fn youtube_editor(ui: &mut egui::Ui, app: &App, url: &str, playlist: &crate::domain::Playlist, inputs: &mut Inputs, actions: &mut Vec<Action>) {
    use crate::accounts::{PlaylistEdit, Privacy};
    let palette = app.palette;
    let id = playlist.id.clone();
    ui.add_space(6.0);
    match &mut inputs.youtube_edit {
        Some((editing, title, description, privacy)) if *editing == id => {
            let mut close = false;
            egui::Frame::new().fill(palette.card).corner_radius(theme::corner(10)).inner_margin(12).show(ui, |ui| {
                ui.label(RichText::new("Edit on YouTube Music").font(theme::bold(14.0)).color(palette.text));
                ui.horizontal(|ui| {
                    ui.label("Name");
                    ui.add(egui::TextEdit::singleline(title).desired_width(320.0));
                });
                ui.label("Description (left empty, YouTube keeps the one it has)");
                ui.add(egui::TextEdit::multiline(description).desired_rows(3).desired_width(400.0));
                ui.horizontal(|ui| {
                    ui.label("Who can see it");
                    for choice in Privacy::ALL {
                        ui.selectable_value(privacy, choice, choice.label());
                    }
                });
                ui.horizontal(|ui| {
                    if primary(ui, &palette, "Save").clicked() {
                        let edit = PlaylistEdit {
                            title: (title.trim() != playlist.title).then(|| title.trim().to_string()),
                            description: Some(description.trim().to_string()).filter(|d| !d.is_empty()),
                            privacy: Some(*privacy),
                        };
                        actions.push(Action::YouTubeEdit { id: id.clone(), url: url.to_string(), edit });
                        close = true;
                    }
                    if secondary(ui, &palette, "Cancel").clicked() {
                        close = true;
                    }
                });
            });
            if close {
                inputs.youtube_edit = None;
            }
        }
        _ => {
            ui.horizontal(|ui| {
                if secondary(ui, &palette, "✎  Edit").on_hover_text("Name, description and who can see it").clicked() {
                    inputs.youtube_edit = Some((id.clone(), playlist.title.clone(), String::new(), Privacy::Private));
                }
                let key = format!("youtube:{id}");
                if inputs.confirm_delete.as_deref() == Some(key.as_str()) {
                    ui.label(RichText::new("Delete it from YouTube Music?").color(palette.error));
                    if ui.button(RichText::new("Delete").color(palette.error)).clicked() {
                        actions.push(Action::YouTubeDelete { id: id.clone(), url: url.to_string() });
                        inputs.confirm_delete = None;
                    }
                    if ui.button("Keep").clicked() {
                        inputs.confirm_delete = None;
                    }
                } else if secondary(ui, &palette, "🗑  Delete").clicked() {
                    inputs.confirm_delete = Some(key);
                }
            });
        }
    }
}

fn queue(ui: &mut egui::Ui, app: &App, actions: &mut Vec<Action>) {
    let palette = app.palette;
    ui.horizontal(|ui| {
        ui.label(RichText::new("Queue").font(theme::bold(28.0)).color(palette.text));
        ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
            if secondary(ui, &palette, "Clear").clicked() {
                actions.push(Action::QueueClear);
            }
            if secondary(ui, &palette, "Clear upcoming").clicked() {
                actions.push(Action::QueueClearUpcoming);
            }
            if secondary(ui, &palette, "🔀  Shuffle upcoming").clicked() {
                actions.push(Action::QueueShuffleUpcoming);
            }
        });
    });
    let q = &app.queue;
    if q.tracks.is_empty() {
        ui.add_space(12.0);
        ui.label(RichText::new("The queue is empty. Play something, or add songs from their ⋯ menu.").color(palette.faint));
        return;
    }
    ui.label(RichText::new(summary(&q.tracks)).color(palette.subtext));
    ui.add_space(8.0);
    heading(ui, &palette, "Now playing and up next", None);
    track_list(ui, app, &q.tracks, RowPlace::Queue, actions);
    if !q.suggestions.is_empty() {
        heading(ui, &palette, "Autoplay", Some("Plays when the queue runs out. Turn it off in Settings."));
        for (index, track) in q.suggestions.iter().enumerate() {
            ui.push_id(("suggestion", index), |ui| {
                ui.horizontal(|ui| {
                    widgets::artwork(ui, &palette, track.artwork_url.as_deref(), 36.0, 5);
                    ui.vertical(|ui| {
                        ui.add(egui::Label::new(RichText::new(&track.title).font(theme::bold(13.5))).truncate());
                        ui.label(RichText::new(track.artist_line()).font(theme::regular(12.5)).color(palette.subtext));
                    });
                    ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                        if ui.small_button("Keep").on_hover_text("Add it to the queue").clicked() {
                            actions.push(Action::KeepSuggestion(index));
                        }
                        if ui.small_button("▶").on_hover_text("Play it now").clicked() {
                            actions.push(Action::PlaySuggestion(index));
                        }
                    });
                });
            });
        }
    } else if app.settings.autoplay && q.up_next().is_empty() && q.suggestions_seed.is_some() {
        ui.add_space(8.0);
        widgets::loading(ui, &palette, "Autoplay is finding what comes next…");
    }
}

pub(super) fn tool_row(ui: &mut egui::Ui, app: &App, tool: Tool, info: &super::ToolInfo, actions: &mut Vec<Action>) {
    let palette = app.palette;
    egui::Frame::new()
        .fill(palette.card)
        .corner_radius(egui::CornerRadius::same(theme::corner(10)))
        .inner_margin(egui::Margin::same(12))
        .show(ui, |ui| {
            ui.set_width(ui.available_width());
            ui.horizontal(|ui| {
                ui.vertical(|ui| {
                    ui.label(RichText::new(tool.name()).font(theme::bold(15.0)));
                    match &info.path {
                        Some(path) => {
                            let version = info.version.clone().unwrap_or_else(|| "found".into());
                            ui.label(RichText::new(format!("{version} — {}", path.display())).font(theme::regular(12.5)).color(palette.subtext));
                        }
                        None => {
                            ui.label(RichText::new("Not found").color(palette.error));
                            if !tools::fetches_itself(tool) {
                                ui.label(RichText::new(tools::how_to_install(tool)).font(theme::regular(12.5)).color(palette.subtext));
                            }
                        }
                    }
                });
                ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                    if let Some(progress) = info.installing {
                        ui.add(egui::ProgressBar::new(progress).desired_width(160.0).show_percentage());
                    } else if tools::fetches_itself(tool) {
                        let label = if info.path.is_some() { "Update" } else { "Install" };
                        if primary(ui, &palette, label).on_hover_text("From the project's own GitHub releases").clicked() {
                            actions.push(Action::InstallTool(tool));
                        }
                    }
                });
            });
        });
    ui.add_space(6.0);
}

pub fn shortcuts_window(app: &App, ctx: &egui::Context, actions: &mut Vec<Action>) {
    let palette = app.palette;
    let mut open = true;
    egui::Window::new("Keyboard shortcuts")
        .open(&mut open)
        .collapsible(false)
        .resizable(false)
        .anchor(egui::Align2::CENTER_CENTER, vec2(0.0, 0.0))
        .show(ctx, |ui| {
            let command = if cfg!(target_os = "macos") { "⌘" } else { "Ctrl+" };
            egui::ScrollArea::vertical().max_height(520.0).show(ui, |ui| {
                egui::Grid::new("shortcuts").num_columns(2).spacing(vec2(28.0, 6.0)).show(ui, |ui| {
                    for action in crate::shortcuts::ACTIONS {
                        let chords = app.settings.shortcuts.in_app_chords(action);
                        if chords.is_empty() {
                            continue;
                        }
                        let keys = chords.iter().map(|c| c.to_string()).collect::<Vec<_>>().join("  or  ");
                        ui.label(RichText::new(keys).font(theme::bold(13.5)).color(palette.accent));
                        ui.label(action.title);
                        ui.end_row();
                    }
                });
            });
            let _ = command;
            ui.add_space(6.0);
            ui.label(RichText::new("Search takes bangs at the start or the end: !ytm, !sc, !yt, !all.").color(palette.faint));
        });
    if !open {
        actions.push(Action::ShowShortcuts(false));
    }
}


fn downloads_page(ui: &mut egui::Ui, app: &App, actions: &mut Vec<Action>) {
    let palette = app.palette;
    ui.horizontal(|ui| {
        ui.label(RichText::new("Downloads").font(theme::bold(28.0)).color(palette.text));
        ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
            if secondary(ui, &palette, "Open the folder").clicked() {
                actions.push(Action::OpenDownloadsFolder);
            }
        });
    });
    let megabytes = app.downloads.total_bytes() as f64 / 1_048_576.0;
    ui.label(
        RichText::new(format!("{} songs that play without the network · {megabytes:.0} MB", app.downloads.entries.len()))
            .color(palette.subtext),
    );
    ui.add_space(8.0);
    if !app.jobs.is_empty() {
        heading(ui, &palette, "Under way", None);
        let mut jobs: Vec<_> = app.jobs.values().collect();
        jobs.sort_by(|a, b| a.0.cmp(&b.0));
        for (what, progress) in jobs {
            ui.horizontal(|ui| {
                ui.add(egui::ProgressBar::new(*progress).desired_width(180.0).show_percentage());
                ui.label(what);
            });
        }
        ui.add_space(8.0);
    }
    if app.downloads.entries.is_empty() {
        ui.label(
            RichText::new("Nothing downloaded yet. A song's ⋯ menu › Download keeps it here to play offline; Save as a file puts an MP3 on your Desktop.")
                .color(palette.faint),
        );
        return;
    }
    let tracks: Vec<Track> = app.downloads.entries.iter().map(|e| e.track.clone()).collect();
    ui.horizontal(|ui| play_buttons(ui, app, &tracks, actions));
    ui.add_space(8.0);
    track_list(ui, app, &tracks, RowPlace::List, actions);
}

pub(super) fn download_settings(ui: &mut egui::Ui, app: &App, actions: &mut Vec<Action>) {
    use crate::downloads::ExportFormat;
    let palette = app.palette;
    ui.horizontal(|ui| {
        ui.label("Save files as");
        for format in [ExportFormat::Mp3, ExportFormat::Original] {
            if ui.selectable_label(app.settings.export_format == format, format.name()).clicked() {
                actions.push(Action::SetExportFormat(format));
            }
        }
    });
    ui.horizontal(|ui| {
        ui.label("Into");
        let shown = app
            .settings
            .export_folder
            .clone()
            .or_else(|| crate::downloads::default_export_folder().map(|p| p.display().to_string()))
            .unwrap_or_default();
        let mut folder = shown.clone();
        let field = ui.add(egui::TextEdit::singleline(&mut folder).desired_width(420.0));
        if field.lost_focus() && folder != shown {
            actions.push(Action::SetExportFolder(Some(folder)));
        }
        if app.settings.export_folder.is_some() && ui.button("Desktop").clicked() {
            actions.push(Action::SetExportFolder(None));
        }
    });
    ui.label(
        RichText::new("An MP3 is made by mpv at 320 kbps, with the song's title, artist, album and cover written into it.")
            .font(theme::regular(12.5))
            .color(palette.faint),
    );
}

pub(super) fn discord_settings(ui: &mut egui::Ui, app: &App, actions: &mut Vec<Action>) {
    use crate::discord::{ActivityKind, Paused, Timestamps};
    let palette = app.palette;
    let mut d = app.settings.discord.clone();
    let mut changed = false;
    ui.horizontal(|ui| {
        changed |= ui.checkbox(&mut d.enabled, "Show what is playing on Discord").changed();
        if d.enabled {
            let (text, colour) = if app.presence.connected() {
                ("Connected", palette.accent)
            } else {
                ("Waiting for Discord", palette.faint)
            };
            ui.label(RichText::new(text).font(theme::regular(12.5)).color(colour));
        }
    });
    if d.enabled {
        egui::Grid::new("discord").num_columns(2).spacing(vec2(14.0, 8.0)).show(ui, |ui| {
            ui.label("Shown as");
            ui.horizontal(|ui| {
                for kind in [ActivityKind::Listening, ActivityKind::Playing] {
                    changed |= ui.selectable_value(&mut d.activity_kind, kind, kind.name()).changed();
                }
            });
            ui.end_row();
            ui.label("First line");
            changed |= ui.add(egui::TextEdit::singleline(&mut d.details_template).desired_width(260.0)).lost_focus();
            ui.end_row();
            ui.label("Second line");
            changed |= ui.add(egui::TextEdit::singleline(&mut d.state_template).desired_width(260.0)).lost_focus();
            ui.end_row();
            ui.label("Clock");
            ui.horizontal(|ui| {
                for stamps in Timestamps::ALL {
                    changed |= ui.selectable_value(&mut d.timestamps, stamps, stamps.name()).changed();
                }
            });
            ui.end_row();
            ui.label("When paused");
            ui.horizontal(|ui| {
                for paused in Paused::ALL {
                    changed |= ui.selectable_value(&mut d.paused, paused, paused.name()).changed();
                }
            });
            ui.end_row();
            ui.label("Button");
            ui.horizontal(|ui| {
                changed |= ui.add(egui::TextEdit::singleline(&mut d.first_button.label).hint_text("Label").desired_width(160.0)).lost_focus();
                changed |= ui.add(egui::TextEdit::singleline(&mut d.first_button.url).hint_text("Link").desired_width(160.0)).lost_focus();
            });
            ui.end_row();
        });
        changed |= ui.checkbox(&mut d.show_cover, "Show the cover").changed();
        changed |= ui.checkbox(&mut d.hide_track_details, "Keep it private: say only that music is playing").changed();
        ui.label(
            RichText::new("{title}, {artist}, {album}, {provider}, {url}, {position} and {duration} are filled in.")
                .font(theme::regular(12.5))
                .color(palette.faint),
        );
    }
    if changed {
        actions.push(Action::SetDiscord(d));
    }
}

/// The quick search bar, Ctrl+F: results as you type, Enter plays, ↑ and ↓ choose, Esc closes.
pub fn quick_search(app: &App, ctx: &egui::Context, quick: Option<&mut super::QuickSearch>, actions: &mut Vec<Action>) {
    let Some(quick) = quick else { return };
    let palette = app.palette;
    let radio = app.search_kind == super::SearchKind::Radio;
    let results: Vec<Track> = match &quick.results {
        Some(Ok(tracks)) => tracks.clone(),
        _ => vec![],
    };
    let (up, down, enter, escape) = ctx.input_mut(|i| {
        (
            i.consume_key(egui::Modifiers::NONE, egui::Key::ArrowUp),
            i.consume_key(egui::Modifiers::NONE, egui::Key::ArrowDown),
            i.key_pressed(egui::Key::Enter),
            i.consume_key(egui::Modifiers::NONE, egui::Key::Escape),
        )
    });
    if escape {
        actions.push(Action::CloseQuickSearch);
        return;
    }
    if down && !results.is_empty() {
        quick.selected = (quick.selected + 1).min(results.len() - 1);
    }
    if up {
        quick.selected = quick.selected.saturating_sub(1);
    }
    egui::Area::new(egui::Id::new("quick-search"))
        .order(egui::Order::Foreground)
        .anchor(egui::Align2::CENTER_TOP, vec2(0.0, 70.0))
        .show(ctx, |ui| {
            egui::Frame::new()
                .fill(palette.card)
                .stroke(Stroke::new(1.0_f32, palette.accent.gamma_multiply(0.6)))
                .corner_radius(egui::CornerRadius::same(theme::corner(16)))
                .inner_margin(egui::Margin::same(14))
                .shadow(egui::Shadow { offset: [0, 8], blur: 32, spread: 0, color: egui::Color32::from_black_alpha(140) })
                .show(ui, |ui| {
                    ui.set_width(560.0);
                    // Music or Radio, small, above the line you type on -- Ctrl+F again flips it.
                    super::radio::kind_picker(ui, app, true, actions);
                    ui.add_space(4.0);
                    let field = ui
                        .horizontal(|ui| {
                            ui.add(
                                egui::TextEdit::singleline(&mut quick.text)
                                    .hint_text(if radio { "A station, a city, or “jazz”" } else { "Search — !sc, !ytm, !yt at the start or the end" })
                                    .font(theme::regular(17.0))
                                    .frame(false)
                                    .desired_width(f32::INFINITY),
                            )
                        })
                        .inner;
                    if quick.focus {
                        field.request_focus();
                        quick.focus = false;
                    }
                    if field.changed() {
                        quick.typed_at = std::time::Instant::now();
                    }
                    let play = |index: usize, actions: &mut Vec<Action>| {
                        actions.push(if radio {
                            Action::PlayStation(results[index].clone())
                        } else {
                            Action::PlayList { tracks: results.clone(), index }
                        });
                        actions.push(Action::CloseQuickSearch);
                    };
                    if enter {
                        if quick.selected < results.len() {
                            play(quick.selected, actions);
                        } else if radio || !quick.text.trim().is_empty() {
                            actions.push(Action::QuickSearchAll(quick.text.clone()));
                        }
                    }
                    let blank = quick.text.trim().is_empty();
                    let footer = super::radio::quick_footer(app.search_kind, blank);
                    if blank && !radio {
                        ui.add_space(6.0);
                        ui.label(RichText::new(footer).font(theme::regular(11.5)).color(palette.faint));
                        return;
                    }
                    ui.separator();
                    match &quick.results {
                        None if radio => widgets::loading(ui, &palette, "Tuning in to the world's stations…"),
                        None => widgets::loading(ui, &palette, "Searching…"),
                        Some(Err(why)) => notice(ui, &palette, why, true),
                        Some(Ok(tracks)) if tracks.is_empty() && radio => {
                            ui.label(RichText::new(format!("No station called “{}”. Try a city, or a kind of music such as “jazz”.", quick.text.trim())).color(palette.faint));
                        }
                        Some(Ok(tracks)) if tracks.is_empty() => {
                            ui.label(RichText::new("Nothing found.").color(palette.faint));
                        }
                        Some(Ok(_)) => {
                            for (index, track) in results.iter().enumerate() {
                                let selected = index == quick.selected;
                                let row = ui.horizontal(|ui| {
                                    if radio {
                                        let (rect, _) = ui.allocate_exact_size(vec2(36.0, 36.0), egui::Sense::hover());
                                        super::radio::logo(ui, &palette, track.artwork_url.as_deref(), rect);
                                    } else {
                                        widgets::artwork(ui, &palette, track.artwork_url.as_deref(), 36.0, 5);
                                    }
                                    ui.vertical(|ui| {
                                        let colour = if selected { palette.accent } else { palette.text };
                                        ui.add(egui::Label::new(RichText::new(&track.title).font(theme::bold(14.0)).color(colour)).truncate());
                                        ui.label(RichText::new(track.artist_line()).font(theme::regular(12.5)).color(palette.subtext));
                                    });
                                });
                                let response = ui.interact(row.response.rect, egui::Id::new(("quick", index)), egui::Sense::click());
                                if selected || response.hovered() {
                                    ui.painter().rect_filled(row.response.rect.expand(2.0), egui::CornerRadius::same(theme::corner(6)), palette.wash(0.08));
                                }
                                if response.clicked() {
                                    play(index, actions);
                                }
                            }
                        }
                    }
                    ui.add_space(4.0);
                    if ui.link(if radio { "All stations on the search page" } else { "All results on the search page" }).clicked() {
                        actions.push(Action::QuickSearchAll(quick.text.clone()));
                    }
                    ui.label(RichText::new(footer).font(theme::regular(11.5)).color(palette.faint));
                });
        });
}

pub(super) fn scrobble_settings(ui: &mut egui::Ui, app: &App, inputs: &mut Inputs, actions: &mut Vec<Action>) {
    use crate::scrobble::Site;
    let palette = app.palette;
    for site in Site::ALL {
        egui::Frame::new()
            .fill(palette.card)
            .corner_radius(egui::CornerRadius::same(theme::corner(10)))
            .inner_margin(egui::Margin::same(12))
            .show(ui, |ui| {
                ui.set_width(ui.available_width());
                ui.horizontal(|ui| {
                    ui.label(RichText::new(site.name()).font(theme::bold(15.0)));
                    match app.scrobblers.get(&site) {
                        Some(account) => {
                            ui.label(RichText::new(format!("Connected as {}", account.username)).color(palette.accent));
                            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                                if secondary(ui, &palette, "Disconnect").clicked() {
                                    actions.push(Action::DisconnectScrobbler(site));
                                }
                            });
                        }
                        None if site == Site::ListenBrainz => {
                            ui.add(
                                egui::TextEdit::singleline(&mut inputs.listenbrainz_token)
                                    .hint_text("Your user token, from listenbrainz.org/settings")
                                    .password(true)
                                    .desired_width(300.0),
                            );
                            if primary(ui, &palette, "Connect").clicked() {
                                actions.push(Action::ConnectListenBrainz(std::mem::take(&mut inputs.listenbrainz_token)));
                            }
                            if ui.link("Get the token").clicked() {
                                actions.push(Action::OpenUrl("https://listenbrainz.org/settings/".into()));
                            }
                        }
                        None => match app.authorizing.as_ref().filter(|a| a.site == site) {
                            Some(auth) => {
                                ui.add(egui::Spinner::new().color(palette.accent));
                                ui.label(RichText::new("Allow Noctorium on the page that opened in your browser…").color(palette.subtext));
                                if let Some(url) = &auth.url {
                                    if ui.link("Open it again").clicked() {
                                        actions.push(Action::OpenUrl(url.clone()));
                                    }
                                    if ui.link("Copy it").clicked() {
                                        actions.push(Action::Copy(url.clone()));
                                    }
                                }
                                if ui.button("Cancel").clicked() {
                                    actions.push(Action::CancelAuthorization);
                                }
                            }
                            None => {
                                if primary(ui, &palette, "Connect").clicked() {
                                    actions.push(Action::ConnectScrobbler(site));
                                }
                            }
                        },
                    }
                });
            });
        ui.add_space(6.0);
    }
}

/// Library › the listener's own YouTube Music and SoundCloud.
fn service_library(ui: &mut egui::Ui, app: &App, actions: &mut Vec<Action>) {
    use crate::domain::ProviderType;
    let palette = app.palette;
    let lib = &app.service_library;
    for (provider, signed_in, likes, playlists) in [
        (ProviderType::YoutubeMusic, app.youtube.is_some(), &lib.youtube_likes, &lib.youtube_playlists),
        (ProviderType::Soundcloud, app.soundcloud.is_some(), &lib.soundcloud_likes, &lib.soundcloud_playlists),
    ] {
        if !signed_in {
            continue;
        }
        heading(ui, &palette, &format!("Your {}", provider.display_name()), None);
        card_strip(ui, ("service", provider.key_name()), |ui| {
            let (art, count) = match likes {
                Some(Ok(tracks)) => (tracks.first().and_then(|t| t.artwork_url.clone()), format!("{} songs", tracks.len())),
                Some(Err(_)) => (None, "Could not be read".to_string()),
                None => (None, "Reading…".to_string()),
            };
            if card(ui, &palette, art.as_deref(), "Liked songs", &count).clicked() {
                actions.push(Action::Open(Page::ServiceLikes(provider)));
            }
            for playlist in playlists {
                let Some(url) = &playlist.source_url else { continue };
                let subtitle = playlist.owner_name.clone().unwrap_or_default();
                if card(ui, &palette, playlist.artwork_url.as_deref(), &playlist.title, &subtitle).clicked() {
                    actions.push(Action::Open(Page::Remote(url.clone())));
                }
            }
        });
    }
}

pub(super) fn account_settings(ui: &mut egui::Ui, app: &App, inputs: &mut Inputs, actions: &mut Vec<Action>) {
    let palette = app.palette;
    egui::Frame::new()
        .fill(palette.card)
        .corner_radius(egui::CornerRadius::same(theme::corner(10)))
        .inner_margin(egui::Margin::same(12))
        .show(ui, |ui| {
            ui.set_width(ui.available_width());
            if let Some((user, _)) = &app.account {
                ui.horizontal(|ui| {
                    ui.label(RichText::new(&user.display_name).font(theme::bold(15.0)));
                    ui.label(RichText::new(&user.email).color(palette.subtext));
                    ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                        if secondary(ui, &palette, "Sign out").clicked() {
                            actions.push(Action::AccountSignOut);
                        }
                    });
                });
                if let Some(stats) = &app.account_stats {
                    ui.add_space(6.0);
                    ui.horizontal(|ui| {
                        ui.spacing_mut().item_spacing.x = 28.0;
                        for (figure, what) in [
                            (stats.streams.to_string(), "plays"),
                            (stats.unique_tracks.to_string(), "songs"),
                            (stats.artists.to_string(), "artists"),
                            (format!("{:.1}", stats.hours), "hours"),
                        ] {
                            ui.vertical(|ui| {
                                ui.label(RichText::new(figure).font(theme::bold(22.0)).color(palette.accent));
                                ui.label(RichText::new(what).color(palette.subtext));
                            });
                        }
                    });
                }
                return;
            }
            ui.horizontal(|ui| {
                ui.selectable_value(&mut inputs.account_new, false, "Sign in");
                ui.selectable_value(&mut inputs.account_new, true, "Make an account");
            });
            egui::Grid::new("account").num_columns(2).spacing(vec2(12.0, 8.0)).show(ui, |ui| {
                if inputs.account_new {
                    ui.label("Name");
                    ui.add(egui::TextEdit::singleline(&mut inputs.account_name).desired_width(260.0));
                    ui.end_row();
                }
                ui.label("Email");
                ui.add(egui::TextEdit::singleline(&mut inputs.account_email).desired_width(260.0));
                ui.end_row();
                ui.label("Password");
                let field = ui.add(egui::TextEdit::singleline(&mut inputs.account_password).password(true).desired_width(260.0));
                ui.end_row();
                let enter = field.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter));
                ui.label("");
                ui.horizontal(|ui| {
                    if app.account_busy {
                        ui.add(egui::Spinner::new().color(palette.accent));
                        return;
                    }
                    let label = if inputs.account_new { "Make the account" } else { "Sign in" };
                    let ready = !inputs.account_email.trim().is_empty() && !inputs.account_password.is_empty();
                    if (primary(ui, &palette, label).clicked() || enter) && ready {
                        let (email, password) = (inputs.account_email.trim().to_string(), std::mem::take(&mut inputs.account_password));
                        actions.push(if inputs.account_new {
                            Action::AccountSignUp { email, password, name: inputs.account_name.clone() }
                        } else {
                            Action::AccountLogIn { email, password }
                        });
                    }
                });
                ui.end_row();
            });
        });
}

/// The services beyond YouTube and SoundCloud. Their songs are matched on YouTube Music to be played, since
/// each lets only its own players make the sound.
fn services_page(ui: &mut egui::Ui, app: &App, inputs: &mut Inputs, actions: &mut Vec<Action>) {
    let palette = app.palette;
    ui.label(RichText::new("Services").font(theme::bold(28.0)).color(palette.text));
    ui.label(
        RichText::new("Their catalogues and your libraries on them. Their songs play by being found on YouTube Music.")
            .color(palette.subtext),
    );
    ui.add_space(6.0);
    notice(ui, &palette, crate::services::BETA_WARNING, false);

    heading(ui, &palette, "Spotify", Some("Its songs are found on YouTube Music to be played. Paste a public playlist or album link into Search to open it."));
    spotify_section(ui, app, inputs, actions);
    heading(ui, &palette, "Apple Music", Some("Search it with !am. Paste an album or playlist link into Search to open it."));
    apple_music_section(ui, app, inputs, actions);
    heading(ui, &palette, "Tidal", Some("Search it with !td. Tidal lets programs in only through an app of your own."));
    tidal_section(ui, app, inputs, actions);
    heading(ui, &palette, "VK Music", Some("Search it with !vk, with your VK sign-in. Paste a VK playlist link into Search to open it."));
    vk_section(ui, app, inputs, actions);
    heading(ui, &palette, "Bandcamp", Some("Search it with !bc. Paste an album or song link into Search to open it. Played as the stream Bandcamp gives anyone."));
    bandcamp_section(ui, app, inputs, actions);
    heading(ui, &palette, "Deezer", Some("Search it with !dz. Paste a playlist or album link into Search to open it."));
    deezer_section(ui, app, inputs, actions);
}

/// Services and Settings › Spotify: its sign-in and what is on it.
pub(super) fn spotify_section(ui: &mut egui::Ui, app: &App, inputs: &mut Inputs, actions: &mut Vec<Action>) {
    let palette = app.palette;
    ui.horizontal(|ui| match &app.services.spotify_name {
        Some(name) => {
            ui.label(RichText::new(format!("Signed in as {name}")).color(palette.accent));
            if ui.button("Sign out").clicked() {
                actions.push(Action::SpotifySignOut);
            }
        }
        None if app.services.spotify_signing_in => {
            widgets::loading(ui, &palette, "Finish signing in on the page that opened in your browser…");
        }
        None => {
            if primary(ui, &palette, "Sign in with Spotify").clicked() {
                actions.push(Action::SpotifySignIn);
            }
            ui.label(RichText::new("for search with !sp, your playlists and your Liked Songs").color(palette.faint));
        }
    });
    if app.services.spotify_name.is_some() {
        card_strip(ui, "spotify-library", |ui| {
            let (art, count) = match &app.services.spotify_likes {
                Some(Ok(t)) => (t.first().and_then(|x| x.artwork_url.clone()), format!("{} songs", t.len())),
                Some(Err(_)) => (None, "Could not be read".into()),
                None => (None, "Reading…".into()),
            };
            if card(ui, &palette, art.as_deref(), "Liked Songs", &count).clicked() {
                actions.push(Action::Open(Page::ServiceLikes(crate::domain::ProviderType::Spotify)));
            }
            for playlist in &app.services.spotify_playlists {
                let Some(url) = &playlist.source_url else { continue };
                let subtitle = playlist.track_count.map(|n| format!("{n} songs")).unwrap_or_default();
                if card(ui, &palette, playlist.artwork_url.as_deref(), &playlist.title, &subtitle).clicked() {
                    actions.push(Action::Open(Page::Remote(url.clone())));
                }
            }
        });
        if !app.services.spotify_top.is_empty() {
            strip(ui, app, "Your top songs", Some("On Spotify"), &app.services.spotify_top, "spotify-top", actions);
        }
        if !app.services.spotify_recent.is_empty() {
            strip(ui, app, "Played lately", Some("On Spotify"), &app.services.spotify_recent, "spotify-recent", actions);
        }
    }
    ui.horizontal(|ui| {
        ui.label("Spotify's data download");
        let field = ui.add(
            egui::TextEdit::singleline(&mut inputs.spotify_export)
                .hint_text("The path of the zip Spotify emailed, or the folder it unpacks to")
                .desired_width(380.0),
        );
        let enter = field.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter));
        if (secondary(ui, &palette, "Import").clicked() || enter) && !inputs.spotify_export.trim().is_empty() {
            actions.push(Action::ImportSpotifyData(inputs.spotify_export.clone()));
        }
    });
    ui.label(
        RichText::new("Its playlists and Liked Songs become playlists of your own here. Importing again brings them up to date.")
            .font(theme::regular(12.5))
            .color(palette.faint),
    );
}
pub(super) const SPOTIFY_ABOUT: &str = "Its songs are found on YouTube Music to be played. Paste a public playlist or album link into Search to open it.";

/// Services and Settings › Apple Music: its sign-in and what is on it.
pub(super) fn apple_music_section(ui: &mut egui::Ui, app: &App, _inputs: &mut Inputs, actions: &mut Vec<Action>) {
    let palette = app.palette;
    match &app.services.apple_charts {
        None => widgets::loading(ui, &palette, "Reading Apple Music's charts…"),
        Some(Err(why)) => notice(ui, &palette, why, true),
        Some(Ok((songs, playlists))) => {
            strip(ui, app, "Top songs", Some(&format!("Apple Music, {}", app.settings.apple_storefront.to_uppercase())), songs, "apple-chart", actions);
            if !playlists.is_empty() {
                card_strip(ui, "apple-chart-playlists", |ui| {
                    for playlist in playlists {
                        let Some(url) = &playlist.source_url else { continue };
                        if card(ui, &palette, playlist.artwork_url.as_deref(), &playlist.title, "Apple Music").clicked() {
                            actions.push(Action::Open(Page::Remote(url.clone())));
                        }
                    }
                });
            }
        }
    }
    ui.horizontal(|ui| match &app.services.apple_library {
        Some(Ok(_)) => {
            ui.label(RichText::new("Your library is read").color(palette.accent));
            if ui.button("Sign out").clicked() {
                actions.push(Action::AppleSignOut);
            }
        }
        _ if app.services.apple_signed_in => widgets::loading(ui, &palette, "Reading your library…"),
        _ => {
            if primary(ui, &palette, "Sign in from your browser").clicked() {
                actions.push(Action::AppleSignIn);
            }
            ui.label(RichText::new("where you are signed in to music.apple.com, for your playlists and songs").color(palette.faint));
        }
    });
    if let Some(Ok((playlists, songs))) = &app.services.apple_library {
        card_strip(ui, "apple-library", |ui| {
            let art = songs.first().and_then(|t| t.artwork_url.as_deref());
            if card(ui, &palette, art, "Your songs", &format!("{} songs", songs.len())).clicked() {
                actions.push(Action::Open(Page::ServiceLikes(crate::domain::ProviderType::AppleMusic)));
            }
            for playlist in playlists {
                let Some(url) = &playlist.source_url else { continue };
                if card(ui, &palette, playlist.artwork_url.as_deref(), &playlist.title, "Your playlist").clicked() {
                    actions.push(Action::Open(Page::Remote(url.clone())));
                }
            }
        });
    }
}
pub(super) const APPLE_MUSIC_ABOUT: &str = "Search it with !am. Paste an album or playlist link into Search to open it.";

/// Services and Settings › Tidal: its sign-in and what is on it.
pub(super) fn tidal_section(ui: &mut egui::Ui, app: &App, inputs: &mut Inputs, actions: &mut Vec<Action>) {
    let palette = app.palette;
    if inputs.tidal_client_id.is_empty() {
        inputs.tidal_client_id = app.settings.tidal_client_id.clone();
    }
    ui.horizontal(|ui| {
        ui.add(egui::TextEdit::singleline(&mut inputs.tidal_client_id).hint_text("Client id").desired_width(220.0));
        ui.add(egui::TextEdit::singleline(&mut inputs.tidal_client_secret).hint_text("Client secret").password(true).desired_width(220.0));
        if secondary(ui, &palette, "Save").clicked() && !inputs.tidal_client_id.trim().is_empty() {
            actions.push(Action::SetTidalApp {
                client_id: inputs.tidal_client_id.clone(),
                client_secret: std::mem::take(&mut inputs.tidal_client_secret),
            });
        }
        if ui.link("Make an app").clicked() {
            actions.push(Action::OpenUrl(crate::tidal::DEVELOPER_PORTAL.into()));
        }
    });
    ui.label(
        RichText::new("At developer.tidal.com, add http://127.0.0.1:8889/noctorium/tidal as the app's redirect address.")
            .font(theme::regular(12.5))
            .color(palette.faint),
    );
    ui.horizontal(|ui| {
        if app.services.tidal_ready {
            ui.label(RichText::new("The catalogue is readable").color(palette.accent));
        }
        match &app.services.tidal_name {
            Some(name) => {
                ui.label(RichText::new(format!("Signed in as {name}")).color(palette.accent));
                if ui.button("Sign out").clicked() {
                    actions.push(Action::TidalSignOut);
                }
            }
            None if app.services.tidal_signing_in => widgets::loading(ui, &palette, "Finish signing in in your browser…"),
            None => {
                if !app.settings.tidal_client_id.is_empty() && secondary(ui, &palette, "Sign in to Tidal").clicked() {
                    actions.push(Action::TidalSignIn);
                }
            }
        }
    });
    if let Some(Ok((songs, playlists))) = &app.services.tidal_collection {
        card_strip(ui, "tidal-collection", |ui| {
            if card(ui, &palette, None, "Your collection", &format!("{} songs", songs.len())).clicked() {
                actions.push(Action::Open(Page::ServiceLikes(crate::domain::ProviderType::Tidal)));
            }
            for playlist in playlists {
                let Some(url) = &playlist.source_url else { continue };
                let subtitle = playlist.track_count.map(|n| format!("{n} songs")).unwrap_or_default();
                if card(ui, &palette, None, &playlist.title, &subtitle).clicked() {
                    actions.push(Action::Open(Page::Remote(url.clone())));
                }
            }
        });
    }
}
pub(super) const TIDAL_ABOUT: &str = "Search it with !td. Tidal lets programs in only through an app of your own.";

/// Services and Settings › VK: its sign-in and what is on it.
pub(super) fn vk_section(ui: &mut egui::Ui, app: &App, _inputs: &mut Inputs, actions: &mut Vec<Action>) {
    let palette = app.palette;
    ui.horizontal(|ui| match &app.services.vk_name {
        Some(name) => {
            ui.label(RichText::new(format!("Signed in as {name}")).color(palette.accent));
            if ui.button("Sign out").clicked() {
                actions.push(Action::VkSignOut);
            }
        }
        None if app.services.vk_signing_in => widgets::loading(ui, &palette, "Reading the browser's sign-in…"),
        None => {
            if primary(ui, &palette, "Sign in from your browser").clicked() {
                actions.push(Action::VkSignIn);
            }
            ui.label(RichText::new("where you are signed in to vk.ru").color(palette.faint));
        }
    });
    if app.services.vk_name.is_some() {
        card_strip(ui, "vk-library", |ui| {
            let (art, count) = match &app.services.vk_music {
                Some(Ok(t)) => (t.first().and_then(|x| x.artwork_url.clone()), format!("{} songs", t.len())),
                Some(Err(_)) => (None, "Could not be read".into()),
                None => (None, "Reading…".into()),
            };
            if card(ui, &palette, art.as_deref(), "My music", &count).clicked() {
                actions.push(Action::Open(Page::ServiceLikes(crate::domain::ProviderType::Vk)));
            }
            for playlist in &app.services.vk_playlists {
                let Some(url) = &playlist.source_url else { continue };
                let subtitle = playlist.track_count.map(|n| format!("{n} songs")).unwrap_or_default();
                if card(ui, &palette, playlist.artwork_url.as_deref(), &playlist.title, &subtitle).clicked() {
                    actions.push(Action::Open(Page::Remote(url.clone())));
                }
            }
        });
        if !app.services.vk_recommended.is_empty() {
            strip(ui, app, "For you", Some("On VK"), &app.services.vk_recommended, "vk-recommended", actions);
        }
    }
}
pub(super) const VK_ABOUT: &str = "Search it with !vk, with your VK sign-in. Paste a VK playlist link into Search to open it.";

/// Services and Settings › Bandcamp: its sign-in and what is on it.
pub(super) fn bandcamp_section(ui: &mut egui::Ui, app: &App, _inputs: &mut Inputs, actions: &mut Vec<Action>) {
    let palette = app.palette;
    for (title, rows, id) in [
        ("Best selling", &app.services.bandcamp_top, "bandcamp-top"),
        ("New arrivals", &app.services.bandcamp_new, "bandcamp-new"),
    ] {
        match rows {
            None => widgets::loading(ui, &palette, "Reading Bandcamp…"),
            Some(Err(why)) => notice(ui, &palette, why, true),
            Some(Ok(tracks)) if !tracks.is_empty() => strip(ui, app, title, Some("On Bandcamp"), tracks, id, actions),
            Some(Ok(_)) => {}
        }
    }
}
pub(super) const BANDCAMP_ABOUT: &str = "Search it with !bc. Paste an album or song link into Search to open it. Played as the stream Bandcamp gives anyone.";

/// Services and Settings › Deezer: its sign-in and what is on it.
pub(super) fn deezer_section(ui: &mut egui::Ui, app: &App, inputs: &mut Inputs, actions: &mut Vec<Action>) {
    let palette = app.palette;
    match &app.services.deezer_chart {
        None => widgets::loading(ui, &palette, "Reading Deezer's charts…"),
        Some(Err(why)) => notice(ui, &palette, why, true),
        Some(Ok((tracks, playlists))) => {
            strip(ui, app, "Top songs", Some("Deezer's charts"), tracks, "deezer-chart", actions);
            if !playlists.is_empty() {
                card_strip(ui, "deezer-chart-playlists", |ui| {
                    for playlist in playlists {
                        let Some(url) = &playlist.source_url else { continue };
                        let subtitle = playlist.owner_name.clone().unwrap_or_else(|| "Deezer".into());
                        if card(ui, &palette, playlist.artwork_url.as_deref(), &playlist.title, &subtitle).clicked() {
                            actions.push(Action::Open(Page::Remote(url.clone())));
                        }
                    }
                });
            }
        }
    }
    ui.horizontal(|ui| {
        ui.label("Your profile");
        if inputs.deezer_profile.is_empty() {
            if let Some(saved) = &app.settings.deezer_profile {
                inputs.deezer_profile = saved.clone();
            }
        }
        let field = ui.add(
            egui::TextEdit::singleline(&mut inputs.deezer_profile)
                .hint_text("https://www.deezer.com/profile/…")
                .desired_width(340.0),
        );
        let enter = field.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter));
        if primary(ui, &palette, "Show").clicked() || enter {
            actions.push(Action::SetDeezerProfile(Some(inputs.deezer_profile.clone())));
        }
        if app.settings.deezer_profile.is_some() && ui.button("Forget").clicked() {
            inputs.deezer_profile.clear();
            actions.push(Action::SetDeezerProfile(None));
        }
    });
    ui.label(
        RichText::new("A public profile needs no sign-in: its loved tracks and playlists are read as anyone can see them.")
            .font(theme::regular(12.5))
            .color(palette.faint),
    );
    if let Some(Ok(profile)) = &app.services.deezer_profile {
        if !profile.loved.is_empty() {
            strip(ui, app, &format!("{}'s loved tracks", profile.name), None, &profile.loved, "deezer-loved", actions);
        }
        if !profile.playlists.is_empty() {
            card_strip(ui, "deezer-mine", |ui| {
                for playlist in &profile.playlists {
                    let Some(url) = &playlist.source_url else { continue };
                    let subtitle = playlist.track_count.map(|n| format!("{n} songs")).unwrap_or_default();
                    if card(ui, &palette, playlist.artwork_url.as_deref(), &playlist.title, &subtitle).clicked() {
                        actions.push(Action::Open(Page::Remote(url.clone())));
                    }
                }
            });
        }
    }
}
pub(super) const DEEZER_ABOUT: &str = "Search it with !dz. Paste a playlist or album link into Search to open it.";

/// Every shortcut, to change: in the window, and -- for the actions that can be -- from any program.
fn shortcuts_page(ui: &mut egui::Ui, app: &App, actions: &mut Vec<Action>) {
    let palette = app.palette;
    back_button(ui, &palette, actions);
    ui.label(RichText::new("Keyboard shortcuts").font(theme::bold(28.0)).color(palette.text));
    ui.label(RichText::new("Change: press the new keys. Esc leaves it as it was; Backspace leaves it with none.").color(palette.subtext));
    ui.add_space(6.0);
    shortcuts_editor(ui, app, actions);
}

/// The shortcuts themselves, on their own page and on Settings › Keyboard shortcuts.
pub(super) fn shortcuts_editor(ui: &mut egui::Ui, app: &App, actions: &mut Vec<Action>) {
    use crate::shortcuts::{Group, ACTIONS};
    let palette = app.palette;
    let mut on = app.settings.shortcuts.global_enabled;
    if ui
        .checkbox(&mut on, "Global shortcuts: these work from any program, through the system's own hot-key service")
        .changed()
    {
        actions.push(Action::SetGlobalShortcuts(on));
    }
    for group in [Group::Playing, Group::Sound, Group::Queue, Group::Places, Group::Window] {
        heading(ui, &palette, group.title(), None);
        egui::Grid::new(("keys", group.title())).num_columns(3).spacing(vec2(18.0, 6.0)).striped(true).show(ui, |ui| {
            for action in ACTIONS.iter().filter(|a| a.group == group) {
                ui.label(action.title);
                ui.horizontal(|ui| {
                    if app.capturing == Some((action.id, false)) {
                        ui.label(RichText::new("Press the keys…").color(palette.accent));
                    } else {
                        let chords = app.settings.shortcuts.in_app_chords(action);
                        let shown = if chords.is_empty() { "None".to_string() } else { chords.iter().map(|c| c.to_string()).collect::<Vec<_>>().join(" or ") };
                        if ui.button(RichText::new(shown).font(theme::bold(13.0))).on_hover_text("Change").clicked() {
                            actions.push(Action::Capture(Some((action.id, false))));
                        }
                        if app.settings.shortcuts.in_app.contains_key(action.id) && ui.small_button("Default").clicked() {
                            actions.push(Action::ResetShortcut(action.id));
                        }
                    }
                });
                ui.horizontal(|ui| {
                    if !action.global || !app.settings.shortcuts.global_enabled {
                        return;
                    }
                    if app.capturing == Some((action.id, true)) {
                        ui.label(RichText::new("Press the keys, with Ctrl, Alt or Shift…").color(palette.accent));
                        return;
                    }
                    let chord = app.settings.shortcuts.global_chord(action);
                    let label = chord.as_ref().map(|c| c.to_string()).unwrap_or_else(|| "No global key".into());
                    if ui.button(RichText::new(label).font(theme::bold(13.0))).on_hover_text("Change the global key").clicked() {
                        actions.push(Action::Capture(Some((action.id, true))));
                    }
                    // A chord another program holds is shown as taken.
                    let registered = app.global_actions.iter().position(|i| std::ptr::eq(&ACTIONS[*i], action));
                    if let Some(position) = registered {
                        if !app.global_granted.get(position).copied().unwrap_or(false) {
                            ui.label(RichText::new("Taken by another program").color(palette.error));
                        }
                    }
                });
                ui.end_row();
            }
        });
    }
    let media = if app.media_keys.is_empty() { "held by another program".to_string() } else { app.media_keys.join(", ") };
    ui.add_space(8.0);
    ui.label(RichText::new(format!("The keyboard's media keys: {media}.")).color(palette.faint));
}

/// Now playing: the song large under the theme's light, its bars measured from the sound itself, and its
/// lyrics or what comes after it beside it.
fn now_playing(ui: &mut egui::Ui, app: &App, actions: &mut Vec<Action>) {
    use crate::looks::{CoverStyle, NowPlayingLayout};
    let palette = app.palette;
    let Some(track) = app.loaded.as_ref().or(app.resolving.as_ref()).or(app.queue.current()).cloned() else {
        ui.add_space(40.0);
        ui.vertical_centered(|ui| {
            ui.label(RichText::new("Nothing playing").font(theme::bold(26.0)).color(palette.subtext));
            ui.label(RichText::new("Play something, and it is shown here large.").color(palette.faint));
        });
        return;
    };
    let layout = app.settings.looks.now_playing;
    // The full-cover layouts are their own backdrop.
    if !matches!(layout, NowPlayingLayout::Immersive | NowPlayingLayout::Split) {
        now_playing_backdrop(ui, app, &track);
    }
    let width = ui.available_width();
    // What is left of the page below here, for the lyrics to scroll within.
    let room = |ui: &egui::Ui| (ui.clip_rect().bottom() - ui.cursor().top() - 30.0).max(220.0);
    let column = |ui: &mut egui::Ui, w: f32, h: f32, add: &mut dyn FnMut(&mut egui::Ui)| {
        ui.allocate_ui_with_layout(vec2(w, h), Layout::top_down(Align::Min), |ui| {
            ui.set_width(w);
            add(ui)
        });
    };
    match layout {
        NowPlayingLayout::SideBySide | NowPlayingLayout::PanelLeft => {
            let panel_width = app.settings.looks.panel_width.width().min(width * 0.5);
            let side = (width - panel_width - 28.0).clamp(200.0, 520.0) / app.settings.looks.cover_size.scale().max(1.0);
            let height = room(ui);
            ui.horizontal_top(|ui| {
                let cover_column = |ui: &mut egui::Ui, actions: &mut Vec<Action>| {
                    now_playing_cover(ui, app, &track, side, app.settings.looks.cover);
                    ui.add_space(14.0);
                    now_playing_details(ui, app, &track, side, false, actions);
                };
                if layout == NowPlayingLayout::PanelLeft {
                    column(ui, panel_width, height, &mut |ui| now_playing_side(ui, app, &track, false, actions));
                    ui.add_space(28.0);
                    column(ui, side, height, &mut |ui| cover_column(ui, actions));
                } else {
                    column(ui, side, height, &mut |ui| cover_column(ui, actions));
                    ui.add_space(28.0);
                    column(ui, panel_width, height, &mut |ui| now_playing_side(ui, app, &track, false, actions));
                }
            });
        }
        NowPlayingLayout::Stage => {
            let height = room(ui);
            let side = (width * 0.45).min(height - 20.0).clamp(220.0, 480.0);
            let details = (width - side - 40.0).clamp(240.0, 440.0);
            ui.add_space(((height - side) / 2.0).max(0.0));
            ui.horizontal_top(|ui| {
                ui.add_space(((width - side - 40.0 - details) / 2.0).max(0.0));
                now_playing_cover(ui, app, &track, side, app.settings.looks.cover);
                ui.add_space(40.0);
                column(ui, details, side, &mut |ui| {
                    ui.add_space((side * 0.18).max(0.0));
                    now_playing_details(ui, app, &track, details, false, actions);
                });
            });
        }
        NowPlayingLayout::Focus => {
            let side = (width * 0.6).min(room(ui) - 250.0).clamp(200.0, 520.0);
            ui.vertical_centered(|ui| {
                ui.add_space(10.0);
                now_playing_cover(ui, app, &track, side, app.settings.looks.cover);
                ui.add_space(18.0);
                now_playing_details(ui, app, &track, side.max(380.0), true, actions);
            });
        }
        NowPlayingLayout::SingAlong => {
            let height = room(ui);
            let side = 240.0_f32.min(width * 0.3);
            ui.horizontal_top(|ui| {
                column(ui, side, height, &mut |ui| {
                    now_playing_cover(ui, app, &track, side, app.settings.looks.cover);
                    ui.add_space(12.0);
                    now_playing_details(ui, app, &track, side, false, actions);
                });
                ui.add_space(28.0);
                column(ui, width - side - 28.0, height, &mut |ui| now_playing_side(ui, app, &track, true, actions));
            });
        }
        NowPlayingLayout::Banner => {
            egui::Frame::new().fill(palette.card.gamma_multiply(0.6)).corner_radius(theme::corner(16)).inner_margin(16).show(ui, |ui| {
                ui.set_width(width - 32.0);
                ui.horizontal(|ui| {
                    now_playing_cover(ui, app, &track, 150.0, app.settings.looks.cover);
                    ui.add_space(20.0);
                    ui.vertical(|ui| now_playing_details(ui, app, &track, (width - 220.0).min(560.0), false, actions));
                });
            });
            ui.add_space(14.0);
            let height = room(ui);
            let half = (width - 28.0) / 2.0;
            ui.horizontal_top(|ui| {
                column(ui, half, height, &mut |ui| lyrics_only(ui, app, &track, actions));
                ui.add_space(28.0);
                column(ui, half, height, &mut |ui| up_next(ui, app, actions));
            });
        }
        NowPlayingLayout::Immersive => {
            let height = room(ui);
            let (area, _) = ui.allocate_exact_size(vec2(width, height), egui::Sense::hover());
            cover_filling(ui, app, &track, area);
            let foot = egui::Rect::from_min_max(egui::pos2(area.left(), area.bottom() - area.height() * 0.55), area.max);
            theme::gradient(ui.painter(), foot, palette.page.gamma_multiply(0.0), palette.page.gamma_multiply(0.94), false);
            let details = egui::Rect::from_min_max(egui::pos2(area.left() + 28.0, area.bottom() - 250.0), egui::pos2(area.left() + 28.0 + (width - 56.0).min(560.0), area.bottom() - 8.0));
            let mut child = ui.new_child(egui::UiBuilder::new().max_rect(details).layout(Layout::top_down(Align::Min)));
            now_playing_details(&mut child, app, &track, details.width(), false, actions);
        }
        NowPlayingLayout::Split => {
            let height = room(ui);
            let (area, _) = ui.allocate_exact_size(vec2(width, height), egui::Sense::hover());
            let left = egui::Rect::from_min_max(area.min, egui::pos2(area.center().x - 12.0, area.bottom()));
            cover_filling(ui, app, &track, left);
            let right = egui::Rect::from_min_max(egui::pos2(area.center().x + 16.0, area.top()), area.max);
            let mut child = ui.new_child(egui::UiBuilder::new().max_rect(right).layout(Layout::top_down(Align::Min)));
            now_playing_details(&mut child, app, &track, right.width(), false, actions);
            child.add_space(8.0);
            let rest = (right.bottom() - child.cursor().top()).max(160.0);
            child.allocate_ui_with_layout(vec2(right.width(), rest), Layout::top_down(Align::Min), |ui| now_playing_side(ui, app, &track, false, actions));
        }
        NowPlayingLayout::CoverFlow => {
            cover_flow(ui, app, width, actions);
            ui.add_space(16.0);
            ui.vertical_centered(|ui| now_playing_details(ui, app, &track, 460.0_f32.min(width), true, actions));
        }
        NowPlayingLayout::Turntable => {
            let height = room(ui);
            let side = (width - app.settings.looks.panel_width.width().min(width * 0.6) - 28.0).clamp(220.0, 460.0);
            ui.horizontal_top(|ui| {
                column(ui, side, height, &mut |ui| {
                    turntable(ui, app, &track, side);
                    ui.add_space(14.0);
                    now_playing_details(ui, app, &track, side, false, actions);
                });
                ui.add_space(28.0);
                column(ui, width - side - 28.0, height, &mut |ui| now_playing_side(ui, app, &track, false, actions));
            });
        }
        NowPlayingLayout::Poster => {
            ui.add_space(20.0);
            ui.vertical_centered(|ui| {
                ui.set_max_width(width.min(900.0));
                ui.add(egui::Label::new(RichText::new(&track.title).font(theme::bold((width / 14.0).clamp(36.0, 72.0))).color(palette.text)).wrap().halign(Align::Center));
                ui.add_space(6.0);
                ui.add(egui::Label::new(RichText::new(track.artist_line()).font(theme::regular(26.0)).color(palette.accent)).halign(Align::Center));
                ui.add_space(24.0);
                now_playing_controls(ui, app, &track, 520.0_f32.min(width), true, actions);
            });
        }
        NowPlayingLayout::Playlist => {
            let height = room(ui);
            ui.horizontal_top(|ui| {
                column(ui, 260.0, height, &mut |ui| {
                    egui::Frame::new().fill(palette.card.gamma_multiply(0.7)).corner_radius(theme::corner(12)).inner_margin(10).show(ui, |ui| {
                        now_playing_cover(ui, app, &track, 240.0, CoverStyle::Rounded);
                        ui.add_space(8.0);
                        ui.add(egui::Label::new(RichText::new(&track.title).font(theme::bold(15.0)).color(palette.text)).wrap());
                        ui.label(RichText::new(track.artist_line()).color(palette.subtext));
                        if let Some(album) = &track.album {
                            ui.label(RichText::new(&album.title).color(palette.faint));
                        }
                    });
                });
                ui.add_space(20.0);
                column(ui, width - 280.0, height, &mut |ui| {
                    ui.label(RichText::new(&track.title).font(theme::bold(28.0)).color(palette.text));
                    now_playing_controls(ui, app, &track, (width - 280.0).min(560.0), false, actions);
                    ui.add_space(8.0);
                    queue_table(ui, app, actions);
                });
            });
        }
    }
}

/// The song's lyrics alone, for the layouts that put what is next elsewhere.
fn lyrics_only(ui: &mut egui::Ui, app: &App, track: &Track, actions: &mut Vec<Action>) {
    let palette = app.palette;
    match app.lyrics.get(&track.queue_key()) {
        Some(super::Loadable::Ready(Some(lyrics))) => {
            ui.label(RichText::new("Lyrics").font(theme::bold(18.0)).color(palette.subtext));
            ui.push_id("only-lyrics", |ui| super::player::lyric_lines(ui, app, lyrics, false, actions));
        }
        Some(super::Loadable::Loading) => widgets::loading(ui, &palette, "Looking for the lyrics…"),
        _ => {
            ui.label(RichText::new("No lyrics for this song.").color(palette.faint));
        }
    }
}

/// The cover filling `rect`, cut from its middle to the rect's shape.
fn cover_filling(ui: &egui::Ui, app: &App, track: &Track, rect: egui::Rect) {
    let palette = app.palette;
    ui.painter().rect_filled(rect, 0, palette.card);
    let Some(url) = &track.artwork_url else { return };
    let square = widgets::square_uv(url);
    let aspect = rect.width() / rect.height();
    let uv = if aspect > 1.0 {
        let h = square.height() / aspect;
        egui::Rect::from_center_size(square.center(), vec2(square.width(), h))
    } else {
        let w = square.width() * aspect;
        egui::Rect::from_center_size(square.center(), vec2(w, square.height()))
    };
    egui::Image::new(url.to_string()).uv(uv).show_loading_spinner(false).paint_at(ui, rect);
}

/// Cover flow: the queue as a row of covers, the one playing large and square, the others turned towards it.
fn cover_flow(ui: &mut egui::Ui, app: &App, width: f32, actions: &mut Vec<Action>) {
    let palette = app.palette;
    let side = (width * 0.32).clamp(180.0, 320.0);
    let (area, _) = ui.allocate_exact_size(vec2(width, side + 20.0), egui::Sense::hover());
    let tracks = &app.queue.tracks;
    let current = app.queue.current_index.max(0) as usize;
    let painter = ui.painter_at(area);
    // Furthest first, so the nearer covers sit on top of them.
    let mut order: Vec<isize> = (-4..=4).collect();
    order.sort_by_key(|o| std::cmp::Reverse(o.abs()));
    for offset in order {
        let index = current as isize + offset;
        if index < 0 || index as usize >= tracks.len() {
            continue;
        }
        let track = &tracks[index as usize];
        let distance = offset.unsigned_abs() as f32;
        let (w, h) = if offset == 0 { (side, side) } else { (side * 0.42, side * 0.78) };
        let x = area.center().x + offset as f32 * (side * 0.34) + offset.signum() as f32 * side * 0.28;
        let rect = egui::Rect::from_center_size(egui::pos2(x, area.center().y), vec2(w, h));
        let response = ui.interact(rect, ui.id().with(("flow", index)), egui::Sense::click());
        widgets::paint_artwork_exact(ui, &palette, track.artwork_url.as_deref(), rect, if offset == 0 { theme::corner(12) } else { 2 });
        if offset != 0 {
            // Turned away: darker the further it is.
            painter.rect_filled(rect, 2, palette.page.gamma_multiply((0.25 + distance * 0.12).min(0.75)));
            if response.on_hover_text(&track.title).clicked() {
                actions.push(Action::QueueJump(index as usize));
            }
        }
    }
}

/// A turntable: the record on its platter, the arm swinging in across it as the song plays.
fn turntable(ui: &mut egui::Ui, app: &App, track: &Track, side: f32) {
    let palette = app.palette;
    let (plinth, _) = ui.allocate_exact_size(vec2(side, side * 0.86), egui::Sense::hover());
    let painter = ui.painter().clone();
    painter.rect_filled(plinth, theme::corner(14), theme::mix(palette.card, egui::Color32::BLACK, 0.25));
    painter.rect_stroke(plinth, theme::corner(14), Stroke::new(1.0_f32, palette.line), egui::StrokeKind::Inside);
    let record = side * 0.74;
    let record_rect = egui::Rect::from_min_size(plinth.left_top() + vec2(side * 0.05, (plinth.height() - record) / 2.0), egui::Vec2::splat(record));
    let mut child = ui.new_child(egui::UiBuilder::new().max_rect(record_rect));
    now_playing_cover(&mut child, app, track, record, crate::looks::CoverStyle::Record);
    // The arm: pivoting at the top right, resting off the record until the song starts, then crossing it.
    let state = app.engine.state();
    let fraction = if state.duration_ms > 0 { (state.position_ms as f32 / state.duration_ms as f32).clamp(0.0, 1.0) } else { 0.0 };
    let on_record = app.loaded.is_some();
    let pivot = egui::pos2(plinth.right() - side * 0.1, plinth.top() + side * 0.12);
    let angle = if on_record { 0.42 + fraction * 0.38 } else { 0.12 };
    let length = side * 0.62;
    let tip = pivot + vec2(-angle.sin(), angle.cos()) * length;
    painter.circle_filled(pivot, side * 0.04, palette.raised);
    painter.circle_stroke(pivot, side * 0.04, Stroke::new(1.0_f32, palette.line));
    painter.line_segment([pivot, tip], Stroke::new((side * 0.018).max(3.0), theme::mix(palette.subtext, egui::Color32::WHITE, 0.3)));
    painter.rect_filled(egui::Rect::from_center_size(tip, vec2(side * 0.05, side * 0.07)), 2, palette.text);
}

/// The queue as a table: number, title, artist, album and length, the song playing lit.
fn queue_table(ui: &mut egui::Ui, app: &App, actions: &mut Vec<Action>) {
    use crate::looks::QueueColumn;
    let palette = app.palette;
    let columns = &app.settings.looks.queue_columns;
    // The wide columns -- title, artist, album -- share what the narrow ones leave, as a table's do.
    let narrow = |c: &QueueColumn| match c {
        QueueColumn::Number => 26.0,
        QueueColumn::Duration => 58.0,
        QueueColumn::Service => 90.0,
        _ => 0.0,
    };
    let status_height = if app.settings.looks.status_bar { 26.0 } else { 0.0 };
    let fixed: f32 = columns.iter().map(narrow).sum();
    // The title gets a little more than the artist and the album; together they fill what is left exactly.
    let weight = |c: &QueueColumn| if *c == QueueColumn::Title { 1.3 } else { 1.0 };
    let total: f32 = columns.iter().filter(|c| narrow(c) == 0.0).map(weight).sum::<f32>().max(1.0);
    let rest = (ui.available_width() - fixed - columns.len() as f32 * 16.0 - 24.0).max(90.0 * total.min(2.0));
    let width_of = |c: &QueueColumn| {
        let n = narrow(c);
        if n > 0.0 { n } else { rest * weight(c) / total }
    };
    // Left-aligned in a column of its own width, cut short with an ellipsis rather than spilling over.
    let cell = |ui: &mut egui::Ui, width: f32, text: RichText, sense: egui::Sense| {
        ui.allocate_ui_with_layout(vec2(width, 18.0), Layout::left_to_right(Align::Center), |ui| {
            ui.set_width(width);
            ui.add(egui::Label::new(text).truncate().sense(sense))
        })
        .inner
    };
    let height = (ui.available_height() - status_height).max(120.0);
    ui.allocate_ui_with_layout(vec2(ui.available_width(), height), Layout::top_down(Align::Min), |ui| {
        // Sideways too, for a window too narrow for every column chosen.
        egui::ScrollArea::both().id_salt("queue-table").auto_shrink([false, false]).show(ui, |ui| {
            egui::Grid::new("queue-table-grid").num_columns(columns.len().max(1)).striped(true).spacing(vec2(16.0, 6.0)).show(ui, |ui| {
                for column in columns {
                    cell(ui, width_of(column), RichText::new(column.heading()).font(theme::bold(12.5)).color(palette.subtext), egui::Sense::hover());
                }
                ui.end_row();
                let current = app.queue.current_index;
                for (index, track) in app.queue.tracks.iter().enumerate() {
                    let playing = index as isize == current;
                    let colour = if playing { palette.accent } else { palette.text };
                    for column in columns {
                        let w = width_of(column);
                        match column {
                            QueueColumn::Number => {
                                cell(ui, w, RichText::new(format!("{}", index + 1)).color(palette.faint), egui::Sense::hover());
                            }
                            QueueColumn::Title => {
                                if cell(ui, w, RichText::new(&track.title).color(colour), egui::Sense::click()).on_hover_text("Play").clicked() {
                                    actions.push(Action::QueueJump(index));
                                }
                            }
                            QueueColumn::Artist => {
                                cell(ui, w, RichText::new(track.artist_line()).color(palette.subtext), egui::Sense::hover());
                            }
                            QueueColumn::Album => {
                                cell(ui, w, RichText::new(track.album.as_ref().map(|a| a.title.as_str()).unwrap_or("")).color(palette.faint), egui::Sense::hover());
                            }
                            QueueColumn::Duration => {
                                cell(ui, w, RichText::new(track.duration_ms.map(format_duration).unwrap_or_default()).color(palette.faint), egui::Sense::hover());
                            }
                            QueueColumn::Service => {
                                cell(ui, w, RichText::new(track.provider.display_name()).color(palette.faint), egui::Sense::hover());
                            }
                        }
                    }
                    ui.end_row();
                }
            });
        });
    });
    if app.settings.looks.status_bar {
        // The stream's format, and how far into the whole queue.
        let state = app.engine.state();
        let before: i64 = app.queue.tracks.iter().take(app.queue.current_index.max(0) as usize).filter_map(|t| t.duration_ms).sum();
        let total = app.queue.total_duration_ms();
        let position = if app.loaded.is_some() { state.position_ms } else { 0 };
        let mut parts = vec![format!(
            "Song {} of {}",
            (app.queue.current_index + 1).max(0),
            app.queue.tracks.len()
        )];
        if total > 0 {
            parts.push(format!("{} of {}", format_duration(before + position), format_duration(total)));
        }
        if let (Some(format), true) = (&state.format, app.loaded.is_some()) {
            parts.push(format.clone());
        }
        egui::Frame::new().fill(palette.card.gamma_multiply(0.8)).inner_margin(egui::Margin::symmetric(8, 4)).show(ui, |ui| {
            ui.set_width(ui.available_width());
            ui.label(RichText::new(parts.join("   ·   ")).font(theme::regular(12.0)).color(palette.subtext));
        });
    }
}

/// Behind Now playing: a light in the colour of the cover, the cover itself dimmed, or nothing.
fn now_playing_backdrop(ui: &egui::Ui, app: &App, track: &Track) {
    use crate::looks::NowPlayingBackdrop;
    let palette = app.palette;
    let area = ui.clip_rect().expand2(vec2(28.0, 20.0));
    let painter = ui.painter_at(area);
    match app.settings.looks.backdrop {
        NowPlayingBackdrop::Plain => {}
        NowPlayingBackdrop::Wash => {
            let colour = app.artwork_colour().unwrap_or(palette.accent);
            widgets::radial(&painter, area.left_top() + vec2(area.width() * 0.25, area.height() * 0.2), vec2(area.width() * 0.75, area.height() * 0.8), colour, 0.32);
            widgets::radial(&painter, area.right_bottom(), vec2(area.width() * 0.6, area.height() * 0.6), colour, 0.16);
        }
        NowPlayingBackdrop::Cover => {
            if let Some(url) = &track.artwork_url {
                // Filled to the page's shape from the middle of the cover, and dimmed well under the writing.
                let side = area.width().max(area.height());
                let square = egui::Rect::from_center_size(area.center(), vec2(side, side));
                egui::Image::new(url.to_string()).show_loading_spinner(false).paint_at(ui, square);
                painter.rect_filled(area, 0, palette.page.gamma_multiply(if palette.light { 0.78 } else { 0.72 }));
            }
        }
    }
}

/// The cover, cut as the listener likes it: rounded, square, round, or set into a turning record.
fn now_playing_cover(ui: &mut egui::Ui, app: &App, track: &Track, side: f32, style: crate::looks::CoverStyle) {
    use crate::looks::CoverStyle;
    let palette = app.palette;
    let side = (side * app.settings.looks.cover_size.scale()).round();
    let url = track.artwork_url.as_deref();
    match style {
        CoverStyle::Rounded => {
            widgets::artwork(ui, &palette, url, side, 18);
        }
        CoverStyle::Square => {
            widgets::artwork(ui, &palette, url, side, 0);
        }
        CoverStyle::Circle => {
            let (rect, _) = ui.allocate_exact_size(egui::Vec2::splat(side), egui::Sense::hover());
            widgets::paint_artwork_exact(ui, &palette, url, rect, (side / 2.0).min(255.0) as u8);
        }
        CoverStyle::Record => {
            let (rect, _) = ui.allocate_exact_size(egui::Vec2::splat(side), egui::Sense::hover());
            let painter = ui.painter();
            let centre = rect.center();
            let radius = side / 2.0;
            painter.circle_filled(centre, radius, egui::Color32::from_rgb(0x12, 0x11, 0x14));
            for groove in 1..9 {
                let r = radius * (0.42 + groove as f32 * 0.065);
                painter.circle_stroke(centre, r, Stroke::new(1.0_f32, egui::Color32::from_white_alpha(14)));
            }
            // The light on the vinyl turns with it: thirty-three and a third turns a minute.
            let playing = app.engine.state().status == crate::mpv::Status::Playing && app.settings.looks.animations;
            let turns = ui.input(|i| i.time) as f32 * (100.0 / 3.0 / 60.0) * std::f32::consts::TAU;
            let angle = if playing { turns } else { 0.6 };
            for offset in [0.0, std::f32::consts::PI] {
                let a = angle + offset;
                let towards = vec2(a.cos(), a.sin());
                let across = vec2(-towards.y, towards.x) * radius * 0.08;
                let near = centre + towards * radius * 0.45;
                let far = centre + towards * radius * 0.95;
                painter.add(egui::Shape::convex_polygon(
                    vec![near - across * 0.4, far - across, far + across, near + across * 0.4],
                    egui::Color32::from_white_alpha(10),
                    Stroke::NONE,
                ));
            }
            let label = egui::Rect::from_center_size(centre, egui::Vec2::splat(side * 0.4));
            widgets::paint_artwork_exact(ui, &palette, url, label, (side * 0.2).min(255.0) as u8);
            painter.circle_filled(centre, (side * 0.018).max(3.0), palette.page);
            if playing {
                ui.ctx().request_repaint();
            }
        }
    }
}

/// The song's name, who made it, where it is from, the seek bar and the visualiser.
fn now_playing_details(ui: &mut egui::Ui, app: &App, track: &Track, width: f32, centred: bool, actions: &mut Vec<Action>) {
    let palette = app.palette;
    let align = if centred { Align::Center } else { Align::Min };
    ui.add(egui::Label::new(RichText::new(&track.title).font(theme::bold(26.0)).color(palette.text)).wrap().halign(align));
    ui.add(egui::Label::new(RichText::new(track.artist_line()).font(theme::regular(17.0)).color(palette.subtext)).halign(align));
    ui.horizontal(|ui| {
        if centred {
            let guess = 60.0 + track.album.as_ref().map(|a| a.title.len() as f32 * 7.0).unwrap_or(0.0);
            ui.add_space(((ui.available_width() - guess) / 2.0).max(0.0));
        }
        widgets::badge(ui, track.provider);
        // The line being sung, where there are synced lyrics and it is wanted; the album otherwise.
        let sung = app
            .settings
            .looks
            .lyric_line
            .then(|| match app.lyrics.get(&track.queue_key()) {
                Some(super::Loadable::Ready(Some(lyrics))) if lyrics.synced && app.loaded.is_some() => {
                    lyrics.current_line(app.engine.state().position_ms + 250).map(|i| lyrics.lines[i].text.clone())
                }
                _ => None,
            })
            .flatten();
        match (sung, &track.album) {
            (Some(line), _) => {
                ui.label(RichText::new(format!("♪ {line}")).color(palette.accent));
            }
            (None, Some(album)) => {
                ui.label(RichText::new(&album.title).color(palette.faint));
            }
            _ => {}
        }
    });
    follow_button(ui, app, track, actions);
    ui.add_space(10.0);
    now_playing_controls(ui, app, track, width, centred, actions);
}

/// The seek bar, the controls under it, and the visualiser.
fn now_playing_controls(ui: &mut egui::Ui, app: &App, track: &Track, width: f32, centred: bool, actions: &mut Vec<Action>) {
    let state = app.engine.state();
    let duration = if state.duration_ms > 0 { state.duration_ms } else { track.duration_ms.unwrap_or(0) };
    let left = if centred { ui.max_rect().center().x - width / 2.0 } else { ui.cursor().min.x };
    let bar = egui::Rect::from_min_size(egui::pos2(left + 44.0, ui.cursor().min.y + 4.0), vec2(width - 88.0, 16.0));
    let mut scrub = super::Inputs::default();
    super::player::seek_bar(ui, app, bar, if app.loaded.is_some() { state.position_ms } else { 0 }, duration, &mut scrub, actions);
    ui.add_space(26.0);
    let controls = egui::Rect::from_min_size(egui::pos2(left, ui.cursor().min.y), vec2(width, 56.0));
    super::player::transport(ui, app, controls, true, 52.0, actions);
    ui.add_space(64.0);
    if app.settings.visualiser {
        if centred {
            ui.allocate_ui_with_layout(vec2(ui.available_width(), 90.0), Layout::top_down(Align::Center), |ui| visualiser(ui, app, width.min(460.0), state.analysis.as_ref()));
        } else {
            visualiser(ui, app, width, state.analysis.as_ref());
        }
    }
}

/// Beside or under the cover: the lyrics when there are some, what plays next when not.
/// Following the artist of a YouTube song, under their name, for the listener signed in to YouTube Music.
fn follow_button(ui: &mut egui::Ui, app: &App, track: &Track, actions: &mut Vec<Action>) {
    use crate::domain::ProviderType;
    if !app.settings.looks.follow_button || app.youtube.is_none() || !matches!(track.provider, ProviderType::YoutubeMusic | ProviderType::YoutubeVideo) {
        return;
    }
    match app.artists.get(&track.id) {
        None => actions.push(Action::LoadArtist(track.id.clone())),
        Some(super::Loadable::Ready(artist)) => {
            let palette = app.palette;
            let text = if artist.following { format!("✓ Following {}", artist.name) } else { format!("＋ Follow {}", artist.name) };
            if secondary(ui, &palette, &text).on_hover_text("On YouTube Music").clicked() {
                actions.push(Action::SetFollowing { video_id: track.id.clone(), channel_id: artist.id.clone(), follow: !artist.following });
            }
        }
        _ => {}
    }
}

/// Beside or under the cover: what is next, the lyrics, or songs like this one, opening on the one chosen
/// in Settings and switched between with the tabs.
fn now_playing_side(ui: &mut egui::Ui, app: &App, track: &Track, centred: bool, actions: &mut Vec<Action>) {
    use crate::looks::NowPlayingTab;
    let palette = app.palette;
    let id = egui::Id::new("now-playing-tab");
    let mut tab: NowPlayingTab = ui.data(|d| d.get_temp(id)).unwrap_or(app.settings.looks.open_on);
    ui.horizontal(|ui| {
        for (choice, name) in [(NowPlayingTab::UpNext, "Up next"), (NowPlayingTab::Lyrics, "Lyrics"), (NowPlayingTab::Related, "Related")] {
            if ui.selectable_label(tab == choice, RichText::new(name).font(theme::bold(15.0))).clicked() {
                tab = choice;
            }
        }
    });
    ui.data_mut(|d| d.insert_temp(id, tab));
    ui.add_space(4.0);
    match tab {
        NowPlayingTab::UpNext => up_next(ui, app, actions),
        NowPlayingTab::Lyrics => match app.lyrics.get(&track.queue_key()) {
            Some(super::Loadable::Ready(Some(lyrics))) => {
                ui.push_id("now-playing-lyrics", |ui| super::player::lyric_lines(ui, app, lyrics, centred, actions));
            }
            Some(super::Loadable::Loading) | None => {
                if !app.settings.lyrics_open && !app.lyrics.contains_key(&track.queue_key()) && secondary(ui, &palette, "🎤  Find the lyrics").clicked() {
                    actions.push(Action::ToggleLyrics);
                }
                widgets::loading(ui, &palette, "Looking for the lyrics…");
            }
            Some(super::Loadable::Failed(why)) => notice(ui, &palette, why, true),
            Some(super::Loadable::Ready(None)) => {
                ui.label(RichText::new("No lyrics source has this song.").color(palette.faint));
            }
        },
        NowPlayingTab::Related => match app.related.get(&track.queue_key()) {
            None => {
                actions.push(Action::LoadRelated(track.clone()));
                widgets::loading(ui, &palette, "Finding songs like this…");
            }
            Some(super::Loadable::Loading) => widgets::loading(ui, &palette, "Finding songs like this…"),
            Some(super::Loadable::Failed(why)) => notice(ui, &palette, why, true),
            Some(super::Loadable::Ready(tracks)) => track_list(ui, app, tracks, RowPlace::List, actions),
        },
    }
}

fn up_next(ui: &mut egui::Ui, app: &App, actions: &mut Vec<Action>) {
    let palette = app.palette;
    let upcoming: Vec<Track> = app.queue.up_next().iter().chain(app.queue.suggestions.iter()).take(12).cloned().collect();
    if upcoming.is_empty() {
        ui.label(RichText::new("Nothing after this song.").color(palette.faint));
    }
    track_list(ui, app, &upcoming, RowPlace::List, actions);
    if !app.settings.lyrics_open && secondary(ui, &palette, "🎤  Show the lyrics").clicked() {
        actions.push(Action::ToggleLyrics);
    }
}

/// Ten bars, one an octave, rising and falling with the sound -- or left and right as a level meter -- from
/// what mpv measured, eased so it moves rather than flickers.
fn visualiser(ui: &mut egui::Ui, app: &App, width: f32, analysis: Option<&crate::audio::Analysis>) {
    if app.settings.looks.visualiser == crate::looks::VisualiserStyle::LevelMeter {
        level_meter(ui, app, width, analysis);
        return;
    }
    let palette = app.palette;
    let (rect, _) = ui.allocate_exact_size(vec2(width, 90.0), egui::Sense::hover());
    let painter = ui.painter_at(rect);
    let bands = analysis.map(|a| a.bands.clone()).unwrap_or_default();
    let count = 10;
    let gap = 6.0;
    let bar_width = (width - gap * (count as f32 - 1.0)) / count as f32;
    let id = egui::Id::new("visualiser-eased");
    let mut eased: Vec<f32> = ui.data(|d| d.get_temp(id)).unwrap_or_else(|| vec![0.0; count]);
    for (i, value) in eased.iter_mut().enumerate() {
        let target = bands.get(i).copied().unwrap_or(0.0);
        // Up at once, down gently, as a meter falls.
        *value = if target > *value { target } else { *value + (target - *value) * 0.18 };
    }
    for (i, value) in eased.iter().enumerate() {
        let height = (value * rect.height()).max(3.0);
        let x = rect.left() + i as f32 * (bar_width + gap);
        let bar = egui::Rect::from_min_max(egui::pos2(x, rect.bottom() - height), egui::pos2(x + bar_width, rect.bottom()));
        let colour = theme::mix(palette.wash(0.55), palette.accent, *value);
        painter.rect_filled(bar, egui::CornerRadius::same(theme::corner(4)), colour);
    }
    ui.data_mut(|d| d.insert_temp(id, eased));
}

/// Left and right, as a mixing desk shows them: green, then amber, then red near the top.
fn level_meter(ui: &mut egui::Ui, app: &App, width: f32, analysis: Option<&crate::audio::Analysis>) {
    let palette = app.palette;
    let (rect, _) = ui.allocate_exact_size(vec2(width, 44.0), egui::Sense::hover());
    let painter = ui.painter_at(rect);
    let levels: Vec<f32> = analysis.map(|a| if a.rms.is_empty() { a.peaks.clone() } else { a.rms.clone() }).unwrap_or_default();
    let id = egui::Id::new("level-meter-eased");
    let mut eased: Vec<f32> = ui.data(|d| d.get_temp(id)).unwrap_or_else(|| vec![0.0; 2]);
    for (i, value) in eased.iter_mut().enumerate() {
        let target = levels.get(i).or(levels.first()).copied().unwrap_or(0.0);
        *value = if target > *value { target } else { *value + (target - *value) * 0.15 };
    }
    let segments = 32;
    let gap = 2.0;
    let segment = (rect.width() - 22.0 - gap * (segments as f32 - 1.0)) / segments as f32;
    for (row, value) in eased.iter().enumerate() {
        let y = rect.top() + row as f32 * 22.0;
        painter.text(egui::pos2(rect.left(), y + 9.0), egui::Align2::LEFT_CENTER, if row == 0 { "L" } else { "R" }, theme::bold(12.0), palette.subtext);
        let lit = (value * segments as f32).round() as usize;
        for i in 0..segments {
            let x = rect.left() + 22.0 + i as f32 * (segment + gap);
            let colour = if i >= segments * 7 / 8 {
                egui::Color32::from_rgb(0xFF, 0x4D, 0x4D)
            } else if i >= segments * 2 / 3 {
                egui::Color32::from_rgb(0xFF, 0xC1, 0x3D)
            } else {
                egui::Color32::from_rgb(0x4C, 0xD9, 0x64)
            };
            let cell = egui::Rect::from_min_size(egui::pos2(x, y + 2.0), vec2(segment, 14.0));
            painter.rect_filled(cell, egui::CornerRadius::same(theme::corner(2)), if i < lit { colour } else { palette.line.gamma_multiply(0.6) });
        }
    }
    ui.data_mut(|d| d.insert_temp(id, eased));
}

/// The Windows themes' title band across the top: a 98 or XP title bar with what is playing, and working
/// minimise, maximise and close buttons.
pub fn skin_title_bar(app: &App, ctx: &egui::Context) {
    use theme::Skin;
    let skin = theme::skin_of(&app.settings.theme);
    if skin == Skin::Standard {
        return;
    }
    let xp = skin == Skin::WindowsXp;
    egui::TopBottomPanel::top("skin-title")
        .exact_height(if xp { 30.0 } else { 24.0 })
        .resizable(false)
        .show_separator_line(false)
        .frame(egui::Frame::new())
        .show(ctx, |ui| {
            let rect = ui.max_rect();
            let painter = ui.painter().clone();
            let title_text = match app.loaded.as_ref().or(app.queue.current()) {
                Some(track) => format!("Noctorium - {} - {}", track.title, track.artist_line()),
                None => "Noctorium".to_string(),
            };
            let glyph_colour;
            if let Some(w) = skin.windows98() {
                painter.rect_filled(rect, 0, theme::rgb(w.face));
                let band = rect.shrink(2.0);
                theme::gradient(&painter, band, theme::rgb(w.title), theme::rgb(w.title_end), true);
                glyph_colour = theme::rgb(w.text);
                painter.text(band.left_center() + vec2(24.0, 0.0), egui::Align2::LEFT_CENTER, &title_text, theme::bold(12.5), theme::rgb(w.title_text));
                egui::Image::from_bytes("bytes://noctorium-mark.png", widgets::MARK_PNG).paint_at(ui, egui::Rect::from_min_size(band.left_center() + vec2(3.0, -8.0), egui::Vec2::splat(16.0)));
            } else {
                let top = egui::Rect::from_min_max(rect.min, egui::pos2(rect.right(), rect.top() + 6.0));
                let middle = egui::Rect::from_min_max(top.left_bottom(), egui::pos2(rect.right(), rect.bottom() - 4.0));
                let foot = egui::Rect::from_min_max(middle.left_bottom(), rect.max);
                theme::gradient(&painter, top, theme::rgb(theme::xp::TITLE_TOP), theme::rgb(theme::xp::TITLE), false);
                painter.rect_filled(middle, 0, theme::rgb(theme::xp::TITLE));
                theme::gradient(&painter, foot, theme::rgb(theme::xp::TITLE), theme::rgb(theme::xp::TITLE_FOOT), false);
                glyph_colour = egui::Color32::WHITE;
                painter.text(rect.left_center() + vec2(31.0, 1.0), egui::Align2::LEFT_CENTER, &title_text, theme::bold(13.5), egui::Color32::from_black_alpha(90));
                painter.text(rect.left_center() + vec2(30.0, 0.0), egui::Align2::LEFT_CENTER, &title_text, theme::bold(13.5), theme::rgb(theme::xp::TITLE_TEXT));
                egui::Image::from_bytes("bytes://noctorium-mark.png", widgets::MARK_PNG).corner_radius(4).paint_at(ui, egui::Rect::from_min_size(rect.left_center() + vec2(7.0, -9.0), egui::Vec2::splat(18.0)));
            }
            // The caption buttons, right to left: close, maximise, minimise.
            let size = if xp { vec2(21.0, 21.0) } else { vec2(16.0, 14.0) };
            let maximised = ctx.input(|i| i.viewport().maximized.unwrap_or(false));
            let buttons: [(&str, egui::ViewportCommand); 3] = [
                ("✕", egui::ViewportCommand::Close),
                (if maximised { "❐" } else { "□" }, egui::ViewportCommand::Maximized(!maximised)),
                ("_", egui::ViewportCommand::Minimized(true)),
            ];
            let mut right = rect.right() - if xp { 5.0 } else { 4.0 };
            for (index, (glyph, command)) in buttons.into_iter().enumerate() {
                let button = egui::Rect::from_min_size(egui::pos2(right - size.x, rect.center().y - size.y / 2.0), size);
                right = button.left() - if xp || index == 0 { 3.0 } else { 0.0 };
                let response = ui.interact(button, ui.id().with(("caption", index)), egui::Sense::click());
                if let Some(w) = skin.windows98() {
                    painter.rect_filled(button, 0, theme::rgb(w.face));
                    theme::bevel(&painter, button, !response.is_pointer_button_down_on(), w);
                } else {
                    let fill = if index == 0 { theme::rgb(theme::xp::CLOSE) } else { theme::rgb(theme::xp::CAPTION_BUTTON) };
                    let fill = if response.hovered() { theme::mix(fill, egui::Color32::WHITE, 0.2) } else { fill };
                    painter.rect_filled(button, 3, fill);
                    painter.rect_stroke(button, 3, Stroke::new(1.0_f32, egui::Color32::WHITE), egui::StrokeKind::Inside);
                }
                painter.text(button.center(), egui::Align2::CENTER_CENTER, glyph, theme::bold(if xp { 13.0 } else { 11.0 }), glyph_colour);
                if response.clicked() {
                    ctx.send_viewport_cmd(command);
                }
            }
        });
}

/// Connect's switch and this computer's name on the listener's other devices.
pub(super) fn connect_settings(ui: &mut egui::Ui, app: &App, inputs: &mut Inputs, actions: &mut Vec<Action>) {
    let mut on = app.settings.connect_enabled;
    if ui.checkbox(&mut on, "Noctorium Connect on (off stops it altogether)").changed() {
        let mut settings = app.settings.clone();
        settings.connect_enabled = on;
        actions.push(Action::SetSettings(Box::new(settings)));
    }
    ui.horizontal(|ui| {
        ui.label("Name this device");
        let name = inputs.connect_name.get_or_insert_with(|| app.settings.connect_device_name.clone());
        let computer = std::env::var("COMPUTERNAME").or_else(|_| std::env::var("HOSTNAME")).unwrap_or_else(|_| "This computer".into());
        let field = ui.add(egui::TextEdit::singleline(name).hint_text(computer).desired_width(220.0));
        let enter = field.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter));
        if (ui.button("Save").clicked() || enter) && name.trim() != app.settings.connect_device_name.trim() {
            let mut settings = app.settings.clone();
            settings.connect_device_name = name.trim().to_string();
            actions.push(Action::SetSettings(Box::new(settings)));
        }
        if ui.button("Open Connect").clicked() {
            actions.push(Action::Open(Page::Connect));
        }
    });
}

/// Checks that yt-dlp, mpv, storage and the network are working, for when something is not.
pub(super) fn diagnostics_settings(ui: &mut egui::Ui, app: &App, actions: &mut Vec<Action>) {
    let palette = app.palette;
    match &app.diagnostics {
        Some(super::Loadable::Loading) => widgets::loading(ui, &palette, "Checking…"),
        Some(super::Loadable::Ready(checks)) => {
            egui::Grid::new("diagnostics").num_columns(3).spacing(vec2(12.0, 4.0)).show(ui, |ui| {
                for (name, ok, detail) in checks {
                    ui.label(RichText::new(if *ok { "✓" } else { "✕" }).color(if *ok { palette.accent } else { palette.error }));
                    ui.label(RichText::new(name).color(palette.text));
                    ui.label(RichText::new(detail).color(palette.subtext));
                    ui.end_row();
                }
            });
            let report: String = checks.iter().map(|(n, ok, d)| format!("{} {n}: {d}\n", if *ok { "OK" } else { "FAIL" })).collect();
            ui.horizontal(|ui| {
                if ui.button("Check again").clicked() {
                    actions.push(Action::RunDiagnostics);
                }
                if ui.button("Copy the results").clicked() {
                    actions.push(Action::Copy(format!("Noctorium Rust {}\n{report}", crate::version())));
                }
            });
        }
        _ => {
            if secondary(ui, &palette, "Run the checks").clicked() {
                actions.push(Action::RunDiagnostics);
            }
        }
    }
}

/// Settings › Stream widgets: what plays, for OBS or Streamlabs, in seven looks.
pub(super) fn widget_settings(ui: &mut egui::Ui, app: &App, actions: &mut Vec<Action>) {
    use crate::widgets::LOOKS;
    let palette = app.palette;
    let mut w = app.settings.widgets.clone();
    let mut changed = false;
    changed |= ui.checkbox(&mut w.enabled, "Serve the stream widgets").changed();
    if w.enabled {
        ui.horizontal_wrapped(|ui| {
            for (id, name, _, _, _) in LOOKS {
                changed |= ui.selectable_value(&mut w.theme, id.to_string(), name).changed();
            }
        });
        if let Some((id, name, about, width, height)) = LOOKS.iter().find(|l| l.0 == w.theme) {
            ui.label(RichText::new(format!("{name}: {about} Size it {width}×{height}.")).color(palette.subtext));
            let address = app
                .widget_server
                .as_ref()
                .map(|s| format!("{}/widget/{id}", s.address))
                .unwrap_or_else(|| format!("http://127.0.0.1:{}/widget/{id}", w.port));
            ui.horizontal(|ui| {
                ui.label(RichText::new(&address).font(theme::bold(14.0)).color(palette.accent));
                if ui.button("Copy").clicked() {
                    actions.push(Action::Copy(address.clone()));
                }
                if ui.button("Preview in the browser").clicked() {
                    actions.push(Action::OpenUrl(address.clone()));
                }
            });
        }
        ui.horizontal(|ui| {
            changed |= ui.checkbox(&mut w.show_cover, "Cover").changed();
            changed |= ui.checkbox(&mut w.show_progress, "Progress").changed();
            changed |= ui.checkbox(&mut w.show_service_logo, "The service's logo").changed();
            changed |= ui.checkbox(&mut w.hide_when_paused, "Hidden while paused").changed();
        });
        ui.horizontal(|ui| {
            ui.label("Port");
            let mut port = w.port as u32;
            let field = ui.add(egui::DragValue::new(&mut port).range(1024..=65535));
            if (field.lost_focus() || field.drag_stopped()) && port as u16 != w.port {
                w.port = port as u16;
                changed = true;
            }
            changed |= ui.checkbox(&mut w.allow_network, "Let other computers in the house show it").changed();
        });
    }
    if changed {
        actions.push(Action::SetWidgets(w));
    }
}

/// Connect: the listener's other devices on this network, to send the music to, bring it back from, and play.
/// Adding a device by its address, and what to try when one does not show up.
fn connect_extras(ui: &mut egui::Ui, app: &App, inputs: &mut Inputs, actions: &mut Vec<Action>) {
    let palette = app.palette;
    ui.add_space(6.0);
    egui::CollapsingHeader::new("Add a device by its address").id_salt("connect-address").show(ui, |ui| {
        ui.label(RichText::new("On a network that keeps devices from hearing each other. The address and port are on the other device's Connect page.").color(palette.faint));
        if app.connect.is_none() {
            ui.label(RichText::new("Connect has to be running first: sign in to your Noctorium account, with Connect on.").color(palette.subtext));
        }
        ui.horizontal(|ui| {
            let field = ui.add(egui::TextEdit::singleline(&mut inputs.connect_address).hint_text("192.168.1.20:45678").desired_width(220.0));
            let enter = field.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter));
            if (ui.button("Add").clicked() || enter) && !inputs.connect_address.trim().is_empty() {
                actions.push(Action::AddConnectAddress(std::mem::take(&mut inputs.connect_address)));
            }
        });
        for address in &app.settings.connect_addresses {
            ui.horizontal(|ui| {
                ui.label(RichText::new(address).color(palette.subtext));
                if ui.small_button("Remove").clicked() {
                    actions.push(Action::RemoveConnectAddress(address.clone()));
                }
            });
        }
        if let Some(connect) = &app.connect {
            ui.label(RichText::new(format!("This computer answers on port {}.", connect.port)).color(palette.faint));
        }
    });
    egui::CollapsingHeader::new("Not finding a device?").id_salt("connect-help").show(ui, |ui| {
        for line in [
            "Both have to be signed in to the same Noctorium account, with Connect on.",
            "Both have to be on the same Wi-Fi or network, not a guest network that keeps devices apart.",
            "On Windows, allow Noctorium through the firewall on private networks when it asks.",
            "On a phone, let Noctorium run in the background, or it stops answering when the screen is off.",
            "Still nothing: add the device by its address above.",
        ] {
            ui.label(RichText::new(format!("•  {line}")).color(palette.subtext));
        }
    });
    ui.add_space(6.0);
}

/// Edit details: what a song is called here, over what its service calls it.
pub fn edit_details_window(app: &mut App, ctx: &egui::Context, actions: &mut Vec<Action>) {
    let palette = app.palette;
    let Some((track, title, artist)) = app.inputs.editing.as_mut() else { return };
    let existing = app.track_edits.get(&track.queue_key()).cloned();
    let mut open = true;
    let mut done = false;
    egui::Window::new("Edit details")
        .collapsible(false)
        .resizable(false)
        .open(&mut open)
        .anchor(egui::Align2::CENTER_CENTER, vec2(0.0, 0.0))
        .show(ctx, |ui| {
            ui.set_width(380.0);
            ui.label(
                RichText::new("What this song is called here, in the queue, in lyrics searches and in what is scrobbled. Only on this computer; the service is not told.")
                    .font(theme::regular(12.5))
                    .color(palette.subtext),
            );
            ui.add_space(6.0);
            ui.label("Title");
            ui.add(egui::TextEdit::singleline(title).desired_width(f32::INFINITY));
            ui.label("Artist");
            ui.add(egui::TextEdit::singleline(artist).desired_width(f32::INFINITY));
            ui.add_space(8.0);
            ui.horizontal(|ui| {
                if primary(ui, &palette, "Save").clicked() {
                    // Only what was changed is an edit, along with what was edited before; the rest stays the
                    // service's own.
                    let before = existing.clone().unwrap_or_default();
                    let title_edit = (title.trim() != track.title.trim() || before.title.is_some()).then(|| title.trim().to_string());
                    let artist_edit = (artist.trim() != track.artist_line().trim() || before.artist.is_some()).then(|| artist.trim().to_string());
                    actions.push(Action::EditTrack { track: track.clone(), title: title_edit, artist: artist_edit });
                    done = true;
                }
                if existing.is_some() && secondary(ui, &palette, "Use the service's").clicked() {
                    actions.push(Action::EditTrack { track: track.clone(), title: None, artist: None });
                    done = true;
                }
                if secondary(ui, &palette, "Cancel").clicked() {
                    done = true;
                }
            });
        });
    if done || !open {
        app.inputs.editing = None;
    }
}

/// The profile: its name, and everything played here, by day.
fn profile_page(ui: &mut egui::Ui, app: &App, inputs: &mut Inputs, actions: &mut Vec<Action>) {
    let palette = app.palette;
    ui.label(RichText::new(&app.settings.profile_name).font(theme::bold(28.0)).color(palette.text));
    ui.label(RichText::new("This computer's profile. It stays on this computer.").color(palette.faint));
    ui.add_space(8.0);
    ui.horizontal(|ui| {
        ui.label("Profile name");
        let name = inputs.profile_name.get_or_insert_with(|| app.settings.profile_name.clone());
        let field = ui.add(egui::TextEdit::singleline(name).desired_width(240.0));
        let enter = field.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter));
        if (ui.button("Save").clicked() || enter) && !name.trim().is_empty() && name.trim() != app.settings.profile_name {
            let mut settings = app.settings.clone();
            settings.profile_name = name.trim().to_string();
            actions.push(Action::SetSettings(Box::new(settings)));
        }
    });
    let mut keep = app.settings.keep_history;
    if ui.checkbox(&mut keep, "Keep a history of what I play (a song goes in once half a minute of it has been heard)").changed() {
        let mut settings = app.settings.clone();
        settings.keep_history = keep;
        actions.push(Action::SetSettings(Box::new(settings)));
    }
    ui.add_space(6.0);
    ui.horizontal(|ui| {
        heading(ui, &palette, "History", Some(&format!("{} songs", app.play_history.len())));
        if !app.play_history.is_empty() {
            if inputs.confirm_delete.as_deref() == Some("history") {
                ui.label(RichText::new("Forget every song in it?").color(palette.error));
                if ui.button(RichText::new("Clear").color(palette.error)).clicked() {
                    actions.push(Action::ClearHistory);
                    inputs.confirm_delete = None;
                }
                if ui.button("Keep").clicked() {
                    inputs.confirm_delete = None;
                }
            } else if secondary(ui, &palette, "Clear history").clicked() {
                inputs.confirm_delete = Some("history".into());
            }
        }
    });
    if app.play_history.is_empty() {
        ui.label(RichText::new("Nothing yet. What you listen to here shows up by day.").color(palette.faint));
        return;
    }
    let shown = inputs.history_shown.max(150);
    let slice = &app.play_history[..app.play_history.len().min(shown)];
    for (day, entries) in crate::store::history_by_day(slice, crate::store::now_seconds()) {
        ui.add_space(6.0);
        ui.label(RichText::new(format!("{day} · {}", entries.len())).font(theme::bold(15.0)).color(palette.subtext));
        let tracks: Vec<Track> = entries.iter().map(|e| e.track.clone()).collect();
        ui.push_id(("history", day), |ui| track_list(ui, app, &tracks, RowPlace::List, actions));
    }
    if app.play_history.len() > shown && secondary(ui, &palette, "Show more").clicked() {
        inputs.history_shown = shown + 300;
    }
}

fn connect_page(ui: &mut egui::Ui, app: &App, inputs: &mut Inputs, actions: &mut Vec<Action>) {
    use crate::connect::{CommandType, DeviceKind};
    let palette = app.palette;
    ui.label(RichText::new("Connect").font(theme::bold(28.0)).color(palette.text));
    ui.label(
        RichText::new("Your Noctorium on the phone, on other computers and in the terminal, on this network: send the music there, bring it back, play and pause it from here.")
            .color(palette.subtext),
    );
    ui.add_space(8.0);
    let Some(connect) = &app.connect else {
        let why = app.connect_problem.clone().unwrap_or_else(|| {
            "Connect works between devices signed in to the same Noctorium account. Sign in under Settings › Noctorium account.".into()
        });
        notice(ui, &palette, &why, app.connect_problem.is_some());
        if app.account.is_none() && primary(ui, &palette, "Sign in to Noctorium").clicked() {
            actions.push(Action::Open(Page::SettingsAt(super::settings::SettingsPage::Account)));
        }
        connect_extras(ui, app, inputs, actions);
        return;
    };
    ui.label(RichText::new(format!("This computer is “{}”.", connect.identity.name)).color(palette.faint));
    connect_extras(ui, app, inputs, actions);
    if let Some(name) = &app.driven_by {
        ui.label(RichText::new(format!("{name} is playing this computer.")).color(palette.accent));
    }
    let peers = connect.peers();
    if peers.is_empty() {
        ui.add_space(10.0);
        widgets::loading(ui, &palette, "Looking for your other devices…");
        ui.label(
            RichText::new("They show up here when Noctorium is open on them, signed in to the same account, on this network.")
                .color(palette.faint),
        );
        ui.ctx().request_repaint_after(std::time::Duration::from_secs(1));
        return;
    }
    for peer in peers {
        egui::Frame::new()
            .fill(palette.card)
            .corner_radius(egui::CornerRadius::same(theme::corner(12)))
            .inner_margin(egui::Margin::same(14))
            .show(ui, |ui| {
                ui.set_width(ui.available_width());
                ui.horizontal(|ui| {
                    let symbol = match peer.kind {
                        DeviceKind::Phone => "📱",
                        DeviceKind::Terminal => "⌨",
                        DeviceKind::Desktop => "💻",
                    };
                    ui.label(RichText::new(symbol).font(theme::regular(26.0)));
                    ui.vertical(|ui| {
                        ui.label(RichText::new(&peer.name).font(theme::bold(16.0)));
                        match &peer.now_playing {
                            Some(now) => {
                                let what = if now.playing { "Playing" } else { "Paused" };
                                ui.label(RichText::new(format!("{what}: {} · {}", now.title, now.artist)).color(palette.subtext));
                            }
                            None => {
                                ui.label(RichText::new("Nothing playing").color(palette.faint));
                            }
                        }
                    });
                    ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                        if peer.now_playing.is_some() {
                            if secondary(ui, &palette, "⏭").on_hover_text("Next").clicked() {
                                actions.push(Action::ConnectCommand(peer.clone(), CommandType::Next));
                            }
                            let playing = peer.now_playing.as_ref().is_some_and(|n| n.playing);
                            let (label, kind) = if playing { ("⏸", CommandType::Pause) } else { ("▶", CommandType::Play) };
                            if secondary(ui, &palette, label).clicked() {
                                actions.push(Action::ConnectCommand(peer.clone(), kind));
                            }
                            if secondary(ui, &palette, "⏮").on_hover_text("Previous").clicked() {
                                actions.push(Action::ConnectCommand(peer.clone(), CommandType::Previous));
                            }
                            if secondary(ui, &palette, "Bring it here").clicked() {
                                actions.push(Action::ConnectBringBack(peer.clone()));
                            }
                        }
                        if !app.queue.tracks.is_empty() && primary(ui, &palette, "Play there").on_hover_text("Send what plays here to it").clicked() {
                            actions.push(Action::ConnectSend(peer.clone()));
                        }
                    });
                });
            });
        ui.add_space(6.0);
    }
    ui.ctx().request_repaint_after(std::time::Duration::from_secs(1));
}
