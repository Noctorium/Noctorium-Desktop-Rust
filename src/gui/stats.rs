//! Stats: the Noctorium account's listens from every device, with the streaks and charts Noctorium Stats has.

use super::{Action, App, Page};
use crate::theme;
use crate::account::FullStats;
use eframe::egui::{self, Align, Color32, CornerRadius, Layout, RichText, Sense, Ui, Vec2};

pub const RANGES: [(&str, &str); 5] = [("7d", "7 days"), ("30d", "30 days"), ("90d", "90 days"), ("365d", "Year"), ("all", "All time")];
const WEEKDAYS: [&str; 7] = ["Mon", "Tue", "Wed", "Thu", "Fri", "Sat", "Sun"];

fn plays(n: u64) -> String {
    if n == 1 { "1 play".into() } else { format!("{n} plays") }
}

fn days(n: u64) -> String {
    if n == 1 { "1 day".into() } else { format!("{n} days") }
}

fn listened(ms: u64) -> String {
    let minutes = ms / 60_000;
    if minutes < 60 { format!("{minutes} min") } else { format!("{} h {} min", minutes / 60, minutes % 60) }
}

/// A service's name and colour, from the code the account records it by; the two YouTubes are one.
pub fn service(code: &str) -> (String, Color32) {
    match code {
        "YOUTUBE_MUSIC" | "YOUTUBE_VIDEO" => ("YouTube Music".into(), Color32::from_rgb(0xFF, 0x4E, 0x45)),
        "SOUNDCLOUD" => ("SoundCloud".into(), Color32::from_rgb(0xFF, 0x7A, 0x1A)),
        "SPOTIFY" => ("Spotify".into(), Color32::from_rgb(0x1E, 0xD7, 0x60)),
        "BANDCAMP" => ("Bandcamp".into(), Color32::from_rgb(0x62, 0x9A, 0xA9)),
        "VK" => ("VK".into(), Color32::from_rgb(0x4C, 0x9B, 0xFF)),
        "LOCAL" => ("Your files".into(), Color32::from_rgb(0xB4, 0x7C, 0xFF)),
        other => {
            let lower = other.to_lowercase().replace('_', " ");
            let mut chars = lower.chars();
            let name = chars.next().map(|c| c.to_uppercase().collect::<String>() + chars.as_str()).unwrap_or_default();
            (name, Color32::from_rgb(0x9A, 0x8F, 0xAD))
        }
    }
}

/// The split by service, largest first, with the YouTubes together.
pub fn services(stats: &FullStats) -> Vec<(String, Color32, u64)> {
    let mut rows: Vec<(String, Color32, u64)> = Vec::new();
    for share in &stats.by_provider {
        let (name, colour) = service(&share.provider);
        match rows.iter_mut().find(|row| row.0 == name) {
            Some(row) => row.2 += share.streams,
            None => rows.push((name, colour, share.streams)),
        }
    }
    rows.sort_by_key(|row| std::cmp::Reverse(row.2));
    rows
}

