pub mod palette;
pub mod shader;

pub use palette::{decode_art_blur_rgba, extract_palette, AlbumPalette};
pub use shader::DynamicBackdropProgram;
