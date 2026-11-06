//! The controls' icons, drawn in the set chosen in Settings › Personalisation: Noctorium's own symbols for
//! Standard, or shapes painted here -- rounded, outlined, sharp, filled, two-tone or line -- the sets the
//! Kotlin desktop offers.

use crate::looks::IconSet;
use crate::theme::Palette;
use eframe::egui::{self, vec2, Align2, Color32, Mesh, Pos2, Rect, Sense, Shape, Stroke, Ui, Vec2};
use std::sync::atomic::{AtomicU8, Ordering};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Icon {
    Play,
    Pause,
    Stop,
    Previous,
    Next,
    Shuffle,
    Repeat,
    RepeatOne,
    Heart,
    HeartFilled,
    Queue,
    Lyrics,
    Volume,
    VolumeLow,
    Mute,
    Moon,
    Plus,
    Connect,
}

impl Icon {
    /// The symbol Standard shows, as it always has.
    pub fn glyph(self) -> &'static str {
        match self {
            Icon::Play => "▶",
            Icon::Pause => "⏸",
            Icon::Stop => "⏹",
            Icon::Previous => "⏮",
            Icon::Next => "⏭",
            Icon::Shuffle => "🔀",
            Icon::Repeat => "🔁",
            Icon::RepeatOne => "🔂",
            Icon::Heart => "♡",
            Icon::HeartFilled => "♥",
            Icon::Queue => "☰",
            Icon::Lyrics => "🎤",
            Icon::Volume => "🔊",
            Icon::VolumeLow => "🔉",
            Icon::Mute => "🔇",
            Icon::Moon => "🌙",
            Icon::Plus => "＋",
            Icon::Connect => "📡",
        }
    }
}

static SET: AtomicU8 = AtomicU8::new(0);

/// The set in force; the app sets it each frame from the settings.
pub fn set_icon_set(set: IconSet) {
    let index = IconSet::ALL_SETS.iter().position(|s| *s == set).unwrap_or(0) as u8;
    SET.store(index, Ordering::Relaxed);
}

pub fn icon_set() -> IconSet {
    IconSet::ALL_SETS.get(SET.load(Ordering::Relaxed) as usize).copied().unwrap_or_default()
}

impl IconSet {
    const ALL_SETS: [IconSet; 7] = [
        IconSet::Standard,
        IconSet::Rounded,
        IconSet::Outlined,
        IconSet::Sharp,
        IconSet::Filled,
        IconSet::TwoTone,
        IconSet::Line,
    ];
}

/// A round button with an icon in it, the way the player's controls are drawn.
pub fn icon_button(ui: &mut Ui, palette: &Palette, icon: Icon, size: f32, active: bool, hint: &str) -> egui::Response {
    let (rect, response) = ui.allocate_exact_size(Vec2::splat(size), Sense::click());
    let colour = if active { palette.accent } else if response.hovered() { palette.text } else { palette.subtext };
    if response.hovered() {
        ui.painter().circle_filled(rect.center(), size / 2.0, palette.raised.gamma_multiply(0.8));
    }
    paint(ui.painter(), Rect::from_center_size(rect.center(), Vec2::splat(size * 0.52)), icon, colour);
    if active {
        ui.painter().circle_filled(rect.center_bottom() + vec2(0.0, -2.0), 2.0, palette.accent);
    }
    response.on_hover_text(hint)
}

