use iced::Color;

/// Dynamic UI accent colors derived from the active album palette (or cool crystal fallback).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct AccentTheme {
    pub primary: Color,
    pub light: Color,
    pub pressed: Color,
    pub glow: Color,
    pub soft_bg: Color,
    pub border: Color,
    pub on_primary: Color,
}

impl AccentTheme {
    /// Default ultra-clear neutral crystal glass accent (pure frosted white/silver, zero blue tint).
    pub fn default_cool() -> Self {
        Self {
            primary: Color::from_rgba(0.97, 0.97, 0.98, 0.92),
            light: Color::from_rgba(1.0, 1.0, 1.0, 0.98),
            pressed: Color::from_rgba(0.84, 0.85, 0.87, 0.88),
            glow: Color::from_rgba(1.0, 1.0, 1.0, 0.22),
            soft_bg: Color::from_rgba(1.0, 1.0, 1.0, 0.11),
            border: Color::from_rgba(1.0, 1.0, 1.0, 0.34),
            on_primary: Color::from_rgb8(14, 15, 18),
        }
    }
}

/// Extracted 4-color palette from album art paired with a contrast-safe dark scrim alpha.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct AlbumPalette {
    pub colors: [[f32; 4]; 4],
    pub scrim_alpha: f32,
}

impl AlbumPalette {
    /// Neutral Ultra-Clear Crystal Glass palette used when idle or no track artwork is active.
    pub fn default_dark() -> Self {
        Self {
            colors: [
                [0.94, 0.94, 0.96, 1.0], // pure crystal highlight
                [0.72, 0.73, 0.76, 1.0], // frosted mist silver
                [0.08, 0.08, 0.09, 1.0], // neutral obsidian depth
                [0.84, 0.85, 0.88, 1.0], // soft glass specular
            ],
            scrim_alpha: 0.38,
        }
    }

    pub fn lerp(&self, target: &Self, t: f32) -> Self {
        let k = t.clamp(0.0, 1.0);
        let mut colors = [[0.0; 4]; 4];
        for i in 0..4 {
            for c in 0..4 {
                colors[i][c] = self.colors[i][c] + (target.colors[i][c] - self.colors[i][c]) * k;
            }
        }
        let scrim_alpha =
            (self.scrim_alpha + (target.scrim_alpha - self.scrim_alpha) * k).clamp(0.38, 0.55);
        Self {
            colors,
            scrim_alpha,
        }
    }

