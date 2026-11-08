//! The Shadow theme, reached only by the code "sh@d0w" in Settings › Experimental: the Kotlin players' secret theme.
//!
//! Beyond its colours: the listener's own picture, if they gave one, dimmed behind everything, and over it a few
//! crimson streaks swept in from the right and a gold ring -- drawn here, Noctorium's own, with nobody's artwork in
//! it. And the takeover the code starts with: "sh@d0w" written all over the window for five seconds, then closing.

use eframe::egui::{self, pos2, vec2, Color32, Mesh, Rect, Shape, Stroke, TextureHandle};
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

/// The theme's key, as the Kotlin players save it.
pub const KEY: &str = "SHADOW";
/// How long the takeover lasts before Noctorium closes.
pub const TAKEOVER: Duration = Duration::from_secs(5);
/// The largest picture kept: it is decoded whole and drawn behind every page.
pub const LARGEST: usize = 25 * 1024 * 1024;

const CRIMSON: Color32 = Color32::from_rgb(0xE3, 0x26, 0x2F);
const GOLD: Color32 = Color32::from_rgb(0xF2, 0xC1, 0x4E);

/// What `bytes` really are by their first bytes -- "png", "jpg", "webp" or "gif" -- whatever their name says.
pub fn kind(bytes: &[u8]) -> Option<&'static str> {
    if bytes.starts_with(&[0x89, b'P', b'N', b'G', 0x0D, 0x0A, 0x1A, 0x0A]) {
        Some("png")
    } else if bytes.starts_with(&[0xFF, 0xD8, 0xFF]) {
        Some("jpg")
    } else if bytes.starts_with(b"RIFF") && bytes.len() >= 12 && &bytes[8..12] == b"WEBP" {
        Some("webp")
    } else if bytes.starts_with(b"GIF8") {
        Some("gif")
    } else {
        None
    }
}

/// Keeps the picture at `source` in `folder`, in place of any kept before, and answers where; an error saying why
/// not for no file, no picture, or one too big.
pub fn keep(source: &Path, folder: &Path) -> Result<PathBuf, String> {
    let bytes = std::fs::read(source).map_err(|_| "Noctorium could not read that file.".to_string())?;
    if bytes.len() > LARGEST {
        return Err("That picture is bigger than 25 MB.".into());
    }
    let kind = kind(&bytes).ok_or("That is not a PNG, JPEG, WebP or GIF picture.")?;
    std::fs::create_dir_all(folder).map_err(|e| e.to_string())?;
    forget(folder);
    let kept = folder.join(format!("shadow-picture.{kind}"));
    std::fs::write(&kept, &bytes).map_err(|e| e.to_string())?;
    Ok(kept)
}

/// Deletes whatever picture was kept in `folder`.
pub fn forget(folder: &Path) {
    for kind in ["png", "jpg", "webp", "gif"] {
        let _ = std::fs::remove_file(folder.join(format!("shadow-picture.{kind}")));
    }
}

/// The picture at `path` as a texture, or None for no picture or one unreadable.
pub fn load(ctx: &egui::Context, path: &str) -> Option<TextureHandle> {
    if path.is_empty() {
        return None;
    }
    let decoded = image::open(path).ok()?;
    // Small enough to draw every frame without strain, however big the photograph.
    let decoded = if decoded.width().max(decoded.height()) > 2400 { decoded.thumbnail(2400, 2400) } else { decoded };
    let rgba = decoded.to_rgba8();
    let image = egui::ColorImage::from_rgba_unmultiplied([rgba.width() as usize, rgba.height() as usize], rgba.as_raw());
    Some(ctx.load_texture("shadow-picture", image, egui::TextureOptions::LINEAR))
}

