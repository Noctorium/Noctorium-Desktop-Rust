//! Radio in the search, as the Kotlin desktop's `RadioSearch.kt` has it: the search box's own small menu says
//! Music or Radio -- Music unless the listener picks the other -- and with Radio chosen the same box finds
//! stations from anywhere in the world, from Radio Browser, with a country to narrow them to where the service
//! chips would be. Ctrl+F (⌘F) pressed again in the quick search bar switches between the two, so nobody who
//! lives on the keyboard has to reach for the menu.

use super::widgets::{self, heading, notice};
use super::{Action, App, Inputs};
use crate::domain::Track;
use crate::radio::RadioCountry;
use crate::theme::{self, Palette};
use eframe::egui::{self, pos2, vec2, Align2, Color32, CornerRadius, Painter, Pos2, Rect, RichText, Sense, Shape, Stroke, Ui, Vec2};

/// What a search looks for: songs from the music services, or radio stations from anywhere in the world.
/// Music unless the listener picks Radio, and Music again each time Noctorium starts.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum SearchKind {
    #[default]
    Music,
    Radio,
}

impl SearchKind {
    pub const ALL: [SearchKind; 2] = [SearchKind::Music, SearchKind::Radio];

    pub fn display_name(self) -> &'static str {
        match self {
            SearchKind::Music => "Music",
            SearchKind::Radio => "Radio",
        }
    }

    /// What each kind of search finds, under its name in the menu.
    pub fn about(self) -> &'static str {
        match self {
            SearchKind::Music => "Songs, artists and albums from your services",
            SearchKind::Radio => "Live stations from every country",
        }
    }

    /// Music to Radio and back.
    pub fn other(self) -> SearchKind {
        match self {
            SearchKind::Music => SearchKind::Radio,
            SearchKind::Radio => SearchKind::Music,
        }
    }
}

/// "Germany", or "Everywhere" for no country.
pub fn country_name(code: Option<&str>, countries: &[RadioCountry]) -> String {
    match code {
        Some(c) => countries.iter().find(|country| country.code == c).map(|country| country.name.clone()).unwrap_or_else(|| c.to_string()),
        None => "Everywhere".into(),
    }
}

/// The countries `filter` picks out, by name or by code, as the picker lists them.
pub fn filter_countries<'a>(countries: &'a [RadioCountry], filter: &str) -> Vec<&'a RadioCountry> {
    let wanted = filter.trim().to_lowercase();
    if wanted.is_empty() {
        return countries.iter().collect();
    }
    countries.iter().filter(|c| c.name.to_lowercase().contains(&wanted) || c.code.to_lowercase() == wanted).collect()
}

/// What the station list is headed with: the most listened-to somewhere, or the stations matching the words.
pub fn radio_heading(query: &str, country: Option<&str>, countries: &[RadioCountry]) -> String {
    let where_ = country_name(country, countries);
    let words = query.trim();
    match (words.is_empty(), country.is_none()) {
        (true, true) => "Most listened-to around the world".into(),
        (true, false) => format!("Most listened-to in {where_}"),
        (false, true) => format!("Stations for “{words}”"),
        (false, false) => format!("Stations for “{words}” in {where_}"),
    }
}

// ---- the icons, drawn: Windows has no flag emoji and egui's fonts no radio --------------------------------

/// A note for songs.
fn paint_note(painter: &Painter, rect: Rect, colour: Color32) {
    let (w, h) = (rect.width(), rect.height());
    let at = |x: f32, y: f32| pos2(rect.left() + x * w, rect.top() + y * h);
    let stroke = Stroke::new((w * 0.12).max(1.4), colour);
    painter.circle_filled(at(0.36, 0.76), w * 0.2, colour);
    painter.line_segment([at(0.53, 0.76), at(0.53, 0.1)], stroke);
    painter.add(Shape::line(vec![at(0.53, 0.1), at(0.82, 0.24), at(0.82, 0.42), at(0.53, 0.3)], stroke));
}

/// A set with its dial and aerial, for stations.
fn paint_radio(painter: &Painter, rect: Rect, colour: Color32) {
    let (w, h) = (rect.width(), rect.height());
    let at = |x: f32, y: f32| pos2(rect.left() + x * w, rect.top() + y * h);
    let stroke = Stroke::new((w * 0.1).max(1.3), colour);
    painter.line_segment([at(0.2, 0.36), at(0.74, 0.08)], stroke);
    let body = Rect::from_min_max(at(0.06, 0.34), at(0.94, 0.92));
    painter.rect_stroke(body, CornerRadius::same((w * 0.12) as u8), stroke, egui::StrokeKind::Inside);
    painter.circle_filled(at(0.34, 0.63), w * 0.13, colour);
    painter.line_segment([at(0.6, 0.55), at(0.8, 0.55)], stroke);
    painter.line_segment([at(0.6, 0.72), at(0.8, 0.72)], stroke);
}

