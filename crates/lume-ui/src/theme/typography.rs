use iced::font::{Family, Weight};
use iced::Font;

pub const PLUS_JAKARTA_REGULAR: &[u8] =
    include_bytes!("../../assets/fonts/PlusJakartaSans-Regular.ttf");
pub const PLUS_JAKARTA_MEDIUM: &[u8] =
    include_bytes!("../../assets/fonts/PlusJakartaSans-Medium.ttf");
pub const PLUS_JAKARTA_SEMIBOLD: &[u8] =
    include_bytes!("../../assets/fonts/PlusJakartaSans-SemiBold.ttf");
pub const PLUS_JAKARTA_BOLD: &[u8] =
    include_bytes!("../../assets/fonts/PlusJakartaSans-Bold.ttf");

pub const FONT_REGULAR: Font = Font {
    family: Family::Name("Plus Jakarta Sans"),
    weight: Weight::Normal,
    ..Font::DEFAULT
};

pub const FONT_MEDIUM: Font = Font {
    family: Family::Name("Plus Jakarta Sans"),
    weight: Weight::Medium,
    ..Font::DEFAULT
};

pub const FONT_SEMIBOLD: Font = Font {
    family: Family::Name("Plus Jakarta Sans"),
    weight: Weight::Semibold,
    ..Font::DEFAULT
};

pub const FONT_BOLD: Font = Font {
    family: Family::Name("Plus Jakarta Sans"),
    weight: Weight::Bold,
    ..Font::DEFAULT
};