    /// Derives a crystal-clear [`AccentTheme`] with at most a subtle <=15% whispered tint
    /// from the dominant album color so the UI always remains unmistakably clear glass.
    pub fn accent_theme(&self) -> AccentTheme {
        if *self == Self::default_dark() {
            return AccentTheme::default_cool();
        }

        let mut best_idx = 0;
        let mut best_sat = -1.0_f32;
        for (i, c) in self.colors.iter().enumerate() {
            let max_c = c[0].max(c[1]).max(c[2]);
            let min_c = c[0].min(c[1]).min(c[2]);
            let sat = if max_c > 0.001 {
                (max_c - min_c) / max_c
            } else {
                0.0
            };
            if sat > best_sat {
                best_sat = sat;
                best_idx = i;
            }
        }

        if best_sat < 0.14 {
            return AccentTheme::default_cool();
        }

        let raw = self.colors[best_idx];
        let mut ar = raw[0].clamp(0.0, 1.0);
        let mut ag = raw[1].clamp(0.0, 1.0);
        let mut ab = raw[2].clamp(0.0, 1.0);

        // Normalize album hue luminance before blending softly into neutral crystal white.
        let luma = (0.2126 * ar + 0.7152 * ag + 0.0722 * ab).max(0.001);
        let target_luma = luma.clamp(0.50, 0.78);
        let scale = target_luma / luma;
        ar = (ar * scale).clamp(0.0, 1.0);
        ag = (ag * scale).clamp(0.0, 1.0);
        ab = (ab * scale).clamp(0.0, 1.0);

        // Whispered <=15% album art tint blended into pure crystal white.
        const TINT_WEIGHT: f32 = 0.15;
        let r = (0.96 * (1.0 - TINT_WEIGHT) + ar * TINT_WEIGHT).clamp(0.0, 1.0);
        let g = (0.96 * (1.0 - TINT_WEIGHT) + ag * TINT_WEIGHT).clamp(0.0, 1.0);
        let b = (0.97 * (1.0 - TINT_WEIGHT) + ab * TINT_WEIGHT).clamp(0.0, 1.0);

        let light_r = (0.99 * (1.0 - TINT_WEIGHT * 0.8) + ar * (TINT_WEIGHT * 0.8)).clamp(0.0, 1.0);
        let light_g = (0.99 * (1.0 - TINT_WEIGHT * 0.8) + ag * (TINT_WEIGHT * 0.8)).clamp(0.0, 1.0);
        let light_b = (1.00 * (1.0 - TINT_WEIGHT * 0.8) + ab * (TINT_WEIGHT * 0.8)).clamp(0.0, 1.0);

        AccentTheme {
            primary: Color::from_rgba(r, g, b, 0.92),
            light: Color::from_rgba(light_r, light_g, light_b, 0.98),
            pressed: Color::from_rgba(
                (r * 0.84).clamp(0.0, 1.0),
                (g * 0.84).clamp(0.0, 1.0),
                (b * 0.84).clamp(0.0, 1.0),
                0.88,
            ),
            glow: Color::from_rgba(
                (0.85 + ar * 0.15).clamp(0.0, 1.0),
                (0.85 + ag * 0.15).clamp(0.0, 1.0),
                (0.85 + ab * 0.15).clamp(0.0, 1.0),
                0.24,
            ),
            soft_bg: Color::from_rgba(
                (0.85 + ar * 0.15).clamp(0.0, 1.0),
                (0.85 + ag * 0.15).clamp(0.0, 1.0),
                (0.85 + ab * 0.15).clamp(0.0, 1.0),
                0.12,
            ),
            border: Color::from_rgba(light_r, light_g, light_b, 0.36),
            on_primary: Color::from_rgb8(14, 15, 18),
        }
    }
}

/// Decodes raw image bytes into a 64×64 RGBA buffer (16 KB) for GPU blurred backdrop rendering.
pub fn decode_art_blur_rgba(image_bytes: &[u8]) -> Option<Vec<u8>> {
    if image_bytes.is_empty() {
        return None;
    }
    let img = image::load_from_memory(image_bytes).ok()?;
    let small = img.resize_exact(64, 64, image::imageops::FilterType::Triangle);
    Some(small.to_rgba8().into_raw())
}

