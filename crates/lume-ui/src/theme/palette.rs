/// Extracted 4-color palette from album art paired with a contrast-safe dark scrim alpha.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct AlbumPalette {
    pub colors: [[f32; 4]; 4],
    pub scrim_alpha: f32,
}

impl AlbumPalette {
    /// Neutral smoky midnight-glass palette used when Dynamic Album Blur is idle.
    pub fn default_dark() -> Self {
        Self {
            colors: [
                [0.18, 0.21, 0.30, 1.0],
                [0.24, 0.19, 0.32, 1.0],
                [0.10, 0.12, 0.18, 1.0],
                [0.16, 0.24, 0.30, 1.0],
            ],
            scrim_alpha: 0.42,
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
}