/// An icon filling `rect`, in `colour`, in the set in force.
pub fn paint(painter: &egui::Painter, rect: Rect, icon: Icon, colour: Color32) {
    let set = icon_set();
    if set == IconSet::Standard || matches!(icon, Icon::Moon | Icon::Connect) {
        painter.text(rect.center(), Align2::CENTER_CENTER, icon.glyph(), crate::theme::regular(rect.height()), colour);
        return;
    }
    let pen = Pen { painter, rect, colour, set };
    match icon {
        Icon::Play => pen.shape(&[(-0.55, -0.8), (0.8, 0.0), (-0.55, 0.8)], (-0.1, 0.0)),
        Icon::Pause => {
            pen.shape(&[(-0.7, -0.8), (-0.2, -0.8), (-0.2, 0.8), (-0.7, 0.8)], (-0.45, 0.0));
            pen.shape(&[(0.2, -0.8), (0.7, -0.8), (0.7, 0.8), (0.2, 0.8)], (0.45, 0.0));
        }
        Icon::Stop => pen.shape(&[(-0.7, -0.7), (0.7, -0.7), (0.7, 0.7), (-0.7, 0.7)], (0.0, 0.0)),
        Icon::Previous => {
            pen.shape(&[(-0.85, -0.75), (-0.6, -0.75), (-0.6, 0.75), (-0.85, 0.75)], (-0.72, 0.0));
            pen.shape(&[(0.8, -0.75), (-0.45, 0.0), (0.8, 0.75)], (0.3, 0.0));
        }
        Icon::Next => {
            pen.shape(&[(0.6, -0.75), (0.85, -0.75), (0.85, 0.75), (0.6, 0.75)], (0.72, 0.0));
            pen.shape(&[(-0.8, -0.75), (0.45, 0.0), (-0.8, 0.75)], (-0.3, 0.0));
        }
        Icon::Shuffle => {
            pen.path(&[(-0.85, -0.55), (-0.35, -0.55), (0.35, 0.55), (0.7, 0.55)]);
            pen.path(&[(-0.85, 0.55), (-0.35, 0.55), (0.35, -0.55), (0.7, -0.55)]);
            pen.shape(&[(0.55, -0.85), (0.95, -0.55), (0.55, -0.25)], (0.68, -0.55));
            pen.shape(&[(0.55, 0.25), (0.95, 0.55), (0.55, 0.85)], (0.68, 0.55));
        }
        Icon::Repeat | Icon::RepeatOne => {
            pen.path(&[(0.45, -0.55), (-0.75, -0.55), (-0.75, 0.55), (-0.3, 0.55)]);
            pen.path(&[(-0.45, 0.55), (0.75, 0.55), (0.75, -0.55), (0.3, -0.55)]);
            pen.shape(&[(0.35, -0.85), (0.75, -0.55), (0.35, -0.25)], (0.48, -0.55));
            pen.shape(&[(-0.35, 0.25), (-0.75, 0.55), (-0.35, 0.85)], (-0.48, 0.55));
            if icon == Icon::RepeatOne {
                painter.text(rect.center(), Align2::CENTER_CENTER, "1", crate::theme::bold(rect.height() * 0.5), colour);
            }
        }
        Icon::Heart | Icon::HeartFilled => {
            let points: Vec<(f32, f32)> = (0..48)
                .map(|i| {
                    let t = i as f32 / 48.0 * std::f32::consts::TAU;
                    let x = 16.0 * t.sin().powi(3);
                    let y = 13.0 * t.cos() - 5.0 * (2.0 * t).cos() - 2.0 * (3.0 * t).cos() - (4.0 * t).cos();
                    (x / 17.5, -(y + 2.5) / 17.5)
                })
                .collect();
            if icon == Icon::HeartFilled {
                pen.fill(&points, (0.0, 0.1), colour);
            } else {
                // Thinner than the other outlines: a thick line round the heart's point juts out below it.
                let outline: Vec<Pos2> = points.iter().map(|p| pen.at(*p)).collect();
                painter.add(Shape::closed_line(outline, Stroke::new(pen.width() * 0.6, colour)));
            }
        }
        Icon::Queue => {
            for y in [-0.6, 0.0, 0.6] {
                pen.path(&[(-0.85, y), (0.3, y)]);
            }
            pen.shape(&[(0.5, 0.25), (0.95, 0.6), (0.5, 0.95)], (0.65, 0.6));
        }
        Icon::Lyrics => {
            let head: Vec<(f32, f32)> = (0..24)
                .map(|i| {
                    let t = i as f32 / 24.0 * std::f32::consts::TAU;
                    (t.cos() * 0.3, -0.45 + t.sin() * 0.45)
                })
                .collect();
            pen.shape(&head, (0.0, -0.45));
            let cup: Vec<(f32, f32)> = (0..=12)
                .map(|i| {
                    let t = i as f32 / 12.0 * std::f32::consts::PI;
                    (t.cos() * 0.55, 0.05 + t.sin() * 0.45)
                })
                .collect();
            pen.path(&cup);
            pen.path(&[(0.0, 0.5), (0.0, 0.9)]);
            pen.path(&[(-0.35, 0.9), (0.35, 0.9)]);
        }
        Icon::Volume | Icon::VolumeLow | Icon::Mute => {
            pen.shape(&[(-0.9, -0.3), (-0.5, -0.3), (-0.05, -0.75), (-0.05, 0.75), (-0.5, 0.3), (-0.9, 0.3)], (-0.4, 0.0));
            match icon {
                Icon::Mute => {
                    pen.path(&[(0.3, -0.35), (0.9, 0.35)]);
                    pen.path(&[(0.3, 0.35), (0.9, -0.35)]);
                }
                _ => {
                    let waves: &[f32] = if icon == Icon::Volume { &[0.4, 0.75] } else { &[0.4] };
                    for r in waves {
                        let arc: Vec<(f32, f32)> = (0..=10)
                            .map(|i| {
                                let t = -0.9 + i as f32 / 10.0 * 1.8;
                                (0.05 + t.cos() * r, t.sin() * r)
                            })
                            .collect();
                        pen.path(&arc);
                    }
                }
            }
        }
        Icon::Plus => {
            pen.path(&[(-0.75, 0.0), (0.75, 0.0)]);
            pen.path(&[(0.0, -0.75), (0.0, 0.75)]);
        }
        Icon::Moon | Icon::Connect => {}
    }
}