pub fn extract_palette(image_bytes: &[u8]) -> AlbumPalette {
    if image_bytes.is_empty() {
        return AlbumPalette::default_dark();
    }
    let Ok(img) = image::load_from_memory(image_bytes) else {
        return AlbumPalette::default_dark();
    };

    let thumb = img
        .resize_exact(32, 32, image::imageops::FilterType::Triangle)
        .to_rgba8();

    let mut total_luma = 0.0_f32;
    let mut pixel_count = 0.0_f32;
    let mut quad_accum = [[0.0_f32; 4]; 4]; // [r, g, b, weight]

    for (x, y, px) in thumb.enumerate_pixels() {
        let r = px[0] as f32 / 255.0;
        let g = px[1] as f32 / 255.0;
        let b = px[2] as f32 / 255.0;

        total_luma += 0.2126 * r + 0.7152 * g + 0.0722 * b;
        pixel_count += 1.0;

        let max_c = r.max(g).max(b);
        let min_c = r.min(g).min(b);
        let sat = if max_c > 0.001 {
            (max_c - min_c) / max_c
        } else {
            0.0
        };
        let weight = 0.25 + sat * 1.75;

        let qx = usize::from(x >= 16);
        let qy = usize::from(y >= 16);
        let q = qy * 2 + qx;

        quad_accum[q][0] += r * weight;
        quad_accum[q][1] += g * weight;
        quad_accum[q][2] += b * weight;
        quad_accum[q][3] += weight;
    }

    if pixel_count <= 0.0 {
        return AlbumPalette::default_dark();
    }

    let mut colors = [[0.0_f32; 4]; 4];
    for i in 0..4 {
        let w = quad_accum[i][3].max(0.0001);
        let mut r = (quad_accum[i][0] / w).clamp(0.0, 1.0);
        let mut g = (quad_accum[i][1] / w).clamp(0.0, 1.0);
        let mut b = (quad_accum[i][2] / w).clamp(0.0, 1.0);

        // Gently lift very dark non-black colors so frosted glass retains luminous depth.
        let luma = 0.2126 * r + 0.7152 * g + 0.0722 * b;
        if luma > 0.02 && luma < 0.18 {
            let boost = (0.18 / luma).min(2.2);
            r = (r * boost).clamp(0.0, 1.0);
            g = (g * boost).clamp(0.0, 1.0);
            b = (b * boost).clamp(0.0, 1.0);
        }

        colors[i] = [r, g, b, 1.0];
    }

    let avg_luma = (total_luma / pixel_count).clamp(0.0, 1.0);
    let scrim_alpha = (0.38 + avg_luma * 0.17).clamp(0.38, 0.55);

    AlbumPalette {
        colors,
        scrim_alpha,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use image::{ImageBuffer, Rgba};
    use std::io::Cursor;

    fn encode_solid_png(r: u8, g: u8, b: u8, size: u32) -> Vec<u8> {
        let img = ImageBuffer::from_pixel(size, size, Rgba([r, g, b, 255]));
        let mut out = Cursor::new(Vec::new());
        img.write_to(&mut out, image::ImageFormat::Png).unwrap();
        out.into_inner()
    }

    #[test]
    fn pure_white_image_clamps_scrim_to_max_0_55() {
        let bytes = encode_solid_png(255, 255, 255, 16);
        let pal = extract_palette(&bytes);
        assert!((pal.scrim_alpha - 0.55).abs() < 0.001);
        assert!(pal.scrim_alpha <= 0.55 && pal.scrim_alpha >= 0.38);
    }

    #[test]
    fn pure_black_image_clamps_scrim_to_min_0_38() {
        let bytes = encode_solid_png(0, 0, 0, 16);
        let pal = extract_palette(&bytes);
        assert!((pal.scrim_alpha - 0.38).abs() < 0.001);
        assert!(pal.scrim_alpha <= 0.55 && pal.scrim_alpha >= 0.38);
    }

    #[test]
    fn corrupt_or_empty_image_bytes_return_safe_default_palette() {
        assert_eq!(extract_palette(&[]), AlbumPalette::default_dark());
        assert_eq!(extract_palette(b"garbage"), AlbumPalette::default_dark());
    }

    #[test]
    fn decode_art_blur_rgba_produces_64x64_rgba_or_none() {
        let bytes = encode_solid_png(180, 90, 45, 24);
        let rgba = decode_art_blur_rgba(&bytes).unwrap();
        assert_eq!(rgba.len(), 64 * 64 * 4);
        assert!(decode_art_blur_rgba(b"not an image").is_none());
    }

    #[test]
    fn accent_theme_falls_back_to_cool_and_adapts_to_colorful_art() {
        let default_accent = AlbumPalette::default_dark().accent_theme();
        assert_eq!(default_accent, AccentTheme::default_cool());

        let gray = extract_palette(&encode_solid_png(120, 120, 120, 16));
        assert_eq!(gray.accent_theme(), AccentTheme::default_cool());

        let magenta = extract_palette(&encode_solid_png(220, 40, 160, 16));
        let accent = magenta.accent_theme();
        assert_ne!(accent, AccentTheme::default_cool());
        assert!(accent.primary.r > accent.primary.g);
        assert!(accent.primary.b > accent.primary.g);
    }
}
