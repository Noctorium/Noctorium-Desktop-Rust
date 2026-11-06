//! The rail down the left and the pages it leads to.

use super::widgets::{self, back_button, card, card_strip, heading, notice, primary, secondary, track_list, RowPlace};
use super::{Action, App, Inputs, Loadable, Page};
use crate::bangs::{self, SearchMode};
use crate::domain::{format_duration, Track};
use crate::theme::{self, PRESETS};
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
                if widgets::nav(ui, &palette, symbol, text, *page == target).clicked() {
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
                        Page::Settings => settings(ui, app, inputs, actions),
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
        if primary(ui, &palette, "Open Settings").clicked() {
            actions.push(Action::Open(Page::Settings));
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

fn strip(
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
    ui.label(RichText::new("Search").font(theme::bold(28.0)).color(palette.text));
    ui.add_space(10.0);
    let field = egui::Frame::new()
        .fill(palette.card)
        .stroke(Stroke::new(1.0_f32, palette.line))
        .corner_radius(egui::CornerRadius::same(theme::corner(22)))
        .inner_margin(egui::Margin::symmetric(16, 10))
        .show(ui, |ui| {
            ui.horizontal(|ui| {
                ui.label(RichText::new("🔍").color(palette.subtext));
                let response = ui.add(
                    egui::TextEdit::singleline(&mut inputs.search)
                        .hint_text("Songs, artists, or a YouTube or SoundCloud link — !sc, !ytm, !yt, !all")
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

    heading(ui, &palette, "Your playlists", Some("Made here, free to mix every service"));
    ui.horizontal(|ui| {
        let field = ui.add(egui::TextEdit::singleline(&mut inputs.new_playlist).hint_text("New playlist name").desired_width(260.0));
        let enter = field.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter));
        if (primary(ui, &palette, "＋ Create").clicked() || enter) && !inputs.new_playlist.trim().is_empty() {
            actions.push(Action::CreatePlaylist(std::mem::take(&mut inputs.new_playlist)));
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
                if card(ui, &palette, art, &playlist.title, &format!("{} songs", playlist.tracks.len())).clicked() {
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

fn settings(ui: &mut egui::Ui, app: &App, inputs: &mut Inputs, actions: &mut Vec<Action>) {
    let palette = app.palette;
    ui.horizontal(|ui| {
        ui.label(RichText::new("Settings").font(theme::bold(28.0)).color(palette.text));
        ui.add_space(16.0);
        ui.add(
            egui::TextEdit::singleline(&mut inputs.settings_query)
                .hint_text("🔍  Search Settings: afterglow, booster, lastfm…")
                .desired_width(320.0),
        );
    });
    // Every word typed must be found among a section's words, or the start of one.
    let query: Vec<String> = inputs.settings_query.to_lowercase().split_whitespace().map(String::from).collect();
    let found = std::cell::Cell::new(false);
    let shows = |words: &str| {
        let hit = query.iter().all(|q| words.split_whitespace().any(|w| w.starts_with(q.as_str()) || w.contains(q.as_str())));
        if hit {
            found.set(true);
        }
        hit
    };

    if shows("theme theme colour color dark light night dusk day crimson catppuccin nord dracula gruvbox rose pine tokyo solarized afterglow cinder velvet smoke glow windows 98 xp look appearance") {
    heading(ui, &palette, "Theme", Some("Noctorium's own, and palettes from editors and terminals. Afterglow's light takes the colour of the cover playing."));
    // Families of one -- Nord, Dracula and the rest -- share a row rather than each taking one.
    let family_of = |preset: &theme::ThemePreset| match preset.family {
        "Noctorium" | "Crimson" | "Catppuccin" | "Glow" | "Windows" | "Yours" => preset.family,
        _ => "Editors and terminals",
    };
    let yours = theme::ThemePreset {
        key: theme::CUSTOM,
        name: "Your own",
        family: "Yours",
        colours: app.settings.looks.custom_theme.colours(),
        glow: None,
    };
    let all: Vec<&theme::ThemePreset> = PRESETS.iter().chain(std::iter::once(&yours)).collect();
    let mut families: Vec<&str> = Vec::new();
    for preset in all.iter().copied() {
        if !families.contains(&family_of(preset)) {
            families.push(family_of(preset));
        }
    }
    for family in families {
        ui.label(RichText::new(family).font(theme::bold(12.5)).color(palette.subtext));
        ui.horizontal_wrapped(|ui| {
            ui.spacing_mut().item_spacing = vec2(10.0, 10.0);
            for preset in all.iter().copied().filter(|p| family_of(p) == family) {
                if widgets::theme_swatch(ui, &palette, preset, app.settings.theme == preset.key).clicked() {
                    actions.push(Action::SetTheme(preset.key));
                }
            }
        });
        ui.add_space(6.0);
    }
    }

    if shows("personalisation personalisation personalization looks browsing cards card buttons hover sidebar pages home parts greeting genres bandcamp animations motion text size bigger smaller zoom clock columns status follow dim panel width tab related time remaining theme accent font typeface serif monospace mono corners sharp round lyrics size alignment centred cover record vinyl circle backdrop wash layout now playing stage focus sing along banner seek progress bar wave neon beads ruler luna slider player slim floating stacked centred spotlight island display taskbar toolbar top bottom icons rounded outlined filled two-tone line glass liquid surfaces full split flow turntable poster big type visualiser meter badges marks letters custom of your own colours colors") {
    looks_settings(ui, app, actions);
    }

    if shows("sound sound equaliser equalizer eq bass treble preset booster boost loud volume loudness normalise normalize sponsorblock sponsor skip intro") {
    sound_settings(ui, app, actions);
    }

    if shows("downloads downloads download offline mp3 save file export folder desktop original quality") {
    download_settings(ui, app, actions);
    }

    if shows("playback playback autoplay radio queue restore speed tempo search goes bang default service picked home shelf recent lately sleep timer fade lyrics source lrclib genius musixmatch spotify premium device everywhere hybrid") {
    heading(ui, &palette, "Playback", None);
    let mut autoplay = app.settings.autoplay;
    if ui.checkbox(&mut autoplay, "Autoplay: when the queue runs out, carry on with songs like the last one").changed() {
        actions.push(Action::SetAutoplay(autoplay));
    }
    let mut restore = app.settings.restore_queue;
    if ui.checkbox(&mut restore, "Bring back the queue, and where the song was, when Noctorium opens").changed() {
        actions.push(Action::SetRestoreQueue(restore));
    }
    ui.horizontal(|ui| {
        ui.label("Speed");
        let mut speed = app.settings.speed;
        let slider = ui.add(egui::Slider::new(&mut speed, 0.5..=2.0).step_by(0.05).suffix("×"));
        if slider.drag_stopped() || (slider.changed() && !slider.dragged()) {
            actions.push(Action::SetSpeed(speed));
        }
        if (app.settings.speed - 1.0).abs() > 0.001 && ui.button("1×").clicked() {
            actions.push(Action::SetSpeed(1.0));
        }
    });
    ui.horizontal(|ui| {
        ui.label("Search goes to");
        egui::ComboBox::from_id_salt("search-mode")
            .selected_text(app.settings.search_mode.display_name())
            .show_ui(ui, |ui| {
                for mode in SearchMode::OFFERED {
                    if ui.selectable_label(app.settings.search_mode == mode, mode.display_name()).clicked() {
                        actions.push(Action::SetSearchMode(mode));
                    }
                }
            });
        ui.label(RichText::new("unless a bang says otherwise").color(palette.faint));
    });
    playback_more(ui, app, actions);
    }

    if shows("accounts accounts account sign in signin login youtube music soundcloud browser cookies firefox chrome edge history") {
    let mut history = app.settings.youtube_history;
    if ui.checkbox(&mut history, "Add what I play to my YouTube Music history (and let its recommendations learn from it)").changed() {
        let mut settings = app.settings.clone();
        settings.youtube_history = history;
        actions.push(Action::SetSettings(Box::new(settings)));
    }
    heading(ui, &palette, "Accounts", Some("Your YouTube Music and SoundCloud, signed in from a browser you already use."));
    ui.horizontal(|ui| {
        ui.label("Sign-ins from");
        let current = app.settings.cookies_browser.clone();
        egui::ComboBox::from_id_salt("cookies")
            .selected_text(current.clone().unwrap_or_else(|| "No browser".into()))
            .show_ui(ui, |ui| {
                if ui.selectable_label(current.is_none(), "No browser").clicked() {
                    actions.push(Action::SetCookies(None));
                }
                for browser in ["firefox", "chrome", "edge", "brave", "chromium", "opera", "vivaldi", "safari"] {
                    if ui.selectable_label(current.as_deref() == Some(browser), browser).clicked() {
                        actions.push(Action::SetCookies(Some(browser.to_string())));
                    }
                }
            });
    });
    ui.label(
        RichText::new("Only browsing uses it: streams are always fetched signed out, which YouTube refuses less. Chrome and Edge lock their cookies while they are open.")
            .font(theme::regular(12.5))
            .color(palette.faint),
    );
    ui.add_space(6.0);
    for provider in [crate::domain::ProviderType::YoutubeMusic, crate::domain::ProviderType::Soundcloud] {
        let who = match provider {
            crate::domain::ProviderType::Soundcloud => app.soundcloud.as_ref().map(|(_, p)| p.name.clone()),
            _ => app.youtube.as_ref().map(|_| app.youtube_name.clone().unwrap_or_else(|| "your account".into())),
        };
        ui.horizontal(|ui| {
            ui.label(RichText::new(provider.display_name()).font(theme::bold(14.5)));
            match who {
                Some(name) => {
                    ui.label(RichText::new(format!("Signed in as {name}")).color(palette.accent));
                    if ui.button("Sign out").clicked() {
                        actions.push(Action::SignOut(provider));
                    }
                }
                None if app.signing_in == Some(provider) => {
                    widgets::loading(ui, &palette, "Reading the browser's sign-in…");
                }
                None => {
                    let label = match &app.settings.cookies_browser {
                        Some(browser) => format!("Sign in from {browser}"),
                        None => "Sign in from your browser".to_string(),
                    };
                    if primary(ui, &palette, &label).clicked() {
                        actions.push(Action::SignIn(provider));
                    }
                }
            }
        });
    }
    ui.label(
        RichText::new("Signed in, your playlists and likes are in the Library, YouTube Music's home is yours, and liking a song likes it there too.")
            .font(theme::regular(12.5))
            .color(palette.faint),
    );
    }

    if shows("tools playback tools yt-dlp ytdlp mpv install update version") {
    heading(ui, &palette, "Playback tools", Some("yt-dlp reads the services; mpv makes the sound."));
    tool_row(ui, app, Tool::YtDlp, &app.yt_dlp, actions);
    tool_row(ui, app, Tool::Mpv, &app.mpv, actions);
    }

    if shows("library library import bring over noctorium kotlin data folder") {
    heading(ui, &palette, "Your library", None);
    ui.horizontal(|ui| {
        if secondary(ui, &palette, "Bring over from Noctorium").on_hover_text("Reads the playlists, pinned songs and history of the Kotlin desktop on this computer").clicked() {
            actions.push(Action::ImportNeighbour);
        }
        if secondary(ui, &palette, "Open the data folder").clicked() {
            actions.push(Action::OpenFolder);
        }
    });
    if let Some(root) = &app.places.root {
        ui.label(RichText::new(root.display().to_string()).font(theme::regular(12.0)).color(palette.faint));
    }
    }

    if shows("window window tray close minimise minimize start startup sign in login boot launch announce notification note opening open page") {
        heading(ui, &palette, "The window", None);
        let mut announce = app.settings.announce_tracks;
        if ui.checkbox(&mut announce, "Say what is playing when a song starts, with a note from the system while the window is away").changed() {
            let mut settings = app.settings.clone();
            settings.announce_tracks = announce;
            actions.push(Action::SetSettings(Box::new(settings)));
        }
        ui.horizontal(|ui| {
            ui.label("Open Noctorium on");
            for page in crate::store::StartPage::ALL {
                if ui.selectable_label(app.settings.start_page == page, page.label()).clicked() && app.settings.start_page != page {
                    let mut settings = app.settings.clone();
                    settings.start_page = page;
                    actions.push(Action::SetSettings(Box::new(settings)));
                }
            }
        });
        let mut tray = app.settings.close_to_tray;
        if ui.checkbox(&mut tray, "Closing the window leaves Noctorium playing in the tray").changed() {
            actions.push(Action::SetCloseToTray(tray));
        }
        ui.horizontal(|ui| {
            ui.label("Open Noctorium when you sign in to the computer");
            for mode in [crate::startup::Mode::Off, crate::startup::Mode::Window, crate::startup::Mode::Tray] {
                if ui.selectable_label(app.startup_mode == mode, mode.name()).clicked() && app.startup_mode != mode {
                    actions.push(Action::SetStartup(mode));
                }
            }
        });
    }

    if shows("widgets stream widgets obs streamlabs overlay browser source twitch now playing") {
        widget_settings(ui, app, actions);
    }

    if shows("keyboard keyboard shortcuts keys hotkeys hot keys global media keys") {
    heading(ui, &palette, "Keyboard", None);
    ui.horizontal(|ui| {
        if secondary(ui, &palette, "Change the shortcuts").clicked() {
            actions.push(Action::Open(Page::Shortcuts));
        }
        if secondary(ui, &palette, "Show them  ?").clicked() {
            actions.push(Action::ShowShortcuts(true));
        }
    });
    let media = if app.media_keys.is_empty() {
        if cfg!(windows) {
            "The keyboard's media keys are held by another program.".to_string()
        } else {
            "The keyboard's media keys are answered on Windows.".to_string()
        }
    } else {
        format!("Media keys from any program: {}", app.media_keys.join(", "))
    };
    ui.label(RichText::new(media).font(theme::regular(12.5)).color(palette.faint));
    }

    if shows("noctorium noctorium account sign in statistics stats plays hours email password connect devices device name network") {
    account_settings(ui, app, inputs, actions);
    connect_settings(ui, app, inputs, actions);
    }

    if shows("scrobbling scrobbling scrobble last.fm lastfm libre.fm librefm listenbrainz") {
    scrobble_settings(ui, app, inputs, actions);
    }

    if shows("discord discord presence status rich profile") {
    discord_settings(ui, app, actions);
    }

    if shows("diagnostics diagnostics check checks problem not working broken troubleshoot test yt-dlp mpv storage network") {
    diagnostics_settings(ui, app, actions);
    }

    if shows("about about updates update version licence license newer github source install") {
    heading(ui, &palette, "About and updates", None);
    let mut check = app.settings.check_updates;
    if ui.checkbox(&mut check, "Look for updates when Noctorium starts (one request to GitHub; nothing is downloaded without asking)").changed() {
        let mut settings = app.settings.clone();
        settings.check_updates = check;
        actions.push(Action::SetSettings(Box::new(settings)));
    }
    if let Some(newer) = &app.newer {
        ui.horizontal(|ui| {
            ui.label(RichText::new(format!("Noctorium {} is out.", newer.version)).color(palette.accent));
            if ui.link("See what is new").clicked() {
                actions.push(Action::OpenUrl(newer.url.clone()));
            }
            if app.installing_update {
                widgets::loading(ui, &palette, "Downloading the installer…");
            } else if newer.installer.is_some() && primary(ui, &palette, "Install it").clicked() {
                actions.push(Action::InstallUpdate);
            }
        });
    }
    ui.label(format!("Noctorium Rust {}: Noctorium for the desktop, in Rust", crate::version()));
    ui.label(
        RichText::new(
            "Noctorium is an independent third-party client. It is not affiliated with Google, YouTube, SoundCloud, Spotify, Bandcamp, VK, Apple, Deezer, TIDAL, Last.fm, ListenBrainz or Discord.",
        )
        .color(palette.faint),
    );
    ui.label(RichText::new("GPL-3.0-only. Lyrics from LRCLIB, Better Lyrics, Karalyr, SyncLRC, lyrics.ovh, Musixmatch, Happi and Genius.").color(palette.faint));
    ui.horizontal(|ui| {
        if ui.link("Source on GitHub").clicked() {
            actions.push(Action::OpenUrl("https://github.com/Noctorium/Noctorium-Desktop-Rust".into()));
        }
    });
    }

    if !query.is_empty() && !found.get() {
        ui.add_space(12.0);
        ui.label(RichText::new("No setting goes by that name.").color(palette.faint));
    }
}

fn tool_row(ui: &mut egui::Ui, app: &App, tool: Tool, info: &super::ToolInfo, actions: &mut Vec<Action>) {
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


/// Settings › Sound: the equaliser, the volume booster, evening out, and SponsorBlock.
fn sound_settings(ui: &mut egui::Ui, app: &App, actions: &mut Vec<Action>) {
    use crate::audio::{band_label, boost, EqualizerPreset, BANDS_HZ, MAX_GAIN_DB};
    let palette = app.palette;
    heading(ui, &palette, "Sound", Some("Shaped by mpv on its way out, for every song from every service."));

    let mut eq = app.settings.equalizer.clone();
    let mut changed = false;
    ui.horizontal(|ui| {
        changed |= ui.checkbox(&mut eq.enabled, "Equaliser").changed();
        ui.add_enabled_ui(eq.enabled, |ui| {
            egui::ComboBox::from_id_salt("eq-preset").selected_text(eq.preset.name()).show_ui(ui, |ui| {
                for preset in EqualizerPreset::ALL {
                    if ui.selectable_label(eq.preset == preset, preset.name()).clicked() {
                        if preset == EqualizerPreset::Custom && eq.preset != EqualizerPreset::Custom {
                            // Your own starts from the curve you were hearing.
                            eq.custom_gains = eq.gains();
                        }
                        eq.preset = preset;
                        changed = true;
                    }
                }
            });
            if ui.button("Flat").on_hover_text("Every band back to nothing").clicked() {
                eq.preset = EqualizerPreset::Flat;
                eq.preamp_db = 0.0;
                changed = true;
            }
        });
    });
    ui.add_enabled_ui(eq.enabled, |ui| {
        ui.horizontal(|ui| {
            ui.spacing_mut().item_spacing.x = 14.0;
            let mut gains = eq.gains();
            let mut released = false;
            for (index, hz) in BANDS_HZ.iter().enumerate() {
                ui.vertical(|ui| {
                    let slider = ui.add(
                        egui::Slider::new(&mut gains[index], -MAX_GAIN_DB..=MAX_GAIN_DB)
                            .vertical()
                            .step_by(0.5)
                            .show_value(false),
                    );
                    if slider.changed() {
                        eq.preset = EqualizerPreset::Custom;
                        eq.custom_gains = gains;
                    }
                    released |= slider.drag_stopped() || (slider.changed() && !slider.dragged());
                    slider.on_hover_text(format!("{:+.1} dB", gains[index]));
                    ui.label(RichText::new(band_label(*hz)).font(theme::regular(11.0)).color(palette.subtext));
                });
            }
            ui.vertical(|ui| {
                let slider = ui.add(
                    egui::Slider::new(&mut eq.preamp_db, -12.0..=12.0).vertical().step_by(0.5).show_value(false),
                );
                released |= slider.drag_stopped() || (slider.changed() && !slider.dragged());
                slider.on_hover_text(format!("Before the bands: {:+.1} dB", eq.preamp_db));
                ui.label(RichText::new("Pre").font(theme::regular(11.0)).color(palette.subtext));
            });
            // Sent once a band is let go of: rewriting mpv's filters on every step of a drag stutters.
            changed |= released;
        });
    });
    if changed {
        actions.push(Action::SetEqualizer(eq));
    }

    ui.add_space(8.0);
    ui.horizontal(|ui| {
        ui.label("Volume booster");
        let mut percent = app.settings.boost_percent as f32;
        let slider = ui.add(
            egui::Slider::new(&mut percent, boost::OFF as f32..=boost::MAX as f32)
                .step_by(boost::STEP as f64)
                .custom_formatter(|v, _| if v as u32 <= boost::OFF { "Off".into() } else { format!("{v:.0}%") }),
        );
        if slider.drag_stopped() || (slider.changed() && !slider.dragged()) {
            actions.push(Action::SetBoost(percent as u32));
        }
        if boost::is_on(app.settings.boost_percent) && ui.button("Off").clicked() {
            actions.push(Action::SetBoost(boost::OFF));
        }
    });
    if boost::is_loud(app.settings.boost_percent) {
        ui.label(RichText::new(boost::LOUD_WARNING).color(palette.error));
    }
    ui.label(
        RichText::new("Louder than the computer's own volume goes, behind a limiter that keeps it from breaking up.")
            .font(theme::regular(12.5))
            .color(palette.faint),
    );

    let mut bars = app.settings.visualiser;
    if ui.checkbox(&mut bars, "Visualiser: bars under the cover on Now playing, measured from the sound").changed() {
        actions.push(Action::SetVisualiser(bars));
    }
    let mut even = app.settings.even_loudness;
    if ui.checkbox(&mut even, "Even out loudness: songs play at about the same volume").changed() {
        actions.push(Action::SetEvenLoudness(even));
    }
    let mut sponsor = app.settings.sponsorblock;
    if ui
        .checkbox(&mut sponsor, "SponsorBlock: skip sponsors, intros and other parts of YouTube videos that are not the music")
        .changed()
    {
        actions.push(Action::SetSponsorBlock(sponsor));
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

fn download_settings(ui: &mut egui::Ui, app: &App, actions: &mut Vec<Action>) {
    use crate::downloads::ExportFormat;
    let palette = app.palette;
    heading(ui, &palette, "Downloads", Some("Songs downloaded play offline; songs saved as files are yours to keep."));
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

fn discord_settings(ui: &mut egui::Ui, app: &App, actions: &mut Vec<Action>) {
    use crate::discord::{ActivityKind, Paused, Timestamps};
    let palette = app.palette;
    heading(ui, &palette, "Discord", Some("What is playing, on your Discord profile, while Discord runs on this computer."));
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
                    let field = ui.add(
                        egui::TextEdit::singleline(&mut quick.text)
                            .hint_text("Search — !sc, !ytm, !yt at the start or the end")
                            .font(theme::regular(17.0))
                            .frame(false)
                            .desired_width(f32::INFINITY),
                    );
                    if quick.focus {
                        field.request_focus();
                        quick.focus = false;
                    }
                    if field.changed() {
                        quick.typed_at = std::time::Instant::now();
                    }
                    if enter {
                        if let Some(track) = results.get(quick.selected) {
                            actions.push(Action::PlayList { tracks: results.clone(), index: quick.selected });
                            actions.push(Action::CloseQuickSearch);
                            let _ = track;
                        } else if !quick.text.trim().is_empty() {
                            actions.push(Action::QuickSearchAll(quick.text.clone()));
                        }
                    }
                    if quick.text.trim().is_empty() {
                        return;
                    }
                    ui.separator();
                    match &quick.results {
                        None => widgets::loading(ui, &palette, "Searching…"),
                        Some(Err(why)) => notice(ui, &palette, why, true),
                        Some(Ok(tracks)) if tracks.is_empty() => {
                            ui.label(RichText::new("Nothing found.").color(palette.faint));
                        }
                        Some(Ok(_)) => {
                            for (index, track) in results.iter().enumerate() {
                                let selected = index == quick.selected;
                                let row = ui.horizontal(|ui| {
                                    widgets::artwork(ui, &palette, track.artwork_url.as_deref(), 36.0, 5);
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
                                    actions.push(Action::PlayList { tracks: results.clone(), index });
                                    actions.push(Action::CloseQuickSearch);
                                }
                            }
                        }
                    }
                    ui.add_space(4.0);
                    if ui.link("All results on the search page").clicked() {
                        actions.push(Action::QuickSearchAll(quick.text.clone()));
                    }
                });
        });
}

fn scrobble_settings(ui: &mut egui::Ui, app: &App, inputs: &mut Inputs, actions: &mut Vec<Action>) {
    use crate::scrobble::Site;
    let palette = app.palette;
    heading(
        ui,
        &palette,
        "Scrobbling",
        Some("What you listen to, on Last.fm, Libre.fm and ListenBrainz: a song counts once half of it, or four minutes, has played."),
    );
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

fn account_settings(ui: &mut egui::Ui, app: &App, inputs: &mut Inputs, actions: &mut Vec<Action>) {
    let palette = app.palette;
    heading(ui, &palette, "Noctorium account", Some("One sign-in for the players and the website, and the statistics of what you listen to."));
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

    // ---- Spotify
    heading(ui, &palette, "Spotify", Some("Its songs are found on YouTube Music to be played. Paste a public playlist or album link into Search to open it."));
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

    // ---- Apple Music
    heading(ui, &palette, "Apple Music", Some("Search it with !am. Paste an album or playlist link into Search to open it."));
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

    // ---- Tidal
    heading(ui, &palette, "Tidal", Some("Search it with !td. Tidal lets programs in only through an app of your own."));
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

    // ---- VK
    heading(ui, &palette, "VK Music", Some("Search it with !vk, with your VK sign-in. Paste a VK playlist link into Search to open it."));
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

    // ---- Bandcamp
    heading(ui, &palette, "Bandcamp", Some("Search it with !bc. Paste an album or song link into Search to open it. Played as the stream Bandcamp gives anyone."));
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

    // ---- Deezer
    heading(ui, &palette, "Deezer", Some("Search it with !dz. Paste a playlist or album link into Search to open it."));
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

/// Every shortcut, to change: in the window, and -- for the actions that can be -- from any program.
fn shortcuts_page(ui: &mut egui::Ui, app: &App, actions: &mut Vec<Action>) {
    use crate::shortcuts::{Group, ACTIONS};
    let palette = app.palette;
    back_button(ui, &palette, actions);
    ui.label(RichText::new("Keyboard shortcuts").font(theme::bold(28.0)).color(palette.text));
    ui.label(RichText::new("Change: press the new keys. Esc leaves it as it was; Backspace leaves it with none.").color(palette.subtext));
    ui.add_space(6.0);
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

/// One row of choices from Settings › Personalisation, each with what it does on hover. True when changed.
fn choice_row<T: crate::looks::Choice>(ui: &mut egui::Ui, palette: &theme::Palette, title: &str, value: &mut T) -> bool {
    let mut changed = false;
    ui.horizontal_wrapped(|ui| {
        ui.add_sized(vec2(130.0, 20.0), egui::Label::new(RichText::new(title).color(palette.subtext)));
        for &choice in T::ALL {
            let response = ui.selectable_value(value, choice, choice.label());
            let response = if choice.description().is_empty() { response } else { response.on_hover_text(choice.description()) };
            changed |= response.changed();
        }
    });
    changed
}

/// Settings › Personalisation: the accent, a theme of your own, and how everything is laid out and drawn.
fn looks_settings(ui: &mut egui::Ui, app: &App, actions: &mut Vec<Action>) {
    use crate::looks::{parse_hex, to_hex, Accent};
    let palette = app.palette;
    let mut looks = app.settings.looks.clone();
    let mut changed = false;
    heading(ui, &palette, "Personalisation", Some("Beyond the theme: the accent, the typeface, the corners, the lyrics and how Now playing looks."));
    changed |= choice_row(ui, &palette, "Accent", &mut looks.accent);
    if looks.accent == Accent::Custom {
        ui.horizontal(|ui| {
            ui.add_space(134.0);
            let field = ui.add_sized(vec2(96.0, 22.0), egui::TextEdit::singleline(&mut looks.custom_accent).hint_text("#B47CFF"));
            changed |= field.changed();
            match parse_hex(&looks.custom_accent) {
                Some(rgb) => {
                    let (swatch, _) = ui.allocate_exact_size(vec2(20.0, 20.0), egui::Sense::hover());
                    ui.painter().rect_filled(swatch, 4, theme::rgb(rgb));
                }
                None => {
                    ui.label(RichText::new("A colour as six hex digits, #RRGGBB.").color(palette.faint));
                }
            }
        });
    }
    changed |= choice_row(ui, &palette, "Typeface", &mut looks.font);
    changed |= choice_row(ui, &palette, "Text size", &mut looks.text_size);
    ui.horizontal(|ui| {
        ui.add_sized(vec2(130.0, 20.0), egui::Label::new(RichText::new("Animations").color(palette.subtext)));
        changed |= ui.checkbox(&mut looks.animations, "Pages fading in, the light easing, records turning; off makes every change instant").changed();
    });
    changed |= choice_row(ui, &palette, "Corners", &mut looks.corners);
    changed |= choice_row(ui, &palette, "Player bar", &mut looks.player_bar);
    changed |= choice_row(ui, &palette, "Player bar at", &mut looks.player_bar_position);
    ui.horizontal_wrapped(|ui| {
        ui.add_sized(vec2(130.0, 20.0), egui::Label::new(RichText::new("Its buttons").color(palette.subtext)));
        for button in crate::looks::PlayerButton::ALL {
            let mut on = looks.player_buttons.contains(&button);
            if ui.checkbox(&mut on, button.label()).changed() {
                changed = true;
                if on {
                    looks.player_buttons.push(button);
                } else {
                    looks.player_buttons.retain(|b| *b != button);
                }
            }
        }
    });
    ui.horizontal(|ui| {
        ui.add_sized(vec2(130.0, 20.0), egui::Label::new(RichText::new("Taskbar clock").color(palette.subtext)));
        changed |= ui.checkbox(&mut looks.taskbar_clock, "The time in the Taskbar layout's tray").changed();
    });
    changed |= choice_row(ui, &palette, "Right-hand time", &mut looks.time_display);
    changed |= choice_row(ui, &palette, "Icons", &mut looks.icons);
    changed |= choice_row(ui, &palette, "Surfaces", &mut looks.surface);
    changed |= choice_row(ui, &palette, "Seek bar", &mut looks.progress);
    ui.add_space(6.0);
    ui.label(RichText::new("Browsing").font(theme::bold(13.0)).color(palette.subtext));
    changed |= choice_row(ui, &palette, "Card size", &mut looks.card_size);
    changed |= choice_row(ui, &palette, "Service badges", &mut looks.badge_policy);
    changed |= choice_row(ui, &palette, "Badges drawn as", &mut looks.badges);
    changed |= choice_row(ui, &palette, "Play and menu", &mut looks.hover_controls);
    ui.horizontal_wrapped(|ui| {
        ui.add_sized(vec2(130.0, 20.0), egui::Label::new(RichText::new("Parts of Home").color(palette.subtext)));
        for part in crate::looks::HomePart::ALL {
            let mut on = !looks.hidden_home.contains(&part);
            if ui.checkbox(&mut on, part.label()).changed() {
                changed = true;
                if on {
                    looks.hidden_home.retain(|p| *p != part);
                } else {
                    looks.hidden_home.push(part);
                }
            }
        }
    });
    ui.horizontal_wrapped(|ui| {
        ui.add_sized(vec2(130.0, 20.0), egui::Label::new(RichText::new("Pages in the sidebar").color(palette.subtext)));
        for page in crate::looks::SidebarPage::ALL {
            let mut on = !looks.hidden_pages.contains(&page);
            if ui.checkbox(&mut on, page.label()).changed() {
                changed = true;
                if on {
                    looks.hidden_pages.retain(|p| *p != page);
                } else {
                    looks.hidden_pages.push(page);
                }
            }
        }
        ui.label(RichText::new("Home and Settings always stay").color(palette.faint));
    });
    let mut genres = app.settings.bandcamp_genres.clone();
    let mut genres_changed = false;
    ui.horizontal_wrapped(|ui| {
        ui.add_sized(vec2(130.0, 20.0), egui::Label::new(RichText::new("Bandcamp on Home").color(palette.subtext)));
        for (tag, name) in crate::bandcamp::GENRES {
            let mut on = genres.iter().any(|g| g == tag);
            if ui.checkbox(&mut on, name).changed() {
                genres_changed = true;
                if on {
                    genres.push(tag.to_string());
                } else {
                    genres.retain(|g| g != tag);
                }
            }
        }
    });
    if genres_changed {
        let mut settings = app.settings.clone();
        settings.bandcamp_genres = genres;
        actions.push(Action::SetSettings(Box::new(settings)));
    }
    ui.add_space(6.0);
    ui.label(RichText::new("Now playing").font(theme::bold(13.0)).color(palette.subtext));
    changed |= choice_row(ui, &palette, "Layout", &mut looks.now_playing);
    changed |= choice_row(ui, &palette, "Cover", &mut looks.cover);
    changed |= choice_row(ui, &palette, "Cover size", &mut looks.cover_size);
    changed |= choice_row(ui, &palette, "Panel width", &mut looks.panel_width);
    changed |= choice_row(ui, &palette, "Panel opens on", &mut looks.open_on);
    ui.horizontal(|ui| {
        ui.add_sized(vec2(130.0, 20.0), egui::Label::new(RichText::new("Under the cover").color(palette.subtext)));
        changed |= ui.checkbox(&mut looks.lyric_line, "The line being sung (off shows the album)").changed();
        changed |= ui.checkbox(&mut looks.follow_button, "A button to follow the artist").changed();
    });
    ui.horizontal_wrapped(|ui| {
        ui.add_sized(vec2(130.0, 20.0), egui::Label::new(RichText::new("Playlist layout").color(palette.subtext)));
        for column in crate::looks::QueueColumn::ALL {
            let mut on = looks.queue_columns.contains(&column);
            if ui.checkbox(&mut on, column.label()).changed() {
                changed = true;
                if on {
                    // Back in its usual place among the others.
                    looks.queue_columns.push(column);
                    let order = |c: &crate::looks::QueueColumn| crate::looks::QueueColumn::ALL.iter().position(|x| x == c);
                    looks.queue_columns.sort_by_key(order);
                } else {
                    looks.queue_columns.retain(|c| *c != column);
                }
            }
        }
        changed |= ui.checkbox(&mut looks.status_bar, "Status bar").changed();
    });
    changed |= choice_row(ui, &palette, "Behind it", &mut looks.backdrop);
    changed |= choice_row(ui, &palette, "Visualiser", &mut looks.visualiser);
    ui.add_space(6.0);
    ui.label(RichText::new("Lyrics").font(theme::bold(13.0)).color(palette.subtext));
    changed |= choice_row(ui, &palette, "Size", &mut looks.lyrics_size);
    changed |= choice_row(ui, &palette, "Alignment", &mut looks.lyrics_alignment);
    ui.horizontal(|ui| {
        ui.add_sized(vec2(130.0, 20.0), egui::Label::new(RichText::new("Dim the others").color(palette.subtext)));
        changed |= ui.checkbox(&mut looks.dim_lyrics, "Dim the lines not being sung").changed();
    });

    ui.add_space(10.0);
    ui.label(RichText::new("A theme of your own").font(theme::bold(13.0)).color(palette.subtext));
    ui.label(RichText::new("Six colours, as hex codes. Pick \"Your own\" under Theme to use them.").color(palette.faint));
    let custom = &mut looks.custom_theme;
    egui::Grid::new("custom-theme").num_columns(3).spacing(vec2(10.0, 6.0)).show(ui, |ui| {
        for (name, value) in [
            ("Page", &mut custom.background),
            ("Panels", &mut custom.panel),
            ("Cards", &mut custom.card),
            ("Writing", &mut custom.text),
            ("Quieter writing", &mut custom.subtext),
            ("Accent", &mut custom.accent),
        ] {
            ui.label(name);
            changed |= ui.add_sized(vec2(96.0, 22.0), egui::TextEdit::singleline(value)).changed();
            match parse_hex(value) {
                Some(rgb) => {
                    let (swatch, _) = ui.allocate_exact_size(vec2(20.0, 20.0), egui::Sense::hover());
                    ui.painter().rect_filled(swatch, 4, theme::rgb(rgb));
                    ui.painter().rect_stroke(swatch, 4, Stroke::new(1.0_f32, palette.line), egui::StrokeKind::Inside);
                }
                None => {
                    ui.label(RichText::new("not a colour").color(palette.error));
                }
            }
            ui.end_row();
        }
    });
    changed |= ui.checkbox(&mut custom.light, "It is a light theme (dark writing on a pale page)").changed();
    let colours = custom.colours();
    let contrast = crate::looks::contrast_ratio(colours.text, colours.background);
    if contrast < 4.5 {
        ui.label(RichText::new(format!("The writing stands out only {contrast:.1}:1 from the page; 4.5:1 or more reads comfortably.")).color(palette.error));
    }
    ui.horizontal(|ui| {
        if app.settings.theme != theme::CUSTOM && primary(ui, &palette, "Use my theme").clicked() {
            actions.push(Action::SetTheme(theme::CUSTOM));
        }
        if secondary(ui, &palette, "Start from the theme in use").on_hover_text("Copy the current theme's colours into these").clicked() {
            let now = theme::colours_of(&app.settings.theme, &app.settings.looks);
            let mut from = app.settings.looks.clone();
            from.custom_theme = crate::looks::CustomTheme {
                background: to_hex(now.background),
                panel: to_hex(now.panel),
                card: to_hex(now.card),
                text: to_hex(now.text),
                subtext: to_hex(now.subtext),
                accent: to_hex(now.accent),
                light: now.light,
            };
            actions.push(Action::SetLooks(Box::new(from)));
        }
        if secondary(ui, &palette, "Reset everything here").clicked() {
            actions.push(Action::SetLooks(Box::default()));
        }
    });
    if changed {
        actions.push(Action::SetLooks(Box::new(looks)));
    }
}

/// The rest of Settings › Playback: what a picked song does, autoplay's source, the sleep timer's fade, the
/// lyrics' first source, and how Spotify's songs play.
fn playback_more(ui: &mut egui::Ui, app: &App, actions: &mut Vec<Action>) {
    use crate::store::{AutoplaySource, HomePlay, SpotifyPlayback};
    let palette = app.palette;
    let mut settings = app.settings.clone();
    let mut changed = false;
    ui.horizontal_wrapped(|ui| {
        ui.label("Playing a song from Home or a search");
        changed |= ui.selectable_value(&mut settings.home_play, HomePlay::Autoplay, "Autoplay after it").on_hover_text("The song, then songs like it, as a radio started from it").changed();
        changed |= ui.selectable_value(&mut settings.home_play, HomePlay::Shelf, "The songs around it").on_hover_text("The song, then the rest of the shelf or the results it was picked from").changed();
    });
    ui.horizontal_wrapped(|ui| {
        ui.label("Autoplay's songs come from");
        changed |= ui.selectable_value(&mut settings.autoplay_from, AutoplaySource::SameService, "The same service").changed();
        changed |= ui.selectable_value(&mut settings.autoplay_from, AutoplaySource::YoutubeMusic, "YouTube Music radio").on_hover_text("YouTube Music's radio after every song, whatever service it came from").changed();
    });
    changed |= ui.checkbox(&mut settings.autoplay_avoid_recent, "Skip songs played lately, so the same few do not keep coming round").changed();
    ui.horizontal_wrapped(|ui| {
        ui.label("The sleep timer fades out over");
        for seconds in [0u32, 15, 30, 60] {
            let label = if seconds == 0 { "No fade".to_string() } else { format!("{seconds} seconds") };
            changed |= ui.selectable_value(&mut settings.sleep_fade_seconds, seconds, label).changed();
        }
    });
    ui.horizontal_wrapped(|ui| {
        ui.label("Open lyrics on");
        for provider in crate::lyrics::Provider::ALL {
            let mut label = RichText::new(provider.display_name());
            if let Some(key) = provider.key_environment() {
                if std::env::var(key).map(|k| k.trim().is_empty()).unwrap_or(true) {
                    label = label.color(palette.faint);
                }
            }
            let response = ui.selectable_value(&mut settings.lyrics_source, provider, label);
            let response = match provider.key_environment() {
                Some(key) => response.on_hover_text(format!("Needs a key in {key}")),
                None => response.on_hover_text("Every source is still asked"),
            };
            changed |= response.changed();
        }
    });
    ui.horizontal_wrapped(|ui| {
        ui.label("Spotify's songs");
        changed |= ui.selectable_value(&mut settings.spotify_playback, SpotifyPlayback::Matched, "Matched and played here").changed();
        changed |= ui
            .selectable_value(&mut settings.spotify_playback, SpotifyPlayback::OnSpotify, "Played on Spotify (Premium)")
            .on_hover_text("Spotify's songs play in your Spotify app, on the device you choose")
            .changed();
    });
    if settings.spotify_playback == SpotifyPlayback::OnSpotify {
        ui.horizontal_wrapped(|ui| {
            ui.label("On");
            if crate::spotify::current().is_none() {
                ui.label(RichText::new("sign in to Spotify under Services first").color(palette.faint));
                return;
            }
            changed |= ui.selectable_value(&mut settings.spotify_device, String::new(), "Whichever is active").changed();
            match &app.spotify_devices {
                Some(Ok(devices)) => {
                    for (id, name, active) in devices {
                        let label = if *active { format!("{name} (active)") } else { name.clone() };
                        changed |= ui.selectable_value(&mut settings.spotify_device, id.clone(), label).changed();
                    }
                }
                Some(Err(why)) => {
                    ui.label(RichText::new(why).color(palette.error));
                }
                None => {}
            }
            if ui.small_button("⟳ Devices").clicked() {
                actions.push(Action::LoadSpotifyDevices);
            }
        });
    }
    ui.horizontal_wrapped(|ui| {
        ui.label("Everywhere (!all) asks");
        for provider in [
            crate::domain::ProviderType::YoutubeMusic,
            crate::domain::ProviderType::YoutubeVideo,
            crate::domain::ProviderType::Soundcloud,
            crate::domain::ProviderType::Bandcamp,
            crate::domain::ProviderType::Spotify,
            crate::domain::ProviderType::Vk,
            crate::domain::ProviderType::Deezer,
            crate::domain::ProviderType::AppleMusic,
            crate::domain::ProviderType::Tidal,
        ] {
            let mut on = settings.hybrid_search.contains(&provider);
            if ui.checkbox(&mut on, provider.display_name()).changed() {
                changed = true;
                if on {
                    settings.hybrid_search.push(provider);
                } else {
                    settings.hybrid_search.retain(|p| *p != provider);
                }
            }
        }
    });
    if changed {
        actions.push(Action::SetSettings(Box::new(settings)));
    }
}

/// Connect's switch and this computer's name on the listener's other devices.
fn connect_settings(ui: &mut egui::Ui, app: &App, inputs: &mut Inputs, actions: &mut Vec<Action>) {
    let palette = app.palette;
    heading(ui, &palette, "Noctorium Connect", Some("Whether this computer finds your other devices and shows up on theirs, and what they call it."));
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
fn diagnostics_settings(ui: &mut egui::Ui, app: &App, actions: &mut Vec<Action>) {
    let palette = app.palette;
    heading(ui, &palette, "Diagnostics", Some("Checks that yt-dlp, mpv, storage and the services can be reached, for when something is not working."));
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
fn widget_settings(ui: &mut egui::Ui, app: &App, actions: &mut Vec<Action>) {
    use crate::widgets::LOOKS;
    let palette = app.palette;
    heading(ui, &palette, "Stream widgets", Some("What is playing, over your stream in OBS or Streamlabs: add the address as a browser source."));
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
        if app.account.is_none() && primary(ui, &palette, "Go to Settings").clicked() {
            actions.push(Action::Open(Page::Settings));
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
