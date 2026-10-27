//! Share images: a 1200×630 card for every post and site, drawn from SVG with the server's fonts.

use crate::render::esc;
use resvg::{tiny_skia, usvg};
use std::sync::{Arc, OnceLock};

fn fonts() -> Arc<usvg::fontdb::Database> {
    static FONTS: OnceLock<Arc<usvg::fontdb::Database>> = OnceLock::new();
    FONTS
        .get_or_init(|| {
            let mut db = usvg::fontdb::Database::new();
            db.load_system_fonts();
            if let Ok(dir) = std::env::var("LANTERNLOG_FONTS") {
                db.load_fonts_dir(dir);
            }
            Arc::new(db)
        })
        .clone()
}

/// Breaks text into lines of at most `width` characters, splitting words only when one is longer than a line.
fn wrap(text: &str, width: usize) -> Vec<String> {
    let (mut lines, mut line) = (Vec::new(), String::new());
    for word in text.split_whitespace() {
        let mut word: Vec<char> = word.chars().collect();
        while word.len() > width {
            if !line.is_empty() {
                lines.push(std::mem::take(&mut line));
            }
            lines.push(word.drain(..width).collect());
        }
        let word: String = word.into_iter().collect();
        if line.is_empty() {
            line = word;
        } else if line.chars().count() + 1 + word.chars().count() <= width {
            line = format!("{line} {word}");
        } else {
            lines.push(std::mem::replace(&mut line, word));
        }
    }
    if !line.is_empty() {
        lines.push(line);
    }
    lines
}

/// The biggest type size at which the title fits, with its lines. Widths assume wide bold letters, so text
/// never reaches the edges.
fn fit(title: &str) -> (u32, Vec<String>) {
    for (size, width, max) in [(80, 20, 3), (68, 24, 3), (58, 28, 4), (50, 33, 5)] {
        let lines = wrap(title, width);
        if lines.len() <= max {
            return (size, lines);
        }
    }
    let mut lines = wrap(title, 33);
    lines.truncate(5);
    if let Some(last) = lines.last_mut() {
        last.push('…');
    }
    (50, lines)
}

/// What a share card shows. The site's name sits at the top unless the card is the site's own.
pub struct Card<'a> {
    pub site: &'a str,
    pub title: &'a str,
    pub subtitle: &'a str,
    pub host: &'a str,
    pub meta: &'a str,
    pub accent: &'a str,
}