struct Pen<'a> {
    painter: &'a egui::Painter,
    rect: Rect,
    colour: Color32,
    set: IconSet,
}

impl Pen<'_> {
    fn at(&self, (x, y): (f32, f32)) -> Pos2 {
        let scale = if self.set == IconSet::Filled { 0.56 } else { 0.5 };
        self.rect.center() + vec2(x, y) * self.rect.width() * scale
    }

    fn width(&self) -> f32 {
        let size = self.rect.width();
        match self.set {
            IconSet::Line => (size * 0.07).max(1.0),
            IconSet::Sharp => size * 0.11,
            IconSet::Filled => size * 0.14,
            _ => size * 0.1,
        }
    }

    /// A closed shape: filled, outlined or both, as the set draws its solid parts.
    fn shape(&self, points: &[(f32, f32)], centre: (f32, f32)) {
        match self.set {
            IconSet::Outlined | IconSet::Line => self.outline(points),
            IconSet::TwoTone => {
                self.fill(points, centre, self.colour.gamma_multiply(0.3));
                self.outline(points);
            }
            IconSet::Rounded => {
                self.fill(points, centre, self.colour);
                // A stroke of the same colour round the shape softens its corners.
                self.outline(points);
            }
            _ => self.fill(points, centre, self.colour),
        }
    }

    /// Filled as a fan from a point every edge can see, which holds for every shape here, the heart's notch too.
    fn fill(&self, points: &[(f32, f32)], centre: (f32, f32), colour: Color32) {
        let mut mesh = Mesh::default();
        mesh.colored_vertex(self.at(centre), colour);
        for p in points {
            mesh.colored_vertex(self.at(*p), colour);
        }
        let n = points.len() as u32;
        for i in 0..n {
            mesh.add_triangle(0, 1 + i, 1 + (i + 1) % n);
        }
        self.painter.add(Shape::mesh(mesh));
    }

    fn outline(&self, points: &[(f32, f32)]) {
        let points: Vec<Pos2> = points.iter().map(|p| self.at(*p)).collect();
        self.painter.add(Shape::closed_line(points, self.stroke()));
    }

    /// A line, which every set draws as a stroke.
    fn path(&self, points: &[(f32, f32)]) {
        let pts: Vec<Pos2> = points.iter().map(|p| self.at(*p)).collect();
        self.painter.add(Shape::line(pts.clone(), self.stroke()));
        // Round ends, except for Sharp, which keeps them square.
        if matches!(self.set, IconSet::Rounded | IconSet::Filled | IconSet::TwoTone | IconSet::Outlined) {
            for end in [pts[0], pts[pts.len() - 1]] {
                self.painter.circle_filled(end, self.width() / 2.0, self.colour);
            }
        }
    }

    fn stroke(&self) -> Stroke {
        Stroke::new(self.width(), self.colour)
    }
}

/// A flat square button, the kind a toolbar has: lit when pointed at, sunk while on.
pub fn flat_button(ui: &mut Ui, palette: &Palette, icon: Icon, active: bool, hint: &str) -> egui::Response {
    let (rect, response) = ui.allocate_exact_size(vec2(30.0, 28.0), Sense::click());
    let painter = ui.painter();
    if let Some(w) = crate::theme::skin().windows98() {
        painter.rect_filled(rect, 0, crate::theme::rgb(w.face));
        if response.hovered() || active || response.is_pointer_button_down_on() {
            crate::theme::bevel(painter, rect, !(active || response.is_pointer_button_down_on()), w);
        }
    } else if active {
        painter.rect_filled(rect, crate::theme::corner(5), palette.wash(0.25));
    } else if response.hovered() {
        painter.rect_filled(rect, crate::theme::corner(5), palette.raised);
    }
    let colour = if active { palette.accent } else { palette.text };
    paint(painter, Rect::from_center_size(rect.center(), Vec2::splat(15.0)), icon, colour);
    response.on_hover_text(hint)
}
