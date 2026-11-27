use iced::widget::svg;
use iced::{Color, Element, Length};

fn render_svg<'a, Message: 'a>(
    bytes: &'static [u8],
    size: f32,
    color: Color,
) -> Element<'a, Message> {
    let handle = svg::Handle::from_memory(bytes);
    svg(handle)
        .width(Length::Fixed(size))
        .height(Length::Fixed(size))
        .style(move |_theme, _status| svg::Style { color: Some(color) })
        .into()
}

pub fn icon_search<'a, Message: 'a>(size: f32, color: Color) -> Element<'a, Message> {
    const SVG: &[u8] = br##"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24" fill="none" stroke="#ffffff" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><circle cx="11" cy="11" r="7"/><path d="m20 20-3.5-3.5"/></svg>"##;
    render_svg(SVG, size, color)
}

pub fn icon_heart<'a, Message: 'a>(
    filled: bool,
    size: f32,
    color: Color,
) -> Element<'a, Message> {
    const SVG_FILLED: &[u8] = br##"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24" fill="#ffffff"><path d="M12 21.35 10.55 20.03C5.4 15.36 2 12.28 2 8.5 2 5.42 4.42 3 7.5 3c1.74 0 3.41.81 4.5 2.09C13.09 3.81 14.76 3 16.5 3 19.58 3 22 5.42 22 8.5c0 3.78-3.4 6.86-8.55 11.54L12 21.35z"/></svg>"##;
    const SVG_OUTLINE: &[u8] = br##"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24" fill="none" stroke="#ffffff" stroke-width="1.9" stroke-linecap="round" stroke-linejoin="round"><path d="M19.5 12.57 12 20l-7.5-7.43A5 5 0 0 1 12 6.01a5 5 0 0 1 7.5 6.56z"/></svg>"##;
    render_svg(if filled { SVG_FILLED } else { SVG_OUTLINE }, size, color)
}

pub fn icon_folder<'a, Message: 'a>(size: f32, color: Color) -> Element<'a, Message> {
    const SVG: &[u8] = br##"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24" fill="none" stroke="#ffffff" stroke-width="1.9" stroke-linecap="round" stroke-linejoin="round"><path d="M3 7a2 2 0 0 1 2-2h4.2l2 2.4H19a2 2 0 0 1 2 2V18a2 2 0 0 1-2 2H5a2 2 0 0 1-2-2V7z"/></svg>"##;
    render_svg(SVG, size, color)
}

pub fn icon_settings<'a, Message: 'a>(size: f32, color: Color) -> Element<'a, Message> {
    const SVG: &[u8] = br##"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24" fill="none" stroke="#ffffff" stroke-width="1.9" stroke-linecap="round" stroke-linejoin="round"><circle cx="12" cy="12" r="3"/><path d="M19.4 15a1.65 1.65 0 0 0 .33 1.82l.06.06a2 2 0 1 1-2.83 2.83l-.06-.06a1.65 1.65 0 0 0-1.82-.33 1.65 1.65 0 0 0-1 1.51V21a2 2 0 1 1-4 0v-.09a1.65 1.65 0 0 0-1-1.51 1.65 1.65 0 0 0-1.82.33l-.06.06a2 2 0 1 1-2.83-2.83l.06-.06a1.65 1.65 0 0 0 .33-1.82 1.65 1.65 0 0 0-1.51-1H3a2 2 0 1 1 0-4h.09a1.65 1.65 0 0 0 1.51-1 1.65 1.65 0 0 0-.33-1.82l-.06-.06a2 2 0 1 1 2.83-2.83l.06.06a1.65 1.65 0 0 0 1.82.33H9a1.65 1.65 0 0 0 1-1.51V3a2 2 0 1 1 4 0v.09a1.65 1.65 0 0 0 1 1.51 1.65 1.65 0 0 0 1.82-.33l.06-.06a2 2 0 1 1 2.83 2.83l-.06.06a1.65 1.65 0 0 0-.33 1.82V9a1.65 1.65 0 0 0 1.51 1H21a2 2 0 1 1 0 4h-.09a1.65 1.65 0 0 0-1.51 1z"/></svg>"##;
    render_svg(SVG, size, color)
}

pub fn icon_playlist<'a, Message: 'a>(size: f32, color: Color) -> Element<'a, Message> {
    const SVG: &[u8] = br##"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24" fill="none" stroke="#ffffff" stroke-width="1.9" stroke-linecap="round" stroke-linejoin="round"><path d="M4 6h12M4 11h12M4 16h8"/><circle cx="17.5" cy="17.5" r="2.5"/><path d="M20 17.5V10"/></svg>"##;
    render_svg(SVG, size, color)
}