pub fn svg(c: &Card) -> String {
    const FONT: &str = "Inter, DejaVu Sans, Helvetica, Arial, sans-serif";
    let (size, lines) = fit(c.title);
    let mut sub = wrap(c.subtitle, 56);
    if sub.len() > 2 {
        sub.truncate(2);
        if let Some(last) = sub.last_mut() {
            last.push('…');
        }
    }
    let line = size * 114 / 100;
    let mut height = size + line * (lines.len() as u32 - 1);
    if !sub.is_empty() {
        height += 30 + 32 + 44 * (sub.len() as u32 - 1);
    }
    let first = 170 + 320u32.saturating_sub(height) / 2 + size * 78 / 100;
    let mut text = String::new();
    for (i, l) in lines.iter().enumerate() {
        text += &format!("<tspan x=\"80\" dy=\"{}\">{}</tspan>", if i == 0 { 0 } else { line }, esc(l));
    }
    let mut subtitle = String::new();
    for (i, l) in sub.iter().enumerate() {
        subtitle += &format!("<tspan x=\"80\" dy=\"{}\">{}</tspan>", if i == 0 { 0 } else { 44 }, esc(l));
    }
    let sub_y = first + line * (lines.len() as u32 - 1) + 30 + 32;
    let mark_text = crate::style::rgb(c.accent)
        .map_or("#ffffff", |a| if crate::style::contrast(a, [255.0; 3]) >= 2.5 { "#ffffff" } else { "#111214" });
    let initial: String = c.site.chars().find(|ch| ch.is_alphanumeric()).map(|ch| ch.to_uppercase().collect()).unwrap_or_default();
    let name = if c.site == c.title { String::new() } else { esc(c.site) };
    format!(
        "<svg xmlns=\"http://www.w3.org/2000/svg\" width=\"1200\" height=\"630\" viewBox=\"0 0 1200 630\"><defs>\
         <linearGradient id=\"bg\" x1=\"0\" y1=\"0\" x2=\"1\" y2=\"1\"><stop offset=\"0\" stop-color=\"#16181c\"/><stop offset=\"1\" stop-color=\"#0b0c0e\"/></linearGradient>\
         <radialGradient id=\"glow\" cx=\"1050\" cy=\"70\" r=\"600\" gradientUnits=\"userSpaceOnUse\"><stop offset=\"0\" stop-color=\"{accent}\" stop-opacity=\".5\"/>\
         <stop offset=\".5\" stop-color=\"{accent}\" stop-opacity=\".12\"/><stop offset=\"1\" stop-color=\"{accent}\" stop-opacity=\"0\"/></radialGradient>\
         <radialGradient id=\"low\" cx=\"60\" cy=\"660\" r=\"440\" gradientUnits=\"userSpaceOnUse\"><stop offset=\"0\" stop-color=\"{accent}\" stop-opacity=\".13\"/>\
         <stop offset=\"1\" stop-color=\"{accent}\" stop-opacity=\"0\"/></radialGradient></defs>\
         <rect width=\"1200\" height=\"630\" fill=\"url(#bg)\"/><rect width=\"1200\" height=\"630\" fill=\"url(#glow)\"/><rect width=\"1200\" height=\"630\" fill=\"url(#low)\"/>\
         <rect x=\"80\" y=\"78\" width=\"52\" height=\"52\" rx=\"14\" fill=\"{accent}\"/>\
         <text x=\"106\" y=\"115\" text-anchor=\"middle\" font-family=\"{FONT}\" font-size=\"30\" font-weight=\"800\" fill=\"{mark_text}\">{initial}</text>\
         <text x=\"150\" y=\"114\" font-family=\"{FONT}\" font-size=\"30\" font-weight=\"600\" fill=\"#e8e7e4\">{name}</text>\
         <text x=\"80\" y=\"{first}\" font-family=\"{FONT}\" font-size=\"{size}\" font-weight=\"800\" letter-spacing=\"{spacing}\" fill=\"#ffffff\">{text}</text>\
         <text x=\"80\" y=\"{sub_y}\" font-family=\"{FONT}\" font-size=\"32\" fill=\"#b8bbc1\">{subtitle}</text>\
         <rect x=\"80\" y=\"514\" width=\"1040\" height=\"1\" fill=\"#ffffff\" fill-opacity=\".14\"/>\
         <text x=\"80\" y=\"562\" font-family=\"{FONT}\" font-size=\"26\" font-weight=\"500\" fill=\"#a3a6ac\">{host}</text>\
         <text x=\"1120\" y=\"562\" text-anchor=\"end\" font-family=\"{FONT}\" font-size=\"26\" fill=\"#a3a6ac\">{meta}</text></svg>",
        accent = esc(c.accent),
        spacing = -(size as f32) * 0.02,
        host = esc(c.host),
        meta = esc(c.meta),
        initial = esc(&initial),
    )
}

pub fn png(svg: &str) -> anyhow::Result<Vec<u8>> {
    let opt = usvg::Options { fontdb: fonts(), ..Default::default() };
    let tree = usvg::Tree::from_str(svg, &opt)?;
    let size = tree.size().to_int_size();
    let mut pixmap = tiny_skia::Pixmap::new(size.width(), size.height()).ok_or_else(|| anyhow::anyhow!("empty image"))?;
    resvg::render(&tree, tiny_skia::Transform::default(), &mut pixmap.as_mut());
    Ok(pixmap.encode_png()?)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn titles_fit_inside_the_card() {
        assert_eq!(fit("Short").1, vec!["Short"]);
        let (size, lines) = fit("How we cut our deploy times from four minutes to forty seconds on a single Hetzner box");
        assert!(size < 80 && lines.len() <= 5 && lines.iter().all(|l| l.chars().count() <= 33));
        assert!(wrap("a-very-long-unbroken-identifier-that-keeps-going-forever", 22).iter().all(|l| l.chars().count() <= 22));
    }
}