/// A globe, not a flag: Windows draws flag emoji as two letters, which says less than the name beside it.
fn paint_globe(painter: &Painter, rect: Rect, colour: Color32) {
    let c = rect.center();
    let r = rect.width().min(rect.height()) * 0.44;
    let stroke = Stroke::new((r * 0.16).max(1.2), colour);
    painter.circle_stroke(c, r, stroke);
    let meridian: Vec<Pos2> = (0..=32)
        .map(|i| {
            let t = i as f32 / 32.0 * std::f32::consts::TAU;
            pos2(c.x + r * 0.45 * t.cos(), c.y + r * t.sin())
        })
        .collect();
    painter.add(Shape::closed_line(meridian, stroke));
    painter.line_segment([pos2(c.x - r, c.y), pos2(c.x + r, c.y)], stroke);
    let off = r * 0.5;
    let half = (r * r - off * off).sqrt();
    painter.line_segment([pos2(c.x - half, c.y - off), pos2(c.x + half, c.y - off)], stroke);
    painter.line_segment([pos2(c.x - half, c.y + off), pos2(c.x + half, c.y + off)], stroke);
}

fn paint_caret(painter: &Painter, centre: Pos2, size: f32, colour: Color32) {
    let s = size * 0.28;
    painter.add(Shape::convex_polygon(vec![pos2(centre.x - s, centre.y - s * 0.5), pos2(centre.x + s, centre.y - s * 0.5), pos2(centre.x, centre.y + s * 0.6)], colour, Stroke::NONE));
}

fn paint_check(painter: &Painter, centre: Pos2, colour: Color32) {
    let stroke = Stroke::new(2.0_f32, colour);
    painter.add(Shape::line(vec![centre + vec2(-6.0, 0.0), centre + vec2(-2.0, 4.0), centre + vec2(6.0, -4.5)], stroke));
}

fn paint_kind(painter: &Painter, rect: Rect, kind: SearchKind, colour: Color32) {
    match kind {
        SearchKind::Music => paint_note(painter, rect, colour),
        SearchKind::Radio => paint_radio(painter, rect, colour),
    }
}

/// A small rounded pill: an icon, a word and a caret.
fn pill(ui: &mut Ui, text: &str, height: f32, font: egui::FontId, fill: Color32, ink: Color32, icon: impl FnOnce(&Painter, Rect, Color32)) -> egui::Response {
    let galley = ui.painter().layout_no_wrap(text.to_string(), font, ink);
    let icon_side = (height * 0.56).round();
    let start = (height * 0.32).round();
    let width = start + icon_side + 6.0 + galley.size().x + 2.0 + height * 0.6;
    let (rect, response) = ui.allocate_exact_size(vec2(width, height), Sense::click());
    let painter = ui.painter();
    let fill = if response.hovered() { fill.gamma_multiply(1.6) } else { fill };
    painter.rect_filled(rect, CornerRadius::same(theme::corner((height / 2.0) as u8)), fill);
    let icon_rect = Rect::from_center_size(pos2(rect.left() + start + icon_side / 2.0, rect.center().y), Vec2::splat(icon_side));
    icon(painter, icon_rect, ink);
    let text_at = pos2(icon_rect.right() + 6.0, rect.center().y - galley.size().y / 2.0);
    painter.galley(text_at, galley.clone(), ink);
    paint_caret(painter, pos2(text_at.x + galley.size().x + 2.0 + height * 0.3, rect.center().y), height * 0.6, ink);
    response.on_hover_cursor(egui::CursorIcon::PointingHand)
}