pub fn icon_plus<'a, Message: 'a>(size: f32, color: Color) -> Element<'a, Message> {
    const SVG: &[u8] = br##"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24" fill="none" stroke="#ffffff" stroke-width="2.1" stroke-linecap="round"><path d="M12 5v14M5 12h14"/></svg>"##;
    render_svg(SVG, size, color)
}

pub fn icon_link<'a, Message: 'a>(size: f32, color: Color) -> Element<'a, Message> {
    const SVG: &[u8] = br##"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24" fill="none" stroke="#ffffff" stroke-width="1.9" stroke-linecap="round" stroke-linejoin="round"><path d="M10 13a5 5 0 0 0 7.07 0l2.83-2.83a5 5 0 0 0-7.07-7.07L11.2 4.7"/><path d="M14 11a5 5 0 0 0-7.07 0L4.1 13.83a5 5 0 0 0 7.07 7.07l1.6-1.6"/></svg>"##;
    render_svg(SVG, size, color)
}

pub fn icon_music<'a, Message: 'a>(size: f32, color: Color) -> Element<'a, Message> {
    const SVG: &[u8] = br##"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24" fill="none" stroke="#ffffff" stroke-width="1.9" stroke-linecap="round" stroke-linejoin="round"><path d="M9 18V5l11-2v13"/><circle cx="6" cy="18" r="3"/><circle cx="17" cy="16" r="3"/></svg>"##;
    render_svg(SVG, size, color)
}

pub fn icon_play<'a, Message: 'a>(size: f32, color: Color) -> Element<'a, Message> {
    const SVG: &[u8] = br##"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24" fill="#ffffff"><path d="M8 5.14v13.72a1 1 0 0 0 1.52.85l10.8-6.86a1 1 0 0 0 0-1.7L9.52 4.29A1 1 0 0 0 8 5.14z"/></svg>"##;
    render_svg(SVG, size, color)
}

pub fn icon_pause<'a, Message: 'a>(size: f32, color: Color) -> Element<'a, Message> {
    const SVG: &[u8] = br##"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24" fill="#ffffff"><rect x="6" y="4.5" width="4.2" height="15" rx="1.4"/><rect x="13.8" y="4.5" width="4.2" height="15" rx="1.4"/></svg>"##;
    render_svg(SVG, size, color)
}

pub fn icon_prev<'a, Message: 'a>(size: f32, color: Color) -> Element<'a, Message> {
    const SVG: &[u8] = br##"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24" fill="#ffffff"><rect x="4.5" y="5" width="2.4" height="14" rx="1"/><path d="M18.5 5.6a1 1 0 0 1 1.5.86v11.08a1 1 0 0 1-1.5.86L9.4 12.86a1 1 0 0 1 0-1.72L18.5 5.6z"/></svg>"##;
    render_svg(SVG, size, color)
}

pub fn icon_next<'a, Message: 'a>(size: f32, color: Color) -> Element<'a, Message> {
    const SVG: &[u8] = br##"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24" fill="#ffffff"><rect x="17.1" y="5" width="2.4" height="14" rx="1"/><path d="M5.5 5.6A1 1 0 0 0 4 6.46v11.08a1 1 0 0 0 1.5.86l9.1-5.54a1 1 0 0 0 0-1.72L5.5 5.6z"/></svg>"##;
    render_svg(SVG, size, color)
}

pub fn icon_shuffle<'a, Message: 'a>(size: f32, color: Color) -> Element<'a, Message> {
    const SVG: &[u8] = br##"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24" fill="none" stroke="#ffffff" stroke-width="1.9" stroke-linecap="round" stroke-linejoin="round"><path d="M16 3h5v5M4 20 21 3M21 16v5h-5M15 15l6 6M4 4l5 5"/></svg>"##;
    render_svg(SVG, size, color)
}

pub fn icon_repeat<'a, Message: 'a>(size: f32, color: Color) -> Element<'a, Message> {
    const SVG: &[u8] = br##"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24" fill="none" stroke="#ffffff" stroke-width="1.9" stroke-linecap="round" stroke-linejoin="round"><path d="m17 2 4 4-4 4"/><path d="M3 11V9a4 4 0 0 1 4-4h14"/><path d="m7 22-4-4 4-4"/><path d="M21 13v2a4 4 0 0 1-4 4H3"/></svg>"##;
    render_svg(SVG, size, color)
}