/// The theme's page over the glow: the picture, dimmed, then the streaks and the ring.
pub fn paint_backdrop(ctx: &egui::Context, picture: Option<&TextureHandle>) {
    let painter = ctx.layer_painter(egui::LayerId::background());
    let screen = ctx.screen_rect();
    if let Some(picture) = picture {
        // Filling the window, cropped rather than squashed.
        let [w, h] = picture.size();
        let scale = (screen.width() / w as f32).max(screen.height() / h as f32);
        let shown = Rect::from_center_size(screen.center(), vec2(w as f32 * scale, h as f32 * scale));
        painter.image(picture.id(), shown, Rect::from_min_max(pos2(0.0, 0.0), pos2(1.0, 1.0)), Color32::from_white_alpha(77));
        let mut shade = Mesh::default();
        shade.colored_vertex(screen.left_top(), Color32::from_black_alpha(90));
        shade.colored_vertex(screen.right_top(), Color32::from_black_alpha(90));
        shade.colored_vertex(screen.right_bottom(), Color32::from_black_alpha(178));
        shade.colored_vertex(screen.left_bottom(), Color32::from_black_alpha(178));
        shade.add_triangle(0, 1, 2);
        shade.add_triangle(0, 2, 3);
        painter.add(Shape::mesh(shade));
    }
    let unit = screen.height();
    for (x, y, opacity) in [(0.94, 0.78, 0.5), (0.99, 0.62, 0.38), (1.03, 0.9, 0.3), (0.9, 0.97, 0.22)] {
        let root = pos2(screen.left() + screen.width() * x, screen.top() + screen.height() * y);
        let tip = root + vec2(-unit * 1.05, -unit * 0.55);
        let width = unit * 0.028;
        // A blade: wide at the root, a point at the tip, fading as it goes.
        let mut blade = Mesh::default();
        let alpha = (0.26 * opacity / 0.5 * 255.0) as u8;
        blade.colored_vertex(root + vec2(0.0, -width), CRIMSON.gamma_multiply(alpha as f32 / 255.0));
        blade.colored_vertex(root + vec2(0.0, width), CRIMSON.gamma_multiply(alpha as f32 / 255.0));
        blade.colored_vertex(tip, Color32::TRANSPARENT);
        blade.add_triangle(0, 1, 2);
        painter.add(Shape::mesh(blade));
    }
    let ring = pos2(screen.right() - unit * 0.16, screen.top() + unit * 0.16);
    let radius = unit * 0.055;
    painter.circle_filled(ring, radius * 1.6, GOLD.gamma_multiply(0.12));
    painter.circle_stroke(ring, radius, Stroke::new(unit * 0.012, GOLD.gamma_multiply(0.55)));
}

/// The takeover, `started` ago: the window black, and "Sh@dow" in small white letters appearing in straight rows
/// and columns, one place after another in no order, until they fill it a little before the end.
pub fn paint_takeover(ctx: &egui::Context, started: Instant) {
    const CELL: egui::Vec2 = vec2(92.0, 30.0);
    let painter = ctx.layer_painter(egui::LayerId::new(egui::Order::Debug, egui::Id::new("shadow-takeover")));
    let screen = ctx.screen_rect();
    painter.rect_filled(screen, 0.0, Color32::BLACK);
    let columns = ((screen.width() / CELL.x) as usize).max(1);
    let rows = ((screen.height() / CELL.y) as usize).max(1);
    let places = columns * rows;
    let through = (started.elapsed().as_secs_f32() / (TAKEOVER.as_secs_f32() * 0.9)).min(1.0);
    let shown = (places as f32 * through) as usize;
    // Centred, so the margins either side are the same.
    let origin = screen.center() - vec2(CELL.x * columns as f32, CELL.y * rows as f32) / 2.0;
    // The same places in the same order from one frame to the next: each place's turn from a cheap hash of it.
    let mut order: Vec<usize> = (0..places).collect();
    order.sort_by_key(|&n| {
        let mut x = (n as u32).wrapping_mul(0x9E37_79B9) ^ 0x85EB_CA6B;
        x ^= x >> 15;
        x = x.wrapping_mul(0x2C1B_3C6D);
        x ^ (x >> 12)
    });
    for &place in &order[..shown] {
        let at = origin + vec2(CELL.x * (place % columns) as f32, CELL.y * (place / columns) as f32);
        painter.text(at, egui::Align2::LEFT_TOP, "Sh@dow", egui::FontId::proportional(14.0), Color32::WHITE);
    }
    ctx.request_repaint();
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_picture_is_known_by_its_first_bytes_and_kept_once() {
        assert_eq!(kind(&[0x89, b'P', b'N', b'G', 0x0D, 0x0A, 0x1A, 0x0A, 1]), Some("png"));
        assert_eq!(kind(&[0xFF, 0xD8, 0xFF, 0xE0]), Some("jpg"));
        assert_eq!(kind(b"RIFF\0\0\0\0WEBPVP8 "), Some("webp"));
        assert_eq!(kind(b"not a picture.png"), None);

        let folder = std::env::temp_dir().join(format!("noctorium-shadow-{}", std::process::id()));
        let source = std::env::temp_dir().join(format!("noctorium-shadow-source-{}", std::process::id()));
        std::fs::write(&source, [0xFF, 0xD8, 0xFF, 0xE0, 9]).unwrap();
        let kept = keep(&source, &folder).unwrap();
        assert_eq!(kept.file_name().unwrap(), "shadow-picture.jpg");
        std::fs::write(&source, b"<html>").unwrap();
        assert!(keep(&source, &folder).is_err());
        assert!(kept.exists(), "a refused file leaves the kept picture be");
        forget(&folder);
        assert!(!kept.exists());
        let _ = std::fs::remove_file(&source);
        let _ = std::fs::remove_dir_all(&folder);
    }
}