/// The search's Music / Radio switch: the kind chosen, as an icon, a word and a caret, sitting above the box
/// rather than in it, so the box is only a search box. No fill and no border -- small text in the accent, washed
/// only while the pointer is on it -- opening a menu of the two.
pub fn kind_picker(ui: &mut Ui, app: &App, quick: bool, actions: &mut Vec<Action>) {
    let palette = app.palette;
    let kind = app.search_kind;
    let ink = palette.accent;
    let galley = ui.painter().layout_no_wrap(kind.display_name().to_string(), theme::bold(12.5), ink);
    let (height, icon_side) = (20.0, 12.0);
    let width = 4.0 + icon_side + 5.0 + galley.size().x + 4.0 + 10.0 + 4.0;
    let (rect, response) = ui.allocate_exact_size(vec2(width, height), Sense::click());
    let painter = ui.painter();
    if response.hovered() {
        painter.rect_filled(rect, CornerRadius::same(theme::corner(5)), palette.wash(0.08));
    }
    let icon_rect = Rect::from_center_size(pos2(rect.left() + 4.0 + icon_side / 2.0, rect.center().y), Vec2::splat(icon_side));
    paint_kind(painter, icon_rect, kind, ink);
    let text_at = pos2(icon_rect.right() + 5.0, rect.center().y - galley.size().y / 2.0);
    painter.galley(text_at, galley.clone(), ink);
    paint_caret(painter, pos2(text_at.x + galley.size().x + 4.0 + 5.0, rect.center().y), 10.0, ink);
    let response = response.on_hover_cursor(egui::CursorIcon::PointingHand).on_hover_text("Music or radio");
    let mut popup = egui::Popup::menu(&response).width(300.0);
    if app.capture_menu.as_deref() == Some(if quick { "quickkind" } else { "kind" }) {
        popup = popup.open(true);
    }
    popup.show(|ui| {
        for option in SearchKind::ALL {
            let (rect, row) = ui.allocate_exact_size(vec2(ui.available_width().max(290.0), 46.0), Sense::click());
            if row.hovered() {
                ui.painter().rect_filled(rect, CornerRadius::same(theme::corner(6)), palette.wash(0.08));
            }
            let icon = Rect::from_center_size(pos2(rect.left() + 20.0, rect.center().y), Vec2::splat(17.0));
            paint_kind(ui.painter(), icon, option, palette.subtext);
            ui.painter().text(pos2(rect.left() + 40.0, rect.center().y - 1.0), Align2::LEFT_BOTTOM, option.display_name(), theme::bold(14.0), palette.text);
            ui.painter().text(pos2(rect.left() + 40.0, rect.center().y + 1.0), Align2::LEFT_TOP, option.about(), theme::regular(11.5), palette.subtext);
            if option == kind {
                paint_check(ui.painter(), pos2(rect.right() - 16.0, rect.center().y), palette.accent);
            }
            if row.on_hover_cursor(egui::CursorIcon::PointingHand).clicked() {
                actions.push(Action::SetSearchKind(option));
                ui.close();
            }
        }
    });
}

/// Where the radio search looks: everywhere, or one country, picked from all of them -- a couple of hundred,
/// so the menu has a box at its top to type a name into.
pub fn country_picker(ui: &mut Ui, app: &App, inputs: &mut Inputs, actions: &mut Vec<Action>) {
    let palette = app.palette;
    let name = country_name(app.radio_country.as_deref(), &app.radio_countries);
    let accent = palette.accent;
    let response = pill(ui, &name, 30.0, theme::bold(13.0), palette.card, palette.text, |p, r, _| paint_globe(p, r, accent)).on_hover_text("Country");
    if response.clicked() {
        inputs.country_filter.clear();
    }
    let mut popup = egui::Popup::menu(&response).width(300.0).close_behavior(egui::PopupCloseBehavior::CloseOnClickOutside);
    if app.capture_menu.as_deref() == Some("country") {
        popup = popup.open(true);
    }
    popup.show(|ui| {
        ui.set_width(300.0);
        let field = ui.add(egui::TextEdit::singleline(&mut inputs.country_filter).hint_text("Find a country").desired_width(f32::INFINITY));
        if ui.memory(|m| m.focused().is_none()) {
            field.request_focus();
        }
        ui.add_space(4.0);
        let filter = inputs.country_filter.clone();
        let shown = filter_countries(&app.radio_countries, &filter);
        let everywhere = filter.trim().is_empty();
        if app.radio_countries.is_empty() {
            ui.label(RichText::new("Fetching the countries…").font(theme::regular(12.0)).color(palette.subtext));
        } else if shown.is_empty() {
            ui.label(RichText::new(format!("No country called “{}”.", filter.trim())).font(theme::regular(12.0)).color(palette.subtext));
        }
        let rows = shown.len() + usize::from(everywhere);
        let row_height = 30.0;
        egui::ScrollArea::vertical().max_height(400.0).auto_shrink([false, true]).show_rows(ui, row_height, rows, |ui, range| {
            for index in range {
                let (rect, row) = ui.allocate_exact_size(vec2(ui.available_width(), row_height), Sense::click());
                if row.hovered() {
                    ui.painter().rect_filled(rect, CornerRadius::same(theme::corner(6)), palette.wash(0.08));
                }
                let (code, chosen) = if everywhere && index == 0 {
                    let globe = Rect::from_center_size(pos2(rect.left() + 18.0, rect.center().y), Vec2::splat(16.0));
                    paint_globe(ui.painter(), globe, palette.subtext);
                    ui.painter().text(pos2(rect.left() + 40.0, rect.center().y), Align2::LEFT_CENTER, "Everywhere", theme::regular(13.5), palette.text);
                    (None, app.radio_country.is_none())
                } else {
                    let country = shown[index - usize::from(everywhere)];
                    ui.painter().text(pos2(rect.left() + 8.0, rect.center().y), Align2::LEFT_CENTER, &country.code, theme::bold(11.0), palette.subtext);
                    let count = ui.painter().text(pos2(rect.right() - 34.0, rect.center().y), Align2::RIGHT_CENTER, country.stations.to_string(), theme::regular(11.0), palette.faint);
                    let name_rect = Rect::from_min_max(pos2(rect.left() + 40.0, rect.top()), pos2(count.left() - 8.0, rect.bottom()));
                    let galley = ui.painter().layout(country.name.clone(), theme::regular(13.5), palette.text, f32::INFINITY);
                    ui.painter().with_clip_rect(name_rect).galley(pos2(name_rect.left(), rect.center().y - galley.size().y / 2.0), galley, palette.text);
                    (Some(country.code.clone()), app.radio_country.as_deref() == Some(country.code.as_str()))
                };
                if chosen {
                    paint_check(ui.painter(), pos2(rect.right() - 14.0, rect.center().y), palette.accent);
                }
                if row.on_hover_cursor(egui::CursorIcon::PointingHand).clicked() {
                    actions.push(Action::SetRadioCountry(code));
                    ui.close();
                }
            }
        });
    });
}