pub fn icon_lyrics<'a, Message: 'a>(size: f32, color: Color) -> Element<'a, Message> {
    const SVG: &[u8] = br##"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24" fill="none" stroke="#ffffff" stroke-width="1.9" stroke-linecap="round" stroke-linejoin="round"><path d="M12 18.5a2.5 2.5 0 0 1-2.5-2.5c0-1.38 1.12-2.5 2.5-2.5V5l7-1.5V14"/><circle cx="16.5" cy="14" r="2.5"/><path d="M4 6h4M4 10h4M4 14h2.5"/></svg>"##;
    render_svg(SVG, size, color)
}

pub fn icon_queue<'a, Message: 'a>(size: f32, color: Color) -> Element<'a, Message> {
    const SVG: &[u8] = br##"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24" fill="none" stroke="#ffffff" stroke-width="1.9" stroke-linecap="round" stroke-linejoin="round"><path d="M4 6h16M4 12h16M4 18h10"/><path d="m18 15 3 2.5-3 2.5v-5z" fill="#ffffff"/></svg>"##;
    render_svg(SVG, size, color)
}

pub fn icon_volume<'a, Message: 'a>(muted: bool, size: f32, color: Color) -> Element<'a, Message> {
    const SVG_ON: &[u8] = br##"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24" fill="none" stroke="#ffffff" stroke-width="1.9" stroke-linecap="round" stroke-linejoin="round"><polygon points="11 5 6 9 2 9 2 15 6 15 11 19 11 5" fill="#ffffff"/><path d="M15.5 8.5a5 5 0 0 1 0 7M18.5 5.8a9 9 0 0 1 0 12.4"/></svg>"##;
    const SVG_MUTED: &[u8] = br##"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24" fill="none" stroke="#ffffff" stroke-width="1.9" stroke-linecap="round" stroke-linejoin="round"><polygon points="11 5 6 9 2 9 2 15 6 15 11 19 11 5" fill="#ffffff"/><path d="m22 9-6 6M16 9l6 6"/></svg>"##;
    render_svg(if muted { SVG_MUTED } else { SVG_ON }, size, color)
}

pub fn icon_back<'a, Message: 'a>(size: f32, color: Color) -> Element<'a, Message> {
    const SVG: &[u8] = br##"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24" fill="none" stroke="#ffffff" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><path d="m15 18-6-6 6-6"/></svg>"##;
    render_svg(SVG, size, color)
}

pub fn icon_more<'a, Message: 'a>(size: f32, color: Color) -> Element<'a, Message> {
    const SVG: &[u8] = br##"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24" fill="#ffffff"><circle cx="5.5" cy="12" r="1.75"/><circle cx="12" cy="12" r="1.75"/><circle cx="18.5" cy="12" r="1.75"/></svg>"##;
    render_svg(SVG, size, color)
}

pub fn icon_minimize<'a, Message: 'a>(size: f32, color: Color) -> Element<'a, Message> {
    const SVG: &[u8] = br##"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24" fill="none" stroke="#ffffff" stroke-width="2" stroke-linecap="round"><path d="M6 12h12"/></svg>"##;
    render_svg(SVG, size, color)
}

pub fn icon_maximize<'a, Message: 'a>(
    maximized: bool,
    size: f32,
    color: Color,
) -> Element<'a, Message> {
    const SVG_MAX: &[u8] = br##"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24" fill="none" stroke="#ffffff" stroke-width="1.8" stroke-linejoin="round"><rect x="6" y="6" width="12" height="12" rx="2"/></svg>"##;
    const SVG_RESTORE: &[u8] = br##"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24" fill="none" stroke="#ffffff" stroke-width="1.8" stroke-linejoin="round"><rect x="5" y="8" width="10" height="10" rx="1.8"/><path d="M9 8V6.5A1.5 1.5 0 0 1 10.5 5h7A1.5 1.5 0 0 1 19 6.5v7a1.5 1.5 0 0 1-1.5 1.5H15"/></svg>"##;
    render_svg(if maximized { SVG_RESTORE } else { SVG_MAX }, size, color)
}

pub fn icon_close<'a, Message: 'a>(size: f32, color: Color) -> Element<'a, Message> {
    const SVG: &[u8] = br##"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24" fill="none" stroke="#ffffff" stroke-width="2" stroke-linecap="round"><path d="M7 7l10 10M17 7 7 17"/></svg>"##;
    render_svg(SVG, size, color)
}