pub fn stats_page(ui: &mut Ui, app: &App, actions: &mut Vec<Action>) {
    let palette = app.palette;
    ui.label(RichText::new("Stats").font(theme::bold(28.0)).color(palette.text));
    ui.add_space(8.0);
    if app.account.is_none() {
        ui.label(
            RichText::new("Your stats come from your Noctorium account: every song played on any device signed in to it, with your streaks and charts.")
                .color(palette.subtext),
        );
        ui.add_space(8.0);
        if ui.button("Sign in").clicked() {
            actions.push(Action::Open(Page::Account));
        }
        return;
    }
    ui.horizontal(|ui| {
        for (code, label) in RANGES {
            if ui.selectable_label(app.stats_range == code, label).clicked() && app.stats_range != code {
                actions.push(Action::StatsRange(code.to_string()));
            }
        }
    });
    ui.add_space(12.0);
    let stats = match &app.full_stats {
        None => {
            ui.add(egui::Spinner::new().color(palette.accent));
            return;
        }
        Some(Err(why)) => {
            ui.label(RichText::new(why).color(palette.subtext));
            return;
        }
        Some(Ok(stats)) => stats,
    };

    let width = ui.available_width().min(1100.0);
    let tiles = [
        ("Listens", stats.streams.to_string()),
        ("Listened", listened(stats.ms_played)),
        ("Songs", stats.unique_tracks.to_string()),
        ("Artists", stats.artists.to_string()),
        ("Current streak", days(stats.streak.current)),
        ("Longest streak", days(stats.streak.longest)),
    ];
    let tile_width = (width - 5.0 * 10.0) / 6.0;
    ui.horizontal(|ui| {
        ui.spacing_mut().item_spacing.x = 10.0;
        for (label, value) in tiles {
            egui::Frame::new().fill(palette.card).corner_radius(CornerRadius::same(14)).inner_margin(14.0).show(ui, |ui| {
                ui.set_width(tile_width - 28.0);
                ui.label(RichText::new(value).font(theme::bold(20.0)).color(palette.text));
                ui.label(RichText::new(label).size(12.0).color(palette.subtext));
            });
        }
    });
    if stats.streams == 0 {
        ui.add_space(12.0);
        ui.label(RichText::new("Nothing played in this span yet.").color(palette.subtext));
        return;
    }

    let heading = |ui: &mut Ui, text: &str| {
        ui.add_space(18.0);
        ui.label(RichText::new(text).font(theme::bold(18.0)).color(palette.text));
        ui.add_space(6.0);
    };
    heading(ui, if stats.timeline.bucket == "month" { "Month by month" } else { "Day by day" });
    bars(ui, app, width, 130.0, &stats.timeline.points.iter().map(|p| p.minutes).collect::<Vec<_>>(), &[]);

    let half = (width - 28.0) / 2.0;
    ui.horizontal_top(|ui| {
        ui.spacing_mut().item_spacing.x = 28.0;
        ui.vertical(|ui| {
            ui.set_width(half);
            heading(ui, "Hour of the day");
            let labels: Vec<String> = stats.hourly.iter().map(|h| if h.hour % 6 == 0 { h.hour.to_string() } else { String::new() }).collect();
            bars(ui, app, half, 90.0, &stats.hourly.iter().map(|h| h.minutes).collect::<Vec<_>>(), &labels);
        });
        ui.vertical(|ui| {
            ui.set_width(half);
            heading(ui, "Day of the week");
            let labels: Vec<String> = stats.weekdays.iter().map(|d| WEEKDAYS.get(d.day.saturating_sub(1) as usize).unwrap_or(&"").to_string()).collect();
            bars(ui, app, half, 90.0, &stats.weekdays.iter().map(|d| d.minutes).collect::<Vec<_>>(), &labels);
        });
    });

    ui.horizontal_top(|ui| {
        ui.spacing_mut().item_spacing.x = 28.0;
        ui.vertical(|ui| {
            ui.set_width(half);
            heading(ui, "Top songs");
            for (i, song) in stats.top_tracks.iter().enumerate() {
                ranked(ui, app, Some(i), &song.title, &song.artist, &plays(song.streams));
            }
        });
        ui.vertical(|ui| {
            ui.set_width(half);
            heading(ui, "Top artists");
            for (i, artist) in stats.top_artists.iter().enumerate() {
                let under = format!("{} {} · {} min", artist.tracks, if artist.tracks == 1 { "song" } else { "songs" }, artist.minutes);
                ranked(ui, app, Some(i), &artist.artist, &under, &plays(artist.streams));
            }
        });
    });

    heading(ui, "By service");
    for (name, colour, count) in services(stats) {
        let share = count as f32 / stats.streams.max(1) as f32;
        ui.allocate_ui_with_layout(Vec2::new(width, 18.0), Layout::left_to_right(Align::Center), |ui| {
            ui.label(RichText::new(&name).color(palette.text));
            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                ui.label(RichText::new(format!("{}%", count * 100 / stats.streams.max(1))).size(12.0).color(palette.subtext));
            });
        });
        let (rect, _) = ui.allocate_exact_size(Vec2::new(width, 6.0), Sense::hover());
        ui.painter().rect_filled(rect, CornerRadius::same(3), palette.card);
        let mut filled = rect;
        filled.set_width((rect.width() * share).max(2.0));
        ui.painter().rect_filled(filled, CornerRadius::same(3), colour);
        ui.add_space(8.0);
    }

    if !stats.recent.is_empty() {
        heading(ui, "Recently played");
    }
    for play in &stats.recent {
        let at = chrono::DateTime::parse_from_rfc3339(&play.played_at)
            .map(|t| t.with_timezone(&chrono::Local).format("%-d %b, %H:%M").to_string())
            .unwrap_or_default();
        let under = format!("{} · {}", play.artist, service(&play.provider).0);
        ui.allocate_ui_with_layout(Vec2::new(width, 34.0), Layout::left_to_right(Align::Center), |ui| {
            ranked(ui, app, None, &play.title, &under, &at);
        });
    }
}