/// The search page with Radio chosen: the stations found, or why there are none.
pub fn results(ui: &mut Ui, app: &App, actions: &mut Vec<Action>) {
    let palette = app.palette;
    let radio = &app.radio;
    if radio.loading && radio.stations.is_empty() {
        widgets::loading(ui, &palette, "Tuning in to the world's stations…");
        return;
    }
    if let Some(why) = &radio.error {
        notice(ui, &palette, why, true);
        return;
    }
    if radio.stations.is_empty() {
        ui.label(RichText::new("No stations found. Try another name, a kind of music such as “jazz”, or Everywhere.").color(palette.subtext));
        return;
    }
    heading(
        ui,
        &palette,
        &radio_heading(&radio.query, app.radio_country.as_deref(), &app.radio_countries),
        Some("From Radio Browser, an open list of the world's stations. Click one to tune in."),
    );
    if radio.loading {
        widgets::loading(ui, &palette, "Tuning in…");
    }
    let on_air = app.loaded.as_ref().or(app.resolving.as_ref()).map(Track::queue_key);
    for station in &radio.stations {
        let key = station.queue_key();
        station_row(ui, &palette, station, on_air.as_deref() == Some(key.as_str()), actions);
        ui.add_space(6.0);
    }
}

/// One station: its logo, its name, where it is from and what it plays, and its badge.
pub fn station_row(ui: &mut Ui, palette: &Palette, station: &Track, on_air: bool, actions: &mut Vec<Action>) {
    let (rect, response) = ui.allocate_exact_size(vec2(ui.available_width(), 64.0), Sense::click());
    let fill = if on_air {
        palette.accent.gamma_multiply(0.18)
    } else if response.hovered() {
        palette.card
    } else {
        palette.card.gamma_multiply(0.6)
    };
    ui.painter().rect_filled(rect, CornerRadius::same(theme::corner(12)), fill);
    if on_air {
        ui.painter().rect_stroke(rect, CornerRadius::same(theme::corner(12)), Stroke::new(1.0_f32, palette.accent.gamma_multiply(0.6)), egui::StrokeKind::Inside);
    }
    let art = Rect::from_min_size(rect.left_top() + vec2(8.0, 8.0), Vec2::splat(48.0));
    logo(ui, palette, station.artwork_url.as_deref(), art);
    let badge_width = 24.0;
    let mut right = rect.right() - 12.0 - badge_width;
    if on_air {
        let label = ui.painter().text(pos2(right - 8.0, rect.center().y), Align2::RIGHT_CENTER, "ON AIR", theme::bold(11.0), palette.accent);
        right = label.left() - 8.0;
    }
    let text = Rect::from_min_max(pos2(art.right() + 12.0, rect.top() + 8.0), pos2(right, rect.bottom() - 8.0));
    let mut text_ui = ui.new_child(egui::UiBuilder::new().max_rect(text).layout(egui::Layout::top_down(egui::Align::Min)));
    text_ui.add_space(4.0);
    text_ui.add(egui::Label::new(RichText::new(&station.title).font(theme::bold(14.5)).color(palette.text)).truncate().selectable(false));
    text_ui.add(egui::Label::new(RichText::new(station.artist_line()).font(theme::regular(12.0)).color(palette.subtext)).truncate().selectable(false));
    let badge = Rect::from_min_size(pos2(rect.right() - 12.0 - badge_width, rect.center().y - 9.0), vec2(badge_width, 18.0));
    let mut badge_ui = ui.new_child(egui::UiBuilder::new().max_rect(badge).layout(egui::Layout::left_to_right(egui::Align::Center)));
    widgets::badge(&mut badge_ui, station.provider);
    if response.on_hover_cursor(egui::CursorIcon::PointingHand).clicked() {
        actions.push(Action::PlayStation(station.clone()));
    }
}