pub fn icon_refresh<'a, Message: 'a>(size: f32, color: Color) -> Element<'a, Message> {
    const SVG: &[u8] = br##"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24" fill="none" stroke="#ffffff" stroke-width="1.9" stroke-linecap="round" stroke-linejoin="round"><path d="M21 12a9 9 0 1 1-2.64-6.36"/><path d="M21 3v6h-6"/></svg>"##;
    render_svg(SVG, size, color)
}

pub fn icon_chevron_up<'a, Message: 'a>(size: f32, color: Color) -> Element<'a, Message> {
    const SVG: &[u8] = br##"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24" fill="none" stroke="#ffffff" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><path d="m18 15-6-6-6 6"/></svg>"##;
    render_svg(SVG, size, color)
}

pub fn icon_chevron_down<'a, Message: 'a>(size: f32, color: Color) -> Element<'a, Message> {
    const SVG: &[u8] = br##"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24" fill="none" stroke="#ffffff" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><path d="m6 9 6 6 6-6"/></svg>"##;
    render_svg(SVG, size, color)
}

pub fn icon_trash<'a, Message: 'a>(size: f32, color: Color) -> Element<'a, Message> {
    const SVG: &[u8] = br##"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24" fill="none" stroke="#ffffff" stroke-width="1.9" stroke-linecap="round" stroke-linejoin="round"><path d="M4 7h16M10 11v6M14 11v6"/><path d="M6 7l1 12a2 2 0 0 0 2 2h6a2 2 0 0 0 2-2l1-12M9 7V4a1 1 0 0 1 1-1h4a1 1 0 0 1 1 1v3"/></svg>"##;
    render_svg(SVG, size, color)
}

pub fn icon_home<'a, Message: 'a>(size: f32, color: Color) -> Element<'a, Message> {
    const SVG: &[u8] = br##"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24" fill="none" stroke="#ffffff" stroke-width="1.9" stroke-linecap="round" stroke-linejoin="round"><path d="M3 10.5 12 3l9 7.5"/><path d="M5 9.5V19a2 2 0 0 0 2 2h10a2 2 0 0 0 2-2V9.5"/><path d="M9.5 21v-6a1.5 1.5 0 0 1 1.5-1.5h2a1.5 1.5 0 0 1 1.5 1.5v6"/></svg>"##;
    render_svg(SVG, size, color)
}

pub fn icon_clock<'a, Message: 'a>(size: f32, color: Color) -> Element<'a, Message> {
    const SVG: &[u8] = br##"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24" fill="none" stroke="#ffffff" stroke-width="1.9" stroke-linecap="round" stroke-linejoin="round"><circle cx="12" cy="12" r="9"/><path d="M12 7v5l3 2"/></svg>"##;
    render_svg(SVG, size, color)
}

pub fn icon_trending<'a, Message: 'a>(size: f32, color: Color) -> Element<'a, Message> {
    const SVG: &[u8] = br##"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24" fill="none" stroke="#ffffff" stroke-width="1.9" stroke-linecap="round" stroke-linejoin="round"><polyline points="22 7 13.5 15.5 8.5 10.5 2 17"/><polyline points="16 7 22 7 22 13"/></svg>"##;
    render_svg(SVG, size, color)
}

pub fn icon_artist<'a, Message: 'a>(size: f32, color: Color) -> Element<'a, Message> {
    const SVG: &[u8] = br##"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24" fill="none" stroke="#ffffff" stroke-width="1.9" stroke-linecap="round" stroke-linejoin="round"><circle cx="12" cy="8" r="4"/><path d="M5 20a7 7 0 0 1 14 0"/></svg>"##;
    render_svg(SVG, size, color)
}

pub fn icon_disc<'a, Message: 'a>(size: f32, color: Color) -> Element<'a, Message> {
    const SVG: &[u8] = br##"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24" fill="none" stroke="#ffffff" stroke-width="1.9" stroke-linecap="round" stroke-linejoin="round"><circle cx="12" cy="12" r="9"/><circle cx="12" cy="12" r="2.5"/></svg>"##;
    render_svg(SVG, size, color)
}

pub fn icon_check<'a, Message: 'a>(size: f32, color: Color) -> Element<'a, Message> {
    const SVG: &[u8] = br##"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24" fill="none" stroke="#ffffff" stroke-width="2.2" stroke-linecap="round" stroke-linejoin="round"><polyline points="20 6 9 17 4 12"/></svg>"##;
    render_svg(SVG, size, color)
}
