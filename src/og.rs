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
            if let Ok(dir) = std::env::var("TUGPRESS_FONTS") {
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
    for (size, width, max) in [(72, 22, 3), (60, 27, 4), (50, 33, 5)] {
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

pub fn svg(site: &str, title: &str, host: &str, accent: &str) -> String {
    let (size, lines) = fit(title);
    let family = "Inter, DejaVu Sans, Helvetica, Arial, sans-serif";
    let mut tspans = String::new();
    for (i, line) in lines.iter().enumerate() {
        let dy = if i == 0 { 0 } else { size * 6 / 5 };
        tspans += &format!("<tspan x=\"80\" dy=\"{dy}\">{}</tspan>", esc(line));
    }
    format!(
        "<svg xmlns=\"http://www.w3.org/2000/svg\" width=\"1200\" height=\"630\" viewBox=\"0 0 1200 630\">\
         <rect width=\"1200\" height=\"630\" fill=\"#111316\"/><rect width=\"1200\" height=\"14\" fill=\"{accent}\"/>\
         <circle cx=\"1130\" cy=\"560\" r=\"170\" fill=\"{accent}\" opacity=\"0.16\"/>\
         <text x=\"80\" y=\"128\" font-family=\"{family}\" font-size=\"34\" font-weight=\"700\" fill=\"{accent}\">{site}</text>\
         <text x=\"80\" y=\"{y}\" font-family=\"{family}\" font-size=\"{size}\" font-weight=\"700\" fill=\"#f4f4f5\">{tspans}</text>\
         <text x=\"80\" y=\"566\" font-family=\"{family}\" font-size=\"28\" fill=\"#a1a1aa\">{host}</text></svg>",
        site = esc(site),
        host = esc(host),
        y = 200 + size,
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
        assert!(size < 72 && lines.len() <= 5 && lines.iter().all(|l| l.chars().count() <= 33));
        assert!(wrap("a-very-long-unbroken-identifier-that-keeps-going-forever", 22).iter().all(|l| l.chars().count() <= 22));
    }
}