/// A station's logo, or a radio drawn in its place: logos are favicons, of any size and some in formats
/// nothing here reads.
pub fn logo(ui: &Ui, palette: &Palette, url: Option<&str>, rect: Rect) {
    let corner = CornerRadius::same(theme::corner(9));
    ui.painter().rect_filled(rect, corner, palette.raised);
    if let Some(url) = url {
        let image = egui::Image::new(url.to_string()).corner_radius(corner).show_loading_spinner(false).fit_to_exact_size(rect.size());
        if let Ok(egui::load::TexturePoll::Ready { .. }) = image.load_for_size(ui.ctx(), rect.size()) {
            image.paint_at(ui, rect);
            return;
        }
    }
    paint_radio(ui.painter(), Rect::from_center_size(rect.center(), rect.size() * 0.5), palette.faint);
}

/// The line under the quick search bar: what the keys do, and the way to the other kind.
pub fn quick_footer(kind: SearchKind, blank: bool) -> String {
    let command = if cfg!(target_os = "macos") { "⌘" } else { "Ctrl" };
    match (kind, blank) {
        (SearchKind::Radio, _) => format!("↑↓ choose  ·  Enter tune in  ·  {command}+F again for Music  ·  Esc close"),
        (SearchKind::Music, true) => format!("{command}+F again for Radio  ·  start or end with !ytm, !sc or !yt for one service"),
        (SearchKind::Music, false) => format!("↑↓ choose  ·  Enter play  ·  {command}+F again for Radio  ·  Esc close"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn countries() -> Vec<RadioCountry> {
        vec![
            RadioCountry { code: "AT".into(), name: "Austria".into(), stations: 3 },
            RadioCountry { code: "DE".into(), name: "Germany".into(), stations: 9 },
        ]
    }

    #[test]
    fn music_by_default_and_the_switch_goes_both_ways() {
        assert_eq!(SearchKind::default(), SearchKind::Music);
        assert_eq!(SearchKind::Music.other(), SearchKind::Radio);
        assert_eq!(SearchKind::Radio.other(), SearchKind::Music);
    }

    #[test]
    fn names_the_country_or_everywhere() {
        assert_eq!(country_name(None, &countries()), "Everywhere");
        assert_eq!(country_name(Some("DE"), &countries()), "Germany");
        assert_eq!(country_name(Some("JP"), &countries()), "JP");
    }

    #[test]
    fn finds_countries_by_name_or_code() {
        let all = countries();
        assert_eq!(filter_countries(&all, "").len(), 2);
        assert_eq!(filter_countries(&all, "germ")[0].code, "DE");
        assert_eq!(filter_countries(&all, "at")[0].code, "AT");
        assert!(filter_countries(&all, "xyz").is_empty());
    }

    #[test]
    fn heads_the_list_with_what_was_asked() {
        let all = countries();
        assert_eq!(radio_heading("", None, &all), "Most listened-to around the world");
        assert_eq!(radio_heading(" ", Some("DE"), &all), "Most listened-to in Germany");
        assert_eq!(radio_heading("jazz", None, &all), "Stations for “jazz”");
        assert_eq!(radio_heading("jazz ", Some("AT"), &all), "Stations for “jazz” in Austria");
    }
}