/// A line of a top list: its place, its name over what it is, and its figure at the right.
fn ranked(ui: &mut Ui, app: &App, index: Option<usize>, title: &str, under: &str, end: &str) {
    let palette = app.palette;
    ui.horizontal(|ui| {
        if let Some(i) = index {
            ui.add_sized([24.0, 20.0], egui::Label::new(RichText::new((i + 1).to_string()).size(13.0).color(palette.subtext)));
        }
        ui.vertical(|ui| {
            ui.add(egui::Label::new(RichText::new(title).color(palette.text)).truncate());
            ui.add(egui::Label::new(RichText::new(under).size(12.0).color(palette.subtext)).truncate());
        });
        ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
            ui.label(RichText::new(end).size(12.0).color(palette.subtext));
        });
    });
    ui.add_space(4.0);
}

/// Bars as tall as their share of the tallest, side by side, with a label under each.
fn bars(ui: &mut Ui, app: &App, width: f32, height: f32, values: &[u64], labels: &[String]) {
    let palette = app.palette;
    let (rect, _) = ui.allocate_exact_size(Vec2::new(width, height + if labels.is_empty() { 0.0 } else { 18.0 }), Sense::hover());
    if values.is_empty() {
        return;
    }
    let most = values.iter().copied().max().unwrap_or(0).max(1) as f32;
    let gap = 2.0;
    let each = (width - gap * (values.len() as f32 - 1.0)) / values.len() as f32;
    let painter = ui.painter();
    for (i, value) in values.iter().enumerate() {
        let x = rect.left() + i as f32 * (each + gap);
        let tall = (*value as f32 / most * height).max(2.0);
        let bar = egui::Rect::from_min_max(egui::pos2(x, rect.top() + height - tall), egui::pos2(x + each, rect.top() + height));
        painter.rect_filled(bar, CornerRadius { nw: 3, ne: 3, sw: 0, se: 0 }, palette.accent);
        if let Some(label) = labels.get(i).filter(|l| !l.is_empty()) {
            painter.text(
                egui::pos2(x, rect.top() + height + 3.0),
                egui::Align2::LEFT_TOP,
                label,
                egui::FontId::proportional(11.0),
                palette.subtext,
            );
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_answer_is_read_and_the_youtubes_are_one() {
        let stats: FullStats = serde_json::from_str(
            r#"{"streams":3,"msPlayed":600000,"byProvider":[{"provider":"YOUTUBE_MUSIC","streams":1},{"provider":"YOUTUBE_VIDEO","streams":1},
                {"provider":"SOUNDCLOUD","streams":1}],"timeline":{"bucket":"day","points":[{"start":"2026-11-27","streams":3,"minutes":10}]},
                "streak":{"current":4,"longest":9},"somethingNew":true}"#,
        )
        .unwrap();
        assert_eq!(stats.streak.longest, 9);
        assert_eq!(stats.timeline.points[0].minutes, 10);
        let names: Vec<(String, u64)> = services(&stats).into_iter().map(|(n, _, c)| (n, c)).collect();
        assert_eq!(names, vec![("YouTube Music".to_string(), 2), ("SoundCloud".to_string(), 1)]);
        assert_eq!(listened(3_720_000), "1 h 2 min");
    }
}
